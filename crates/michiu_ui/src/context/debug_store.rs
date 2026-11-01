#[cfg(feature = "trace-entity")]
use crate::{ActiveAnimation, ActiveTransition, BasicLayout, ModInner, VisualProperty};
#[expect(unused)]
use crate::{
    ActiveAnimationsSparse, ActiveDragState, ActiveEntitiesVec, ActiveExternalVisualHashSet,
    ActiveInteractionStates, ActiveMasksSecondary, ActiveResizeHoverOption,
    ActiveTransitionsSparse, Backdrop, BaseBasicLayoutsSecondary, BaseFlexLayoutsSecondary,
    BaseVisualPropertiesSecondary, BasicLayoutsSecondary, BatchType, CapacityConfig,
    ChildrenSecondary, ClipRectsSecondary, ComponentMask, Context, DespawnedQueueVec,
    DirtyLayoutEntitiesVec, DirtyRenderEntitiesVec, DndDragPropertiesSparse,
    DndDropPropertiesSparse, EffectId, EffectToEntitySecondary, EffectiveZindicesSecondary,
    ElementState, EntitiesSlot, Entity, EntityEffectsSecondary, ExternalTextureSparse,
    ExternalVisualSparse, FlatDfsSequenceVec, FlexLayoutsSecondary, InputContentsSparse,
    InteractionPropertiesSecondary, LayoutPoint, LayoutRect, LayoutSize, MichiuRenderer,
    MichiuString, MichiuTagRegistry, Modifiers, MouseButton, ParentsSecondary, PendingActions,
    PendingDcompRelease, PendingEntityEffectsVec, PrevClipRectsSecondary, PrevRectsSecondary,
    ProvidersSparseSecondary, QuadInstance, RectsSecondary, RenderData, ResizingState,
    ResolvedBasicSecondary, ResolvedFlexSecondary, ScrollOffsetsSecondary, ScrollSizesSecondary,
    ScrollbarStylesSparse, SelectedRectsSparse, SelectionStartIndexSparse, SessionRootsVec,
    SessionSpawnedVec, SignalId, SortCacheVec, SortedEntitiesVec, SubscribersSecondary,
    TaffyNodesSecondary, TextCacheKey, TextCacheValue, TextContentsSparse, TextSelectionsSparse,
    TextSpansSparse, TextureAtlas, VirtualKey, VisualPropertiesSecondary, WebviewEntitiesVec,
};
use cosmic_text::Buffer;
use rustc_hash::FxHashMap;
use slotmap::{SecondaryMap, SparseSecondaryMap};
use std::{
    borrow::Cow,
    fmt::Debug,
    panic::Location,
    path::PathBuf,
    sync::{
        Arc,
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
pub(crate) struct InspecterSecondary(SecondaryMap<Entity, MichiuTraceRecord>);

#[cfg(feature = "trace-error")]
#[derive(Clone, Default, derive_more::Deref, derive_more::DerefMut, derive_more::IntoIterator)]
#[into_iterator(owned, ref, ref_mut)]
pub struct MichiuTraceVec(Vec<MichiuTraceRecord>);

// ================================================================
// ================================================================

#[cfg(feature = "trace-error")]
pub struct DebugStore {
    // Context が起動した時間を0秒とする
    pub(crate) boot_time: Instant,
    pub(crate) frame: u64,
    // グローバルなエラー用にルート要素のIDを持っておく。
    pub(crate) dbg_root: Option<Entity>,
    pub(crate) dbg_tx: Option<InspectorSender>,
    // メインスレッドでループ中に溜めておく一時キュー
    pub(crate) dbg_trace_queue: MichiuTraceVec,
}

#[cfg(not(feature = "trace-error"))]
pub(crate) struct DebugStore {
    pub(crate) dbg_root: Option<Entity>,
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
    pub(crate) fn new() -> Self {
        Self {
            boot_time: Instant::now(),
            frame: 0,
            dbg_root: None,
            dbg_tx: None,
            dbg_trace_queue: MichiuTraceVec(Vec::new()),
        }
    }

    #[allow(unused)]
    #[inline]
    #[must_use]
    pub(crate) fn with_inspector(inspector: &MichiuInspector) -> Self {
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
    pub(crate) fn take_trace_queue(&mut self) -> Option<MichiuTraceVec> {
        if self.dbg_trace_queue.is_empty() {
            return None;
        }

        // 直前の容量を取得
        let next_capacity = self.dbg_trace_queue.capacity().max(512);

        Some(std::mem::replace(
            &mut self.dbg_trace_queue,
            MichiuTraceVec(Vec::with_capacity(next_capacity)),
        ))
    }

    #[inline]
    pub fn boot_time_mut(&mut self) -> &mut Instant {
        &mut self.boot_time
    }

    #[inline]
    pub fn dbg_root_mut(&mut self) -> &mut Option<Entity> {
        &mut self.dbg_root
    }

    #[inline]
    pub fn trace_queue_mut(&mut self) -> &mut MichiuTraceVec {
        &mut self.dbg_trace_queue
    }

    #[inline]
    pub fn tx_mut(&mut self) -> &mut Option<InspectorSender> {
        &mut self.dbg_tx
    }

    #[inline]
    pub fn frame_mut(&mut self) -> &mut u64 {
        &mut self.frame
    }
}

// ================================================================
// ================================================================

#[cfg(feature = "trace-error")]
#[derive(Debug, Clone)]
pub struct InspectorSender {
    // 1フレーム分のバッチを丸ごと送るチャネル
    pub tx: SyncSender<MichiuTraceVec>,
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
    /// Wait for the next batch (blocking)
    ///
    /// # Errors
    ///
    /// Returns [`std::sync::mpsc::RecvError`] if all senders have been dropped,
    /// indicating that no more batches will be received.
    pub fn recv(&self) -> std::result::Result<TraceBatch, std::sync::mpsc::RecvError> {
        self.rx.recv()
    }

    /// Attempt to retrieve the data in a non-blocking manner.
    ///
    /// # Errors
    ///
    /// - [`std::sync::mpsc::TryRecvError::Empty`]:
    ///   Returns if the channel is currently empty.
    /// - [`std::sync::mpsc::TryRecvError::Disconnected`]:
    ///   Returns if all senders have been dropped.
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
    storage: Arc<parking_lot::RwLock<InspecterSecondary>>,
    // サブスクライバの送信口を束ねて管理
    subscribers: Arc<parking_lot::Mutex<Vec<SyncSender<TraceBatch>>>>,
}

#[cfg(feature = "trace-error")]
impl Default for MichiuInspector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "trace-error")]
impl MichiuInspector {
    #[expect(unused)]
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        use parking_lot::{Mutex, RwLock};

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
                let mut subs = subs_clone.lock();
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

                let mut storage = storage_clone.write();
                for trace in shared_batch.iter() {
                    if let Some(id) = trace.id {
                        // trace 自体も clone せず Arc<MichiuTraceRecord> にする？
                        storage.insert(id, trace.clone());
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

    /// Start a new event subscription.
    ///
    /// If `buffer_size` is None, the default is 128.
    #[track_caller]
    #[inline]
    #[must_use]
    pub fn subscribe(&self, buffer_size: Option<usize>) -> TraceSubscription {
        let bs = buffer_size.unwrap_or(128);
        let (tx, rx) = std::sync::mpsc::sync_channel(bs);
        self.subscribers.lock().push(tx);
        TraceSubscription { rx }
    }

    #[inline]
    #[must_use]
    pub(crate) fn sender(&self) -> InspectorSender {
        self.sender.clone()
    }

    /// Get the latest snapshot
    #[track_caller]
    #[inline]
    #[must_use]
    pub fn get(&self, id: Entity) -> Option<MichiuTraceRecord> {
        self.storage.read().get(id).cloned()
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
pub enum MichiuTrace {
    None,

    /// Record the app's initialization point
    #[cfg(feature = "trace-lifecycle")]
    Init {
        capacity: Option<Arc<CapacityConfig>>,
        add: Option<&'static str>,
    },

    /// Creating a root entity and recording a raw pointer to `Context`
    #[cfg(feature = "trace-lifecycle")]
    BuildEntity {
        old: Option<usize>,
        current: usize,
        root: Entity,
        add: Option<&'static str>,
    },

    /// Record the bound `Context` in the thread-local storage
    #[cfg(feature = "trace-lifecycle")]
    Context {
        current: Option<usize>,
        state: ContextState,
        add: Option<&'static str>,
    },

    /// Record Entity Spawns
    #[cfg(feature = "trace-entity")]
    EntitySpawn {
        parent: Option<Entity>,
        node_id: taffy::NodeId,
        add: Option<&'static str>,
    },

    /// Record entity modifier resolution
    #[cfg(feature = "trace-entity")]
    EntityModifier {
        stage: ModifierStage,
        mask: ComponentMask,
        style: Option<Arc<ModInner>>,
        resolved_visual: Option<Arc<VisualProperty>>,
        resolved_layout: Option<Arc<BasicLayout>>,
        add: Option<&'static str>,
    },

    /// Record hit tests
    #[cfg(feature = "trace-lifecycle")]
    HitTest {
        found: Option<Entity>,
        hit_x: f32,
        hit_y: f32,
        add: Option<&'static str>,
    },

    /// Recording Entity Reactivity
    #[cfg(feature = "trace-entity")]
    EntityReactive {
        signal: Option<SignalId>,
        effect: Option<EffectId>,
        kinds: ReactiveKinds,
        add: Option<&'static str>,
    },

    /// Record the occurrence of entity events
    #[cfg(feature = "trace-lifecycle")]
    Event {
        kinds: Arc<TraceEventList>,
        add: Option<&'static str>,
    },

    /// Track changes to an entities pseudo-classes
    #[cfg(feature = "trace-entity")]
    EntityStateUpdate {
        flag: ComponentMask,
        current_masks: ComponentMask,
        actived: bool,
        add: Option<&'static str>,
    },

    /// Record the animations associated with an entity
    #[cfg(feature = "trace-entity")]
    EntityAnimation {
        kinds: FrameKinds,
        animation: Option<Arc<[ActiveAnimation]>>,
        transition: Option<Arc<[ActiveTransition]>>,
        add: Option<&'static str>,
    },

    /// Record the moment a dirty flag is set on an entity
    #[cfg(feature = "trace-entity")]
    EntityDirtyQueue(Arc<DirtyQueueTrace>),

    /// Recording the Construction of a DFS-Ordered Array
    #[cfg(feature = "trace-lifecycle")]
    Dfs {
        after: Arc<[Entity]>,
        add: Option<&'static str>,
    },

    /// Recording the Layout Resolution Phase
    #[cfg(feature = "trace-lifecycle")]
    Layout {
        stage: LayoutStage,
        add: Option<&'static str>,
    },

    /// Updating the ExternalVisual Entity
    #[cfg(feature = "trace-entity")]
    EntityExternalVisual {
        add: Option<&'static str>,
    },

    /// Record the text layout calculations for an entity
    #[cfg(feature = "trace-entity")]
    EntityText {
        add: Option<&'static str>,
    },

    /// Record the occurrence of animations, transitions, and auto-scrolling
    #[cfg(feature = "trace-lifecycle")]
    Animation {
        kinds: FrameKinds,
        add: Option<&'static str>,
    },

    /// Logging the creation of a sorted array based on visibility and z-index
    #[cfg(feature = "trace-lifecycle")]
    Sorted {
        after: Arc<[Entity]>,
        add: Option<&'static str>,
    },

    /// Recording the collection of rendering data and the construction of `QuadInstance`
    #[cfg(feature = "trace-lifecycle")]
    PrepareRender {
        stage: RenderStage,
        data: Option<Arc<RendererViewTrace>>,
        add: Option<&'static str>,
    },

    /// Recording Wgpu's `write_buffer`
    #[cfg(feature = "trace-lifecycle")]
    WriteBuffer {
        staging: Arc<[QuadInstance]>,
        add: Option<&'static str>,
    },

    /// Record Wgpu's `present`
    #[cfg(feature = "trace-lifecycle")]
    Present {
        add: Option<&'static str>,
    },

    /// Record the `Commit` in Direct Composition
    #[cfg(feature = "trace-lifecycle")]
    Commit {
        add: Option<&'static str>,
    },

    /// Recording the total number of entities and the number of entities with the “dirty” flag
    #[cfg(feature = "trace-lifecycle")]
    ClearDirtyEntities {
        total_entities: usize,
        active_entities: usize,
        dirty_layouts: usize,
        dirty_renders: usize,
        add: Option<&'static str>,
    },

    /// Record the Entity of the despawned entity
    #[cfg(feature = "trace-entity")]
    EntityDespawn {
        add: Option<&'static str>,
    },

    /// Record fallbacks and cache misses
    #[cfg(feature = "trace-entity")]
    EntityInfo {
        detail: MichiuInfo,
        fallback: Option<Arc<dyn std::fmt::Debug + Send + Sync>>,
        add: Option<&'static str>,
    },

    /// Log all internal errors
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
pub enum ContextState {
    Bind,
    Drop,
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum StyleState {
    None,
    Static,
    Dynamic,
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum ModifierStage {
    Build,
    ResolvedVisual,
    ResolvedLayout,
    TriggerAnimations,
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct SpawnTrace {
    pub id: Entity,
    pub parent: Option<Entity>,
    pub node_id: taffy::NodeId,
    pub add: Option<&'static str>,
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct DirtyQueueTrace {
    pub kinds: QueueDirtyKinds,
    pub dirty_entities: Option<Vec<Entity>>,
    pub sort: Option<bool>,
    pub structure: Option<bool>,
    pub add: Option<&'static str>,
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum QueueDirtyKinds {
    None,
    Layout,
    Render,
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum ReactiveKinds {
    None,
    /// Creating a Signal
    CreateSignal,
    /// Provides a signal context for the specified entity or the root entity
    Provide,
    /// Searching for Root Entity
    FindRoot,
    /// Traverse the tree backward toward the parent and resolve `ReadSignal`
    FindReadSignal,
    /// Traverse the tree backward toward the parent and resolve `WriteSignal`
    FindWriteSignal,
    /// Dependency Tracking
    Tracking,
    /// Reading Signals
    Getting,
    /// Rewriting Signals
    Writing,
    /// Evaluate the specified effect in the main thread's context
    ExecuteEffect,
    /// Resolving Running Effects via Thread-Local
    ResolveEffect,
    /// Link Effects by Category
    RegisterEffect,
    /// Register dynamic effects on entities and perform an initial evaluation
    CreateEffect,
    /// Complete the evaluation of all pending effects
    PendingEffect,
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutStage {
    None,
    /// Start Layout Calculations
    Start,
    /// Execute the queued initial evaluations
    FirstEffects,
    /// Early Return if No Calculations Are Needed
    EarlyReturn(DirtyReason),
    /// Reconstructing a DFS-Ordered Array
    RebuildDfs,
    /// Solutions for Each Style
    ResolveLayout,
    /// Synchronizing Changes to the Taffy Tree
    SyncTaffy,
    /// First Layout Calculation
    FirstMeasure,
    /// Text Layout Calculations
    TextMeasure,
    /// First Output Area
    FirstOutputRect,
    /// Scroll Size Calculation
    ScrollSize,
    /// Synchronizing Scroll Bar Styles
    SyncScrollBar,
    /// Second Layout Calculation
    FinalMeasure,
    /// Final Output Area
    FinalOutputRect,
    /// Synchronizing Input Content
    UpdateInputContents,
    /// Scroll Offset Synchronization
    SyncScrollOffsets,
    /// Clearing the Dirty Flag
    ClearDirty,
    /// Completion of Layout Calculations
    End,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DirtyReason {
    pub window_resized: bool,
    pub has_dirty_entities: bool,
    pub is_structure_dirty: bool,
    pub is_empty_output_rect: bool,
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderStage {
    None,
    /// Rendering Phase Begins
    Start,
    /// Calculating the Effective `z_index`, Visibility, and Sorting Entities Based on These Factors
    SortedEntities,
    /// Cache all characters contained in the specified entity in the atlas
    FirstGlyphsCache,
    /// If the Atlas is cleared, rebuild the Atlas and recache it.
    FullGlyphsCache,
    /// Packing
    CollectDate(InstanceKinds),
    /// Batch Flash
    FlushBatch(FlatBufferTrace),
    /// Add an Instance
    PushInstance,
    /// Clearing the Dirty Flag
    ClearDirty,
    End,
}

#[derive(Debug, Clone)]
pub struct RendererViewTrace {
    pub render_data: RenderData,
    pub atlas: TextureAtlas,
    pub text_cache: FxHashMap<TextCacheKey, TextCacheValue>,
    pub queue: wgpu::Queue,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InstanceKinds {
    pub has_external_visual_ready: bool,
    pub has_external_texture: bool,
    pub has_normal_entity: bool,
    pub has_selection_highlight: bool,
    pub has_background: bool,
    pub has_text: bool,
    pub has_fallback_border: bool,
    pub has_caret: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlatBufferTrace {
    pub instances_len: usize,
    pub last_flushed_offset: usize,
    pub scissor_rect: LayoutRect,
    pub batch_type: BatchType,
}

// ================================================================
// ================================================================

#[cfg(feature = "trace-error")]
type MichiuDuration = Duration;

#[cfg(not(feature = "trace-error"))]
type MichiuDuration = [u8; 0];

// ================================================================
// ================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeStamp {
    None,
    /// Time Elapsed Since Initialization
    SinceBoot(MichiuDuration),
    /// Start Measurement
    Start(MichiuDuration),
    /// Current Stage (Time Elapsed Since the Previous Stage)
    Elapsed(MichiuDuration),
    /// Measurement Complete (Total Time)
    End(Duration),
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
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
    Undo,
    Redo,
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum FrameKinds {
    None,
    Transition,
    Animation,
    AutoScroll,
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
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
    pub id: Option<Entity>,
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
        let id: Option<$crate::Entity> = None;
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
        let id: Option<$crate::Entity> = None;
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
        let id: Option<$crate::Entity> = None;
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
                time: $cx.debug.since_boot(),
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
        let _: &$crate::MichiuRenderer = $r;

        if let Some(tx) = $cx.debug.dbg_tx.clone() {
            $cx.debug.frame += 1;
            if let Some(batch) = $cx.debug.take_trace_queue() {
                tx.send_batch(batch);
            }
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
    pub win_is_resized: bool,
    pub win_last_size: Option<LayoutSize>,
    pub win_default_himc: Option<usize>,
}

#[derive(Clone)]
pub struct SystemStoreSnapshot {
    pub sys_text_buffers: SparseSecondaryMap<Entity, Buffer>,
}

#[derive(Clone)]
pub struct ReactiveStoreSnapshot {
    pub react_subscribers: SubscribersSecondary,
    pub react_entity_effects: EntityEffectsSecondary,
    pub react_effect_to_entity: EffectToEntitySecondary,
    pub react_pending_entity_effects: PendingEntityEffectsVec,
    pub react_providers: ProvidersSparseSecondary,
}

#[derive(Clone)]
pub struct EventStoreSnapshot {
    pub evt_interaction_states: ActiveInteractionStates,
    pub evt_current_pointer_position: Option<LayoutPoint>,
    pub evt_pending_actions: PendingActions,
}

#[derive(Clone)]
pub struct ContentStoreSnapshot {
    pub cont_text_contents: TextContentsSparse,
    pub cont_text_spans: TextSpansSparse,
    pub cont_input_contents: InputContentsSparse,
    pub cont_external_textures: ExternalTextureSparse,
    pub cont_external_visual: ExternalVisualSparse,
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
    pub topo_effective_z_indices: EffectiveZindicesSecondary,
    pub topo_sorted_entities: SortedEntitiesVec,
    pub topo_sort_cache: SortCacheVec,
    pub topo_is_structure_dirty: bool,
    pub topo_is_sort_dirty: bool,
    pub topo_webview_entities: WebviewEntitiesVec,
    pub topo_despawned_queue: DespawnedQueueVec,
    pub topo_tag_registry: MichiuTagRegistry,
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
    pub lay_taffy_tree: SendTaffyTree<Entity>,
    pub lay_taffy_nodes: TaffyNodesSecondary,
    pub lay_basic: BasicLayoutsSecondary,
    pub lay_flex: FlexLayoutsSecondary,
    pub lay_base_basic: BaseBasicLayoutsSecondary,
    pub lay_base_flex: BaseFlexLayoutsSecondary,
    pub lay_resolved_basic: ResolvedBasicSecondary,
    pub lay_resolved_flex: ResolvedFlexSecondary,
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
    pub rnd_active_external_visual: ActiveExternalVisualHashSet,
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
    pub temp_uv_map: SecondaryMap<Entity, [f32; 4]>,
    pub text_cache: FxHashMap<TextCacheKey, TextCacheValue>,
    pub webview_static_caches: FxHashMap<Entity, wgpu::TextureView>,
    pub render_data: RenderData,
    pub external_bind_groups: FxHashMap<Entity, (wgpu::TextureView, wgpu::BindGroup)>,
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
    pub pending_removals: Vec<(Entity, Option<wgpu::Texture>, Option<MichiuError>)>,
    pub pending_dcomp_releases: Vec<PendingDcompRelease>,
    pub current_backdrop: Backdrop,
    pub resize_cooldown_frames: u32,
}

#[derive(Debug, Clone)]
pub struct PromotedVisualSnapshot {
    pub entity_id: Entity,
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
                win_is_resized: cx.window.win_is_resized,
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
            },
            reactive: ReactiveStoreSnapshot {
                react_subscribers: cx.reactive.react_subscribers.clone(),
                react_entity_effects: cx.reactive.react_entity_effects.clone(),
                react_effect_to_entity: cx.reactive.react_effect_to_entity.clone(),
                react_pending_entity_effects: cx.reactive.react_pending_entity_effects.clone(),
                react_providers: cx.reactive.react_providers.clone(),
            },
            events: EventStoreSnapshot {
                evt_interaction_states: cx.events.evt_interaction_states,
                evt_current_pointer_position: cx.events.evt_current_pointer_position,
                evt_pending_actions: cx.events.evt_pending_actions.clone(),
            },
            contents: ContentStoreSnapshot {
                cont_text_contents: cx.contents.cont_text_contents.clone(),
                cont_text_spans: cx.contents.cont_text_spans.clone(),
                cont_input_contents: cx.contents.cont_input_contents.clone(),
                cont_external_textures: cx.contents.cont_external_textures.clone(),
                cont_external_visual: cx.contents.cont_external_visual.clone(),
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
                topo_effective_z_indices: cx.topology.topo_effective_z_indices.clone(),
                topo_sorted_entities: cx.topology.topo_sorted_entities.clone(),
                topo_sort_cache: cx.topology.topo_sort_cache.clone(),
                topo_is_structure_dirty: cx.topology.topo_is_structure_dirty,
                topo_is_sort_dirty: cx.topology.topo_is_sort_dirty,
                topo_webview_entities: cx.topology.topo_webview_entities.clone(),
                topo_despawned_queue: cx.topology.topo_despawned_queue.clone(),
                topo_tag_registry: cx.topology.topo_tag_registry.clone(),
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
                rnd_active_external_visual: cx.renders.rnd_active_external_visual.clone(),
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
            - An uninitialized or dummy Entity was used."
    )]
    EntityNotFound { id: Entity },

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
        "Tag {type_name:?} not found.\n\
            Possible causes:\n\
            - The tag is not set, or the name is incorrect.\n\
            - The TypeId/Entity was already despawned/destroyed (dangling ID)."
    )]
    TagNotFound { type_name: &'static str },

    #[error(
        "Component '{component}' not found for Entity {id:?}.\n\
            Possible causes:\n\
            - The component was not registered during spawn.\n
            - The component was removed/detached from the entity.\n\
            - The entity does not possess this property."
    )]
    ComponentNotFound { id: Entity, component: &'static str },

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
        "No active UI entity found in the current thread.\n\
            Possible causes:\n\
            - Called a function that requires an entity context outside of entity build/update lifecycle."
    )]
    NoActiveEntity,

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

    #[error("'{caller}' Not supported dynamic nested entities inside")]
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

    #[error("Failed to create DispatcherQueueController.")]
    DispatcherQueueControllerCreationFailed,

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

    #[error("failed to resolve path '{path}': {source}")]
    CanonicalizeFailed {
        path: PathBuf,
        #[source]
        source: Arc<std::io::Error>,
    },

    #[error("failed to read file '{path}': {source}")]
    ReadStringFailed {
        path: PathBuf,
        #[source]
        source: Arc<std::io::Error>,
    },

    #[error("target path '{path}' does not have a parent directory")]
    NoParentDirectory { path: PathBuf },

    #[error("failed to initialize file watcher: {source}")]
    WatcherInitFailed {
        #[source]
        source: Arc<notify::Error>,
    },

    #[error("failed to watch path '{path}': {source}")]
    WatchTargetFailed {
        path: PathBuf,
        #[source]
        source: Arc<notify::Error>,
    },

    #[error("failed to send task: the main thread has been terminated (receiver disconnected)")]
    SendError,
}

// ================================================================
// ================================================================

pub trait OptionTraceExt<T> {
    /// If a value exists, return it; if it is None, log a trace, immediately flush, and then raise a panic.
    #[track_caller]
    fn unwrap_or_trace<F>(self, id: Option<Entity>, debug: &mut DebugStore, err: F) -> T
    where
        F: FnOnce() -> MichiuError;

    /// If a value exists, return it; if it is None, log a trace and return the default value.
    #[track_caller]
    fn unwrap_or_default_trace(self, id: Option<Entity>, debug: &mut DebugStore) -> T
    where
        T: Default + Debug + Send + Sync + 'static;

    /// If a value exists, return it; otherwise, log a trace and return the specified fallback value.
    #[track_caller]
    fn unwrap_or_fallback_trace(self, id: Option<Entity>, fallback: T, debug: &mut DebugStore) -> T
    where
        T: Clone + Debug + Send + Sync + 'static;

    /// If a value exists, return it; if it is None, log a trace and generate a fallback value using a closure.
    #[track_caller]
    fn unwrap_or_else_trace<F>(self, id: Option<Entity>, debug: &mut DebugStore, f: F) -> T
    where
        T: Default + Clone + Debug + Send + Sync + 'static,
        F: FnOnce() -> T;
}

impl<T> OptionTraceExt<T> for Option<T> {
    #[expect(clippy::panic)]
    #[track_caller]
    #[inline]
    fn unwrap_or_trace<F>(self, id: Option<Entity>, debug: &mut DebugStore, err: F) -> T
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

    #[track_caller]
    #[inline]
    fn unwrap_or_default_trace(self, id: Option<Entity>, debug: &mut DebugStore) -> T
    where
        T: Default + Debug + Send + Sync + 'static,
    {
        #[cfg(not(feature = "trace-entity"))]
        {
            let _ = (id, debug);
            self.unwrap_or_default()
        }

        #[cfg(feature = "trace-entity")]
        if let Some(val) = self {
            val
        } else {
            trace_entity!(id, debug, || MichiuTrace::EntityInfo {
                detail: MichiuInfo::ValueNotFound,
                fallback: Some(Arc::new(T::default())),
                add: Some(std::any::type_name::<T>()),
            });

            T::default()
        }
    }

    #[track_caller]
    #[inline]
    fn unwrap_or_fallback_trace(self, id: Option<Entity>, fallback: T, debug: &mut DebugStore) -> T
    where
        T: Clone + Debug + Send + Sync + 'static,
    {
        #[cfg(not(feature = "trace-entity"))]
        {
            let _ = (id, debug);
            self.unwrap_or(fallback)
        }

        #[cfg(feature = "trace-entity")]
        if let Some(val) = self {
            val
        } else {
            trace_entity!(id, debug, || MichiuTrace::EntityInfo {
                detail: MichiuInfo::ValueNotFound,
                fallback: Some(Arc::new(fallback.clone())),
                add: Some(std::any::type_name::<T>()),
            });

            fallback
        }
    }

    #[track_caller]
    #[inline]
    fn unwrap_or_else_trace<F>(self, id: Option<Entity>, debug: &mut DebugStore, f: F) -> T
    where
        T: Default + Clone + Debug + Send + Sync + 'static,
        F: FnOnce() -> T,
    {
        #[cfg(not(feature = "trace-entity"))]
        {
            let _ = (id, debug);
            self.unwrap_or_else(f)
        }

        #[cfg(feature = "trace-entity")]
        if let Some(val) = self {
            val
        } else {
            let val = f();
            trace_entity!(id, debug, || MichiuTrace::EntityInfo {
                detail: MichiuInfo::ValueNotFound,
                fallback: Some(Arc::new(val.clone())),
                add: Some(std::any::type_name::<T>()),
            });

            val
        }
    }
}

pub trait ResultTraceExt<T> {
    /// If a value exists, return it; if it is `Err`, log a trace, immediately flush, and then panic.
    #[track_caller]
    fn unwrap_or_trace(self, id: Option<Entity>, debug: &mut DebugStore) -> T;
}

impl<T> ResultTraceExt<T> for Result<T> {
    #[expect(clippy::panic)]
    #[track_caller]
    #[inline]
    fn unwrap_or_trace(self, id: Option<Entity>, debug: &mut DebugStore) -> T {
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
    /// If a value exists, return it; if it is `Err`, log a trace, immediately flush, and then panic.
    #[track_caller]
    fn unwrap_or_trace(self, id: Option<Entity>, debug: &mut DebugStore) -> T;
}

impl<T> TaffyResultTraceExt<T> for taffy::TaffyResult<T> {
    #[expect(clippy::panic)]
    #[track_caller]
    #[inline]
    fn unwrap_or_trace(self, id: Option<Entity>, debug: &mut DebugStore) -> T {
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
    /// If a value exists, return it; if it's `Err`, log a trace, immediately flush, and then panic.
    #[track_caller]
    fn unwrap_or_trace(self, id: Option<Entity>, debug: &mut DebugStore) -> T;
}

impl<T> WindowsResultTraceExt<T> for windows_core::Result<T> {
    #[expect(clippy::panic)]
    #[track_caller]
    #[inline]
    fn unwrap_or_trace(self, id: Option<Entity>, debug: &mut DebugStore) -> T {
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

#[cfg(test)]
#[expect(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[cfg(feature = "trace-error")]
    fn create_dummy_record(debug: &DebugStore) -> MichiuTraceRecord {
        MichiuTraceRecord {
            id: None,
            frame: debug.frame,
            time: debug.since_boot(),
            func: "test_fn",
            loc: caller_location(),
            trace: MichiuTrace::None,
            #[cfg(feature = "snapshot")]
            cx: None,
            #[cfg(feature = "snapshot")]
            renderer: None,
        }
    }

    // 1. 初期状態の検証
    // `test_debug_store_new_and_default`
    //  - `DebugStore::new()` および `DebugStore::default()` で初期化。
    //  - `frame` が 0、`dbg_root` が `None`、`dbg_tx` が `None`、`dbg_trace_queue` が空であることを確認。
    //
    // 2. インスペクター連携
    // `test_debug_store_with_inspector`
    //  - `MichiuInspector::new()` を生成し、`DebugStore::with_inspector(&inspector)` で構築。
    //  - `dbg_tx` が `Some` に設定されることを確認。
    //
    // `test_debug_store_set_inspector`
    //  - デフォルトの `DebugStore` に対し `DebugStore::set_inspector(&mut store, &inspector)` を呼び出す。
    //  - `dbg_tx` が `None` から `Some` に更新されることを確認。
    //
    // 3. トレースキューの取り出し（`take_trace_queue`）
    // `test_take_trace_queue_when_empty`
    //  - キューが空の状態で `take_trace_queue()` を呼び出した場合、`None` が返ることを確認。
    //
    // `test_take_trace_queue_with_elements`
    //  - キューにダミーのレコードを追加した状態で `take_trace_queue()` を呼び出す。
    //  - `Some(batch)` が返り、元の `dbg_trace_queue` が空になることを確認。
    //
    // `test_take_trace_queue_capacity_retention`
    //  - キューをクリアした後の `dbg_trace_queue` の capacity が、直前の容量と 512 の大きい方にリセットされる仕様（`max(512)`）の検証。
    //    - レコード数 1 の場合：取り出し後の capacity >= 512
    //    - レコード数 1000 の場合：取り出し後の capacity >= 1000
    //
    // 4. 起動経過時間（`since_boot`）
    // `test_debug_store_since_boot`
    //  - `store.since_boot()` の戻り値が `TimeStamp::SinceBoot(duration)` であることを確認。
    //  - 微小なスリープを挟んで取得した値が単調増加していることを確認。
    //
    // 5. 可変アクセサ（`_mut`）経由での更新
    // `test_debug_store_mut_accessors`
    //  - `frame_mut()`: 値のインクリメントおよび更新確認。
    //  - `dbg_root_mut()`: `Entity` のセット・リセット確認。
    //  - `trace_queue_mut()`: 外部からの直接 push / clear 操作の反映確認。
    //  - `boot_time_mut()`: `Instant` の書き換え確認。
    //  - `tx_mut()`: 送信チャンネルの直接クリア・設定確認。

    #[test]
    #[cfg(feature = "trace-error")]
    fn test_debug_store_new_and_default() {
        let store1 = DebugStore::new();
        assert_eq!(store1.frame, 0);
        assert!(store1.dbg_root.is_none());
        assert!(store1.dbg_tx.is_none());
        assert!(store1.dbg_trace_queue.is_empty());

        let store2 = DebugStore::default();
        assert_eq!(store2.frame, 0);
        assert!(store2.dbg_root.is_none());
        assert!(store2.dbg_tx.is_none());
        assert!(store2.dbg_trace_queue.is_empty());
    }

    #[test]
    #[cfg(feature = "trace-error")]
    fn test_debug_store_with_inspector() {
        let inspector = MichiuInspector::new();
        let store = DebugStore::with_inspector(&inspector);
        assert!(store.dbg_tx.is_some());
        assert_eq!(store.frame, 0);
        assert!(store.dbg_trace_queue.is_empty());
    }

    #[test]
    #[cfg(feature = "trace-error")]
    fn test_debug_store_set_inspector() {
        let inspector = MichiuInspector::new();
        let mut store = DebugStore::new();
        assert!(store.dbg_tx.is_none());

        DebugStore::set_inspector(&mut store, &inspector);
        assert!(store.dbg_tx.is_some());
    }

    #[test]
    #[cfg(feature = "trace-error")]
    fn test_take_trace_queue_when_empty() {
        let mut store = DebugStore::new();
        assert!(store.take_trace_queue().is_none());
    }

    #[test]
    #[cfg(feature = "trace-error")]
    fn test_take_trace_queue_with_elements() {
        let mut store = DebugStore::new();
        let record = create_dummy_record(&store);
        store.dbg_trace_queue.push(record);

        let batch = store.take_trace_queue();
        assert!(batch.is_some());
        let batch = batch.unwrap();
        assert_eq!(batch.len(), 1);
        assert!(store.dbg_trace_queue.is_empty());
    }

    #[test]
    #[cfg(feature = "trace-error")]
    fn test_take_trace_queue_capacity_retention() {
        let mut store = DebugStore::new();

        // 1件追加時：取り出し後の新規バッファは最低512のcapacityを確保する
        let record = create_dummy_record(&store);
        store.dbg_trace_queue.push(record);
        let _ = store.take_trace_queue();
        assert!(store.dbg_trace_queue.capacity() >= 512);

        // 1000件確保時：直前の容量(1000以上)が引き継がれる
        store.dbg_trace_queue.reserve(1000);
        let cap_before = store.dbg_trace_queue.capacity();
        assert!(cap_before >= 1000);

        let record = create_dummy_record(&store);
        store.dbg_trace_queue.push(record);
        let _ = store.take_trace_queue();
        assert!(store.dbg_trace_queue.capacity() >= cap_before);
    }

    #[test]
    #[cfg(feature = "trace-error")]
    fn test_debug_store_since_boot() {
        let store = DebugStore::new();
        let time1 = store.since_boot();

        std::thread::sleep(Duration::from_millis(5));
        let time2 = store.since_boot();

        match (time1, time2) {
            (TimeStamp::SinceBoot(d1), TimeStamp::SinceBoot(d2)) => {
                assert!(d2 > d1);
            }
            _ => panic!("Expected TimeStamp::SinceBoot"),
        }
    }

    #[test]
    #[cfg(feature = "trace-error")]
    fn test_debug_store_mut_accessors() {
        let mut store = DebugStore::new();

        // frame_mut
        *store.frame_mut() = 42;
        assert_eq!(store.frame, 42);

        // trace_queue_mut
        let record = create_dummy_record(&store);
        store.trace_queue_mut().push(record);
        assert_eq!(store.dbg_trace_queue.len(), 1);

        // tx_mut
        let inspector = MichiuInspector::new();
        *store.tx_mut() = Some(inspector.sender());
        assert!(store.dbg_tx.is_some());
        *store.tx_mut() = None;
        assert!(store.dbg_tx.is_none());

        // boot_time_mut
        let past = Instant::now().checked_sub(Duration::from_secs(10)).unwrap();
        *store.boot_time_mut() = past;
        match store.since_boot() {
            TimeStamp::SinceBoot(d) => assert!(d >= Duration::from_secs(10)),
            _ => panic!("Expected TimeStamp::SinceBoot"),
        }
    }

    // 1. `MichiuInspector` の検証
    //
    // `test_inspector_subscription_and_delivery`
    //  - `MichiuInspector::new()` を生成し、`subscribe()` で受信ハンドルを取得。
    //  - `inspector.sender().send_batch(...)` でバッチを送信。
    //  - サブスクライバの `recv()` で送られたバッチが取得できること。
    //
    // `test_inspector_storage_and_get`
    //  - `id: Some(entity)` を持つレコードを含むバッチを送信。
    //  - ワーカースレッドによる処理後、`inspector.get(entity)` で該当レコードが取得できること。
    //  - `id: None` のレコードは storage に影響を与えないこと。
    //
    // `test_inspector_subscriber_disconnect`
    //  - サブスクライバを drop した後、再度バッチを送信。
    //  - ワーカースレッドがパニックせず、内部の送信先リストから切断された購読者が除外されること。
    //
    // `test_inspector_sender_buffer_full`
    //  - 受信しないサブスクライバ（小バッファ `buffer_size: Some(1)`）に対して容量を超えるバッチを送信。
    //  - メイン側がブロックされずに `send_batch` を完了できること（非同期/ノンブロッキング検証）。

    #[test]
    #[cfg(feature = "trace-error")]
    fn test_inspector_subscription_and_delivery() {
        let inspector = MichiuInspector::new();
        let sub = inspector.subscribe(Some(16));

        let store = DebugStore::new();
        let mut batch = MichiuTraceVec(Vec::new());
        batch.push(create_dummy_record(&store));

        inspector.sender().send_batch(batch);

        let received = sub.recv().expect("Failed to receive batch");
        assert_eq!(received.len(), 1);
    }

    #[test]
    #[cfg(feature = "trace-error")]
    fn test_inspector_subscriber_disconnect() {
        let inspector = MichiuInspector::new();
        {
            let _sub = inspector.subscribe(Some(16));
            // スコープ終了で sub がドロップされる
        }

        let store = DebugStore::new();
        let mut batch = MichiuTraceVec(Vec::new());
        batch.push(create_dummy_record(&store));

        // ドロップ済みでもメイン側がパニックせず送信を完了できる
        inspector.sender().send_batch(batch);
    }

    // 2. トレースマクロ群の検証
    //
    // `test_current_fn_macro`
    //  - `current_fn!()` の戻り値が空でなく、実行中のテスト関数名を含む文字列であること。
    //
    // `test_trace_error_macro_with_and_without_tx`
    //  - `dbg_tx: None` の場合：マクロを実行しても `dbg_trace_queue` が増加しないこと。
    //  - `dbg_tx: Some` の場合：マクロ実行によりキューにレコードが 1 件追加されること。
    //  - レコードの `func`, `loc`, `frame`, `id` が意図した値であること。
    //
    // `test_trace_lifecycle_and_trace_entity_macros`
    //  - 有効化された feature の下で、同様に `dbg_tx` の有無に応じてキューへ追加されること。
    //
    // 3. `OptionTraceExt` の検証
    //
    // `test_option_unwrap_or_trace_some`
    //  - `Some(v).unwrap_or_trace(...)` が `v` を返し、パニックしないこと。
    //
    // `test_option_unwrap_or_trace_none_panics`
    //  - `None.unwrap_or_trace(...)` が指定エラーメッセージでパニックすること。
    //  - `dbg_tx: Some` の場合、パニック直前にトレースキューがフラッシュされること。
    //
    // `test_option_unwrap_or_default_trace`
    //  - `Some(v)` の場合は `v` を返す。
    //  - `None` の場合は `T::default()` を返す（`trace-entity` 有効時はトレースが記録されること）。
    //
    // `test_option_unwrap_or_fallback_trace`
    //  - `Some(v)` の場合は `v` を返し、`None` の場合は指定したフォールバック値を返すこと。
    //
    // `test_option_unwrap_or_else_trace`
    //  - `Some(v)` の場合は `v` を返し、クロージャが実行されないこと。
    //  - `None` の場合はクロージャを実行した値を返すこと。

    #[test]
    fn test_current_fn_macro() {
        let name = current_fn!();
        #[cfg(feature = "trace-error")]
        assert!(name.contains("test_current_fn_macro"));
        #[cfg(not(feature = "trace-error"))]
        assert_eq!(name, "");
    }

    #[test]
    #[cfg(feature = "trace-error")]
    fn test_trace_error_macro() {
        let mut store = DebugStore::new();

        // dbg_tx が None の場合はキューイングされない
        trace_error!(None, &mut store, || MichiuTrace::Error {
            detail: MichiuError::RootEntityNotFound,
            add: None,
        });
        assert!(store.dbg_trace_queue.is_empty());

        // dbg_tx を設定した場合はキューイングされる
        let inspector = MichiuInspector::new();
        DebugStore::set_inspector(&mut store, &inspector);

        trace_error!(None, &mut store, || MichiuTrace::Error {
            detail: MichiuError::RootEntityNotFound,
            add: None,
        });
        assert_eq!(store.dbg_trace_queue.len(), 1);
        assert!(matches!(
            store.dbg_trace_queue[0].trace,
            MichiuTrace::Error {
                detail: MichiuError::RootEntityNotFound,
                add: None,
            }
        ));
    }

    #[test]
    fn test_option_unwrap_or_trace_ok() {
        let mut store = DebugStore::new();
        let opt = Some(100);
        let val = opt.unwrap_or_trace(None, &mut store, || MichiuError::RootEntityNotFound);
        assert_eq!(val, 100);
    }

    #[test]
    #[cfg(feature = "trace-error")]
    fn test_option_unwrap_or_trace_panic_and_record() {
        let inspector = MichiuInspector::new();
        let sub = inspector.subscribe(Some(16));
        let mut store = DebugStore::with_inspector(&inspector);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let opt: Option<i32> = None;
            opt.unwrap_or_trace(None, &mut store, || MichiuError::RootEntityNotFound);
        }));

        assert!(result.is_err());
        let received = sub.recv().expect("Failed to receive flushed error batch");
        assert_eq!(received.len(), 1);
        assert!(matches!(
            received[0].trace,
            MichiuTrace::Error {
                detail: MichiuError::RootEntityNotFound,
                add: None,
            }
        ));
    }

    #[test]
    fn test_option_unwrap_or_default_trace() {
        let mut store = DebugStore::new();
        let opt_some = Some(42);
        assert_eq!(opt_some.unwrap_or_default_trace(None, &mut store), 42);

        let opt_none: Option<i32> = None;
        assert_eq!(opt_none.unwrap_or_default_trace(None, &mut store), 0);
    }

    #[test]
    fn test_option_unwrap_or_fallback_trace() {
        let mut store = DebugStore::new();
        let opt_some = Some("exists");
        assert_eq!(
            opt_some.unwrap_or_fallback_trace(None, "fallback", &mut store),
            "exists"
        );

        let opt_none: Option<&str> = None;
        assert_eq!(
            opt_none.unwrap_or_fallback_trace(None, "fallback", &mut store),
            "fallback"
        );
    }

    #[test]
    fn test_option_unwrap_or_else_trace() {
        let mut store = DebugStore::new();
        let mut fallback_called = false;

        let opt_some = Some(10);
        let val1 = opt_some.unwrap_or_else_trace(None, &mut store, || {
            fallback_called = true;
            99
        });
        assert_eq!(val1, 10);
        assert!(!fallback_called);

        let opt_none: Option<i32> = None;
        let val2 = opt_none.unwrap_or_else_trace(None, &mut store, || {
            fallback_called = true;
            99
        });
        assert_eq!(val2, 99);
        assert!(fallback_called);
    }

    // 4. `Result` 系拡張トレイトの検証
    //
    // `test_result_unwrap_or_trace` (`michiu_ui::Result`)
    //  - `Ok(val)`: `val` をそのまま返すこと。
    //  - `Err(MichiuError)`: パニックし、`dbg_tx` が設定されている場合はエラーレコードが送信されること。
    //
    // `test_taffy_result_unwrap_or_trace`
    //  - `Ok(val)`: `val` をそのまま返すこと。
    //  - `Err(taffy::TaffyError)`: パニックすること。
    //
    // `test_windows_result_unwrap_or_trace`
    //  - `Ok(val)`: `val` をそのまま返すこと。
    //  - `Err(windows_core::Error)`: パニックすること。

    #[test]
    fn test_result_unwrap_or_trace() {
        let mut store = DebugStore::new();

        let ok_res: Result<i32> = Ok(123);
        assert_eq!(ok_res.unwrap_or_trace(None, &mut store), 123);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let err_res: Result<i32> = Err(MichiuError::RootEntityNotFound);
            err_res.unwrap_or_trace(None, &mut store);
        }));
        assert!(result.is_err());
    }

    #[test]
    fn test_taffy_result_unwrap_or_trace() {
        let mut store = DebugStore::new();

        let ok_res: taffy::TaffyResult<i32> = Ok(10);
        assert_eq!(ok_res.unwrap_or_trace(None, &mut store), 10);

        let mut tree = taffy::TaffyTree::<()>::new();
        let leaf = tree.new_leaf(taffy::Style::default()).unwrap();
        // 子を持たないノードから0番目の子を取得しようとして TaffyError を発生させる
        let err_res = tree.child_at_index(leaf, 0);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            err_res.unwrap_or_trace(None, &mut store);
        }));
        assert!(result.is_err());
    }

    #[test]
    fn test_windows_result_unwrap_or_trace() {
        let mut store = DebugStore::new();

        let ok_res: windows_core::Result<i32> = Ok(20);
        assert_eq!(ok_res.unwrap_or_trace(None, &mut store), 20);

        let err_res: windows_core::Result<i32> =
            Err(windows_core::Error::from_hresult(windows_core::HRESULT(-1)));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            err_res.unwrap_or_trace(None, &mut store);
        }));
        assert!(result.is_err());
    }

    // 5. `MichiuError` の検証
    // 1. `From` トレイトによる自動型変換（`?` 演算子の連携）
    // `#[from]` 属性が付与されている外部ライブラリのエラーが、`Into` / `From` で正しく `MichiuError` に変換されるか。
    //  * `windows_core::Error` $\to$ `MichiuError::WindowsApiError`
    //  * `taffy::TaffyError` $\to$ `MichiuError::TaffyError`
    //  * `wgpu::CreateSurfaceError` $\to$ `MichiuError::CreateSurfaceError`
    //  * `wgpu::RequestAdapterError` $\to$ `MichiuError::RequestAdapterError`
    //  * `wgpu::RequestDeviceError` $\to$ `MichiuError::RequestDeviceError`
    //
    // 2. エラーチェーン（`std::error::Error::source`）の検証
    // `#[source]` 属性が指定されているバリアントにおいて、`.source()` から元の根本原因エラーを取り出せるか。
    //  * `source()` が `Some` を返すこと。
    //  * `Arc` でラップされた `ImageLoadFailed` や `CanonicalizeFailed` などで内部ソースが保持されていること。
    //
    // 3. 表示メッセージ（`Display` / `to_string()`）の書式検証
    // バリアントごとに埋め込んだパラメータ（`Entity`, `SignalId`, `caller` 名など）が、意図通りエラー文字列に反映されているか。
    //  * 代表的なバリアント（`EntityNotFound`, `ComponentNotFound`, `ScopeViolation`, `GlyphAllocationFailed` など）で文字列書式を確認。
    //
    // 4. トレイト境界の保証（`Send` / `Sync` / `Clone`）
    // UIスレッドやワーカースレッド間を行き来するため、型がスレッドセーフ（`Send + Sync`）かつクローン可能であることを静的に保証する。
    //  * コンパイル時型アサーション（関数定義による制約チェック）。

    #[test]
    fn test_michiu_error_traits_boundary() {
        // Send + Sync + Clone + 'static を満たしていることを静的に検証
        fn assert_traits<T: Send + Sync + Clone + std::error::Error + 'static>() {}
        assert_traits::<MichiuError>();
    }

    #[test]
    fn test_michiu_error_from_conversions() {
        // windows_core::Error -> MichiuError
        let win_err = windows_core::Error::from_hresult(windows_core::HRESULT(-1));
        let err: MichiuError = win_err.into();
        assert!(matches!(err, MichiuError::WindowsApiError { .. }));

        // taffy::TaffyError -> MichiuError
        let mut tree = taffy::TaffyTree::<()>::new();
        let leaf = tree.new_leaf(taffy::Style::default()).unwrap();
        let taffy_err = tree.child_at_index(leaf, 0).unwrap_err();
        let err: MichiuError = taffy_err.into();
        assert!(matches!(err, MichiuError::TaffyError { .. }));
    }

    #[test]
    fn test_michiu_error_source_chain() {
        use std::error::Error;

        // #[source] を持つバリアントで source() が取得できるか
        let io_err = Arc::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        let err = MichiuError::CanonicalizeFailed {
            path: PathBuf::from("test/path"),
            source: Arc::clone(&io_err),
        };
        assert!(err.source().is_some());

        // sourceを持たないバリアントでは None が返るか
        let err_no_source = MichiuError::RootEntityNotFound;
        assert!(err_no_source.source().is_none());
    }

    #[test]
    fn test_michiu_error_display_formatting() {
        let err = MichiuError::ComponentNotFound {
            id: slotmap::Key::null(),
            component: "Transform",
        };
        let msg = err.to_string();
        assert!(msg.contains("Component 'Transform' not found"));

        let err = MichiuError::ScopeViolation {
            caller: "use_signal",
        };
        let msg = err.to_string();
        assert!(msg.contains("'use_signal' must be called inside"));

        let err = MichiuError::GlyphAllocationFailed {
            width: 64,
            height: 64,
            atlas_w: 1024,
            atlas_h: 1024,
        };
        let msg = err.to_string();
        assert!(msg.contains("64x64"));
        assert!(msg.contains("1024x1024"));
    }
}
