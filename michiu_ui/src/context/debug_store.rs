use crate::{
    ComponentMask, Context, DrawBatch, EffectId, ElementState, EntityId, LayoutPoint, LayoutRect,
    Modifiers, MouseButton, Point, QuadInstance, RenderData, SignalId, UserAction, VirtualKey,
    define_secondary,
};
use slotmap::SecondaryMap;
use std::{
    borrow::Cow,
    ops::Range,
    path::PathBuf,
    sync::{
        Arc, RwLock,
        mpsc::{Receiver, Sender, SyncSender},
    },
    time::{Duration, Instant},
};
use thiserror::Error;

#[derive(
    Debug, Clone, Default, derive_more::Deref, derive_more::DerefMut, derive_more::IntoIterator,
)]
#[into_iterator(owned, ref, ref_mut)]
pub(crate) struct InspecterSecondary(SecondaryMap<EntityId, MichiuTrace>);

#[derive(
    Debug, Clone, Default, derive_more::Deref, derive_more::DerefMut, derive_more::IntoIterator,
)]
#[into_iterator(owned, ref, ref_mut)]
pub(crate) struct MichiuTraceVec(Vec<(EntityId, MichiuTrace)>);

pub struct DebugStore {
    // グローバルなエラー用にルート要素のIDを持っておく。
    pub(crate) dbg_root: Option<EntityId>,
    pub(crate) dbg_tx: Option<InspectorSender>,
    // メインスレッドでループ中に溜めておく一時キュー
    pub(crate) dbg_trace_queue: MichiuTraceVec,
}

impl Default for DebugStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugStore {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
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
    pub(crate) fn trace(id: EntityId, debug: &mut DebugStore, trace: MichiuTrace) {
        if debug.dbg_tx.is_some() {
            debug.dbg_trace_queue.push((id, trace));
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
}

// ================================================================
/*
 メインスレッド（UI・描画）
   - ループ中：dbg_trace_queue.push((id, trace))
   - フレーム末：take してワーカーへ送信
───────────────────────────────────────────────────────
 転送: Vec<(EntityId, MichiuTrace)>
───────────────────────────────────────────────────────
 ワーカースレッド
   - 届いた Vec をループして SecondaryMap に詰め替える
   - 最新状態を shared_storage に反映
───────────────────────────────────────────────────────
 読み取り (get)
───────────────────────────────────────────────────────
 ユーザー / デバッグツール
   ・inspector.get(id) で Entity ごとの最新状態を取得
───────────────────────────────────────────────────────
 */
// ================================================================

#[derive(Debug, Clone)]
pub struct InspectorSender {
    // 1フレーム分のバッチを丸ごと送るチャネル
    tx: SyncSender<MichiuTraceVec>,
}

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

#[derive(Debug, Clone)]
pub struct MichiuInspector {
    sender: InspectorSender,
    // ワーカースレッドが更新し、ユーザーが読み取るための共有ストレージ
    storage: Arc<RwLock<InspecterSecondary>>,
}

// ================================================================
// ================================================================

impl Default for MichiuInspector {
    fn default() -> Self {
        Self::new()
    }
}

impl MichiuInspector {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel(256);
        let storage = Arc::new(RwLock::new(InspecterSecondary(SecondaryMap::new())));

        let storage_clone = Arc::clone(&storage);
        // ワーカースレッド起動
        std::thread::spawn(move || {
            Self::worker_loop(&rx, &storage_clone);
        });

        Self {
            sender: InspectorSender { tx },
            storage,
        }
    }

    #[inline]
    fn worker_loop(
        rx: &Receiver<MichiuTraceVec>,
        shared_storage: &Arc<RwLock<InspecterSecondary>>,
    ) {
        // ワーカースレッドのローカルなデータ
        let mut local_storage = InspecterSecondary(SecondaryMap::new());

        while let Ok(batch) = rx.recv() {
            // ローカルデータをバッチで更新
            // 同一フレーム内に複数回同じEntityが来ても、
            // 順番に処理されるので最終的に一番最後の最新状態が残る
            // 後々リングバッファ等に保存してタイムラインとして残すことも検討
            for (entity, trace) in batch {
                local_storage.insert(entity, trace);
            }

            // 詰め替え終わった最新スナップショットを共有ストレージに反映
            if let Ok(mut lock) = shared_storage.write() {
                *lock = local_storage.clone();
            }
        }
    }

    #[inline]
    #[must_use]
    pub fn sender(&self) -> InspectorSender {
        self.sender.clone()
    }

    #[inline]
    #[must_use]
    pub fn get(&self, id: EntityId) -> Option<MichiuTrace> {
        self.storage.read().unwrap().get(id).cloned()
    }
}

// ================================================================
// ================================================================

impl Context {
    #[inline]
    pub fn set_inspector(&mut self, inspector: &MichiuInspector) {
        DebugStore::set_inspector(&mut self.debug, inspector);
    }

    /// ループ中の各所で呼ぶ。チャネルには投げず一時キューに詰めるだけ
    #[inline]
    pub(crate) fn trace(&mut self, id: EntityId, trace: MichiuTrace) {
        DebugStore::trace(id, &mut self.debug, trace);
    }

    /// フレームの最後で呼んで一括転送
    #[inline]
    pub(crate) fn flush_trace(&mut self) {
        DebugStore::flush_trace(&mut self.debug);
    }
}

// ================================================================
// ================================================================

// グローバルなエラーについて要検討
#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum MichiuTrace {
    Spawn(Arc<SpawnTrace>),
    HitTest {
        time: TimeStamp,
        target: EntityId,
        x: f32,
        y: f32,
        func: &'static str,
        add: Option<Cow<'static, str>>,
    },
    Reactive {
        time: TimeStamp,
        signal: Option<SignalId>,
        effect: Option<EffectId>,
        kinds: ReactiveKinds,
        func: &'static str,
        add: Option<Cow<'static, str>>,
    },
    Event {
        time: TimeStamp,
        kinds: TraceEventList,
        func: &'static str,
        add: Option<Cow<'static, str>>,
    },
    StateUpdate {
        time: TimeStamp,
        flag: ComponentMask,
        current: ComponentMask,
        actived: bool,
        func: &'static str,
        add: Option<Cow<'static, str>>,
    },
    QueueDirty(Arc<QueueDirtyTrace>),
    Dfs {
        time: TimeStamp,
        after: Arc<[EntityId]>,
        func: &'static str,
        add: Option<Cow<'static, str>>,
    },
    Layout {
        time: TimeStamp,
        stage: LayoutStage,
        func: &'static str,
        add: Option<Cow<'static, str>>,
    },
    Text {
        time: TimeStamp,
        func: &'static str,
        add: Option<Cow<'static, str>>,
    },
    Frame {
        time: TimeStamp,
        kinds: FrameKinds,
        func: &'static str,
        add: Option<Cow<'static, str>>,
    },
    Sorted {
        time: TimeStamp,
        after: Arc<[EntityId]>,
        func: &'static str,
        add: Option<Cow<'static, str>>,
    },
    PrepareRender {
        time: TimeStamp,
        stage: RenderStage,
        data: Arc<[RenderData]>,
        func: &'static str,
        add: Option<Cow<'static, str>>,
    },
    WriteBuffer {
        time: TimeStamp,
        staging: Arc<[QuadInstance]>,
        func: &'static str,
        add: Option<Cow<'static, str>>,
    },
    Present {
        time: TimeStamp,
        func: &'static str,
        add: Option<Cow<'static, str>>,
    },
    Commit {
        time: TimeStamp,
        func: &'static str,
        add: Option<Cow<'static, str>>,
    },
    Despawn {
        time: TimeStamp,
        func: &'static str,
        add: Option<Cow<'static, str>>,
    },
    // 特定のエンティティに帰属させにくいエラーは、
    // この画面、あるいはアプリ全体のルートコンポーネントがアセットを読み込もうとして失敗したと解釈し、
    // とりあえずルート要素のIDにまとめる
    Error {
        root: EntityId,
        time: TimeStamp,
        detail: MichiuError,
        fallback: Option<&'static str>,
        func: &'static str,
        add: Option<Cow<'static, str>>,
    },
}

// ================================================================
// ================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct SpawnTrace {
    time: TimeStamp,
    entities: Vec<EntityId>,
    root: EntityId,
    parents: Vec<EntityId>,
    children: Vec<EntityId>,
    func: &'static str,
    add: Option<Cow<'static, str>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueueDirtyTrace {
    time: TimeStamp,
    layout: Vec<EntityId>,
    render: Vec<EntityId>,
    sort: bool,
    structure: bool,
    func: &'static str,
    add: Option<Cow<'static, str>>,
}

#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum ReactiveKinds {
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

#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum LayoutStage {
    /// レイアウト計算の開始
    Enter,
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
    /// テキストレイアウト計算
    TextMeasure,
    /// 1回目の出力領域
    FirstOutputRect,
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

#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum RenderStage {
    /// レンダリングフェーズ開始
    Enter,
    /// 指定された要素に含まれるすべての文字をアトラスにキャッシュ
    FirstGlyphsCache,
    /// アトラスのクリアが起きた場合、アトラスを再構築して再度キャッシュ
    FullGlyphsCache,
    /// パッキング開始
    CollectData,
    /// バッチのフラッシュ
    FlushBatch,
    /// インスタンス作成
    CreateInstance,
    /// ダーティフラグのクリア
    ClearDirty,
    /// 終了
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimeStamp {
    /// 計測開始
    Start(Instant),
    /// 途中のステージ（直前のステージからの経過時間）
    Elapsed(Duration),
    /// 計測終了（全体の合計時間）
    End(Duration),
}

#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum TraceEventList {
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
        current: Cow<'static, str>,
        list: Vec<Cow<'static, str>>,
    },
    Redo {
        text: Cow<'static, str>,
        list: Vec<Cow<'static, str>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum FrameKinds {
    Transition,
    Animation,
    AutoScroll,
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

pub(crate) struct MichiuStopwatch {
    pub(crate) start: Instant,
    pub(crate) last: Instant,
}

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
