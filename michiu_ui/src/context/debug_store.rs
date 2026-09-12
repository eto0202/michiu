use crate::{
    ComponentMask, Context, DrawBatch, EffectId, ElementState, EntityId, LayoutPoint, LayoutRect,
    Modifiers, MouseButton, Point, QuadInstance, RenderData, SignalId, UserAction, VirtualKey,
    define_secondary,
};
use slotmap::SecondaryMap;
use std::{
    borrow::Cow,
    ops::Range,
    panic::Location,
    path::PathBuf,
    sync::{
        Arc, Mutex, RwLock,
        mpsc::{Receiver, Sender, SyncSender},
    },
    time::{Duration, Instant},
};
use thiserror::Error;

// ================================================================
// ================================================================

#[cfg(feature = "logging")]
#[derive(
    Debug, Clone, Default, derive_more::Deref, derive_more::DerefMut, derive_more::IntoIterator,
)]
#[into_iterator(owned, ref, ref_mut)]
pub(crate) struct InspecterSecondary(SecondaryMap<EntityId, MichiuTraceRecord>);

#[cfg(feature = "logging")]
#[derive(
    Debug, Clone, Default, derive_more::Deref, derive_more::DerefMut, derive_more::IntoIterator,
)]
#[into_iterator(owned, ref, ref_mut)]
pub struct MichiuTraceVec(Vec<MichiuTraceRecord>);

// ================================================================
// ================================================================

#[cfg(feature = "logging")]
pub struct DebugStore {
    /// Context が起動した時間を0秒とする
    pub(crate) boot_time: Instant,
    // グローバルなエラー用にルート要素のIDを持っておく。
    pub(crate) dbg_root: Option<EntityId>,
    pub(crate) dbg_tx: Option<InspectorSender>,
    // メインスレッドでループ中に溜めておく一時キュー
    pub(crate) dbg_trace_queue: MichiuTraceVec,
}

#[cfg(not(feature = "logging"))]
pub(crate) struct DebugStore {
    pub(crate) dbg_root: Option<EntityId>,
}
#[cfg(not(feature = "logging"))]
impl DebugStore {
    pub(crate) fn new() -> Self {
        Self { dbg_root: None }
    }
}

#[cfg(feature = "logging")]
impl Default for DebugStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "logging")]
impl DebugStore {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            boot_time: Instant::now(),
            dbg_root: None,
            dbg_tx: None,
            dbg_trace_queue: MichiuTraceVec(Vec::new()),
        }
    }

    #[inline]
    pub(crate) fn set_inspector(debug: &mut DebugStore, inspector: &MichiuInspector) {
        debug.dbg_tx = Some(inspector.sender());
    }

    /// ループ中の各所で呼ぶ。チャネルには投げず一時キューに詰めるだけ
    #[inline]
    pub(crate) fn trace(id: Option<EntityId>, debug: &mut DebugStore, trace: MichiuTraceRecord) {
        if debug.dbg_tx.is_some() {
            debug.dbg_trace_queue.push(trace);
        }
    }

    /// フレームの最後で呼んで一括転送
    #[inline]
    pub(crate) fn flush_trace(debug: &mut DebugStore) {
        if let Some(ref tx) = debug.dbg_tx
            && !debug.dbg_trace_queue.is_empty()
        {
            let batch = std::mem::take(&mut debug.dbg_trace_queue);
            tx.send_batch(batch);
        }
    }

    /// 起動時からの経過時間を取得
    #[inline]
    pub(crate) fn elapsed(&self) -> TimeStamp {
        TimeStamp::Start(self.boot_time.elapsed())
    }
}

// ================================================================
// ================================================================

#[cfg(feature = "logging")]
#[derive(Debug, Clone)]
pub struct InspectorSender {
    // 1フレーム分のバッチを丸ごと送るチャネル
    tx: SyncSender<MichiuTraceVec>,
}

#[cfg(feature = "logging")]
impl InspectorSender {
    #[inline]
    pub(crate) fn send_batch(&self, batch: MichiuTraceVec) {
        // バッファが一杯ならメインを待たせずに捨てる
        // TODO: 溢れた場合は警告ログを挟む
        let _ = self.tx.try_send(batch);
    }
}

// ================================================================
// ================================================================

// 購読者に届くデータ型（1フレーム分のバッチ）
#[cfg(feature = "logging")]
pub type TraceBatch = Arc<MichiuTraceVec>;

// サブスクライバのハンドル
#[cfg(feature = "logging")]
pub struct TraceSubscription {
    rx: Receiver<TraceBatch>,
}

#[cfg(feature = "logging")]
impl TraceSubscription {
    /// 次のバッチを待つ（ブロッキング）
    pub fn recv(&self) -> std::result::Result<TraceBatch, std::sync::mpsc::RecvError> {
        self.rx.recv()
    }

    /// ノンブロッキングで取得を試みる
    pub fn try_recv(&self) -> std::result::Result<TraceBatch, std::sync::mpsc::TryRecvError> {
        self.rx.try_recv()
    }
}

// ================================================================
// ================================================================

#[cfg(feature = "logging")]
#[derive(Debug, Clone)]
pub struct MichiuInspector {
    sender: InspectorSender,
    // ワーカースレッドが更新し、ユーザーが読み取るための共有ストレージ
    storage: Arc<RwLock<InspecterSecondary>>,
    // サブスクライバの送信口を束ねて管理
    subscribers: Arc<Mutex<Vec<SyncSender<TraceBatch>>>>,
}

#[cfg(feature = "logging")]
impl Default for MichiuInspector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "logging")]
impl MichiuInspector {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel(256);
        let storage = Arc::new(RwLock::new(InspecterSecondary(SecondaryMap::new())));
        let subscribers: Arc<Mutex<Vec<SyncSender<TraceBatch>>>> = Arc::new(Mutex::new(Vec::new()));

        let storage_clone = Arc::clone(&storage);
        let subs_clone = Arc::clone(&subscribers);

        // ワーカースレッド
        std::thread::spawn(move || {
            let mut local_storage = InspecterSecondary(SecondaryMap::new());

            while let Ok(batch) = rx.recv() {
                let shared_batch = Arc::new(batch);

                // 購読者全員へ一斉配信（詰まっている、切断されたものは破棄）
                if let Ok(mut subs) = subs_clone.lock() {
                    subs.retain(|sub_tx| {
                        // try_send で送れない場合は捨てるか、
                        // 接続が切れていればリストから除去する
                        match sub_tx.try_send(Arc::clone(&shared_batch)) {
                            Ok(()) | Err(std::sync::mpsc::TrySendError::Full(_)) => {
                                // サブスクライバ側が遅延して溢れた場合（接続は維持）
                                true
                            }
                            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                                // サブスクライバがdropされたので登録解除
                                false
                            }
                        }
                    });
                }

                // 最新状態をローカルに反映
                for trace in shared_batch.iter() {
                    if let Some(id) = trace.id {
                        local_storage.insert(id, trace.clone());
                    }
                }

                // 最新スナップショットの公開
                if let Ok(mut lock) = storage_clone.write() {
                    *lock = local_storage.clone();
                }
            }
        });

        Self {
            sender: InspectorSender { tx },
            storage,
            subscribers,
        }
    }

    /// 新しいイベント購読を開始する
    /// `buffer_size` が None の場合、デフォルトで 128
    #[inline]
    #[must_use]
    pub fn subscribe(&self, buffer_size: Option<usize>) -> TraceSubscription {
        let bs = buffer_size.unwrap_or(128);
        let (tx, rx) = std::sync::mpsc::sync_channel(bs);
        self.subscribers.lock().unwrap().push(tx);
        TraceSubscription { rx }
    }

    #[inline]
    #[must_use]
    pub(crate) fn sender(&self) -> InspectorSender {
        self.sender.clone()
    }

    /// 最新のスナップショットを取得する
    #[inline]
    #[must_use]
    pub fn get(&self, id: EntityId) -> Option<MichiuTraceRecord> {
        self.storage.read().unwrap().get(id).cloned()
    }
}

// ================================================================
// ================================================================

impl Context {
    #[cfg(feature = "logging")]
    #[inline]
    pub fn set_inspector(&mut self, inspector: &MichiuInspector) {
        DebugStore::set_inspector(&mut self.debug, inspector);
    }

    /// ループ中の各所で呼ぶ。チャネルには投げず一時キューに詰めるだけ
    #[cfg(feature = "logging")]
    #[inline]
    pub(crate) fn trace(&mut self, id: Option<EntityId>, trace: MichiuTraceRecord) {
        DebugStore::trace(id, &mut self.debug, trace);
    }

    /// フレームの最後で呼んで一括転送
    #[cfg(feature = "logging")]
    #[inline]
    pub(crate) fn flush_trace(&mut self) {
        DebugStore::flush_trace(&mut self.debug);
    }
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum MichiuTrace {
    None,
    Spawn(Arc<SpawnTrace>),
    HitTest {
        target: Option<EntityId>,
        x: f32,
        y: f32,
        add: Option<&'static str>,
    },
    Reactive {
        signal: Option<SignalId>,
        effect: Option<EffectId>,
        kinds: ReactiveKinds,
        add: Option<&'static str>,
    },
    Event {
        kinds: TraceEventList,
        add: Option<&'static str>,
    },
    StateUpdate {
        flag: ComponentMask,
        current: ComponentMask,
        actived: bool,
        add: Option<&'static str>,
    },
    QueueDirty(Arc<QueueDirtyTrace>),
    Dfs {
        after: Arc<[EntityId]>,
        add: Option<&'static str>,
    },
    Layout {
        stage: LayoutStage,
        add: Option<&'static str>,
    },
    Text {
        add: Option<&'static str>,
    },
    Frame {
        kinds: FrameKinds,
        add: Option<&'static str>,
    },
    Sorted {
        after: Arc<[EntityId]>,
        add: Option<&'static str>,
    },
    PrepareRender {
        stage: RenderStage,
        data: Option<Arc<[RenderData]>>,
        add: Option<&'static str>,
    },
    WriteBuffer {
        staging: Arc<[QuadInstance]>,
        add: Option<&'static str>,
    },
    Present {
        add: Option<&'static str>,
    },
    Commit {
        add: Option<&'static str>,
    },
    Despawn {
        add: Option<&'static str>,
    },
    // 特定のエンティティに帰属させにくいエラーは、
    // この画面、あるいはアプリ全体のルートコンポーネントがアセットを読み込もうとして失敗したと解釈し、
    // とりあえずルート要素のIDにまとめる
    Error {
        detail: MichiuError,
        fallback: Option<&'static str>,
        add: Option<&'static str>,
    },
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct SpawnTrace {
    pub time: TimeStamp,
    pub entities: Option<Vec<EntityId>>,
    pub root: Option<EntityId>,
    pub parents: Option<Vec<EntityId>>,
    pub children: Option<Vec<EntityId>>,
    pub func: &'static str,
    pub loc: &'static Location<'static>,
    pub add: Option<&'static str>,
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct QueueDirtyTrace {
    pub time: TimeStamp,
    pub kinds: QueueDirtyKinds,
    pub masks: Option<ComponentMask>,
    pub entities: Option<Vec<EntityId>>,
    pub sort: Option<bool>,
    pub structure: Option<bool>,
    pub func: &'static str,
    pub loc: &'static Location<'static>,
    pub add: Option<&'static str>,
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum QueueDirtyKinds {
    None,
    Layout,
    Render,
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum ReactiveKinds {
    None,
    /// Signal の作成
    CreateSignal,
    /// 指定された要素、もしくはルート要素に対してシグナルコンテキストを提供
    Provide,
    /// ルート要素の探索
    FindRoot,
    /// ツリーを親に向かって遡り `ReadSignal` を解決
    FindReadSignal,
    /// ツリーを親に向かって遡り `WriteSignal` を解決
    FindWriteSignal,
    /// 依存関係のトラッキング
    Tracking,
    /// シグナルの読み取り
    Getting,
    /// シグナルの書き換え
    Writing,
    /// 指定されたエフェクトをメインスレッドのコンテキスト下で評価
    ExecuteEffect,
    /// スレッドローカル経由で実行中エフェクトを解決
    ResolveEffect,
    /// エフェクトをカテゴリ指定付きで紐づけ
    RegisterEffect,
    /// 要素に動的エフェクトを登録し初期評価を実行
    CreateEffect,
    /// 溜まっているすべてのエフェクトを評価完了させる
    PendingEffect,
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum LayoutStage {
    None,
    /// レイアウト計算の開始
    Start,
    /// 溜めてある初回評価を実行
    FirstEffects,
    /// 計算が必要ない場合は早期リターン
    EarlyReturn,
    /// DFS配列の高速再構築
    RebuildDfs,
    /// 各スタイルの解決
    ResolveStyle,
    /// Taffy ツリーへの差分同期
    SyncTaffy,
    /// 1回目のレイアウト計算
    FirstMeasure,
    /// レイアウト計算中
    ProcessingMeasure,
    /// テキストレイアウト計算
    TextMeasure,
    /// テキストレイアウト計算中
    ProcessingTextMeasure,
    /// 1回目の出力領域
    FirstOutputRect,
    /// 出力領域の処理中
    ProcessingOutputRect,
    /// スクロールサイズ計算
    ScrollSize,
    /// スクロールバーのスタイルの同期
    ResolveScrollBar,
    /// 2回目レイアウト計算
    FinalMeasure,
    /// 最終的な出力領域
    FinalOutputRect,
    /// Input コンテンツの同期
    UpdateInputContents,
    /// スクロールオフセット同期
    SyncScrollOffsets,
    /// ダーティフラグのクリア
    ClearDirty,
    /// レイアウト計算の終了
    End,
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum RenderStage {
    None,
    /// レンダリングフェーズ開始
    Start,
    /// 実効 `z_index` の計算と可視性、それらに基づく要素のソート
    SortedEntities,
    /// 指定された要素に含まれるすべての文字をアトラスにキャッシュ
    FirstGlyphsCache,
    /// アトラスのクリアが起きた場合、アトラスを再構築して再度キャッシュ
    FullGlyphsCache,
    /// パッキング
    CollectDate,
    /// バッチのフラッシュ
    FlushBatch,
    /// インスタンスを追加
    PushInstance,
    /// ダーティフラグのクリア
    ClearDirty,
    /// 終了
    End,
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeStamp {
    None,
    /// 計測開始（アプリ起動からの経過時間）
    Start(MichiuDuration),
    /// 途中のステージ（直前のステージからの経過時間）
    Elapsed(MichiuDuration),
    /// 計測終了（全体の合計時間）
    End(Duration),
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum TraceEventList {
    None,
    PointerMove {
        x: f32,
        y: f32,
    },
    PointerButton {
        button: MouseButton,
        state: ElementState,
        modifiers: Modifiers,
    },
    PointerDoubleClick {
        modifiers: Modifiers,
    },
    MouseWheel {
        x: f32,
        y: f32,
    },
    Keyboard {
        key: VirtualKey,
        state: ElementState,
        modifiers: Modifiers,
    },
    Character {
        char: char,
    },
    Ime {
        is_open: bool,
        conversion_mode: u32,
        sentence_mode: u32,
        keyboard_layout_id: u32,
        composition_text: Cow<'static, str>,
        result_text: Cow<'static, str>,
        caret_position: Option<LayoutPoint>,
        composition_cursor: usize,
        composition_attrs: Vec<u8>,
    },
    FileDropped {
        path: Arc<[PathBuf]>,
    },
    Paste {
        text: Cow<'static, str>,
    },
    Cut {
        text: Cow<'static, str>,
    },
    Undo {
        previous: Cow<'static, str>,
        list: Vec<Cow<'static, str>>,
    },
    Redo {
        previous: Cow<'static, str>,
        list: Vec<Cow<'static, str>>,
    },
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum FrameKinds {
    None,
    Transition,
    Animation,
    AutoScroll,
}

// ================================================================
// ================================================================

#[cfg(feature = "logging")]
#[derive(Debug, Clone)]
pub struct MichiuTraceRecord {
    pub id: Option<EntityId>,
    pub time: TimeStamp,
    pub func: &'static str,
    pub loc: &'static Location<'static>,
    pub trace: MichiuTrace,
}

// ================================================================
// ================================================================

#[cfg(feature = "logging")]
type MichiuInstant = Instant;
#[cfg(feature = "logging")]
type MichiuDuration = Duration;

#[cfg(not(feature = "logging"))]
type MichiuInstant = [u8; 0];
#[cfg(not(feature = "logging"))]
type MichiuDuration = [u8; 0];

#[cfg(feature = "logging")]
pub(crate) struct MichiuStopwatch {
    pub(crate) start: MichiuInstant,
    pub(crate) last: MichiuInstant,
}

#[cfg(feature = "logging")]
impl MichiuStopwatch {
    #[inline]
    pub(crate) fn new() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            last: now,
        }
    }

    // 途中の経過時間を作って記録を更新
    #[inline]
    pub(crate) fn elapsed(&mut self) -> TimeStamp {
        let now = Instant::now();
        let diff = now.duration_since(self.last);
        self.last = now;
        TimeStamp::Elapsed(diff)
    }

    // 最後の合計時間を出す
    #[inline]
    pub(crate) fn total(&self) -> TimeStamp {
        TimeStamp::End(Instant::now().duration_since(self.start))
    }
}

// ================================================================
// ================================================================

#[cfg(feature = "logging")]
#[inline]
#[track_caller]
pub(crate) const fn caller_location() -> &'static Location<'static> {
    Location::caller()
}

// ================================================================
// ================================================================

/// 実行中の関数名を取得するマクロ
#[cfg(feature = "logging")]
#[macro_export]
macro_rules! current_fn {
    () => {{
        fn __f() {}
        fn __type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let mut name = __type_name_of(__f);
        if let Some(stripped) = name.strip_suffix("::__f") {
            name = stripped;
        }
        while let Some(stripped) = name.strip_suffix("::{{closure}}") {
            name = stripped;
        }
        name
    }};
}

#[cfg(not(feature = "logging"))]
#[macro_export]
macro_rules! current_fn {
    () => {
        ""
    };
}

// ================================================================
// ================================================================

#[cfg(feature = "logging")]
#[macro_export]
macro_rules! trace {
    (None, $debug:expr, $trace_fn:expr) => {
        let id: Option<EntityId> = None;
        $crate::trace!(id, $debug, $trace_fn);
    };

    ($id:expr, $debug:expr, $trace_fn:expr) => {
        let debug = &mut *$debug;
        let _: &DebugStore = debug;
        let _: &mut dyn FnMut() -> MichiuTrace = &mut $trace_fn;

        if debug.dbg_tx.is_some() {
            let record = $crate::MichiuTraceRecord {
                id: $id,
                time: debug.elapsed(),
                func: $crate::current_fn!(),
                loc: $crate::caller_location(),
                trace: $trace_fn(),
            };
            debug.dbg_trace_queue.push(record);
        }
    };
}

#[cfg(not(feature = "logging"))]
#[macro_export]
macro_rules! trace {
    (None, $debug:expr, $trace_fn:expr) => {
        let id: Option<EntityId> = None;
        $crate::trace!(id, $debug, $trace_fn);
    };

    ($id:expr, $debug:expr, $trace_fn:expr) => {
        let debug = &mut *$debug;
        let _: &DebugStore = debug;
        let _: &mut dyn FnMut() -> MichiuTrace = &mut $trace_fn;
    };
}

// ================================================================
// ================================================================

/// `ComposedRenderer` の `draw()` の最後でフラッシュする。
#[cfg(feature = "logging")]
#[macro_export]
macro_rules! flush_trace {
    ($debug:expr) => {
        let debug = &mut *$debug;
        let _: &DebugStore = debug;

        if let Some(ref tx) = debug.dbg_tx
            && !debug.dbg_trace_queue.is_empty()
        {
            let batch = std::mem::take(&mut debug.dbg_trace_queue);
            tx.send_batch(batch);
        }
    };
}

#[cfg(not(feature = "logging"))]
#[macro_export]
macro_rules! flush_trace {
    ($debug:expr) => {
        let debug = &mut *$debug;
        let _: &DebugStore = debug;
    };
}

// ================================================================
// ================================================================

pub type Result<T> = std::result::Result<T, MichiuError>;

// 後々追加
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum MichiuError {
    #[error(
        "Entity {id:?} not found.\n\
            Possible causes:\n\
            - The entity was already despawned/destroyed (dangling ID).\n\
            - An uninitialized or dummy EntityId was used."
    )]
    EntityNotFound { id: EntityId },

    #[error(
        "Component '{component}' not found for Entity {id:?}.\n\
            Possible causes:\n\
            - The component was not registered during spawn.\n
            - The component was removed/detached from the entity.\n\
            - The entity does not possess this property."
    )]
    ComponentNotFound {
        id: EntityId,
        component: &'static str,
    },

    #[error(
        "Signal {0:?} not found.\n\
            Possible causes: It has already been disposed or never registered."
    )]
    SignalDisposed(SignalId),

    #[error(
        "Effect {0:?} not found.\n\
            Possible causes: It has already been disposed or never registered."
    )]
    EffectDisposed(EffectId),
}

// ================================================================
// ================================================================
