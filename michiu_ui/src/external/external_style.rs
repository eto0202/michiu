use crate::{Context, MichiuError, ReadSignal};
use notify::Watcher;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

/// 外部スタイル定義フォーマットをパース・構築するトレイト。
pub trait ExternalStyle: Send + Sync + 'static {
    /// シグナルへ格納されるスタイルデータ型（`ThisStyle`, `HashMap<String, ThisStyle>`, 独自テーマ型）。
    type Output: Clone + Send + 'static;

    /// ソース文字列とファイルパスからスタイルデータを構築する。
    fn load(&self, path: &Path, content: &str) -> crate::Result<Self::Output>;

    /// 静的なワンショット読み込み（監視なし）。
    fn load_file(&self, path: impl AsRef<Path>) -> crate::Result<Self::Output> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| MichiuError::ReadStringFailed {
            path: path.to_path_buf(),
            source: Arc::new(e),
        })?;
        self.load(path, &content)
    }

    /// ホットリロード監視用ビルダーの生成。
    fn into_watcher(self, path: impl Into<PathBuf>) -> StyleWatcher<Self>
    where
        Self: Sized,
    {
        StyleWatcher::new(self, path.into())
    }
}

pub struct StyleWatcher<L> {
    loader: L,
    path: PathBuf,
}

impl<L: ExternalStyle> StyleWatcher<L> {
    pub fn new(loader: L, path: PathBuf) -> Self {
        Self { loader, path }
    }

    /// `Context` にバインドしてファイル監視を開始し、シグナルとガードを返却。
    pub fn watch(
        self,
        cx: &mut Context,
    ) -> crate::Result<(ReadSignal<L::Output>, StyleWatchGuard)> {
        let raw_path =
            std::fs::canonicalize(&self.path).map_err(|e| MichiuError::CanonicalizeFailed {
                path: self.path.clone(),
                source: Arc::new(e),
            })?;
        // Windows の UNC プレフィックスを除去して通常パスに揃える
        let path = strip_unc_prefix(raw_path);

        // 初期ロード
        let content =
            std::fs::read_to_string(&path).map_err(|e| MichiuError::ReadStringFailed {
                path: path.clone(),
                source: Arc::new(e),
            })?;
        let initial_data = self.loader.load(&path, &content)?;
        let (read_sig, write_sig) = cx.create_signal(initial_data);

        let loader = Arc::new(self.loader);
        let path = path.clone();
        let task_sender = cx.task_sender();

        // notify 監視の初期化
        let loader_clone = Arc::clone(&loader);
        let path_clone = path.clone();
        let sender = task_sender.clone();

        // レースコンディション対策用の世代管理のカウンター
        let version = Arc::new(AtomicU64::new(0));
        let version_clone = Arc::clone(&version);

        let mut watcher =
            notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    // 届いたイベントの発生パスの中に、監視対象の絶対パスが含まれているか検証
                    let has_target_file = event.paths.iter().any(|p| {
                        let p_clean = strip_unc_prefix(p.clone());
                        p_clean == path_clone
                    });

                    // 対象ファイルがあり、かつ単なる読込以外のすべての書き込み・リネームイベントを許容
                    if has_target_file && !event.kind.is_access() {
                        let current_version = version_clone.fetch_add(1, Ordering::SeqCst) + 1;

                        let thread_path = path_clone.clone();
                        let thread_loader = Arc::clone(&loader_clone);
                        let thread_sender = sender.clone();
                        let thread_version = Arc::clone(&version_clone);

                        std::thread::spawn(move || {
                            // 完了を少しだけ待って成功率をあげる。
                            // エディタの書き込み中にspawnが走ってパースエラーになるケース。
                            // 複数回イベントが走るケース。
                            std::thread::sleep(Duration::from_millis(30));

                            // スリープ中に新しいイベントが来たら即リターン
                            if thread_version.load(Ordering::SeqCst) != current_version {
                                return;
                            }

                            if let Ok(raw_content) = std::fs::read_to_string(&thread_path)
                                && let Ok(parsed_data) =
                                    thread_loader.load(&thread_path, &raw_content)
                            {
                                let _ = thread_sender.send(move |cx| {
                                    // 一応チェック
                                    if thread_version.load(Ordering::SeqCst) == current_version {
                                        write_sig.set(parsed_data);
                                        if let Some(root) = cx.find_root_entity() {
                                            cx.mark_dirty(root);
                                        }
                                    }
                                });
                            }
                        });
                    }
                }
            })
            .map_err(|e| MichiuError::WatcherInitFailed {
                source: Arc::new(e),
            })?;

        let parent_dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));

        watcher
            .watch(parent_dir, notify::RecursiveMode::NonRecursive)
            .map_err(|e| MichiuError::WatchTargetFailed {
                path: parent_dir.to_path_buf(),
                source: Arc::new(e),
            })?;

        Ok((read_sig, StyleWatchGuard { _watcher: watcher }))
    }
}

/// 監視スレッドの生存期間を管理する RAII ガード。
pub struct StyleWatchGuard {
    _watcher: notify::RecommendedWatcher,
}

#[inline]
fn strip_unc_prefix(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let s = path.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }
    path
}
