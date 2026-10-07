use crate::{Context, MichiuError, ReadSignal};
use notify::Watcher;
use std::{
    borrow::Cow,
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

/// An RAII guard that manages the lifetime of a monitoring thread.
pub struct DataWatchGuard {
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

/// Structure for monitoring changes to `L`
pub struct DataWatcher<L> {
    loader: L,
    path: PathBuf,
}

impl<L: ExternalData> DataWatcher<L> {
    #[inline]
    pub fn new(loader: L, path: PathBuf) -> Self {
        Self { loader, path }
    }

    /// Binds to `Context` to start file monitoring, then returns a signal and a guard.
    pub fn watch(self, cx: &mut Context) -> crate::Result<(ReadSignal<L::Output>, DataWatchGuard)> {
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

        Ok((read_sig, DataWatchGuard { _watcher: watcher }))
    }
}

/// A trait for parsing and constructing external data.
pub trait ExternalData: Send + Sync + 'static {
    /// Data types stored in the signal (`ThisStyle`, `HashMap<String, ThisStyle>`, custom data).
    type Output: Clone + Send + 'static;

    /// Construct data from the source string and file path.
    fn load(&self, path: &Path, content: &str) -> crate::Result<Self::Output>;

    /// Static loading (no monitoring).
    fn load_file(&self, path: impl AsRef<Path>) -> crate::Result<Self::Output> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| MichiuError::ReadStringFailed {
            path: path.to_path_buf(),
            source: Arc::new(e),
        })?;
        self.load(path, &content)
    }

    /// Creating a hot-reload watcher.
    fn into_watcher(self, path: impl Into<PathBuf>) -> DataWatcher<Self>
    where
        Self: Sized,
    {
        DataWatcher::new(self, path.into())
    }
}

/// A container that stores external data output for each key (namespace).
#[derive(Debug, Clone, Default)]
pub struct ExternalDataSet<T> {
    sheets: HashMap<String, T>,
}

impl<T> ExternalDataSet<T> {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            sheets: HashMap::new(),
        }
    }

    #[inline]
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&T> {
        self.sheets.get(key)
    }

    #[inline]
    pub fn insert(&mut self, key: impl Into<String>, val: T) {
        self.sheets.insert(key.into(), val);
    }
}

pub struct StyleSetEntry<L> {
    pub key: Cow<'static, str>,
    pub path: PathBuf,
    pub loader: L,
}

// 監視用エントリ情報の事前解決
struct WatchTarget<L> {
    key: Cow<'static, str>,
    clean_path: PathBuf,
    parent_dir: PathBuf,
    loader: Arc<L>,
    version: Arc<AtomicU64>,
}

/// A builder for managing multiple `ExternalData` objects
#[derive(Default)]
pub struct ExternalDataSetBuilder<L> {
    entries: Vec<StyleSetEntry<L>>,
}

impl<L: ExternalData> ExternalDataSetBuilder<L> {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    #[inline]
    #[must_use]
    pub fn add(
        mut self,
        key: impl Into<Cow<'static, str>>,
        path: impl Into<PathBuf>,
        loader: L,
    ) -> Self {
        self.entries.push(StyleSetEntry {
            key: key.into(),
            path: path.into(),
            loader,
        });
        self
    }

    /// Initially load all sheets, start batch monitoring, and return signals and guards.
    pub fn watch(
        self,
        cx: &mut Context,
    ) -> crate::Result<(ReadSignal<ExternalDataSet<L::Output>>, DataWatchGuard)> {
        let mut initial_set = ExternalDataSet::new();

        let mut watch_targets = Vec::with_capacity(self.entries.len());

        for entry in self.entries {
            let raw_path = std::fs::canonicalize(&entry.path).map_err(|e| {
                MichiuError::CanonicalizeFailed {
                    path: entry.path.clone(),
                    source: Arc::new(e),
                }
            })?;
            let clean_path = strip_unc_prefix(raw_path);

            // 初期同期ロード
            let content = std::fs::read_to_string(&clean_path).map_err(|e| {
                MichiuError::ReadStringFailed {
                    path: clean_path.clone(),
                    source: Arc::new(e),
                }
            })?;
            let output = entry.loader.load(&clean_path, &content)?;
            initial_set.insert(entry.key.clone(), output);

            let parent_dir = clean_path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();

            watch_targets.push(WatchTarget {
                key: entry.key,
                clean_path,
                parent_dir,
                loader: Arc::new(entry.loader),
                version: Arc::new(AtomicU64::new(0)),
            });
        }

        let (read_sig, write_sig) = cx.create_signal(initial_set);
        let targets = Arc::new(watch_targets);
        let targets_clone = Arc::clone(&targets);
        let task_sender = cx.task_sender();

        let mut watcher =
            notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    if event.kind.is_access() {
                        return;
                    }

                    // イベント対象に含まれるファイルを検索
                    for target in targets_clone.iter() {
                        let has_file = event
                            .paths
                            .iter()
                            .any(|p| strip_unc_prefix(p.clone()) == target.clean_path);

                        if has_file {
                            // ファイルごとに世代をインクリメント
                            let current_version = target.version.fetch_add(1, Ordering::SeqCst) + 1;

                            let thread_path = target.clean_path.clone();
                            let thread_loader = Arc::clone(&target.loader);
                            let thread_sender = task_sender.clone();
                            let key = target.key.clone();
                            let thread_version = Arc::clone(&target.version);

                            // ワーカースレッドで部分再パース
                            std::thread::spawn(move || {
                                std::thread::sleep(Duration::from_millis(30));

                                // 後続の新しいイベントが既に発火していたらパース自体をスキップ
                                if thread_version.load(Ordering::SeqCst) != current_version {
                                    return;
                                }

                                if let Ok(raw_content) = std::fs::read_to_string(&thread_path)
                                    && let Ok(new_output) =
                                        thread_loader.load(&thread_path, &raw_content)
                                {
                                    let _ = thread_sender.send(move |cx| {
                                        if thread_version.load(Ordering::SeqCst) == current_version
                                        {
                                            let mut current_set = read_sig.get();
                                            current_set.insert(key, new_output);
                                            write_sig.set(current_set);
                                            if let Some(root) = cx.find_root_entity() {
                                                cx.mark_dirty(root);
                                            }
                                        }
                                    });
                                }
                            });
                        }
                    }
                }
            })
            .map_err(|e| MichiuError::WatcherInitFailed {
                source: Arc::new(e),
            })?;

        // 各親ディレクトリを監視登録（重複排除）
        let mut watched_dirs = std::collections::HashSet::new();
        for target in targets.iter() {
            if watched_dirs.insert(&target.parent_dir) {
                watcher
                    .watch(&target.parent_dir, notify::RecursiveMode::NonRecursive)
                    .map_err(|e| MichiuError::WatchTargetFailed {
                        path: target.parent_dir.clone(),
                        source: Arc::new(e),
                    })?;
            }
        }

        Ok((read_sig, DataWatchGuard { _watcher: watcher }))
    }
}
