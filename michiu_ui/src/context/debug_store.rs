#[allow(unused)]
use crate::{
    ActiveAnimationsSparse, ActiveDragState, ActiveEntitiesVec, ActiveInteractionStates,
    ActiveMasksSecondary, ActiveResizeHoverOption, ActiveTransitionsSparse, ActiveWebviewsHashSet,
    Backdrop, BaseBasicLayoutsSecondary, BaseFlexLayoutsSecondary, BaseVisualPropertiesSecondary,
    BasicLayoutsSecondary, CapacityConfig, ChildrenSecondary, ClipRectsSecondary, ComponentMask,
    ComposedRenderer, Context, DespawnedQueueVec, DfsIndicesSecondary, DirtyLayoutEntitiesVec,
    DirtyRenderEntitiesVec, DndDragPropertiesSparse, DndDropPropertiesSparse, EffectId,
    EffectToElementSecondary, EffectiveZindicesSecondary, ElementEffectsSecondary, ElementState,
    EntitiesSlot, EntityId, ExternalTextureSparse, FlatDfsSequenceVec, FlexLayoutsSecondary,
    GridLayoutsSparse, InputContentsSparse, InteractionPropertiesSecondary, LayoutPoint,
    LayoutSize, MichiuString, Modifiers, MouseButton, ParentsSecondary, PendingDcompRelease,
    PendingElementEffectsVec, PrevClipRectsSecondary, PrevRectsSecondary, ProvidersSparseSecondary,
    QuadInstance, RectsSecondary, RenderData, ResizingState, ResolvedBasicSecondary,
    ResolvedFlexSecondary, ResolvedGridSparse, ScrollOffsetsSecondary, ScrollSizesSecondary,
    ScrollbarStylesSparse, SelectedRectsSparse, SelectionStartIndexSparse, SessionRootsVec,
    SessionSpawnedVec, SignalId, SortCacheVec, SortedEntitiesVec, SubscribersSecondary,
    TaffyNodesSecondary, TextCacheKey, TextCacheValue, TextContentsSparse, TextSelectionsSparse,
    TextSpansSparse, TextureAtlas, UiaPropertiesSparse, VirtualKey, VisualPropertiesSecondary,
    WebviewContentsSparse, WebviewEntitiesVec,
};
use cosmic_text::Buffer;
use rustc_hash::FxHashMap;
use slotmap::{SecondaryMap, SparseSecondaryMap};
use std::{
    borrow::Cow,
    panic::Location,
    path::PathBuf,
    sync::{
        Arc, Mutex, RwLock,
        mpsc::{Receiver, SyncSender},
    },
    time::{Duration, Instant},
};
use thiserror::Error;
#[cfg(feature = "snapshot")]
use windows_core::Interface;

// ================================================================
// ================================================================

#[cfg(feature = "trace-error")]
#[derive(Clone, Default, derive_more::Deref, derive_more::DerefMut, derive_more::IntoIterator)]
#[into_iterator(owned, ref, ref_mut)]
pub(crate) struct InspecterSecondary(SecondaryMap<EntityId, MichiuTraceRecord>);

#[cfg(feature = "trace-error")]
#[derive(Clone, Default, derive_more::Deref, derive_more::DerefMut, derive_more::IntoIterator)]
#[into_iterator(owned, ref, ref_mut)]
pub struct MichiuTraceVec(Vec<MichiuTraceRecord>);

// ================================================================
// ================================================================

#[cfg(feature = "trace-error")]
pub struct DebugStore {
    /// Context が起動した時間を0秒とする
    pub(crate) boot_time: Instant,
    pub(crate) frame: u64,
    // グローバルなエラー用にルート要素のIDを持っておく。
    pub(crate) dbg_root: Option<EntityId>,
    pub(crate) dbg_tx: Option<InspectorSender>,
    // メインスレッドでループ中に溜めておく一時キュー
    pub(crate) dbg_trace_queue: MichiuTraceVec,
}

#[cfg(not(feature = "trace-error"))]
pub(crate) struct DebugStore {
    pub(crate) dbg_root: Option<EntityId>,
}
#[cfg(not(feature = "trace-error"))]
impl DebugStore {
    pub(crate) fn new() -> Self {
        Self { dbg_root: None }
    }
}

#[cfg(feature = "trace-error")]
impl Default for DebugStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "trace-error")]
impl DebugStore {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            boot_time: Instant::now(),
            frame: 0,
            dbg_root: None,
            dbg_tx: None,
            dbg_trace_queue: MichiuTraceVec(Vec::new()),
        }
    }

    #[inline]
    #[must_use]
    pub fn with_inspector(inspector: &MichiuInspector) -> Self {
        Self {
            boot_time: Instant::now(),
            frame: 0,
            dbg_root: None,
            dbg_tx: Some(inspector.sender()),
            dbg_trace_queue: MichiuTraceVec(Vec::new()),
        }
    }

    #[inline]
    pub(crate) fn set_inspector(debug: &mut DebugStore, inspector: &MichiuInspector) {
        debug.dbg_tx = Some(inspector.sender());
    }

    /// 起動時からの経過時間を取得
    #[inline]
    pub(crate) fn since_boot(&self) -> TimeStamp {
        TimeStamp::SinceBoot(self.boot_time.elapsed())
    }

    #[inline]
    pub(crate) fn take_trace_queue(&mut self) -> MichiuTraceVec {
        // 直前の容量を取得
        let next_capacity = self.dbg_trace_queue.capacity().max(512);

        std::mem::replace(
            &mut self.dbg_trace_queue,
            MichiuTraceVec(Vec::with_capacity(next_capacity)),
        )
    }
}

// ================================================================
// ================================================================

#[cfg(feature = "trace-error")]
#[derive(Debug, Clone)]
pub struct InspectorSender {
    // 1フレーム分のバッチを丸ごと送るチャネル
    tx: SyncSender<MichiuTraceVec>,
}

#[cfg(feature = "trace-error")]
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
#[cfg(feature = "trace-error")]
pub type TraceBatch = Arc<MichiuTraceVec>;

// サブスクライバのハンドル
#[cfg(feature = "trace-error")]
pub struct TraceSubscription {
    rx: Receiver<TraceBatch>,
}

#[cfg(feature = "trace-error")]
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

#[cfg(feature = "trace-error")]
#[derive(Clone)]
pub struct MichiuInspector {
    sender: InspectorSender,
    // ワーカースレッドが更新し、ユーザーが読み取るための共有ストレージ
    storage: Arc<RwLock<InspecterSecondary>>,
    // サブスクライバの送信口を束ねて管理
    subscribers: Arc<Mutex<Vec<SyncSender<TraceBatch>>>>,
}

#[cfg(feature = "trace-error")]
impl Default for MichiuInspector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "trace-error")]
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

                if let Ok(mut storage) = storage_clone.write() {
                    for trace in shared_batch.iter() {
                        if let Some(id) = trace.id {
                            // trace 自体も clone せず Arc<MichiuTraceRecord> にする？
                            storage.insert(id, trace.clone());
                        }
                    }
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
    #[cfg(feature = "trace-error")]
    #[inline]
    pub fn set_inspector(&mut self, inspector: &MichiuInspector) {
        DebugStore::set_inspector(&mut self.debug, inspector);
    }
}

// ================================================================
// ================================================================

#[derive(Clone)]
#[repr(u8)]
pub enum MichiuTrace {
    None,

    #[cfg(feature = "trace-lifecycle")]
    Frame(u64),

    #[cfg(feature = "trace-lifecycle")]
    Init {
        capacity: Option<Arc<CapacityConfig>>,
        add: Option<&'static str>,
    },

    #[cfg(feature = "trace-lifecycle")]
    Build {
        old_addr: Option<usize>,
        new_addr: usize,
        marker: usize,
        root: EntityId,
        add: Option<&'static str>,
    },

    #[cfg(feature = "trace-lifecycle")]
    Spawn(Arc<SpawnTrace>),

    #[cfg(feature = "trace-lifecycle")]
    HitTest {
        target: Option<EntityId>,
        x: f32,
        y: f32,
        add: Option<&'static str>,
    },

    #[cfg(feature = "trace-lifecycle")]
    Reactive {
        signal: Option<SignalId>,
        effect: Option<EffectId>,
        kinds: ReactiveKinds,
        add: Option<&'static str>,
    },

    #[cfg(feature = "trace-lifecycle")]
    Event {
        kinds: TraceEventList,
        add: Option<&'static str>,
    },

    #[cfg(feature = "trace-lifecycle")]
    StateUpdate {
        flag: ComponentMask,
        current_masks: ComponentMask,
        actived: bool,
        add: Option<&'static str>,
    },

    #[cfg(feature = "trace-lifecycle")]
    QueueDirty(Arc<DirtyQueueTrace>),

    #[cfg(feature = "trace-lifecycle")]
    Dfs {
        after: Arc<[EntityId]>,
        add: Option<&'static str>,
    },

    #[cfg(feature = "trace-lifecycle")]
    Layout {
        stage: LayoutStage,
        add: Option<&'static str>,
    },

    #[cfg(feature = "trace-lifecycle")]
    Text {
        add: Option<&'static str>,
    },

    #[cfg(feature = "trace-lifecycle")]
    Animation {
        kinds: FrameKinds,
        add: Option<&'static str>,
    },

    #[cfg(feature = "trace-lifecycle")]
    Sorted {
        after: Arc<[EntityId]>,
        add: Option<&'static str>,
    },

    #[cfg(feature = "trace-lifecycle")]
    PrepareRender {
        stage: RenderStage,
        data: Option<Arc<[RenderData]>>,
        add: Option<&'static str>,
    },

    #[cfg(feature = "trace-lifecycle")]
    WriteBuffer {
        staging: Arc<[QuadInstance]>,
        add: Option<&'static str>,
    },

    #[cfg(feature = "trace-lifecycle")]
    Present {
        add: Option<&'static str>,
    },

    #[cfg(feature = "trace-lifecycle")]
    Commit {
        add: Option<&'static str>,
    },

    #[cfg(feature = "trace-lifecycle")]
    Despawn {
        add: Option<&'static str>,
    },

    #[cfg(feature = "trace-lifecycle")]
    Info {
        detail: MichiuInfo,
        fallback: Option<Arc<dyn std::fmt::Debug + Send + Sync>>,
        add: Option<&'static str>,
    },

    #[cfg(feature = "trace-error")]
    Error {
        detail: MichiuError,
        add: Option<&'static str>,
    },

    #[cfg(feature = "snapshot")]
    Snapshot,
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct SpawnTrace {
    pub entities: Option<Vec<EntityId>>,
    pub root: Option<EntityId>,
    pub parents: Option<Vec<EntityId>>,
    pub children: Option<Vec<EntityId>>,
    pub add: Option<&'static str>,
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct DirtyQueueTrace {
    pub kinds: QueueDirtyKinds,
    pub masks: Option<ComponentMask>,
    pub dirty_entities: Option<Vec<EntityId>>,
    pub sort: Option<bool>,
    pub structure: Option<bool>,
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
    /// DFS配列の再構築
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
    /// 初期化からの経過時間
    SinceBoot(MichiuDuration),
    /// 計測開始
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

#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum MichiuInfo {
    None,
    ValueNotFound,
    CacheNotFound,
    CacheChanged,
}

// ================================================================
// ================================================================

#[cfg(feature = "trace-error")]
#[derive(Clone)]
pub struct MichiuTraceRecord {
    pub id: Option<EntityId>,
    pub frame: u64,
    pub time: TimeStamp,
    pub func: &'static str,
    pub loc: &'static Location<'static>,
    pub trace: MichiuTrace,
    #[cfg(feature = "snapshot")]
    pub cx: Option<Arc<ContextSnapshot>>,
    #[cfg(feature = "snapshot")]
    pub renderer: Option<Arc<RendererSnapshot>>,
}

// ================================================================
// ================================================================

#[cfg(feature = "trace-error")]
type MichiuInstant = Instant;
#[cfg(feature = "trace-error")]
type MichiuDuration = Duration;

#[cfg(not(feature = "trace-error"))]
type MichiuInstant = [u8; 0];
#[cfg(not(feature = "trace-error"))]
type MichiuDuration = [u8; 0];

#[cfg(feature = "trace-error")]
pub(crate) struct MichiuStopwatch {
    pub(crate) start: MichiuInstant,
    pub(crate) last: MichiuInstant,
}

#[cfg(feature = "trace-error")]
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

#[cfg(feature = "trace-error")]
#[inline]
#[track_caller]
pub(crate) const fn caller_location() -> &'static Location<'static> {
    Location::caller()
}

// ================================================================
// ================================================================

/// 実行中の関数名を取得するマクロ
#[cfg(feature = "trace-error")]
#[macro_export]
macro_rules! current_fn {
    () => {{
        fn __f() {}
        std::any::type_name_of_val(&__f)
    }};
}

#[cfg(not(feature = "trace-error"))]
#[macro_export]
macro_rules! current_fn {
    () => {
        ""
    };
}

// ================================================================
// ================================================================

#[cfg(feature = "trace-error")]
#[macro_export]
#[doc(hidden)]
macro_rules! _make_record {
    ($id:expr, $debug:expr, $trace:expr) => {
        $crate::MichiuTraceRecord {
            id: $id,
            frame: $debug.frame,
            time: $debug.since_boot(),
            func: $crate::current_fn!(),
            loc: $crate::caller_location(),
            trace: $trace,
            #[cfg(feature = "snapshot")]
            cx: None,
            #[cfg(feature = "snapshot")]
            renderer: None,
        }
    };
}

// ================================================================
// ================================================================

#[cfg(feature = "trace-error")]
#[macro_export]
macro_rules! trace_error {
    (None, $debug:expr, $trace_fn:expr) => {
        let id: Option<$crate::EntityId> = None;
        $crate::trace_error!(id, $debug, $trace_fn);
    };

    ($id:expr, $debug:expr, $trace_fn:expr) => {
        let debug = &mut *$debug;
        let _: &DebugStore = debug;
        let _: &mut dyn FnMut() -> MichiuTrace = &mut $trace_fn;

        if debug.dbg_tx.is_some() {
            let record = $crate::_make_record!($id, debug, $trace_fn());
            debug.dbg_trace_queue.push(record);
        }
    };
}

#[cfg(not(feature = "trace-error"))]
#[macro_export]
macro_rules! trace_error {
    ($id:expr, $debug:expr, $trace_fn:expr) => {
        let _ = ($id, $debug);
    };
}

// ================================================================
// ================================================================

#[cfg(feature = "trace-lifecycle")]
#[macro_export]
macro_rules! trace_lifecycle {
    (None, $debug:expr, $trace_fn:expr) => {
        let id: Option<$crate::EntityId> = None;
        $crate::trace_lifecycle!(id, $debug, $trace_fn);
    };
    
    ($id:expr, $debug:expr, $trace_fn:expr) => {
        let debug = &mut *$debug;
        let _: &$crate::DebugStore = debug;
        let _: &mut dyn FnMut() -> $crate::MichiuTrace = &mut $trace_fn;

        if debug.dbg_tx.is_some() {
            let record = $crate::_make_record!($id, debug, $trace_fn());
            debug.dbg_trace_queue.push(record);
        }
    };
}

#[cfg(not(feature = "trace-lifecycle"))]
#[macro_export]
macro_rules! trace_lifecycle {
    ($id:expr, $debug:expr, $trace_fn:expr) => {
        let _ = $debug;
    };
}

// ================================================================
// ================================================================

#[cfg(feature = "trace-entity")]
#[macro_export]
macro_rules! trace_entity {
    (None, $debug:expr, $trace_fn:expr) => {
        let id: Option<$crate::EntityId> = None;
        $crate::trace!(id, $debug, $trace_fn);
    };

    ($id:expr, $debug:expr, $trace_fn:expr) => {
        let debug = &mut *$debug;
        let _: &DebugStore = debug;
        let _: &mut dyn FnMut() -> MichiuTrace = &mut $trace_fn;

        if debug.dbg_tx.is_some() {
            let record = $crate::_make_record!($id, debug, $trace_fn());
            debug.dbg_trace_queue.push(record);
        }
    };
}

#[cfg(not(feature = "trace-entity"))]
#[macro_export]
macro_rules! trace_entity {
    ($id:expr, $debug:expr, $trace_fn:expr) => {
        let _ = ($id, $debug);
    };
}

// ================================================================
// ================================================================

/// `ComposedRenderer` の `draw()` の最後でフラッシュする。
#[cfg(feature = "snapshot")]
#[macro_export]
macro_rules! flush_trace {
    ($cx:expr, $r:expr) => {
        let _: &Context = $cx;
        let _: &ComposedRenderer = $r;

        if let Some(ref tx) = $cx.debug.dbg_tx.clone() {
            $cx.debug.frame += 1;
            let record = $crate::MichiuTraceRecord {
                id: None,
                frame: $cx.debug.frame,
                time: $cx.debug.elapsed(),
                func: $crate::current_fn!(),
                loc: $crate::caller_location(),
                trace: $crate::MichiuTrace::Snapshot,
                cx: Some(std::sync::Arc::new($crate::ContextSnapshot::flush($cx))),
                renderer: Some(std::sync::Arc::new($crate::RendererSnapshot::flush(
                    $cx, $r,
                ))),
            };
            $cx.debug.dbg_trace_queue.push(record);
            let batch = $cx.debug.take_trace_queue();
            tx.send_batch(batch);
        }
    };
}

#[cfg(all(feature = "trace-error", not(feature = "snapshot")))]
#[macro_export]
macro_rules! flush_trace {
    ($cx:expr, $r:expr) => {
        let _: &$crate::Context = $cx;
        let _: &$crate::ComposedRenderer = $r;

        if let Some(tx) = $cx.debug.dbg_tx.clone() {
            $cx.debug.frame += 1;
            let batch = $cx.debug.take_trace_queue();
            tx.send_batch(batch);
        }
    };
}

#[cfg(not(feature = "trace-error"))]
#[macro_export]
macro_rules! flush_trace {
    ($cx:expr, $r:expr) => {
        let _ = ($cx, $r);
    };
}

// ================================================================
// ================================================================

#[cfg(feature = "snapshot")]
#[derive(Clone)]
pub struct ContextSnapshot {
    pub frame: u64,
    pub window: WindowStoreSnapshot,
    pub system: SystemStoreSnapshot,
    pub reactive: ReactiveStoreSnapshot,
    pub events: EventStoreSnapshot,
    pub contents: ContentStoreSnapshot,
    pub topology: TopologyStoreSnapshot,
    pub states: StateStoreSnapshot,
    pub layouts: LayoutStoreSnapshot,
    pub renders: RenderStoreSnapshot,
    pub outputs: OutputStoreSnapshot,
}

#[derive(Clone)]
pub struct WindowStoreSnapshot {
    pub win_scale_factor: f32,
    pub win_is_resizing: bool,
    pub win_last_size: Option<LayoutSize>,
    pub win_default_himc: Option<usize>,
}

#[derive(Clone)]
pub struct SystemStoreSnapshot {
    pub sys_text_buffers: SparseSecondaryMap<EntityId, Buffer>,
    pub sys_uia_properties: UiaPropertiesSparse,
}

#[derive(Clone)]
pub struct ReactiveStoreSnapshot {
    pub react_subscribers: SubscribersSecondary,
    pub react_element_effects: ElementEffectsSecondary,
    pub react_effect_to_element: EffectToElementSecondary,
    pub react_pending_element_effects: PendingElementEffectsVec,
    pub react_providers: ProvidersSparseSecondary,
}

#[derive(Clone)]
pub struct EventStoreSnapshot {
    pub evt_interaction_states: ActiveInteractionStates,
    pub evt_current_pointer_position: Option<LayoutPoint>,
}

#[derive(Clone)]
pub struct ContentStoreSnapshot {
    pub cont_text_contents: TextContentsSparse,
    pub cont_text_spans: TextSpansSparse,
    pub cont_input_contents: InputContentsSparse,
    pub cont_external_textures: ExternalTextureSparse,
    pub cont_webview_contents: WebviewContentsSparse,
    pub cont_cut_text: Option<MichiuString>,
}

#[derive(Clone)]
pub struct TopologyStoreSnapshot {
    pub topo_entities: EntitiesSlot,
    pub topo_active_entities: ActiveEntitiesVec,
    pub topo_active_masks: ActiveMasksSecondary,
    pub topo_parents: ParentsSecondary,
    pub topo_children: ChildrenSecondary,
    pub topo_session_spawned: SessionSpawnedVec,
    pub topo_session_roots: SessionRootsVec,
    pub topo_flat_dfs_sequence: FlatDfsSequenceVec,
    pub topo_dfs_indices: DfsIndicesSecondary,
    pub topo_effective_z_indices: EffectiveZindicesSecondary,
    pub topo_sorted_entities: SortedEntitiesVec,
    pub topo_sort_cache: SortCacheVec,
    pub topo_is_structure_dirty: bool,
    pub topo_is_sort_dirty: bool,
    pub topo_webview_entities: WebviewEntitiesVec,
    pub topo_despawned_queue: DespawnedQueueVec,
}

#[derive(Clone)]
pub struct StateStoreSnapshot {
    pub dnd: DndStoreSnapshot,
    pub resize: ResizeStoreSnapshot,
    pub scroll: ScrollStoreSnapshot,
    pub edit: TextEditStoreSnapshot,
}

#[derive(Clone)]
pub struct DndStoreSnapshot {
    pub dnd_drag_properties: DndDragPropertiesSparse,
    pub dnd_drop_properties: DndDropPropertiesSparse,
    pub dnd_active_drag_state: Option<ActiveDragState>,
}

#[derive(Clone)]
pub struct ResizeStoreSnapshot {
    pub res_resizing_state: Option<ResizingState>,
    pub res_active_resize_hover: ActiveResizeHoverOption,
}

#[derive(Clone)]
pub struct ScrollStoreSnapshot {
    pub sc_offsets: ScrollOffsetsSecondary,
    pub sc_sizes: ScrollSizesSecondary,
}

#[derive(Clone)]
pub struct TextEditStoreSnapshot {
    pub edit_selections: TextSelectionsSparse,
    pub edit_selection_start_index: SelectionStartIndexSparse,
    pub edit_selected_rects: SelectedRectsSparse,
}

#[derive(Clone)]
pub struct LayoutStoreSnapshot {
    pub scrollbar: ScrollbarStoreSnapshot,
    pub lay_dirty_entities: DirtyLayoutEntitiesVec,
    pub lay_taffy_tree: SendTaffyTree<EntityId>,
    pub lay_taffy_nodes: TaffyNodesSecondary,
    pub lay_basic: BasicLayoutsSecondary,
    pub lay_flex: FlexLayoutsSecondary,
    pub lay_grid: GridLayoutsSparse,
    pub lay_base_basic: BaseBasicLayoutsSecondary,
    pub lay_base_flex: BaseFlexLayoutsSecondary,
    pub lay_resolved_basic: ResolvedBasicSecondary,
    pub lay_resolved_flex: ResolvedFlexSecondary,
    pub lay_resolved_grid: ResolvedGridSparse,
}

#[derive(Clone, derive_more::Deref, derive_more::DerefMut)]
pub struct SendTaffyTree<T>(pub taffy::TaffyTree<T>);

unsafe impl<T: Send> Send for SendTaffyTree<T> {}
unsafe impl<T: Sync> Sync for SendTaffyTree<T> {}

impl<T> From<taffy::TaffyTree<T>> for SendTaffyTree<T> {
    #[inline]
    fn from(tree: taffy::TaffyTree<T>) -> Self {
        Self(tree)
    }
}

#[derive(Clone)]
pub struct ScrollbarStoreSnapshot {
    pub bar_styles: ScrollbarStylesSparse,
}

#[cfg(feature = "snapshot")]
#[derive(Clone)]
pub struct RenderStoreSnapshot {
    pub rnd_dirty_entities: DirtyRenderEntitiesVec,
    pub rnd_visual: VisualPropertiesSecondary,
    pub rnd_base_visual: BaseVisualPropertiesSecondary,
    pub rnd_interaction: InteractionPropertiesSecondary,
    pub rnd_active_transitions: ActiveTransitionsSparse,
    pub rnd_active_animations: ActiveAnimationsSparse,
    pub rnd_active_webviews: ActiveWebviewsHashSet,
    pub rnd_last_tick_time: Option<Instant>,
}

#[derive(Clone)]
pub struct OutputStoreSnapshot {
    pub out_rects: RectsSecondary,
    pub out_clip_rects: ClipRectsSecondary,
    pub out_prev_rects: PrevRectsSecondary,
    pub out_prev_clip_rects: PrevClipRectsSecondary,
}

#[derive(Clone)]
pub struct RendererSnapshot {
    pub frame: u64,
    pub wgpu: WgpuRendererSnapshot,
    pub dcomp: ComposedRendererSnapshot,
}

#[derive(Clone)]
pub struct WgpuRendererSnapshot {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub pipeline: wgpu::RenderPipeline,
    pub punchout_pipeline: wgpu::RenderPipeline,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub instance_buffer: wgpu::Buffer,
    pub instance_buffer_capacity: usize,
    pub instance_staging: Vec<QuadInstance>,
    pub config_buffer: wgpu::Buffer,
    pub config_bind_group: wgpu::BindGroup,
    pub config_bind_group_layout: wgpu::BindGroupLayout,
    pub atlas: TextureAtlas,
    pub temp_uv_map: SecondaryMap<EntityId, [f32; 4]>,
    pub text_cache: FxHashMap<TextCacheKey, TextCacheValue>,
    pub webview_static_caches: FxHashMap<EntityId, wgpu::TextureView>,
    pub render_data: RenderData,
    pub external_bind_groups: FxHashMap<EntityId, (wgpu::TextureView, wgpu::BindGroup)>,
}

#[derive(Clone)]
pub struct ComposedRendererSnapshot {
    pub hwnd: usize,
    pub layout_size: LayoutSize,
    pub scale_factor: f32,
    pub wic_factory: usize,
    pub dcomp_device: usize,
    pub dcomp_target: usize,
    pub root_visual: usize,
    pub wgpu_visual: usize,
    pub webview_env: Option<usize>,
    pub promoted_visuals: Vec<PromotedVisualSnapshot>,
    pub pending_removals: Vec<(EntityId, Option<wgpu::Texture>, Option<MichiuError>)>,
    pub pending_dcomp_releases: Vec<PendingDcompRelease>,
    pub current_backdrop: Backdrop,
    pub resize_cooldown_frames: u32,
}

#[derive(Debug, Clone)]
pub struct PromotedVisualSnapshot {
    pub entity_id: EntityId,
    pub visual: usize,
    pub transform: Option<usize>,
    pub webview_controller: Option<usize>,
    pub is_capturing: bool,
    pub is_visible: bool,
}

// ================================================================
// ================================================================

#[cfg(feature = "snapshot")]
impl ContextSnapshot {
    #[inline]
    pub(crate) fn flush(cx: &Context) -> Self {
        Self {
            frame: cx.debug.frame,
            window: WindowStoreSnapshot {
                win_scale_factor: cx.window.win_scale_factor,
                win_is_resizing: cx.window.win_is_resizing,
                win_last_size: cx.window.win_last_size,
                win_default_himc: cx.window.win_default_himc.map(|h| h.0 as usize),
            },
            system: SystemStoreSnapshot {
                sys_text_buffers: cx
                    .system
                    .sys_text_buffers
                    .borrow()
                    .iter()
                    .map(|(id, rc_buf)| (id, (**rc_buf).clone()))
                    .collect(),
                sys_uia_properties: cx.system.sys_uia_properties.clone(),
            },
            reactive: ReactiveStoreSnapshot {
                react_subscribers: cx.reactive.react_subscribers.clone(),
                react_element_effects: cx.reactive.react_element_effects.clone(),
                react_effect_to_element: cx.reactive.react_effect_to_element.clone(),
                react_pending_element_effects: cx.reactive.react_pending_element_effects.clone(),
                react_providers: cx.reactive.react_providers.clone(),
            },
            events: EventStoreSnapshot {
                evt_interaction_states: cx.events.evt_interaction_states,
                evt_current_pointer_position: cx.events.evt_current_pointer_position,
            },
            contents: ContentStoreSnapshot {
                cont_text_contents: cx.contents.cont_text_contents.clone(),
                cont_text_spans: cx.contents.cont_text_spans.clone(),
                cont_input_contents: cx.contents.cont_input_contents.clone(),
                cont_external_textures: cx.contents.cont_external_textures.clone(),
                cont_webview_contents: cx.contents.cont_webview_contents.clone(),
                cont_cut_text: cx.contents.cont_cut_text.clone(),
            },
            topology: TopologyStoreSnapshot {
                topo_entities: cx.topology.topo_entities.clone(),
                topo_active_entities: cx.topology.topo_active_entities.clone(),
                topo_active_masks: cx.topology.topo_active_masks.clone(),
                topo_parents: cx.topology.topo_parents.clone(),
                topo_children: cx.topology.topo_children.clone(),
                topo_session_spawned: cx.topology.topo_session_spawned.clone(),
                topo_session_roots: cx.topology.topo_session_roots.clone(),
                topo_flat_dfs_sequence: cx.topology.topo_flat_dfs_sequence.clone(),
                topo_dfs_indices: cx.topology.topo_dfs_indices.clone(),
                topo_effective_z_indices: cx.topology.topo_effective_z_indices.clone(),
                topo_sorted_entities: cx.topology.topo_sorted_entities.clone(),
                topo_sort_cache: cx.topology.topo_sort_cache.clone(),
                topo_is_structure_dirty: cx.topology.topo_is_structure_dirty,
                topo_is_sort_dirty: cx.topology.topo_is_sort_dirty,
                topo_webview_entities: cx.topology.topo_webview_entities.clone(),
                topo_despawned_queue: cx.topology.topo_despawned_queue.clone(),
            },
            states: StateStoreSnapshot {
                dnd: DndStoreSnapshot {
                    dnd_drag_properties: cx.states.dnd.dnd_drag_properties.clone(),
                    dnd_drop_properties: cx.states.dnd.dnd_drop_properties.clone(),
                    dnd_active_drag_state: cx.states.dnd.dnd_active_drag_state.clone(),
                },
                resize: ResizeStoreSnapshot {
                    res_resizing_state: cx.states.resize.res_resizing_state.clone(),
                    res_active_resize_hover: cx.states.resize.res_active_resize_hover,
                },
                scroll: ScrollStoreSnapshot {
                    sc_offsets: cx.states.scroll.sc_offsets.clone(),
                    sc_sizes: cx.states.scroll.sc_sizes.clone(),
                },
                edit: TextEditStoreSnapshot {
                    edit_selections: cx.states.edit.edit_selections.clone(),
                    edit_selection_start_index: cx.states.edit.edit_selection_start_index.clone(),
                    edit_selected_rects: cx.states.edit.edit_selected_rects.clone(),
                },
            },
            layouts: LayoutStoreSnapshot {
                scrollbar: ScrollbarStoreSnapshot {
                    bar_styles: cx.layouts.scrollbar.bar_styles.clone(),
                },
                lay_dirty_entities: cx.layouts.lay_dirty_entities.clone(),
                lay_taffy_tree: cx.layouts.lay_taffy_tree.clone().into(),
                lay_taffy_nodes: cx.layouts.lay_taffy_nodes.clone(),
                lay_basic: cx.layouts.lay_basic.clone(),
                lay_flex: cx.layouts.lay_flex.clone(),
                lay_grid: cx.layouts.lay_grid.clone(),
                lay_base_basic: cx.layouts.lay_base_basic.clone(),
                lay_base_flex: cx.layouts.lay_base_flex.clone(),
                lay_resolved_basic: cx.layouts.lay_resolved_basic.clone(),
                lay_resolved_flex: cx.layouts.lay_resolved_flex.clone(),
                lay_resolved_grid: cx.layouts.lay_resolved_grid.clone(),
            },
            renders: RenderStoreSnapshot {
                rnd_dirty_entities: cx.renders.rnd_dirty_entities.clone(),
                rnd_visual: cx.renders.rnd_visual.clone(),
                rnd_base_visual: cx.renders.rnd_base_visual.clone(),
                rnd_interaction: cx.renders.rnd_interaction.clone(),
                rnd_active_transitions: cx.renders.rnd_active_transitions.clone(),
                rnd_active_animations: cx.renders.rnd_active_animations.clone(),
                rnd_active_webviews: cx.renders.rnd_active_webviews.clone(),
                rnd_last_tick_time: cx.renders.rnd_last_tick_time,
            },
            outputs: OutputStoreSnapshot {
                out_rects: cx.outputs.out_rects.clone(),
                out_clip_rects: cx.outputs.out_clip_rects.clone(),
                out_prev_rects: cx.outputs.out_prev_rects.clone(),
                out_prev_clip_rects: cx.outputs.out_prev_clip_rects.clone(),
            },
        }
    }
}

// ================================================================
// ================================================================

#[cfg(feature = "snapshot")]
impl RendererSnapshot {
    #[inline]
    pub(crate) fn flush(cx: &Context, r: &ComposedRenderer) -> Self {
        Self {
            frame: cx.debug.frame,
            wgpu: WgpuRendererSnapshot {
                device: r.wgpu_renderer.device.clone(),
                queue: r.wgpu_renderer.queue.clone(),
                config: r.wgpu_renderer.config.clone(),
                pipeline: r.wgpu_renderer.pipeline.clone(),
                punchout_pipeline: r.wgpu_renderer.punchout_pipeline.clone(),
                vertex_buffer: r.wgpu_renderer.vertex_buffer.clone(),
                index_buffer: r.wgpu_renderer.index_buffer.clone(),
                instance_buffer: r.wgpu_renderer.instance_buffer.clone(),
                instance_buffer_capacity: r.wgpu_renderer.instance_buffer_capacity,
                instance_staging: r.wgpu_renderer.instance_staging.clone(),
                config_buffer: r.wgpu_renderer.config_buffer.clone(),
                config_bind_group: r.wgpu_renderer.config_bind_group.clone(),
                config_bind_group_layout: r.wgpu_renderer.config_bind_group_layout.clone(),
                atlas: r.wgpu_renderer.atlas.clone(),
                temp_uv_map: r.wgpu_renderer.temp_uv_map.clone(),
                text_cache: r.wgpu_renderer.text_cache.clone(),
                webview_static_caches: r.wgpu_renderer.webview_static_caches.clone(),
                render_data: r.wgpu_renderer.render_data.clone(),
                external_bind_groups: r.wgpu_renderer.external_bind_groups.clone(),
            },
            dcomp: ComposedRendererSnapshot {
                hwnd: r.hwnd.0 as usize,
                layout_size: r.layout_size,
                scale_factor: r.scale_factor,
                wic_factory: r.wic_factory.as_raw() as usize,
                dcomp_device: r.dcomp_device.as_raw() as usize,
                dcomp_target: r.dcomp_target.as_raw() as usize,
                root_visual: r.root_visual.as_raw() as usize,
                wgpu_visual: r.wgpu_visual.as_raw() as usize,
                webview_env: r.webview_env.borrow().as_ref().map(|w| w.as_raw() as usize),
                promoted_visuals: r
                    .promoted_visuals
                    .iter()
                    .map(|v| PromotedVisualSnapshot {
                        entity_id: v.entity_id,
                        visual: v.visual.as_raw() as usize,
                        transform: v.transform.clone().map(|t| t.as_raw() as usize),
                        webview_controller: v
                            .webview_controller
                            .borrow()
                            .as_ref()
                            .map(|w| w.as_raw() as usize),
                        is_capturing: v.is_capturing,
                        is_visible: v.is_visible,
                    })
                    .collect(),
                pending_removals: r.pending_removals.borrow().clone(),
                pending_dcomp_releases: r.pending_dcomp_releases.clone(),
                current_backdrop: r.current_backdrop,
                resize_cooldown_frames: r.resize_cooldown_frames,
            },
        }
    }
}

// ================================================================
// ================================================================

pub type Result<T> = std::result::Result<T, MichiuError>;

// 後々追加
#[derive(Error, Debug, Clone)]
pub enum MichiuError {
    #[error(
        "Entity {id:?} not found.\n\
            Possible causes:\n\
            - The entity was already despawned/destroyed (dangling ID).\n\
            - An uninitialized or dummy EntityId was used."
    )]
    EntityNotFound { id: EntityId },

    #[error(
        "Roor entity not found.\n\
            Possible causes:\n\
            - The entity was already despawned/destroyed (dangling ID)."
    )]
    RootEntityNotFound,

    #[error(
        "Signal {id:?} not found.\n\
            Possible causes:\n\
            - The signal was already despawned/destroyed (dangling ID).\n\
            - An uninitialized or dummy SignalId was used."
    )]
    SignalNotFound { id: SignalId },

    #[error(
        "Effect {id:?} not found.\n\
            Possible causes:\n\
            - The effect was already despawned/destroyed (dangling ID).\n\
            - An uninitialized or dummy EffectId was used."
    )]
    EffectNotFound { id: EffectId },

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

    #[error(
        "No active UI element found in the current thread.\n\
            Possible causes:\n\
            - Called a function that requires an element context outside of element build/update lifecycle."
    )]
    NoActiveElement,

    #[error(
        "No active effect found in the current thread.\n\
            Possible causes:\n\
            - Attempted to track reactive dependencies outside of an effect evaluation scope."
    )]
    NoActiveEffect,

    #[error(
        "'{caller}' must be called inside a dynamic reactive context or an active event handler context."
    )]
    ScopeViolation { caller: &'static str },

    #[error("'{caller}' Not supported dynamic nested elements inside")]
    UnsupportedDynamicNesting { caller: &'static str },

    #[error("Downcast failed: Expected type `{expected}`, but the actual type did not match.")]
    DowncastFailed { expected: &'static str },

    #[error(
        "Cyclic dependency / Infinite loop detected.\n\
         Effect {effect_id:?} recursively triggered itself.
         To prevent stack overflow, this recursive run has been skipped."
    )]
    RecursiveEffectDetected { effect_id: EffectId },

    #[error("Windows API Error: {source}")]
    WindowsApiError {
        #[source]
        #[from]
        source: windows_core::Error,
    },

    #[error("Taffy Error: {source}")]
    TaffyError {
        #[source]
        #[from]
        source: taffy::TaffyError,
    },

    #[error("Failded surface creation: {source}")]
    CreateSurfaceError {
        #[source]
        #[from]
        source: wgpu::CreateSurfaceError,
    },

    #[error("Failded surface creation: {source}")]
    RequestAdapterError {
        #[source]
        #[from]
        source: wgpu::RequestAdapterError,
    },

    #[error("Failded request device: {source}")]
    RequestDeviceError {
        #[source]
        #[from]
        source: wgpu::RequestDeviceError,
    },

    #[error("Failed to create D3D11 hardware device.")]
    D3d11DeviceCreationFailed,

    #[error(
        "DCompDeviceManager has not been initialized.\n\
         Please perform the initialization process first."
    )]
    UninitializedDeviceManager,

    #[error("Failed to initialize webview2: {source}")]
    WebView2Error {
        #[source]
        source: windows_core::Error,
    },

    #[error("Failed to lock the OS's global memory. (GlobalLock returned null)")]
    GlobalLockFailed,

    #[error("The image data is invalid or insufficient in size. (PNG header size < 128)")]
    InvalidImageData,

    #[error("Internal error: Callback was executed multiple times.")]
    DuplicateCallback,

    #[error(
        "Failed to place glyphs in the texture atlas.\n\
         Either the character size exceeds the maximum limit, or the atlas is too small.\n\
         Requested glyph: {width}x{height}\n\
         Current atlas max capacity: {atlas_w}x{atlas_h}
         "
    )]
    GlyphAllocationFailed {
        width: u32,
        height: u32,
        atlas_w: u32,
        atlas_h: u32,
    },

    #[error("Image '{path}' loading failed: {source}")]
    ImageLoadFailed {
        path: std::path::PathBuf,
        #[source]
        source: Arc<image::ImageError>,
    },

    #[error("Failed to create custom cursor: {0}")]
    CursorCreationFailed(String),
}

// ================================================================
// ================================================================

pub trait OptionTraceExt<T> {
    /// 値があれば返し、None ならトレースを記録して即座にフラッシュしたあとパニックする
    #[track_caller]
    fn unwrap_or_trace<F>(self, id: Option<EntityId>, debug: &mut DebugStore, err: F) -> T
    where
        F: FnOnce() -> MichiuError;
}

impl<T> OptionTraceExt<T> for Option<T> {
    #[allow(clippy::panic)]
    #[track_caller]
    #[inline]
    fn unwrap_or_trace<F>(self, id: Option<EntityId>, debug: &mut DebugStore, err: F) -> T
    where
        F: FnOnce() -> MichiuError,
    {
        if let Some(val) = self {
            val
        } else {
            let error_detail = err();

            #[cfg(feature = "trace-error")]
            {
                trace_error!(id, debug, || MichiuTrace::Error {
                    detail: error_detail.clone(),
                    add: None,
                });

                if let Some(ref tx) = debug.dbg_tx {
                    let batch = std::mem::take(&mut debug.dbg_trace_queue);
                    tx.send_batch(batch);
                }
            }

            panic!("unwrap_or_trace: {error_detail}");
        }
    }
}

pub trait ResultTraceExt<T> {
    /// 値があれば返し、Err ならトレースを記録して即座にフラッシュしたあとパニックする
    #[track_caller]
    fn unwrap_or_trace(self, id: Option<EntityId>, debug: &mut DebugStore) -> T;
}

impl<T> ResultTraceExt<T> for Result<T> {
    #[allow(clippy::panic)]
    #[track_caller]
    #[inline]
    fn unwrap_or_trace(self, id: Option<EntityId>, debug: &mut DebugStore) -> T {
        match self {
            Ok(val) => val,
            Err(e) => {
                #[cfg(feature = "trace-error")]
                {
                    trace_error!(id, debug, || MichiuTrace::Error {
                        detail: e.clone(),
                        add: None,
                    });

                    if let Some(ref tx) = debug.dbg_tx {
                        let batch = std::mem::take(&mut debug.dbg_trace_queue);
                        tx.send_batch(batch);
                    }
                }

                panic!("unwrap_or_trace: {e}");
            }
        }
    }
}

pub trait TaffyResultTraceExt<T> {
    /// 値があれば返し、Err ならトレースを記録して即座にフラッシュしたあとパニックする
    #[track_caller]
    fn unwrap_or_trace(self, id: Option<EntityId>, debug: &mut DebugStore) -> T;
}

impl<T> TaffyResultTraceExt<T> for taffy::TaffyResult<T> {
    #[allow(clippy::panic)]
    #[track_caller]
    #[inline]
    fn unwrap_or_trace(self, id: Option<EntityId>, debug: &mut DebugStore) -> T {
        match self {
            Ok(val) => val,
            Err(e) => {
                #[cfg(feature = "trace-error")]
                {
                    trace_error!(id, debug, || MichiuTrace::Error {
                        detail: MichiuError::TaffyError { source: e.clone() },
                        add: None,
                    });

                    if let Some(ref tx) = debug.dbg_tx {
                        let batch = std::mem::take(&mut debug.dbg_trace_queue);
                        tx.send_batch(batch);
                    }
                }

                panic!("unwrap_or_trace: {e}");
            }
        }
    }
}

pub trait WindowsResultTraceExt<T> {
    /// 値があれば返し、Err ならトレースを記録して即座にフラッシュしたあとパニックする
    #[track_caller]
    fn unwrap_or_trace(self, id: Option<EntityId>, debug: &mut DebugStore) -> T;
}

impl<T> WindowsResultTraceExt<T> for windows_core::Result<T> {
    #[allow(clippy::panic)]
    #[track_caller]
    #[inline]
    fn unwrap_or_trace(self, id: Option<EntityId>, debug: &mut DebugStore) -> T {
        match self {
            Ok(val) => val,
            Err(e) => {
                #[cfg(feature = "trace-error")]
                {
                    trace_error!(id, debug, || MichiuTrace::Error {
                        detail: MichiuError::WindowsApiError { source: e.clone() },
                        add: None,
                    });

                    if let Some(ref tx) = debug.dbg_tx {
                        let batch = std::mem::take(&mut debug.dbg_trace_queue);
                        tx.send_batch(batch);
                    }
                }

                panic!("unwrap_or_trace: {e}");
            }
        }
    }
}
