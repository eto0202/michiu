#![allow(unused)]
use crate::{
    ActiveAnimation, ActiveTransition, AlignContent, AlignItems, AlignSelf, BasicLayout, BatchType,
    BoxShadow, BoxSizing, Color, CornerRadius, CursorIcon, Direction, Display, DragPayload,
    DragPlaceholderParent, DragProperty, DrawBatch, DropProperty, DropTarget, EdgeInsets, EffectId,
    Element, ElementState, EventListeners, FlexDirection, FlexLayout, FlexWrap, GlobalCursorIcon,
    GridAutoFlow, GridLayout, GridLine, GridPlacement, IDENTITY_MATRIX, ImageSource, ImeState,
    InputContents, InteractionStates, InteractionStyles, JustifyContent, LayoutOverflow,
    LayoutPoint, LayoutRect, LayoutSize, Length, Modifiers, MouseButton, MovieProperty,
    MovieSource, Overflow, PlaybackCount, PointerEvents, Position, QuadInstance, ReadSignal, Rect,
    RenderData, ScrollbarDisplay, ScrollbarMode, ScrollbarStyle, SignalId, Size, StyleTarget,
    TextAlign, TextEngine, TextSpan, ThisStyle, TransitionValue, UiaValue, UserSelect, Val,
    VirtualKey, VisualProperty, WebView2Contents, WriteSignal, bind_context, bitmap::*,
    with_context,
};
use slotmap::{KeyData, SecondaryMap, SlotMap, SparseSecondaryMap, new_key_type};
use smallvec::SmallVec;
use std::{
    any::TypeId,
    borrow::Cow,
    cell::RefCell,
    collections::{HashMap, HashSet},
    marker::PhantomData,
    path::PathBuf,
    sync::{
        Arc,
        mpsc::{Receiver, Sender},
    },
    time::{Duration, Instant},
};
use taffy::TaffyTree;
use windows::Win32::{
    Foundation::{HANDLE, HGLOBAL},
    Graphics::DirectWrite::{DWRITE_HIT_TEST_METRICS, IDWriteTextLayout},
    System::{
        DataExchange::{
            CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
        },
        Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
    },
};

new_key_type! {
    /// UI内の各要素（Entity）を識別する一意な世代管理ID
    pub struct EntityId;
}

// Taffyスタイルを一括解決するヘルパー
pub(crate) fn resolve_taffy_style(
    basic: &BasicLayout,
    flex: &FlexLayout,
    grid: Option<&GridLayout>,
    scrollbar: Option<&ScrollbarStyle>,
) -> taffy::Style {
    let mut style: taffy::Style = taffy::Style {
        display: basic.display.into(),
        box_sizing: basic.box_sizing.into(),
        direction: basic.direction.into(),
        overflow: basic.overflow.into(),
        position: basic.position.into(),
        inset: basic.inset.into(),
        size: basic.size.into(),
        min_size: basic.min_size.into(),
        max_size: basic.max_size.into(),
        aspect_ratio: basic.aspect_ratio,
        margin: basic.margin.into(),
        padding: basic.padding.into(),
        border: basic.border.into(),
        align_items: flex.align_items.map(|f| f.into()),
        align_self: flex.align_self.map(|f| f.into()),
        justify_items: flex.justify_items.map(|f| f.into()),
        justify_self: flex.justify_self.map(|f| f.into()),
        align_content: flex.align_content.map(|f| f.into()),
        justify_content: flex.justify_content.map(|f| f.into()),
        gap: flex.gap.into(),
        flex_direction: flex.flex_direction.into(),
        flex_wrap: flex.flex_wrap.into(),
        flex_basis: flex.flex_basis.into(),
        flex_grow: flex.flex_grow,
        flex_shrink: flex.flex_shrink,
        scrollbar_width: if let Some(sb) = scrollbar
            && sb.mode == ScrollbarMode::Layout
            && sb.display != ScrollbarDisplay::None
        {
            sb.width
        } else {
            0.0
        },

        ..Default::default()
    };
    if let Some(g) = grid {
        style.grid_template_rows = g.grid_template_rows.clone();
        style.grid_template_columns = g.grid_template_columns.clone();
        style.grid_auto_rows = g.grid_auto_rows.clone();
        style.grid_auto_columns = g.grid_auto_columns.clone();
        style.grid_auto_flow = g.grid_auto_flow.into();
        style.grid_template_areas = g.grid_template_areas.clone();
        style.grid_template_column_names = g.grid_template_column_names.clone();
        style.grid_template_row_names = g.grid_template_row_names.clone();
        style.grid_row = g.grid_row.clone().into();
        style.grid_column = g.grid_column.clone().into();
    }
    style
}

pub(crate) type TaskSenderType = Sender<Box<dyn FnOnce(&mut Context) + Send + 'static>>;
/// メインスレッド（UIスレッド）に対して、スレッドセーフに任意のタスクを送信する送信端。
#[derive(Clone)]
pub struct TaskSender {
    pub(crate) inner: TaskSenderType,
    // コアから Win32 を隠蔽するためのウェイクアップコールバック
    pub(crate) waker: Option<Arc<dyn Fn() + Send + Sync + 'static>>,
}

impl TaskSender {
    /// ワーカースレッド等からメインスレッドで実行してほしい処理（クロージャ）を送信します。
    /// ライブラリ内部で自動的に Box に包むため、呼び出し側での Box::new は不要です。
    #[allow(clippy::result_unit_err)]
    pub fn send<F>(&self, f: F) -> Result<(), ()>
    where
        F: FnOnce(&mut Context) + Send + 'static,
    {
        // 内部で Box::new に包んで送信し、複雑なエラー型はシンプルな Result<(), ()> に変換して隠蔽する
        self.inner.send(Box::new(f)).map_err(|_| ())?;

        // タスク送信に成功したら即座にメインスレッドをウェイクアップさせる
        if let Some(ref waker) = self.waker {
            waker();
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EffectCategory {
    None,
    Style,
    Text,
    Input,
    Image,
    Movie,
    WebView2,
    Contents,
    UiaName,
    UiaAutomationId,
    ActiveState,
    SelectState,
    DisableState,
    FocusState,
}

#[derive(Debug, Clone)]
pub(crate) struct ScrollBarState {
    pub(crate) style: ScrollbarStyle,

    // レイアウトツリーに動的挿入される Element の EntityId
    pub(crate) v_track_id: Option<EntityId>,
    pub(crate) v_thumb_id: Option<EntityId>,
    pub(crate) h_track_id: Option<EntityId>,
    pub(crate) h_thumb_id: Option<EntityId>,

    // ホバー・ドラッグのランタイム状態
    pub(crate) v_thumb_hovered: bool,
    pub(crate) v_thumb_dragged: bool,
    pub(crate) h_thumb_hovered: bool,
    pub(crate) h_thumb_dragged: bool,

    pub(crate) drag_start_mouse: LayoutPoint,
    pub(crate) drag_start_offset: LayoutPoint,

    // 一時表示（Transient）モードの表示制御用
    pub(crate) last_scroll_time: Option<Instant>,
}

#[derive(Debug, Clone)]
pub(crate) struct ActiveDragState {
    pub(crate) source_entity: EntityId,      // ドラッグ元の要素
    pub(crate) placeholder_entity: EntityId, // ルートまたは親に浮かせているプレースホルダー
    pub(crate) current_drop_target: Option<EntityId>, // 現在ホバー侵入中のドロップターゲット要素
    pub(crate) start_mouse_pos: LayoutPoint, // ドラッグ開始時のマウス座標
    pub(crate) start_rect: LayoutRect,       // ドラッグ元の初期サイズ・座標
    pub(crate) click_offset: LayoutPoint,    // ドラッグ開始時のマウスと要素左上端の相対的なズレ
    pub(crate) original_parent: Option<EntityId>,
}

// 利用者用 Context を用意して安定APIはそちらで公開
// pub struct EventContext<'a> {
//    cx: &'a mut Context,
// }
// RawContext 側で全てのAPIを公開
// Facade化するのもあり
pub struct Context {
    // 1. 存在 ＆ トポロジー（階層・親子）管理
    /// 全要素の生存期間を管理するプライマリマップ
    pub(crate) entities: SlotMap<EntityId, ()>,
    /// 単方向の親ID参照。親子ポインタを排除した木構造の表現
    pub(crate) parents: SecondaryMap<EntityId, Option<EntityId>>,
    /// 子要素のIDリスト。ヒープ割り当てを防ぐため SmallVec を採用
    pub(crate) children: SecondaryMap<EntityId, SmallVec<[EntityId; 4]>>,
    /// Taffy側のノードIDとの1対1マッピングテーブル（レイアウト結果の同期に使用）
    pub(crate) taffy_nodes: SecondaryMap<EntityId, taffy::NodeId>,

    // 2. 特徴・アクセス選別マスク
    /// 各要素がどのSoAプロパティ（コンポーネント）を有効化しているかを示すビットマスク
    pub active_masks: SecondaryMap<EntityId, ComponentMask>,

    // 3. 走査・変更管理（Dirty配列）
    /// 画面に表示されているアクティブな全要素のIDを詰め込んだ1次元配列。
    /// 描画やイベント走査はこの1つの配列のみを回す。
    pub active_entities: Vec<EntityId>,

    /// 今フレームでレイアウト（基本/Flex/Grid）に何らかの変更があった要素のリスト。
    /// Taffy同期フェーズが完了するとクリア。
    pub(crate) dirty_layout_entities: Vec<EntityId>,

    /// 今フレームでビジュアル（色/枠線/角丸など）に何らかの変更があった要素のリスト。
    /// wgpuへのインスタンスバッファ転送が完了するとクリア。
    pub(crate) dirty_render_entities: Vec<EntityId>,

    // 4. SoAレイアウトデータ領域（Taffy用：ホット / コールド分離）
    // 別々の配列ではなく、AoSチャンクとしてまとめて格納しキャッシュ効率を最大化
    /// 基本レイアウト（ホット：Copy可能）
    pub(crate) basic_layouts: SecondaryMap<EntityId, BasicLayout>,
    /// Flexレイアウト（ホット：Copy可能）
    pub(crate) flex_layouts: SecondaryMap<EntityId, FlexLayout>,
    /// Gridレイアウト（コールド：重い動的配列を含むため、設定された要素のみ確保）
    pub(crate) grid_layouts: SparseSecondaryMap<EntityId, GridLayout>,
    // 5. SoAビジュアル・ステート領域（wgpu / 合成用ホットデータ）
    /// 要素の描画パラメータ群。
    pub(crate) visual_properties: SecondaryMap<EntityId, VisualProperty>,
    /// ホバーやプレス等の動的ステートに対応するスタイルオーバーライド群
    pub(crate) interaction_properties: SecondaryMap<EntityId, InteractionStyles>,

    pub(crate) base_basic_layouts: SecondaryMap<EntityId, BasicLayout>,
    pub(crate) base_visual_properties: SecondaryMap<EntityId, VisualProperty>,

    /// テキストの実体。すべての要素が持つわけではないため、Sparse で管理
    pub(crate) text_contents: SparseSecondaryMap<EntityId, Cow<'static, str>>,
    pub(crate) text_spans: SparseSecondaryMap<EntityId, Vec<TextSpan>>,
    /// 要素ごとの入力エンジン状態
    pub input_contents: SparseSecondaryMap<EntityId, InputContents>,
    /// 画像の実体。必要な要素のみ Sparse で管理
    pub(crate) image_sources: SparseSecondaryMap<EntityId, ImageSource>,
    /// 動画の再生プロパティ。必要な要素のみ Sparse で管理
    pub(crate) movie_properties: SparseSecondaryMap<EntityId, MovieProperty>,

    // 7. 計算結果データ領域（アウトプット）
    /// Taffyによるレイアウト計算後の、確定した画面上の物理的な矩形（x, y, w, h）。
    /// ヒットテスト（当たり判定）やwgpuの描画パスはこのデータだけを見て動く。
    pub rects: SecondaryMap<EntityId, LayoutRect>,
    /// 親要素の overflow 等によって切り取られた、実際に画面上に表示される制限 viewport。
    /// wgpu の scissor rect の設定や、ヒットテストの範囲制限に用いる。
    pub(crate) clip_rects: SecondaryMap<EntityId, LayoutRect>,
    /// スクロールコンテナの現在のスクロールオフセット (x, y)
    pub(crate) scroll_offsets: SecondaryMap<EntityId, LayoutPoint>,
    /// 各要素に紐づくスクロールバースタイリング
    pub(crate) scrollbar_styles: SparseSecondaryMap<EntityId, ScrollBarState>,

    // 前回値キャッシュ用のダブルバッファ
    pub(crate) prev_rects: SecondaryMap<EntityId, LayoutRect>,
    pub(crate) prev_clip_rects: SecondaryMap<EntityId, LayoutRect>,

    // 8. インタラクション・イベント追跡用ランタイム状態
    /// 現在のポインタの物理座標（ドラッグの移動量算出などに使用）
    pub(crate) current_pointer_position: Option<LayoutPoint>,
    /// 対称的に整理された、グローバルなインタラクション対象状態
    pub interaction_states: InteractionStates,
    /// イベントハンドラを格納するSoA
    pub(crate) event_listeners: SparseSecondaryMap<EntityId, EventListeners>,

    /// UI Automation プロパティマップ (PropertyID -> Value)
    pub(crate) uia_properties: SparseSecondaryMap<EntityId, Vec<(i32, UiaValue)>>,

    /// 永続化された TaffyTree。これにより毎フレーム構築を完全に回避
    pub(crate) taffy: TaffyTree<EntityId>,
    /// DFS順にフラットに並べた要素ID。座標の非再帰・高速直線走査に使用
    flat_dfs_sequence: Vec<EntityId>,
    /// UIツリーの親子関係に変更があったか
    is_structure_dirty: bool,
    // ウィンドウサイズの変更検知用キャッシュ
    last_window_size: Option<LayoutSize>,
    /// 現在のビルドセッションで新しく生成（Spawn）された要素のリスト
    pub(crate) session_spawned: Vec<EntityId>,
    /// セッション終了時に、親がいなくても破棄してはならないルート要素のリスト
    pub(crate) session_roots: Vec<EntityId>,

    /// 全てのシグナルの実値を保持するストレージ（メインスレッド専用）
    pub(crate) signals: SlotMap<SignalId, Box<dyn std::any::Any>>,
    /// 各シグナルに対して、どの一連のエフェクトが購読（依存）しているかを追跡する
    pub(crate) subscribers: SecondaryMap<SignalId, SmallVec<[EffectId; 4]>>,
    /// 登録された全エフェクトのクロージャを格納するストレージ
    pub(crate) effects: SlotMap<EffectId, Effects>,
    /// 要素（EntityId）に紐づく、カテゴリ分けされた動的エフェクトリスト
    pub(crate) element_effects: SecondaryMap<EntityId, SmallVec<[(EffectCategory, EffectId); 4]>>,
    // ワーカースレッドからのメインスレッドディスパッチ用チャネル
    task_receiver: Receiver<TaskRecv>,
    task_sender: TaskSender,

    pub(crate) text_engine: TextEngine,

    /// 要素ごとに現在実行中のトランジションリスト
    pub(crate) active_transitions: SparseSecondaryMap<EntityId, Vec<ActiveTransition>>,
    /// 要素ごとに現在再生中のキーフレームアニメーションリスト
    pub(crate) active_animations: SparseSecondaryMap<EntityId, Vec<ActiveAnimation>>,

    /// 各要素の WebView2 の詳細な設定・URL情報（コールドデータ）
    pub(crate) webview_contents: SparseSecondaryMap<EntityId, WebView2Contents>,

    /// DComp 側で初期化（コントローラー生成）が完了して表示準備が整った WebView2 の一覧
    pub(crate) active_webviews: HashSet<EntityId>,

    /// ウィンドウの枠線ドラッグ等によるインタラクティブなリサイズ処理の最中であるかを示すフラグ
    pub is_window_resizing: bool,
    /// 現在のウィンドウの物理DPIスケール因数
    pub scale_factor: f32,

    /// アニメーション更新頻度の制御用
    pub(crate) last_tick_time: Option<Instant>,

    // 選択された文字範囲
    pub(crate) text_selections: SparseSecondaryMap<EntityId, std::ops::Range<usize>>,
    pub(crate) selection_start_index: SparseSecondaryMap<EntityId, usize>,
    pub(crate) selected_rects: SparseSecondaryMap<EntityId, Vec<LayoutRect>>,
    // DWrite レイアウトキャッシュSoA
    pub(crate) dwrite_layouts: RefCell<SparseSecondaryMap<EntityId, IDWriteTextLayout>>,

    /// 各要素が提供する型 (TypeId) とその SignalId のマッピング
    pub(crate) providers: SparseSecondaryMap<EntityId, HashMap<TypeId, SignalId>>,
    /// エフェクトIDから所有する要素IDへの逆引き用
    pub(crate) effect_to_element: SecondaryMap<EffectId, EntityId>,
    /// 構築中に登録され、トポロジー完成まで初回評価が保留されている要素エフェクトのキュー
    pub(crate) pending_element_effects: Vec<EffectId>,

    /// 現在リサイズドラッグ中の要素の情報
    pub(crate) resizing_state: Option<ResizingState>,
    // ドラッグ開始直前のホバー中に算出されたリサイズ方向
    pub(crate) active_resize_hover: Option<(EntityId, ResizeDirection)>,

    /// ドラッグ可能な要素の設定
    pub(crate) drag_properties: SparseSecondaryMap<EntityId, DragProperty>,
    /// ドロップ受け入れ先要素の設定
    pub(crate) drop_properties: SparseSecondaryMap<EntityId, DropProperty>,
    /// 現在進行中の D&D セッション状態
    pub(crate) active_drag_state: Option<ActiveDragState>,
}

pub(crate) type Effects = Box<dyn FnMut(&mut Context)>;
pub(crate) type TaskRecv = Box<dyn FnOnce(&mut Context) + Send + 'static>;

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    pub fn new() -> Self {
        // タスク送受信チャネルの初期化
        let (tx, rx) = std::sync::mpsc::channel();

        Self {
            entities: SlotMap::with_key(),
            parents: SecondaryMap::new(),
            children: SecondaryMap::new(),
            taffy_nodes: SecondaryMap::new(),
            active_masks: SecondaryMap::new(),
            active_entities: Vec::new(),
            dirty_layout_entities: Vec::new(),
            dirty_render_entities: Vec::new(),
            basic_layouts: SecondaryMap::new(),
            flex_layouts: SecondaryMap::new(),
            grid_layouts: SparseSecondaryMap::new(),
            text_contents: SparseSecondaryMap::new(),
            text_spans: SparseSecondaryMap::new(),
            image_sources: SparseSecondaryMap::new(),
            movie_properties: SparseSecondaryMap::new(),
            input_contents: SparseSecondaryMap::new(),
            visual_properties: SecondaryMap::new(),
            interaction_properties: SecondaryMap::new(),
            base_basic_layouts: SecondaryMap::new(),
            base_visual_properties: SecondaryMap::new(),
            rects: SecondaryMap::new(),
            prev_rects: SecondaryMap::new(),
            prev_clip_rects: SecondaryMap::new(),
            clip_rects: SecondaryMap::new(),
            scroll_offsets: SecondaryMap::new(),
            event_listeners: SparseSecondaryMap::new(),
            current_pointer_position: None,
            interaction_states: InteractionStates::new(),
            uia_properties: SparseSecondaryMap::new(),
            flat_dfs_sequence: Vec::new(),
            is_structure_dirty: true,
            taffy: TaffyTree::new(),
            last_window_size: None,
            session_spawned: Vec::new(),
            session_roots: Vec::new(),
            signals: SlotMap::with_key(),
            subscribers: SecondaryMap::new(),
            effects: SlotMap::with_key(),
            element_effects: SecondaryMap::new(),
            task_receiver: rx,
            task_sender: TaskSender {
                inner: tx,
                waker: None,
            },
            text_engine: TextEngine::new(),
            active_transitions: SparseSecondaryMap::new(),
            active_animations: SparseSecondaryMap::new(),
            webview_contents: SparseSecondaryMap::new(),
            active_webviews: HashSet::new(),
            is_window_resizing: false,
            scale_factor: 1.0,
            last_tick_time: None,
            text_selections: SparseSecondaryMap::new(),
            selection_start_index: SparseSecondaryMap::new(),
            selected_rects: SparseSecondaryMap::new(),
            dwrite_layouts: RefCell::new(SparseSecondaryMap::new()),
            scrollbar_styles: SparseSecondaryMap::new(),
            providers: SparseSecondaryMap::new(),
            effect_to_element: SecondaryMap::new(),
            pending_element_effects: Vec::new(),
            resizing_state: None,
            active_resize_hover: None,
            drag_properties: SparseSecondaryMap::new(),
            drop_properties: SparseSecondaryMap::new(),
            active_drag_state: None,
        }
    }

    /// 要素を新規に生成（Spawn）
    pub(crate) fn spawn(&mut self, parent_id: Option<EntityId>) -> EntityId {
        let id = self.entities.insert(());

        self.parents.insert(id, parent_id);
        self.children.insert(id, SmallVec::new());
        self.active_masks.insert(id, ComponentMask::new(0)); // 初期状態はどのプロパティも無効

        // Leaf ノード作成時に、Context として自分自身の ID を登録する
        let node = self
            .taffy
            .new_leaf_with_context(taffy::Style::default(), id)
            .unwrap();
        self.taffy_nodes.insert(id, node);

        self.active_entities.push(id);

        // 新規作成された要素は、当然レイアウトと描画の対象となる
        // カスタムスタイルが当てられるまではデフォルト（Style::default）を再利用するため
        // mark_layout_dirty(id) の呼び出しを完全にスキップして、Taffyへの無駄な伝播をカット
        self.mark_render_dirty(id);
        self.is_structure_dirty = true; // 構造変化をマーク

        self.session_spawned.push(id);

        id
    }

    // セッションの開始マーカーを取得
    pub(crate) fn start_session(&mut self) -> usize {
        self.session_spawned.len()
    }

    // ルート要素として保護するIDを登録
    pub(crate) fn register_root(&mut self, id: EntityId) {
        self.session_roots.push(id);
    }

    // セッションのクリーンアップを実行
    pub(crate) fn end_session(&mut self, start_marker: usize) {
        // start_marker 以降に生成された要素をスキャン
        let spawned_in_session: Vec<EntityId> =
            self.session_spawned.drain(start_marker..).collect();

        for id in spawned_in_session {
            // 親が存在しない
            let has_no_parent = self.parents.get(id).copied().flatten().is_none();
            // ルート要素としても登録されていない
            let is_not_root = !self.session_roots.contains(&id);

            // 上記を満たす完全な孤児を自動で一掃
            if has_no_parent && is_not_root {
                self.despawn_internal(id);
            }
        }

        // ルートリストをクリア
        self.session_roots.clear();
    }

    /// ワーカースレッドなど、どこからでも安全にクローンしてタスクを送信できるスレッドセーフな送信端を取得します。
    pub fn task_sender(&self) -> TaskSender {
        self.task_sender.clone()
    }

    /// ウィンドウ生成後に起床用コールバックを登録します。
    pub fn set_waker<F>(&mut self, f: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.task_sender.waker = Some(Arc::new(f));
    }

    /// Context インスタンスから直接シグナルを生成します。
    /// これにより build_ui の外側（メインスレッド上）でもシグナルを定義できます。
    pub fn create_signal<T: Send + 'static>(
        &mut self,
        initial_value: T,
    ) -> (ReadSignal<T>, WriteSignal<T>) {
        let id = self.signals.insert(Box::new(initial_value));
        self.subscribers.insert(id, SmallVec::new());
        (
            ReadSignal {
                id,
                _marker: PhantomData,
            },
            WriteSignal {
                id,
                _marker: PhantomData,
            },
        )
    }

    /// メインスレッドの毎フレーム開始時（またはイベントハンドラの先頭など）に呼び出され、
    /// バックグラウンドから届いたシグナル更新タスクなどの処理を安全に一括実行します。
    pub fn process_main_thread_tasks(&mut self) {
        let _context_guard = bind_context(self);
        // キューに溜まっているクロージャをすべてメインスレッドのコンテキスト上で実行
        while let Ok(task) = self.task_receiver.try_recv() {
            task(self);
        }
    }

    /// 要素にエフェクトをカテゴリ指定付きで紐づけて登録します。
    /// 同一カテゴリのエフェクトが既に存在する場合、自動的に古いエフェクトを破棄してから上書きします。
    pub(crate) fn register_element_effect(
        &mut self,
        element_id: EntityId,
        category: EffectCategory,
        effect_id: EffectId,
    ) {
        if let Some(effects) = self.element_effects.get_mut(element_id) {
            // 同一カテゴリのエフェクトが既に登録されていれば、古いものを破棄
            if let Some(pos) = effects.iter().position(|(cat, _)| *cat == category) {
                let (_, old_effect_id) = effects.remove(pos);
                self.effects.remove(old_effect_id); // SoA から古いエフェクト実体を削除
            }
            effects.push((category, effect_id));
        } else {
            self.element_effects
                .insert(element_id, smallvec::smallvec![(category, effect_id)]);
        }
    }

    /// 要素に動的エフェクト（Style、Text等のリアクティブクロージャ）を安全に登録し、初期評価を実行します。
    pub(crate) fn create_element_effect<F>(
        &mut self,
        element_id: EntityId,
        category: EffectCategory,
        f: F,
    ) -> EffectId
    where
        F: FnMut(&mut Context) + 'static,
    {
        let effect_id = self.effects.insert(Box::new(f));

        // 初回評価が走る前に要素との紐付けを確実に登録
        self.effect_to_element.insert(effect_id, element_id);

        // 要素のエフェクトリストに登録し、既存の同じカテゴリの古いエフェクトは自動破棄
        if !self.element_effects.contains_key(element_id) {
            self.element_effects
                .insert(element_id, smallvec::smallvec![]);
        }
        let list = self.element_effects.get_mut(element_id).unwrap();
        if let Some(pos) = list.iter().position(|(cat, _)| *cat == category) {
            let (_, old_id) = list.remove(pos);
            self.effects.remove(old_id);
            self.effect_to_element.remove(old_id);
            self.pending_element_effects.retain(|&x| x != old_id); // キューから古いものを排除
        }
        list.push((category, effect_id));

        // 即時実行を廃止。トポロジーが整うまで初回評価を一時保留
        self.pending_element_effects.push(effect_id);

        effect_id
    }

    /// トポロジーが完全に完成したビルド完了後、または同期直前に、溜めてある初回評価を一挙に安全実行します
    pub(crate) fn evaluate_pending_element_effects(&mut self) {
        if self.pending_element_effects.is_empty() {
            return;
        }

        // 評価中に別のネストしたエフェクトが追加されるケースを許容するため、drain で一度排出して処理
        let pending: Vec<EffectId> = self.pending_element_effects.drain(..).collect();
        for effect_id in pending {
            if self.effects.contains_key(effect_id) {
                crate::execute_effect(effect_id);
            }
        }
    }

    /// 指定された要素に対してシグナルコンテキストを提供します
    pub(crate) fn provide_context<T: Send + 'static>(&mut self, id: EntityId, signal_id: SignalId) {
        if !self.providers.contains_key(id) {
            self.providers.insert(id, std::collections::HashMap::new());
        }
        let map = self.providers.get_mut(id).unwrap();
        map.insert(std::any::TypeId::of::<T>(), signal_id);
    }

    /// 要素の階層トポロジーを親（Ancestor）に向かって遡り、最初に見つかった型 T の ReadSignal を解決して返します
    pub(crate) fn use_provided_from<T: Clone + 'static>(
        &self,
        id: EntityId,
    ) -> Option<ReadSignal<T>> {
        let mut curr = Some(id);
        let type_id = std::any::TypeId::of::<T>();

        while let Some(curr_id) = curr {
            if let Some(map) = self.providers.get(curr_id)
                && let Some(&signal_id) = map.get(&type_id)
            {
                return Some(ReadSignal::new(signal_id));
            }
            // トポロジー親を安全に探索
            curr = self.parents.get(curr_id).copied().flatten();
        }
        None
    }

    /// 現在のスレッドローカルコンテキスト（アクティブなエフェクト、またはイベントハンドラ）から、
    /// 自動的に対象の要素を特定し、親ツリーを遡って型 T の ReadSignal を解決します。
    pub fn use_provided<T: Clone + 'static>(&self) -> ReadSignal<T> {
        // 1. ACTIVE_EFFECT（エフェクト実行中）から解決を試みる
        let element_id = if let Some(active_effect_id) =
            crate::signal::ACTIVE_EFFECT.with(|cell| cell.get())
        {
            self.effect_to_element
                .get(active_effect_id)
                .copied()
                .expect("use_provided failed: active effect is not associated with any UI Element")
        } else if let Some(active_element_id) =
            crate::signal::ACTIVE_ELEMENT.with(|cell| cell.get())
        {
            // 2. ACTIVE_EFFECTがNoneであれば、ACTIVE_ELEMENT（イベントハンドラ実行中）にフォールバック
            active_element_id
        } else {
            panic!(
                "use_provided must be called inside a dynamic style, text, content closure, or an active event handler context"
            );
        };

        // 3. 親ツリーを遡って解決
        self.use_provided_from::<T>(element_id)
                .unwrap_or_else(|| {
                    panic!(
                        "Dependency resolution failed: No Provider found in ancestor sub-tree for type: '{}'",
                        std::any::type_name::<T>()
                    )
                })
    }

    /// 現在のスレッドローカルコンテキストから、
    /// 親ツリーを自動的に遡って解決した型 T のシグナルに対する同期書き込み用端（WriteSignal）を取得します。
    pub fn use_provided_setter<T: Send + 'static>(&self) -> WriteSignal<T> {
        let element_id = if let Some(active_effect_id) =
            crate::signal::ACTIVE_EFFECT.with(|cell| cell.get())
        {
            self.effect_to_element
                .get(active_effect_id)
                .copied()
                .expect("use_provided_setter failed: active effect not associated with an Element")
        } else if let Some(active_element_id) =
            crate::signal::ACTIVE_ELEMENT.with(|cell| cell.get())
        {
            active_element_id
        } else {
            panic!(
                "use_provided_setter must be called inside a dynamic reactive context or an active event handler context"
            );
        };

        let mut curr = Some(element_id);
        let type_id = std::any::TypeId::of::<T>();

        while let Some(curr_id) = curr {
            if let Some(map) = self.providers.get(curr_id)
                && let Some(&signal_id) = map.get(&type_id)
            {
                return WriteSignal {
                    id: signal_id,
                    _marker: std::marker::PhantomData,
                };
            }
            curr = self.parents.get(curr_id).copied().flatten();
        }
        panic!(
            "Dependency resolution failed: No Provider Setter found in ancestor sub-tree for type: '{}'",
            std::any::type_name::<T>()
        )
    }

    /// 親子関係の追加と、永続Taffy構造のリアルタイム同期。
    /// 子がすでに別の親に属している場合は古い親からデタッチします。
    pub(crate) fn add_child(&mut self, parent: EntityId, child: EntityId) {
        // 子がすでに別の親に属しているか検証
        if let Some(Some(old_parent)) = self.parents.get(child).copied()
            && old_parent != parent
        {
            // 1. 古い親の children SoA リストから自分自身を安全に削除
            if let Some(old_children) = self.children.get_mut(old_parent) {
                old_children.retain(|x| *x != child);
            }

            // 2. 古い親の Taffy ノードから安全にデタッチ
            if let Some(&old_parent_node) = self.taffy_nodes.get(old_parent)
                && let Some(&child_node) = self.taffy_nodes.get(child)
                && let Ok(taffy_children) = self.taffy.children(old_parent_node)
                && taffy_children.contains(&child_node)
            {
                let _ = self.taffy.remove_child(old_parent_node, child_node);
            }

            // 3. 古い親側の Taffy 順序とレイアウトを再同期して Dirty マーク
            self.resync_taffy_children_order(old_parent);
            self.mark_layout_dirty(old_parent);
        }

        // 新しい親の親子関係を更新
        self.parents.insert(child, Some(parent));
        if let Some(children_list) = self.children.get_mut(parent)
            && !children_list.contains(&child)
        {
            children_list.push(child);
        }

        // 新しい親の Taffy ツリーの親子関係を永続的に更新
        if let Some(&parent_node) = self.taffy_nodes.get(parent)
            && let Some(&child_node) = self.taffy_nodes.get(child)
        {
            let _ = self.taffy.add_child(parent_node, child_node);
        }

        self.mark_layout_dirty(parent);
        self.is_structure_dirty = true;
    }

    /// 一括解放
    pub fn clear(&mut self) {
        self.entities.clear();
        self.parents.clear();
        self.children.clear();
        self.taffy_nodes.clear();
        self.active_masks.clear();
        self.active_entities.clear();
        self.dirty_layout_entities.clear();
        self.dirty_render_entities.clear();
        self.basic_layouts.clear();
        self.flex_layouts.clear();
        self.grid_layouts.clear();
        self.text_contents.clear();
        self.text_spans.clear();
        self.image_sources.clear();
        self.movie_properties.clear();
        self.input_contents.clear();
        self.visual_properties.clear();
        self.interaction_properties.clear();
        self.base_basic_layouts.clear();
        self.base_visual_properties.clear();
        self.rects.clear();
        self.prev_rects.clear();
        self.prev_clip_rects.clear();
        self.clip_rects.clear();
        self.scroll_offsets.clear();
        self.event_listeners.clear();
        self.uia_properties.clear();
        self.flat_dfs_sequence.clear();
        self.is_structure_dirty = true;
        self.taffy = TaffyTree::new();
        self.signals.clear();
        self.subscribers.clear();
        self.effects.clear();
        self.active_transitions.clear();
        self.active_animations.clear();
        self.webview_contents.clear();
        self.active_webviews.clear();
        self.text_selections.clear();
        self.selection_start_index.clear();
        self.selected_rects.clear();
        self.dwrite_layouts.borrow_mut().clear();
        self.scrollbar_styles.clear();
        self.providers.clear();
        self.effect_to_element.clear();
        self.pending_element_effects.clear();
        self.resizing_state = None;
        self.active_resize_hover = None;
        self.drag_properties.clear();
        self.drop_properties.clear();
        self.active_drag_state = None;
        // 溜まっている未処理タスクをすべて排出してクリーンアップ
        while self.task_receiver.try_recv().is_ok() {}
    }

    /// 外部公開用API: ハンドルを指定して要素を安全に破棄します。
    ///
    /// 親を持たないルート要素の破棄（手動での寿命管理）に使用します。
    /// 子要素が存在する場合は、自動的に再帰破棄されます。
    #[inline]
    pub fn despawn(&mut self, handle: Element) {
        self.despawn_internal(handle.id);
    }

    /// 要素を安全に破棄（Despawn）。親が消えた場合子はフレーム末尾のクリーンアップフェーズで自動修復・一掃
    pub(crate) fn despawn_internal(&mut self, id: EntityId) {
        if self.entities.contains_key(id) {
            // 親側の Taffy 子要素リストから、自身のノードを安全に削除 (remove_child)
            if let Some(Some(parent_id)) = self.parents.get(id)
                && let Some(&parent_node) = self.taffy_nodes.get(*parent_id)
                && let Some(&child_node) = self.taffy_nodes.get(id)
                && let Ok(taffy_children) = self.taffy.children(parent_node)
                && taffy_children.contains(&child_node)
            {
                let _ = self.taffy.remove_child(parent_node, child_node);
            }

            // 自身の Taffy ノードを TaffyTree から削除
            if let Some(node) = self.taffy_nodes.remove(id) {
                let _ = self.taffy.remove(node);
            }

            // 子要素を再帰的に despawn
            if let Some(children_list) = self.children.remove(id) {
                for child_id in children_list {
                    self.despawn_internal(child_id);
                }
            }

            // 親の children リストから自身を除外
            if let Some(Some(parent_id)) = self.parents.get(id)
                && let Some(parent_children) = self.children.get_mut(*parent_id)
            {
                parent_children.retain(|x| *x != id);
            }

            // 要素に紐づいていた全エフェクトを自動クリーンアップ
            // エフェクトのクリーンアップ時に逆引きマップからも削除
            if let Some(effects) = self.element_effects.remove(id) {
                for (_, effect_id) in effects {
                    self.effects.remove(effect_id);
                    self.effect_to_element.remove(effect_id);
                    self.pending_element_effects.retain(|&x| x != effect_id);
                }
            }

            self.entities.remove(id);
            self.parents.remove(id);
            self.active_masks.remove(id);

            // SoA側の全データも一斉削除
            self.basic_layouts.remove(id);
            self.flex_layouts.remove(id);
            self.grid_layouts.remove(id);
            self.text_contents.remove(id);
            self.text_spans.remove(id);
            self.image_sources.remove(id);
            self.movie_properties.remove(id);
            self.input_contents.remove(id);
            self.visual_properties.remove(id);
            self.interaction_properties.remove(id);
            self.base_basic_layouts.remove(id);
            self.base_visual_properties.remove(id);
            self.rects.remove(id);
            self.clip_rects.remove(id);
            self.scroll_offsets.remove(id);
            self.event_listeners.remove(id);
            self.uia_properties.remove(id);
            self.active_transitions.remove(id);
            self.active_animations.remove(id);
            self.webview_contents.remove(id);
            self.text_selections.remove(id);
            self.selection_start_index.remove(id);
            self.selected_rects.remove(id);
            self.dwrite_layouts.borrow_mut().remove(id);
            self.scrollbar_styles.remove(id);
            self.providers.remove(id);

            self.drag_properties.remove(id);
            self.drop_properties.remove(id);

            // 現在の D&D セッションに含まれる要素が破棄された場合はセッションを安全にリセット
            if let Some(ref state) = self.active_drag_state
                && (state.source_entity == id || state.placeholder_entity == id)
            {
                self.active_drag_state = None;
            }

            // ダーティキュー、DFSシーケンス、アクティブ走査用の一時配列から
            // デスポーンされた無効な ID をその場で即時に抹消クリーンアップします。
            self.dirty_layout_entities.retain(|&x| x != id);
            self.dirty_render_entities.retain(|&x| x != id);
            self.active_entities.retain(|&x| x != id);
            self.flat_dfs_sequence.retain(|&x| x != id);
            self.session_spawned.retain(|&x| x != id);
            self.session_roots.retain(|&x| x != id);

            self.is_structure_dirty = true;
        }
    }

    /// 親要素の特定の古い子要素を、順序（インデックス）を維持したまま新しい子要素へ直接差し替えます。
    pub(crate) fn replace_child(
        &mut self,
        parent: EntityId,
        old_child: EntityId,
        new_child: EntityId,
    ) {
        // Taffy ツリー側の同期（古いノードを外し、新しいノードをアタッチ）
        // 修正: 古いノードの削除は、直後の despawn_internal が一貫して安全に行うため、
        // ここでの手動 remove_child を撤廃し、Taffy 側への新規アタッチ（add_child）のみを行います。
        if let Some(&parent_node) = self.taffy_nodes.get(parent)
            && let Some(&new_node) = self.taffy_nodes.get(new_child)
        {
            let _ = self.taffy.add_child(parent_node, new_node);
        }

        // children リスト内のインデックス位置を特定して直接置換
        if let Some(children_list) = self.children.get_mut(parent)
            && let Some(pos) = children_list.iter().position(|&x| x == old_child)
        {
            children_list[pos] = new_child;
        }

        // 親子参照の更新
        self.parents.insert(new_child, Some(parent));

        // 古い子要素（およびその子孫）を完全に安全デスポーン
        // この中で Taffy からの remove_child も安全に実行されます
        self.despawn_internal(old_child);

        self.mark_layout_dirty(parent);
        self.is_structure_dirty = true;
    }

    /// 指定された親コンテナにアタッチされている DComp / Taffy 側のすべての子ノードの物理順序を
    /// 内部 SoA リスト（self.children）の順序に沿って一括して再同期）します。
    pub(crate) fn resync_taffy_children_order(&mut self, parent_id: EntityId) {
        if let Some(&parent_node) = self.taffy_nodes.get(parent_id) {
            // 一旦現在登録されているすべての子ノードを Taffy 側から安全にデタッチ
            if let Ok(taffy_children) = self.taffy.children(parent_node) {
                for child_node in taffy_children {
                    let _ = self.taffy.remove_child(parent_node, child_node);
                }
            }
            // 最新の並び替え順序リストの順に従って、Taffy 側に再アタッチ
            if let Some(children_list) = self.children.get(parent_id).cloned() {
                for child_id in children_list {
                    if let Some(&child_node) = self.taffy_nodes.get(child_id) {
                        let _ = self.taffy.add_child(parent_node, child_node);
                    }
                }
            }
        }
    }

    /// デスポーン済みの無効な EntityId を各走査・Dirty配列から一括して排除。
    pub(crate) fn gc_inactive_entities(&mut self) {
        // SlotMap (entities) にキーが存在するもの（生存している要素）だけを保持する
        self.active_entities
            .retain(|&id| self.entities.contains_key(id));
        self.dirty_layout_entities
            .retain(|&id| self.entities.contains_key(id));
        self.dirty_render_entities
            .retain(|&id| self.entities.contains_key(id));
    }

    /// レイアウト変更フラグを立てる（Taffy同期要求）
    pub(crate) fn mark_layout_dirty(&mut self, id: EntityId) {
        let mut curr = id;
        // Taffy 側の該当ノードのレイアウトキャッシュを無効化
        if let Some(&taffy_node) = self.taffy_nodes.get(curr) {
            let _ = self.taffy.mark_dirty(taffy_node);
        }

        loop {
            if let Some(mask) = self.active_masks.get_mut(curr) {
                // すでにレイアウトキューに登録済み（STATE_QUEUED_LAYOUT がオン）なら
                // 多重登録を防ぎつつ、それより上の親はすでに Dirty 化されているため探索を早期ブレイク
                if !mask.has(STATE_QUEUED_LAYOUT) {
                    mask.set(STATE_QUEUED_LAYOUT); // 自身を Dirty マーク
                    self.dirty_layout_entities.push(curr);
                } else {
                    break;
                }
            }

            // 親要素（先祖）をルートまで辿って Dirty フラグを連鎖伝播させる
            if let Some(Some(parent_id)) = self.parents.get(curr).copied() {
                curr = parent_id;
            } else {
                break;
            }
        }
    }

    /// 描画変更フラグを立てる（wgpu転送要求）
    pub(crate) fn mark_render_dirty(&mut self, id: EntityId) {
        if let Some(mask) = self.active_masks.get_mut(id) {
            // すでにレンダーキューに登録済み（STATE_QUEUED_RENDER がオン）なら早期リターン
            if !mask.has(STATE_QUEUED_RENDER) {
                mask.set(STATE_QUEUED_RENDER); // フラグをオンにして多重登録を防ぐ
                self.dirty_render_entities.push(id);
            }
        }
    }

    /// 現在、システム内部に再描画要求（Dirtyマークされた要素）があるか判定します。
    pub fn is_render_dirty(&self) -> bool {
        // dirty_render_entities に何か登録されている、またはレイアウトに Dirty がある場合
        !self.dirty_render_entities.is_empty()
            || !self.dirty_layout_entities.is_empty()
            || self.is_structure_dirty
    }

    /// 非再帰スタックによるフラットDFS配列の高速構築
    fn rebuild_flat_dfs_sequence(&mut self, root: EntityId) {
        self.flat_dfs_sequence.clear();

        // あらかじめ実用的なスタック深度を確保しておきメモリ再確保を削減
        let mut stack = Vec::with_capacity(32);
        stack.push(root);

        while let Some(id) = stack.pop() {
            self.flat_dfs_sequence.push(id);

            // 左側の子が先にポップされるように、右側（末尾）の子から逆順にスタックへプッシュ
            if let Some(children) = self.children.get(id) {
                let len = children.len();
                for i in (0..len).rev() {
                    stack.push(children[i]);
                }
            }
        }

        self.is_structure_dirty = false;
    }

    /// キャッシュコヒーレントな直列DFS同期（1次元直線ループ同期）
    /// Taffy自動計算を完全内包
    pub fn sync_layout_and_render_list(&mut self, root: EntityId, window_size: LayoutSize) {
        // 同期処理の開始時に自身をバインドする
        let _context_guard = bind_context(self);
        // レイアウトが再計算される前に、溜まっているすべてのエフェクトを評価完了させる
        self.evaluate_pending_element_effects();
        // ウィンドウサイズの変更検知
        let window_resized = if self.last_window_size != Some(window_size) {
            self.last_window_size = Some(window_size);
            true
        } else {
            false
        };

        // 構造変更がなく、スタイル変更（レイアウト変更要求）もなく、ウィンドウサイズも変わっていないなら、
        // Taffy計算も、ダブルバッファスワップもすべてスキップして即時帰還する。
        if self.dirty_layout_entities.is_empty()
            && !self.is_structure_dirty
            && !window_resized
            && !self.rects.is_empty()
        {
            return;
        }

        if self.is_structure_dirty {
            self.rebuild_flat_dfs_sequence(root);
        }

        // 全スクロールバー関連IDを一括抽出
        let mut scrollbar_el_ids = HashSet::new();
        for sb_state in self.scrollbar_styles.values() {
            if let Some(track_id) = sb_state.v_track_id {
                scrollbar_el_ids.insert(track_id);
            }
            if let Some(thumb_id) = sb_state.v_thumb_id {
                scrollbar_el_ids.insert(thumb_id);
            }
            if let Some(track_id) = sb_state.h_track_id {
                scrollbar_el_ids.insert(track_id);
            }
            if let Some(thumb_id) = sb_state.h_thumb_id {
                scrollbar_el_ids.insert(thumb_id);
            }
        }

        // 1. Taffy永続ツリーへの差分同期
        for id in &self.dirty_layout_entities {
            // スクロールバー専用要素は手動で物理座標を同期させるため、Taffyへの登録更新を完全にバイパス
            if scrollbar_el_ids.contains(id) {
                continue;
            }

            let (mut basic, flex, grid) = self.resolve_active_layouts(*id);

            // もしこの要素が現在アニメーション中（active_transitions に存在）であれば、
            // resolve_active_layouts が強制マージした目標値を拒否し、
            // tick_transitions が毎フレーム更新している現在値に上書きし直して Taffy に送信。
            if let Some(active_list) = self.active_transitions.get(*id) {
                for t_state in active_list {
                    match t_state.property_list {
                        PropertyList::Width => {
                            if let Some(layout) = self.basic_layouts.get(*id) {
                                basic.size.width = layout.size.width;
                            }
                        }
                        PropertyList::Height => {
                            if let Some(layout) = self.basic_layouts.get(*id) {
                                basic.size.height = layout.size.height;
                            }
                        }
                        _ => {}
                    }
                }
            }

            let sb_style = self.scrollbar_styles.get(*id).map(|s| &s.style);
            let taffy_style = resolve_taffy_style(&basic, &flex, grid.as_ref(), sb_style);
            let taffy_node = self.taffy_nodes[*id];

            self.taffy.set_style(taffy_node, taffy_style).unwrap();
        }

        // 2. Taffy のレイアウト再計算
        if let Some(&root_node) = self.taffy_nodes.get(root) {
            // 計測関数をクロージャとして定義
            let measure_func = |known_dims: taffy::Size<Option<f32>>,
                                available_space: taffy::Size<taffy::AvailableSpace>,
                                _node_id: taffy::NodeId,
                                context: Option<&mut EntityId>,
                                _style: &taffy::Style|
             -> taffy::Size<f32> {
                // 幅と高さの両方がすでにスタイル（known_dims）として解決されている場合はそれを最優先する
                if let (Some(w), Some(h)) = (known_dims.width, known_dims.height) {
                    return taffy::Size {
                        width: w,
                        height: h,
                    };
                }

                if let Some(&id) = context.as_deref() {
                    // テキスト内容を持っているかチェック
                    // (クロージャの外側の self (= Context) は直接キャプチャできないため、
                    //  一時的に bind_context されているスレッドローカル経由で取得)
                    return with_context(|cx| {
                        if cx.active_masks[id].has(COMP_INPUT_CONTENT)
                            && let Some(contents) = cx.input_contents.get(id)
                            && let Some(layout_rect) = contents.last_layout
                        {
                            return taffy::Size {
                                width: known_dims.width.unwrap_or(layout_rect.width),
                                height: known_dims.height.unwrap_or(layout_rect.height),
                            };
                        }

                        if cx.active_masks[id].has(COMP_TEXT_CONTENT) {
                            let text = cx.text_contents.get(id).map(|s| s.as_ref()).unwrap_or("");
                            let (font_size, font_family, font_weight, font_style) = cx
                                .visual_properties
                                .get(id)
                                .map(|v| {
                                    (
                                        v.font_size.unwrap_or(16.0),
                                        v.font_family.as_deref(),
                                        v.font_weight,
                                        v.font_style,
                                    )
                                })
                                // もし該当要素に VisualProperty 自体がなければデフォルト値をあてる
                                .unwrap_or((16.0, None, None, None));

                            let max_width = None;

                            let spans = cx.text_spans.get(id).map(|s| s.as_slice()).unwrap_or(&[]);

                            // DirectWrite を使用して正確なサイズを計測
                            let size = cx.text_engine.measure_text(
                                text,
                                font_size,
                                font_family,
                                font_weight,
                                font_style,
                                max_width,
                                spans,
                            );

                            // 計測した文字自体の正確なサイズをここでインプット要素にキャッシュする
                            if cx.active_masks[id].has(COMP_INPUT_CONTENT)
                                && let Some(contents) = cx.input_contents.get_mut(id)
                            {
                                contents.last_layout =
                                    Some(LayoutRect::new(0.0, 0.0, size.width, size.height));
                            }

                            // 文字のみのサイズ
                            return taffy::Size {
                                width: known_dims.width.unwrap_or(size.width),
                                height: known_dims.height.unwrap_or(size.height),
                            };
                        }

                        // テキストも入力も持たない空の div 等の場合、
                        // スタイルに割り当てられたサイズがあればそれを優先して返し、無ければ ZERO とする
                        taffy::Size {
                            width: known_dims.width.unwrap_or(0.0),
                            height: known_dims.height.unwrap_or(0.0),
                        }
                    });
                }
                taffy::Size::ZERO
            };

            let _ = self.taffy.compute_layout_with_measure(
                root_node,
                taffy::Size {
                    width: taffy::AvailableSpace::Definite(window_size.width),
                    height: taffy::AvailableSpace::Definite(window_size.height),
                },
                measure_func,
            );
        }

        self.active_entities.clear();

        // スワップおよび一旦コンテンツの rects のみを確定 (scroll_size を正しく算出するため)
        std::mem::swap(&mut self.rects, &mut self.prev_rects);
        std::mem::swap(&mut self.clip_rects, &mut self.prev_clip_rects);

        self.rects.clear();
        self.clip_rects.clear();

        let flat_len = self.flat_dfs_sequence.len();
        let initial_clip = LayoutRect::new(0.0, 0.0, window_size.width, window_size.height);

        // 1次元非再帰・静的キャッシュバイパスループ
        for i in 0..flat_len {
            let id = self.flat_dfs_sequence[i];

            // スクロールバー専用子要素は手動で物理座標を強制更新するため、この走査ループから完全にスルー
            if scrollbar_el_ids.contains(&id) {
                continue;
            }

            let parent_id_opt = self.parents.get(id).copied().flatten();

            // 親の移動・リサイズ状態を検証
            let mut parent_changed = false;
            if let Some(parent_id) = parent_id_opt {
                let prev_parent_rect = self.prev_rects.get(parent_id);
                let curr_parent_rect = self.rects.get(parent_id);
                let prev_parent_clip = self.prev_clip_rects.get(parent_id);
                let curr_parent_clip = self.clip_rects.get(parent_id);
                let is_parent_dirty = self.active_masks[parent_id].has(STATE_QUEUED_LAYOUT);

                if prev_parent_rect != curr_parent_rect
                    || prev_parent_clip != curr_parent_clip
                    || is_parent_dirty
                {
                    parent_changed = true; // 親が動いた、サイズが変わった、クリップが変わった、または親にレイアウト変更がある
                }
            }

            // 静的キャッシュバイパス判定
            let has_style_changed = self.active_masks[id].has(STATE_QUEUED_LAYOUT);

            if !window_resized
                && !has_style_changed
                && !parent_changed
                && self.prev_rects.contains_key(id)
            {
                // 自分自身のスタイルが変わっておらず、親も動いていない、かつモニターリサイズもされていないならキャッシュ利用
                let cached_rect = self.prev_rects[id];
                self.rects.insert(id, cached_rect);

                // クリップも同様にキャッシュ再利用
                let cached_clip = self.prev_clip_rects[id];
                self.clip_rects.insert(id, cached_clip);

                self.active_entities.push(id);
                continue;
            }

            // キャッシュが使えない場合のみ、Taffyから実データを引き出す
            let local_rect = if let Some(&taffy_node) = self.taffy_nodes.get(id) {
                if let Ok(layout) = self.taffy.layout(taffy_node) {
                    LayoutRect::new(
                        layout.location.x,
                        layout.location.y,
                        layout.size.width,
                        layout.size.height,
                    )
                } else {
                    LayoutRect::ZERO
                }
            } else {
                LayoutRect::ZERO
            };

            let (abs_rect, parent_clip) = if let Some(parent_id) = parent_id_opt {
                let parent_rect = self.rects[parent_id];
                let parent_clip = self.clip_rects[parent_id];

                let is_absolute = self
                    .basic_layouts
                    .get(id)
                    .map(|l| l.position == Position::Absolute)
                    .unwrap_or(false);

                let parent_scroll = if is_absolute {
                    LayoutPoint::ZERO
                } else {
                    self.scroll_offsets
                        .get(parent_id)
                        .copied()
                        .unwrap_or(LayoutPoint::ZERO)
                };

                let abs_x = parent_rect.x + local_rect.x - parent_scroll.x;
                let abs_y = parent_rect.y + local_rect.y - parent_scroll.y;

                (
                    LayoutRect::new(abs_x, abs_y, local_rect.width, local_rect.height),
                    parent_clip,
                )
            } else {
                (
                    LayoutRect::new(
                        local_rect.x,
                        local_rect.y,
                        local_rect.width,
                        local_rect.height,
                    ),
                    initial_clip,
                )
            };

            self.rects.insert(id, abs_rect);
            let mask = self.active_masks[id];

            if mask.has(COMP_INPUT_CONTENT)
                && let Some(contents) = self.input_contents.get_mut(id)
            {
                contents.last_bounds = Some(abs_rect);
            }

            // サイズが0.0の要素や画面外の要素の描画スキップ処理は、将来レンダラー側（wgpu等）に委譲。
            let current_clip = if mask.has(STYLE_OVERFLOW) {
                parent_clip.intersect(&abs_rect)
            } else {
                parent_clip
            };
            self.clip_rects.insert(id, current_clip);

            // 常に1次元DFS順でアクティブ要素リストに登録する
            self.active_entities.push(id);
        }

        // スクロールバー要素（Track & Thumb）のサイズ・配置・不透明度を一括同期更新
        let scrollbar_ids: Vec<EntityId> = self.scrollbar_styles.keys().collect();
        for id in scrollbar_ids {
            let sb_state = self.scrollbar_styles.get(id).cloned().unwrap();
            let container_rect = self.rects[id];
            let scroll_size = self.get_scroll_size(id);
            let current_scroll = self
                .scroll_offsets
                .get(id)
                .copied()
                .unwrap_or(LayoutPoint::ZERO);

            // 親コンテナのボーダーおよびパディング厚を取得
            let (basic, _, _) = self.resolve_active_layouts(id);
            let border_right = match basic.border.right {
                Length::Px(v) => v,
                _ => 0.0,
            };
            let border_bottom = match basic.border.bottom {
                Length::Px(v) => v,
                _ => 0.0,
            };
            let border_left = match basic.border.left {
                Length::Px(v) => v,
                _ => 0.0,
            };
            let border_top = match basic.border.top {
                Length::Px(v) => v,
                _ => 0.0,
            };

            let padding_bottom = match basic.padding.bottom {
                Length::Px(v) => v,
                _ => 0.0,
            };
            let padding_right = match basic.padding.right {
                Length::Px(v) => v,
                _ => 0.0,
            };
            let padding_left = match basic.padding.left {
                Length::Px(v) => v,
                _ => 0.0,
            };
            let padding_top = match basic.padding.top {
                Length::Px(v) => v,
                _ => 0.0,
            };

            // ウィンドウ内の有効表示サイズを算出
            let visible_w = if window_size.width > 0.0 {
                let left = container_rect.x.max(0.0);
                let right = (container_rect.x + container_rect.width).min(window_size.width);
                (right - left).max(0.0)
            } else {
                container_rect.width
            };

            let visible_h = if window_size.height > 0.0 {
                let top = container_rect.y.max(0.0);
                let bottom = (container_rect.y + container_rect.height).min(window_size.height);
                (bottom - top).max(0.0)
            } else {
                container_rect.height
            };

            // 全体表示サイズから、ボーダーとパディングを差し引いた内枠の有効表示領域サイズを算出
            let content_w =
                (visible_w - border_left - border_right - padding_left - padding_right).max(0.0);
            let content_h =
                (visible_h - border_top - border_bottom - padding_top - padding_bottom).max(0.0);

            // 内枠の有効表示領域と、同じく内枠基準の scroll_size を精密に比較する
            let show_v_bar = scroll_size.height > content_h;
            let show_h_bar = scroll_size.width > content_w;

            // 縦スクロールバーの同期
            if let Some(v_track) = sb_state.v_track_id {
                let show = show_v_bar && sb_state.style.display != ScrollbarDisplay::None;

                let mut v_track_visible = false;
                let mut v_track_opacity = 1.0f32;

                if show {
                    if sb_state.style.display == ScrollbarDisplay::Always
                        || sb_state.style.display == ScrollbarDisplay::Auto
                    {
                        v_track_visible = true;
                    } else if sb_state.style.display == ScrollbarDisplay::Transient
                        && let Some(last) = sb_state.last_scroll_time
                    {
                        let elapsed = last.elapsed();
                        if elapsed < Duration::from_millis(1000) {
                            v_track_visible = true;
                        } else if elapsed < Duration::from_millis(1500) {
                            v_track_visible = true;
                            v_track_opacity = 1.0 - (elapsed.as_secs_f32() - 1.0) / 0.5;
                        }
                    }
                }

                let track_node = self.taffy_nodes[v_track];
                if v_track_visible {
                    let mut user_track_h = None;
                    if let Some(ref track_style) = sb_state.style.v_track
                        && let Val::Px(val) = track_style.inner.basic_layout.size.height
                    {
                        user_track_h = Some(val);
                    }

                    // 有効可視サイズを起点にすることで画面外突き出しをクランプ
                    let track_h = if let Some(h) = user_track_h {
                        h
                    } else {
                        (visible_h
                            - border_top
                            - border_bottom
                            - (if show_h_bar {
                                sb_state.style.width
                            } else {
                                0.0
                            }))
                        .max(0.0)
                    };

                    let track_right = if sb_state.style.mode == ScrollbarMode::Layout {
                        -sb_state.style.width
                    } else {
                        0.0
                    };

                    let update_layouts = |layout: &mut BasicLayout| {
                        layout.display = Display::Flex;
                        layout.size.height = Val::Px(track_h);
                        layout.inset.top = Val::Px(0.0);
                        layout.inset.right = Val::Px(track_right);
                    };

                    if let Some(layout) = self.basic_layouts.get_mut(v_track) {
                        update_layouts(layout);
                    }
                    if let Some(layout) = self.base_basic_layouts.get_mut(v_track) {
                        update_layouts(layout);
                    }

                    if let Some(vis) = self.visual_properties.get_mut(v_track) {
                        vis.opacity = Some(v_track_opacity);
                    }
                    if let Some(vis) = self.base_visual_properties.get_mut(v_track) {
                        vis.opacity = Some(v_track_opacity);
                    }

                    let (basic, flex, _) = self.resolve_active_layouts(v_track);
                    let taffy_style = resolve_taffy_style(&basic, &flex, None, None);
                    let _ = self.taffy.set_style(track_node, taffy_style);
                } else {
                    let hide_layouts = |layout: &mut BasicLayout| {
                        layout.display = Display::None;
                    };
                    if let Some(layout) = self.basic_layouts.get_mut(v_track) {
                        hide_layouts(layout);
                    }
                    if let Some(layout) = self.base_basic_layouts.get_mut(v_track) {
                        hide_layouts(layout);
                    }
                    let _ = self.taffy.set_style(
                        track_node,
                        taffy::Style {
                            display: taffy::Display::None,
                            ..Default::default()
                        },
                    );
                }
            }

            // A-1. 縦つまみ (V-Thumb) の同期
            if let Some(v_thumb) = sb_state.v_thumb_id {
                let show = show_v_bar && sb_state.style.display != ScrollbarDisplay::None;

                let mut v_thumb_visible = false;
                let mut v_thumb_opacity = 1.0f32;

                if show {
                    if sb_state.style.display == ScrollbarDisplay::Always
                        || sb_state.style.display == ScrollbarDisplay::Auto
                    {
                        v_thumb_visible = true;
                    } else if sb_state.style.display == ScrollbarDisplay::Transient
                        && let Some(last) = sb_state.last_scroll_time
                    {
                        let elapsed = last.elapsed();
                        if elapsed < Duration::from_millis(1000) {
                            v_thumb_visible = true;
                        } else if elapsed < Duration::from_millis(1500) {
                            v_thumb_visible = true;
                            v_thumb_opacity = 1.0 - (elapsed.as_secs_f32() - 1.0) / 0.5;
                        }
                    }
                }

                let thumb_node = self.taffy_nodes[v_thumb];
                if v_thumb_visible {
                    let track_h = (visible_h
                        - border_top
                        - border_bottom
                        - (if show_h_bar {
                            sb_state.style.width
                        } else {
                            0.0
                        }))
                    .max(0.0);

                    let mut initial_thumb_h = track_h;
                    if let Some(ref thumb_style) = sb_state.style.v_thumb
                        && let Val::Px(val) = thumb_style.inner.basic_layout.size.height
                    {
                        initial_thumb_h = val;
                    }

                    // コンテンツ比率 (分母・分子に一貫してクランプ済み表示領域を採用)
                    let view_ratio = if scroll_size.height > 0.0 {
                        (visible_h / scroll_size.height).min(1.0)
                    } else {
                        1.0
                    };

                    let calculated_h = initial_thumb_h * view_ratio;

                    // ユーザー指定の min_size と max_size で正確にクランプ
                    let mut min_h = 24.0;
                    if let Some(ref thumb_style) = sb_state.style.v_thumb
                        && let Val::Px(val) = thumb_style.inner.basic_layout.min_size.height
                    {
                        min_h = val;
                    }
                    let mut max_h = track_h;
                    if let Some(ref thumb_style) = sb_state.style.v_thumb
                        && let Val::Px(val) = thumb_style.inner.basic_layout.max_size.height
                    {
                        max_h = val;
                    }

                    let thumb_height = calculated_h.max(min_h).min(max_h).min(track_h);

                    let mut margin_top = 0.0;
                    let mut margin_bottom = 0.0;
                    if let Some(ref thumb_style) = sb_state.style.v_thumb {
                        if let Val::Px(val) = thumb_style.inner.basic_layout.margin.top {
                            margin_top = val;
                        }
                        if let Val::Px(val) = thumb_style.inner.basic_layout.margin.bottom {
                            margin_bottom = val;
                        }
                    }

                    let scroll_ratio = if scroll_size.height > visible_h {
                        (current_scroll.y / (scroll_size.height - visible_h)).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };

                    let max_thumb_y =
                        (track_h - thumb_height - margin_top - margin_bottom).max(0.0);
                    let thumb_y = max_thumb_y * scroll_ratio;

                    let mut thumb_width = sb_state.style.width;

                    let mut pad_right = 0.0;
                    let mut pad_left = 0.0;
                    if let Some(ref thumb_style) = sb_state.style.v_thumb {
                        if let Val::Px(w) = thumb_style.inner.basic_layout.size.width {
                            thumb_width = w.min(sb_state.style.width);
                        }
                        if let Length::Px(val) = thumb_style.inner.basic_layout.padding.right {
                            pad_right = val;
                        }
                        if let Length::Px(val) = thumb_style.inner.basic_layout.padding.left {
                            pad_left = val;
                        }
                    }

                    let thumb_x = if pad_right > 0.0 {
                        sb_state.style.width - thumb_width - pad_right
                    } else if pad_left > 0.0 {
                        pad_left
                    } else {
                        (sb_state.style.width - thumb_width) * 0.5
                    };

                    let update_layouts = |layout: &mut BasicLayout| {
                        layout.display = Display::Flex;
                        layout.size.height = Val::Px(thumb_height);
                        layout.size.width = Val::Px(thumb_width);
                        layout.inset.top = Val::Px(thumb_y);
                        layout.inset.left = Val::Px(thumb_x);
                    };

                    // base_basic_layouts も同時同期することで、ホバー時の強制上書きによる位置ガタつきを完璧に阻止します
                    if let Some(layout) = self.basic_layouts.get_mut(v_thumb) {
                        update_layouts(layout);
                    }
                    if let Some(layout) = self.base_basic_layouts.get_mut(v_thumb) {
                        update_layouts(layout);
                    }

                    if let Some(vis) = self.visual_properties.get_mut(v_thumb) {
                        vis.opacity = Some(v_thumb_opacity);
                    }
                    if let Some(vis) = self.base_visual_properties.get_mut(v_thumb) {
                        vis.opacity = Some(v_thumb_opacity);
                    }

                    let (basic, flex, _) = self.resolve_active_layouts(v_thumb);
                    let taffy_style = resolve_taffy_style(&basic, &flex, None, None);
                    let _ = self.taffy.set_style(thumb_node, taffy_style);
                } else {
                    let hide_layouts = |layout: &mut BasicLayout| {
                        layout.display = Display::None;
                    };
                    if let Some(layout) = self.basic_layouts.get_mut(v_thumb) {
                        hide_layouts(layout);
                    }
                    if let Some(layout) = self.base_basic_layouts.get_mut(v_thumb) {
                        hide_layouts(layout);
                    }
                    let _ = self.taffy.set_style(
                        thumb_node,
                        taffy::Style {
                            display: taffy::Display::None,
                            ..Default::default()
                        },
                    );
                }
            }

            // 2. 横スクロールバー (H-Track) の同期
            if let Some(h_track) = sb_state.h_track_id {
                let show = show_h_bar && sb_state.style.display != ScrollbarDisplay::None;

                let mut h_track_visible = false;
                let mut h_track_opacity = 1.0f32;

                if show {
                    if sb_state.style.display == ScrollbarDisplay::Always
                        || sb_state.style.display == ScrollbarDisplay::Auto
                    {
                        h_track_visible = true;
                    } else if sb_state.style.display == ScrollbarDisplay::Transient
                        && let Some(last) = sb_state.last_scroll_time
                    {
                        let elapsed = last.elapsed();
                        if elapsed < Duration::from_millis(1000) {
                            h_track_visible = true;
                        } else if elapsed < Duration::from_millis(1500) {
                            h_track_visible = true;
                            h_track_opacity = 1.0 - (elapsed.as_secs_f32() - 1.0) / 0.5;
                        }
                    }
                }

                let track_node = self.taffy_nodes[h_track];
                if h_track_visible {
                    let track_w = (visible_w
                        - border_left
                        - border_right
                        - (if show_v_bar {
                            sb_state.style.width
                        } else {
                            0.0
                        }))
                    .max(0.0);

                    let track_bottom = if sb_state.style.mode == ScrollbarMode::Layout {
                        -sb_state.style.width
                    } else {
                        0.0
                    };

                    let update_layouts = |layout: &mut BasicLayout| {
                        layout.display = Display::Flex;
                        layout.size.width = Val::Px(track_w);
                        layout.inset.bottom = Val::Px(track_bottom);
                        layout.inset.left = Val::Px(0.0);
                    };

                    if let Some(layout) = self.basic_layouts.get_mut(h_track) {
                        update_layouts(layout);
                    }
                    if let Some(layout) = self.base_basic_layouts.get_mut(h_track) {
                        update_layouts(layout);
                    }

                    if let Some(vis) = self.visual_properties.get_mut(h_track) {
                        vis.opacity = Some(h_track_opacity);
                    }
                    if let Some(vis) = self.base_visual_properties.get_mut(h_track) {
                        vis.opacity = Some(h_track_opacity);
                    }

                    let (basic, flex, _) = self.resolve_active_layouts(h_track);
                    let taffy_style = resolve_taffy_style(&basic, &flex, None, None);
                    let _ = self.taffy.set_style(track_node, taffy_style);
                } else {
                    let hide_layouts = |layout: &mut BasicLayout| {
                        layout.display = Display::None;
                    };
                    if let Some(layout) = self.basic_layouts.get_mut(h_track) {
                        hide_layouts(layout);
                    }
                    if let Some(layout) = self.base_basic_layouts.get_mut(h_track) {
                        hide_layouts(layout);
                    }
                    let _ = self.taffy.set_style(
                        track_node,
                        taffy::Style {
                            display: taffy::Display::None,
                            ..Default::default()
                        },
                    );
                }
            }

            // B-1. 横つまみ (H-Thumb) の同期
            if let Some(h_thumb) = sb_state.h_thumb_id {
                let show = show_h_bar && sb_state.style.display != ScrollbarDisplay::None;

                let mut h_thumb_visible = false;
                let mut h_thumb_opacity = 1.0f32;

                if show {
                    if sb_state.style.display == ScrollbarDisplay::Always
                        || sb_state.style.display == ScrollbarDisplay::Auto
                    {
                        h_thumb_visible = true;
                    } else if sb_state.style.display == ScrollbarDisplay::Transient
                        && let Some(last) = sb_state.last_scroll_time
                    {
                        let elapsed = last.elapsed();
                        if elapsed < Duration::from_millis(1000) {
                            h_thumb_visible = true;
                        } else if elapsed < Duration::from_millis(1500) {
                            h_thumb_visible = true;
                            h_thumb_opacity = 1.0 - (elapsed.as_secs_f32() - 1.0) / 0.5;
                        }
                    }
                }

                let thumb_node = self.taffy_nodes[h_thumb];
                if h_thumb_visible {
                    let track_w = (visible_w
                        - border_left
                        - border_right
                        - (if show_v_bar {
                            sb_state.style.width
                        } else {
                            0.0
                        }))
                    .max(0.0);

                    let mut initial_thumb_w = track_w;
                    if let Some(ref thumb_style) = sb_state.style.h_thumb
                        && let Val::Px(w) = thumb_style.inner.basic_layout.size.width
                    {
                        initial_thumb_w = w;
                    }

                    let view_ratio = if scroll_size.width > 0.0 {
                        (visible_w / scroll_size.width).min(1.0)
                    } else {
                        1.0
                    };

                    let calculated_w = initial_thumb_w * view_ratio;

                    let mut min_w = 24.0;
                    if let Some(ref thumb_style) = sb_state.style.h_thumb
                        && let Val::Px(val) = thumb_style.inner.basic_layout.min_size.width
                    {
                        min_w = val;
                    }
                    let mut max_w = track_w;
                    if let Some(ref thumb_style) = sb_state.style.h_thumb
                        && let Val::Px(val) = thumb_style.inner.basic_layout.max_size.width
                    {
                        max_w = val;
                    }

                    let thumb_width = calculated_w.max(min_w).min(max_w).min(track_w);

                    let mut margin_left = 0.0;
                    let mut margin_right = 0.0;
                    if let Some(ref thumb_style) = sb_state.style.h_thumb {
                        if let Val::Px(val) = thumb_style.inner.basic_layout.margin.left {
                            margin_left = val;
                        }
                        if let Val::Px(val) = thumb_style.inner.basic_layout.margin.right {
                            margin_right = val;
                        }
                    }

                    let scroll_ratio = if scroll_size.width > visible_w {
                        (current_scroll.x / (scroll_size.width - visible_w)).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };

                    let max_thumb_x = (track_w - thumb_width - margin_left - margin_right).max(0.0);
                    let thumb_x = max_thumb_x * scroll_ratio;

                    let mut thumb_height = sb_state.style.width;
                    let mut pad_top = 0.0;
                    let mut pad_bottom = 0.0;
                    if let Some(ref thumb_style) = sb_state.style.h_thumb {
                        if let Val::Px(val) = thumb_style.inner.basic_layout.size.height {
                            thumb_height = val.min(sb_state.style.width);
                        }
                        if let Length::Px(val) = thumb_style.inner.basic_layout.padding.top {
                            pad_top = val;
                        }
                        if let Length::Px(val) = thumb_style.inner.basic_layout.padding.bottom {
                            pad_bottom = val;
                        }
                    }

                    let thumb_y = if pad_bottom > 0.0 {
                        sb_state.style.width - thumb_height - pad_bottom
                    } else if pad_top > 0.0 {
                        pad_top
                    } else {
                        (sb_state.style.width - thumb_height) * 0.5
                    };

                    let update_layouts = |layout: &mut BasicLayout| {
                        layout.display = Display::Flex;
                        layout.size.width = Val::Px(thumb_width);
                        layout.size.height = Val::Px(thumb_height);
                        layout.inset.left = Val::Px(thumb_x);
                        layout.inset.top = Val::Px(thumb_y);
                    };

                    if let Some(layout) = self.basic_layouts.get_mut(h_thumb) {
                        update_layouts(layout);
                    }
                    if let Some(layout) = self.base_basic_layouts.get_mut(h_thumb) {
                        update_layouts(layout);
                    }

                    if let Some(vis) = self.visual_properties.get_mut(h_thumb) {
                        vis.opacity = Some(h_thumb_opacity);
                    }
                    if let Some(vis) = self.base_visual_properties.get_mut(h_thumb) {
                        vis.opacity = Some(h_thumb_opacity);
                    }

                    let (basic, flex, _) = self.resolve_active_layouts(h_thumb);
                    let taffy_style = resolve_taffy_style(&basic, &flex, None, None);
                    let _ = self.taffy.set_style(thumb_node, taffy_style);
                } else {
                    let hide_layouts = |layout: &mut BasicLayout| {
                        layout.display = Display::None;
                    };
                    if let Some(layout) = self.basic_layouts.get_mut(h_thumb) {
                        hide_layouts(layout);
                    }
                    if let Some(layout) = self.base_basic_layouts.get_mut(h_thumb) {
                        hide_layouts(layout);
                    }
                    let _ = self.taffy.set_style(
                        thumb_node,
                        taffy::Style {
                            display: taffy::Display::None,
                            ..Default::default()
                        },
                    );
                }
            }
        }

        // スクロールバー専用要素のサイズ・位置が確定したため、
        // 差分計算を走らせてマージンやパディングを考慮した物理位置を Taffy 内部で正確に解決
        if let Some(&root_node) = self.taffy_nodes.get(root) {
            let _ = self.taffy.compute_layout_with_measure(
                root_node,
                taffy::Size {
                    width: taffy::AvailableSpace::Definite(window_size.width),
                    height: taffy::AvailableSpace::Definite(window_size.height),
                },
                |known_dims: taffy::Size<Option<f32>>,
                 _available_space: taffy::Size<taffy::AvailableSpace>,
                 _node_id: taffy::NodeId,
                 context: Option<&mut EntityId>,
                 _style: &taffy::Style|
                 -> taffy::Size<f32> {
                    if let Some(&id) = context.as_deref() {
                        return with_context(|cx| {
                            if cx.active_masks[id].has(COMP_INPUT_CONTENT)
                                && let Some(contents) = cx.input_contents.get(id)
                                && let Some(layout_rect) = contents.last_layout
                            {
                                return taffy::Size {
                                    width: known_dims.width.unwrap_or(layout_rect.width),
                                    height: known_dims.height.unwrap_or(layout_rect.height),
                                };
                            }

                            // 2回目パスはキャッシュサイズを即時引き出して高速マッピング
                            if let Some(&rect) = cx.rects.get(id) {
                                taffy::Size {
                                    width: known_dims.width.unwrap_or(rect.width),
                                    height: known_dims.height.unwrap_or(rect.height),
                                }
                            } else {
                                taffy::Size::ZERO
                            }
                        });
                    }
                    taffy::Size::ZERO
                },
            );
        }

        // スクロールバー要素も含めて、Taffy から最終確定位置をすべて引き出して rects にマウント
        self.active_entities.clear();

        for i in 0..flat_len {
            let id = self.flat_dfs_sequence[i];

            let local_rect = if let Some(&taffy_node) = self.taffy_nodes.get(id) {
                if let Ok(layout) = self.taffy.layout(taffy_node) {
                    LayoutRect::new(
                        layout.location.x,
                        layout.location.y,
                        layout.size.width,
                        layout.size.height,
                    )
                } else {
                    LayoutRect::ZERO
                }
            } else {
                LayoutRect::ZERO
            };

            let (abs_rect, parent_clip) =
                if let Some(parent_id) = self.parents.get(id).copied().flatten() {
                    let parent_rect = self.rects[parent_id];
                    let parent_clip = self.clip_rects[parent_id];

                    let is_absolute = self
                        .basic_layouts
                        .get(id)
                        .map(|l| l.position == Position::Absolute)
                        .unwrap_or(false);

                    let parent_scroll = if is_absolute {
                        LayoutPoint::ZERO
                    } else {
                        self.scroll_offsets
                            .get(parent_id)
                            .copied()
                            .unwrap_or(LayoutPoint::ZERO)
                    };

                    let abs_x = parent_rect.x + local_rect.x - parent_scroll.x;
                    let abs_y = parent_rect.y + local_rect.y - parent_scroll.y;

                    (
                        LayoutRect::new(abs_x, abs_y, local_rect.width, local_rect.height),
                        parent_clip,
                    )
                } else {
                    (
                        LayoutRect::new(
                            local_rect.x,
                            local_rect.y,
                            local_rect.width,
                            local_rect.height,
                        ),
                        initial_clip,
                    )
                };

            self.rects.insert(id, abs_rect);
            let mask = self.active_masks[id];

            if mask.has(COMP_INPUT_CONTENT)
                && let Some(contents) = self.input_contents.get_mut(id)
            {
                contents.last_bounds = Some(abs_rect);
            }

            let current_clip = if mask.has(STYLE_OVERFLOW) {
                parent_clip.intersect(&abs_rect)
            } else {
                parent_clip
            };
            self.clip_rects.insert(id, current_clip);

            self.active_entities.push(id);
        }

        // 全アクティブコンテナのスクロールオフセット自動クランプ同期
        for i in 0..flat_len {
            let id = self.flat_dfs_sequence[i];
            if self.scroll_offsets.contains_key(id) {
                let current = self.scroll_offsets[id];
                // 枠サイズの変更があった場合など、現在の位置からはみ出していれば自動クランプ調整
                self.scroll_to(id, current.x, current.y);
            }
        }

        // 全ての座標確定と絶対クリップ範囲の同期が完了した最末尾で、
        // 一括して Dirty フラグの完全クリアおよびキューリストのリセットを実行
        self.clear_layout_dirty();
    }

    pub fn clear_layout_dirty(&mut self) {
        for id in self.dirty_layout_entities.drain(..) {
            if let Some(mask) = self.active_masks.get_mut(id) {
                mask.unset(STATE_QUEUED_LAYOUT);
            }
        }
        self.dirty_layout_entities.clear();
    }

    /// 描画（レンダー）ダーティ状態として登録された要素をすべてクリアします。
    pub fn clear_render_dirty(&mut self) {
        for id in self.dirty_render_entities.drain(..) {
            if let Some(mask) = self.active_masks.get_mut(id) {
                mask.unset(STATE_QUEUED_RENDER);
            }
        }
        self.dirty_render_entities.clear();
    }

    /// 現在の全アクティブ要素から、wgpu 用の前面・背面描画バッチを生成します
    pub fn collect_render_data(&self) -> RenderData {
        let mut batches = Vec::new();
        let mut current_instances = Vec::new();
        let mut current_ids = Vec::new();
        let mut last_clip = None;

        // 現在のバッチの種類 (通常)
        let mut current_batch_type = BatchType::Normal;

        // 静的なデフォルト値（一度だけ確保して使い回す）
        let default_visual = VisualProperty::default();

        // 各要素の実効 z_index を親から子へカスケード（伝播）して計算
        let mut effective_z_indices = SecondaryMap::with_capacity(self.active_entities.len());

        // flat_dfs_sequence は必ず親から子への順でフラットに並んでいるため、前方1方向の走査で完結
        for &id in &self.flat_dfs_sequence {
            let self_z = self.visual_properties.get(id).and_then(|v| v.z_index);

            let parent_z = self
                .parents
                .get(id)
                .copied()
                .flatten()
                .and_then(|pid| effective_z_indices.get(pid).copied());

            // 自身に z_index 指定があればそれを最優先し、
            // なければ親の実効 z_index を継承する（双方になければデフォルト 0）
            let eff_z = self_z.or(parent_z).unwrap_or(0);
            effective_z_indices.insert(id, eff_z);
        }

        // 実効 z_index で active_entities を安定ソート
        let mut sorted_entities = self.active_entities.clone();
        sorted_entities.sort_by_key(|&id| effective_z_indices.get(id).copied().unwrap_or(0));

        for &id in &sorted_entities {
            let rect = self.rects[id];
            if rect.width <= 0.0 || rect.height <= 0.0 {
                continue;
            }

            let clip = self.clip_rects[id];

            let is_webview = self.active_masks[id].has(COMP_WEBVIEW_CONTENT);

            // コントローラーがまだ初期化されていない（active_webviewsに入っていない）場合は、
            // 紺色の背景を通常通り描き込み、デスクトップが透けるのを完全に防止します。
            let is_webview_ready = is_webview && self.active_webviews.contains(&id);

            if is_webview_ready {
                // 1. 今まで溜まっている「通常（Normal）」のバッチがあれば一旦フラッシュ
                if !current_instances.is_empty() {
                    batches.push(DrawBatch {
                        scissor_rect: last_clip.unwrap_or(LayoutRect::ZERO),
                        instances: std::mem::take(&mut current_instances),
                        entity_ids: std::mem::take(&mut current_ids),
                        batch_type: current_batch_type,
                    });
                }

                let (basic, _, _) = self.resolve_active_layouts(id);
                let visual = self.visual_properties.get(id).unwrap_or(&default_visual);
                let origin = visual
                    .transform_origin
                    .map(|p| [p.x, p.y])
                    .unwrap_or([0.5, 0.5]);

                let full_transform = visual.transform.unwrap_or(IDENTITY_MATRIX);
                let packed_transform = [
                    full_transform[0], // X軸基底
                    full_transform[1], // Y軸基底
                    full_transform[3], // 平行移動部
                ];

                // 不透明度を 1.0 固定にせず、要素自身の opacity 値を引き渡す。
                // これにより、くり抜く強度がブレンドステート OneMinusSrcAlpha に正しく乗り、
                // wgpu側の親の背景色が適度に残ることでグループ合成を模倣しデスクトップ透過を防ぐ
                let punchout_opacity = visual.opacity.unwrap_or(1.0);

                // くり抜き（Punchout）用のインスタンスを作成して登録
                // wgpu のバッファの背景を、角丸を維持したまま完全に透明に上書き消去するためのインスタンス
                let punchout_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    // アルファを 1.0 で出力させることで、Destination Out ブレンドが
                    // 反応して背景アルファを完全に 0.0 にくり抜くようになります。
                    color: Color::WHITE, // 白（アルファ減算用）
                    corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO), // 角丸に沿ってくり抜く
                    border_width: EdgeInsets::ZERO, // くり抜き時は枠線は不要
                    border_color: Color::TRANSPARENT,
                    border_lengths: EdgeInsets::ZERO,
                    opacity_mode_sizing: [punchout_opacity, 0.0, 0.0, 0.0],
                    uv_max: [0.0; 2],
                    uv_min: [0.0; 2],
                    gradient_end_color: Color::TRANSPARENT,
                    gradient_angle: 0.0,
                    _padding: 0.0,
                    shadow_color: Color::TRANSPARENT,
                    shadow_params: [0.0; 4],
                };
                current_instances.push(punchout_instance);
                current_ids.push(id);

                // くり抜き用のバッチとして即座にフラッシュ
                batches.push(DrawBatch {
                    scissor_rect: clip,
                    instances: std::mem::take(&mut current_instances),
                    entity_ids: std::mem::take(&mut current_ids),
                    batch_type: BatchType::Punchout, // ★くり抜き用パイプラインを指示
                });

                // 3. 次に「前面装飾（通常）」用のインスタンスを作成して登録
                // (くり抜かれた透明の窓の上に、枠線、角丸のアウトライン、影などをブレンド描画する)
                let border_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    color: Color::TRANSPARENT, // 背景は透明
                    corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
                    border_width: EdgeInsets {
                        top: basic.border.top.into(),
                        right: basic.border.right.into(),
                        bottom: basic.border.bottom.into(),
                        left: basic.border.left.into(),
                    },
                    border_color: visual.border_color.unwrap_or(Color::TRANSPARENT),
                    border_lengths: visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0)),
                    opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), 0.0, 0.0, 0.0],
                    uv_max: [0.0; 2],
                    uv_min: [0.0; 2],
                    gradient_end_color: Color::TRANSPARENT,
                    gradient_angle: 0.0,
                    _padding: 0.0,
                    shadow_color: Color::WHITE,
                    shadow_params: [0.0; 4],
                };
                current_instances.push(border_instance);
                current_ids.push(id);

                current_batch_type = BatchType::Normal; // 以降はまた通常バッチに戻す
                last_clip = Some(clip);
                continue;
            }

            // 非アクティブな WebView2（静止キャッシュ画像）のバッチ隔離
            // 一般要素と絶対にバッチを混在させないことで、テクスチャ（アトラス）の相互汚染を100%防止します。
            let is_webview_static = is_webview && !is_webview_ready;

            if is_webview_static {
                // 1. 現在溜まっている一般UIインスタンスがあれば一度ここで強制フラッシュ
                if !current_instances.is_empty() {
                    batches.push(DrawBatch {
                        scissor_rect: last_clip.unwrap_or(LayoutRect::ZERO),
                        instances: std::mem::take(&mut current_instances),
                        entity_ids: std::mem::take(&mut current_ids),
                        batch_type: current_batch_type,
                    });
                }

                // 2. この静止 WebView2 専用のバッチを直ちに単独構築
                let (basic, _, _) = self.resolve_active_layouts(id);
                let visual = self.visual_properties.get(id).unwrap_or(&default_visual);
                let origin = visual
                    .transform_origin
                    .map(|p| [p.x, p.y])
                    .unwrap_or([0.5, 0.5]);

                let full_transform = visual.transform.unwrap_or(IDENTITY_MATRIX);
                let packed_transform = [
                    full_transform[0], // X軸基底
                    full_transform[1], // Y軸基底
                    full_transform[3], // 平行移動部
                ];

                let static_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    color: Color::TRANSPARENT,
                    corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
                    border_width: EdgeInsets {
                        top: basic.border.top.into(),
                        right: basic.border.right.into(),
                        bottom: basic.border.bottom.into(),
                        left: basic.border.left.into(),
                    },
                    border_color: visual.border_color.unwrap_or(Color::TRANSPARENT),
                    border_lengths: visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0)),
                    opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), 0.0, 0.0, 0.0],
                    uv_max: [0.0; 2],
                    uv_min: [0.0; 2],
                    gradient_end_color: Color::TRANSPARENT,
                    gradient_angle: 0.0,
                    _padding: 0.0,
                    shadow_color: Color::TRANSPARENT,
                    shadow_params: [0.0; 4],
                };
                current_instances.push(static_instance);
                current_ids.push(id);

                batches.push(DrawBatch {
                    scissor_rect: clip,
                    instances: std::mem::take(&mut current_instances),
                    entity_ids: std::mem::take(&mut current_ids),
                    batch_type: BatchType::Normal, // 静止キャッシュ表示用通常描画
                });

                let border_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    color: Color::TRANSPARENT,
                    corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
                    border_width: EdgeInsets {
                        top: basic.border.top.into(),
                        right: basic.border.right.into(),
                        bottom: basic.border.bottom.into(),
                        left: basic.border.left.into(),
                    },
                    border_color: visual.border_color.unwrap_or(Color::TRANSPARENT),
                    border_lengths: visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0)),
                    opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), 0.0, 0.0, 0.0],
                    uv_max: [0.0; 2],
                    uv_min: [0.0; 2],
                    gradient_end_color: Color::TRANSPARENT,
                    gradient_angle: 0.0,
                    _padding: 0.0,
                    shadow_color: Color::WHITE,
                    shadow_params: [0.0; 4],
                };
                current_instances.push(border_instance);
                current_ids.push(id);

                batches.push(DrawBatch {
                    scissor_rect: clip,
                    instances: std::mem::take(&mut current_instances),
                    entity_ids: std::mem::take(&mut current_ids),
                    batch_type: BatchType::Normal,
                });

                last_clip = Some(clip);
                continue;
            }

            // 初回の初期化を安全にキャッチし、異なるクリップ境界の時に新しいバッチを作成する
            if let Some(prev_clip) = last_clip {
                if clip != prev_clip {
                    if !current_instances.is_empty() {
                        let batch = DrawBatch {
                            scissor_rect: prev_clip,
                            instances: std::mem::take(&mut current_instances),
                            entity_ids: std::mem::take(&mut current_ids),
                            batch_type: current_batch_type,
                        };
                        // 構築したバッチを確実にプッシュ（フラッシュバグの修正）
                        batches.push(batch);
                    }
                    last_clip = Some(clip);
                }
            } else {
                // 最初の要素（root）の時点で、確実にその要素のクリップ矩形で初期化します
                last_clip = Some(clip);
            }

            let (basic, _, _) = self.resolve_active_layouts(id);
            let visual = self.visual_properties.get(id).unwrap_or(&default_visual);

            // 選択ハイライト背景のwgpu側への差し込み
            // キャッシュされた選択背景矩形群を描画
            if let Some(rects) = self.selected_rects.get(id) {
                let border_left = match basic.border.left {
                    Length::Px(v) => v,
                    _ => 0.0,
                };
                let padding_left = match basic.padding.left {
                    Length::Px(v) => v,
                    _ => 0.0,
                };
                let border_top = match basic.border.top {
                    Length::Px(v) => v,
                    _ => 0.0,
                };
                let padding_top = match basic.padding.top {
                    Length::Px(v) => v,
                    _ => 0.0,
                };

                let sel_bg = visual
                    .select_bg_color
                    .unwrap_or(Color::rgba_f32(0.0, 0.47, 0.84, 0.35));

                for metric_rect in rects {
                    let sel_rect = LayoutRect::new(
                        rect.x + border_left + padding_left + metric_rect.x,
                        rect.y + border_top + padding_top + metric_rect.y,
                        metric_rect.width,
                        metric_rect.height,
                    );

                    let full_transform = visual.transform.unwrap_or(IDENTITY_MATRIX);
                    let packed_transform = [
                        full_transform[0], // X軸基底
                        full_transform[1], // Y軸基底
                        full_transform[3], // 平行移動部
                    ];

                    let sel_instance = QuadInstance {
                        rect: sel_rect,
                        transform: packed_transform,
                        transform_origin: [0.5, 0.5],
                        color: sel_bg,
                        corner_radius: CornerRadius::ZERO,
                        border_width: EdgeInsets::ZERO,
                        border_color: Color::TRANSPARENT,
                        border_lengths: EdgeInsets::ZERO,
                        // mode = -1.0（デコレーター上書きをバイパス）
                        opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), -1.0, 0.0, 0.0],
                        uv_max: [0.0; 2],
                        uv_min: [0.0; 2],
                        gradient_end_color: Color::TRANSPARENT,
                        gradient_angle: 0.0,
                        _padding: 0.0,
                        shadow_color: Color::TRANSPARENT,
                        shadow_params: [0.0; 4],
                    };
                    current_instances.push(sel_instance);
                    current_ids.push(id);
                }
            }

            // 背景色とテキストの多重描画の解決
            let is_text = self.active_masks[id].has(COMP_TEXT_CONTENT);
            let has_bg = visual.bg_color.is_some()
                || visual.bg_gradient.is_some()
                || visual.border_color.is_some()
                || visual.shadow_params.is_some();

            let box_sizing_val = match basic.box_sizing {
                BoxSizing::BorderBox => 0.0f32,
                BoxSizing::ContentBox => 1.0f32,
            };

            // テキスト要素かつ背景・枠線・影などを持つ場合、まず背景用のインスタンスを先に差し込む
            if is_text && has_bg {
                let bg_color = visual.bg_color.unwrap_or(Color::TRANSPARENT);
                let (gradient_end_color, gradient_angle, bg_mode) = match visual.bg_gradient {
                    Some(g) => (g.end_color, g.angle, 1.0f32),
                    None => (bg_color, 0.0, 0.0f32), // mode = -1.0 (装飾モードとしてテキストをバイパス)
                };

                let full_transform = visual.transform.unwrap_or(IDENTITY_MATRIX);
                let packed_transform = [
                    full_transform[0], // X軸基底
                    full_transform[1], // Y軸基底
                    full_transform[3], // 平行移動部
                ];

                let bg_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: visual
                        .transform_origin
                        .map(|p| [p.x, p.y])
                        .unwrap_or([0.5, 0.5]),
                    color: bg_color,
                    corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
                    border_width: EdgeInsets {
                        top: basic.border.top.into(),
                        right: basic.border.right.into(),
                        bottom: basic.border.bottom.into(),
                        left: basic.border.left.into(),
                    },
                    border_color: visual.border_color.unwrap_or(Color::TRANSPARENT),
                    border_lengths: visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0)),
                    opacity_mode_sizing: [
                        visual.opacity.unwrap_or(1.0),
                        bg_mode,
                        box_sizing_val,
                        0.0,
                    ],
                    uv_max: [0.0; 2],
                    uv_min: [0.0; 2],
                    gradient_end_color,
                    gradient_angle,
                    _padding: 0.0,
                    shadow_color: Color::WHITE,
                    shadow_params: [0.0; 4],
                };
                current_instances.push(bg_instance);
                current_ids.push(id);
            }

            // 以下、通常のテキスト/背景描画を重ねる（選択矩形が文字の下に）
            // テキスト要素の場合は「テキストの色」、それ以外は「背景色」を color にセットする
            let color = if self.active_masks[id].has(COMP_TEXT_CONTENT) {
                visual.text_color.unwrap_or(Color::BLACK)
            } else {
                visual.bg_color.unwrap_or(Color::TRANSPARENT)
            };

            // グラデーションの解決 (WebView2 がアクティブな場合は、グラデーションもキャンセルして透明化)
            let (gradient_end_color, gradient_angle, mode) = match visual.bg_gradient {
                Some(g) => (g.end_color, g.angle, 1.0f32),
                None => (color, 0.0, 0.0f32),
            };

            // テキスト要素で背景を分離描画した場合、テキストレイヤー側の枠線は不要
            let border_width = if is_text && has_bg {
                EdgeInsets::ZERO
            } else {
                EdgeInsets {
                    top: basic.border.top.into(),
                    right: basic.border.right.into(),
                    bottom: basic.border.bottom.into(),
                    left: basic.border.left.into(),
                }
            };

            let border_lengths = if is_text && has_bg {
                EdgeInsets::ZERO
            } else {
                visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0))
            };

            let border_color = if is_text && has_bg {
                Color::TRANSPARENT
            } else {
                visual.border_color.unwrap_or(Color::TRANSPARENT)
            };

            // テキスト要素で背景を分離描画した場合、テキストレイヤー側の影（BoxShadow）は不要
            let shadow_color = if is_text && has_bg {
                Color::TRANSPARENT // wgpu_renderer側で影を完全無効化させます
            } else {
                Color::WHITE
            };

            let origin = visual
                .transform_origin
                .map(|p| [p.x, p.y])
                .unwrap_or([0.5, 0.5]); // デフォルトは中心

            let full_transform = visual.transform.unwrap_or(IDENTITY_MATRIX);
            let packed_transform = [
                full_transform[0], // X軸基底
                full_transform[1], // Y軸基底
                full_transform[3], // 平行移動部
            ];

            // SoAからGPU用インスタンスデータへ変換
            let instance = QuadInstance {
                rect,
                transform: packed_transform,
                transform_origin: origin,
                color,
                corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
                border_width,
                border_color,
                border_lengths,
                opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), mode, 0.0, 0.0],
                uv_max: [0.0; 2],
                uv_min: [0.0; 2],
                gradient_end_color,
                gradient_angle,
                _padding: 0.0,
                shadow_color,
                shadow_params: [0.0; 4],
            };

            current_instances.push(instance);
            current_ids.push(id);

            let is_input = self.active_masks[id].has(COMP_INPUT_CONTENT);
            let is_focused = self.interaction_states.focused == Some(id);

            if is_input
                && is_focused
                && let Some(contents) = self.input_contents.get(id)
            {
                let now_instant = Instant::now();
                // 現在のミリ秒から点滅周期を自動計算
                let show_caret = if let Some(last) = contents.last_interacted_time
                    && now_instant.duration_since(last) < Duration::from_millis(300)
                {
                    true // キー入力や移動の操作から 500ms 未満のときは、点滅させずに常時表示
                } else if contents.is_blink {
                    let freq = contents
                        .blink_frequency
                        .unwrap_or(Duration::from_millis(530)) // Windows標準
                        .as_millis();
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                    (now / freq).is_multiple_of(2)
                } else {
                    contents.has_caret
                };

                if show_caret {
                    let visual = self.visual_properties.get(id).unwrap_or(&default_visual);
                    let font_size = visual.font_size.unwrap_or(16.0);

                    let border_top = match basic.border.top {
                        Length::Px(v) => v,
                        _ => 0.0,
                    };
                    let border_left = match basic.border.left {
                        Length::Px(v) => v,
                        _ => 0.0,
                    };
                    let padding_top = match basic.padding.top {
                        Length::Px(v) => v,
                        _ => 0.0,
                    };
                    let padding_left = match basic.padding.left {
                        Length::Px(v) => v,
                        _ => 0.0,
                    };

                    let scale = self.scale_factor;

                    // 1. [太さ変化の解消] X座標をDPIスケーリング後の物理ピクセルグリッドに完全にスナップ
                    let logical_x = rect.x + border_left + padding_left + contents.measured_caret_x;
                    let aligned_x = (logical_x * scale).round() / scale;

                    let line_height = contents.caret_line_height;
                    let caret_width = contents.caret_width.unwrap_or(1.5);

                    // キャレット高さを、明示指定された縮小サイズにするか、
                    // デフォルトでは「行高全体の85%（文字のインク境界に完璧に一致する高さ）」に設定
                    let caret_height = contents.caret_height.unwrap_or(line_height * 0.85);

                    // 2. キャレットサイズ縮小時も、行に対して「垂直中央配置」されるよう動的オフセットを算出
                    let vertical_center_offset = (line_height - caret_height) * 0.5;

                    let logical_y = rect.y
                        + border_top
                        + padding_top
                        + contents.measured_caret_y
                        + contents.caret_offset;

                    let aligned_y = ((logical_y + vertical_center_offset) * scale).round() / scale;
                    let aligned_width = (caret_width * scale).round().max(1.0) / scale;
                    let aligned_height = (caret_height * scale).round().max(1.0) / scale;

                    let caret_rect =
                        LayoutRect::new(aligned_x, aligned_y, aligned_width, aligned_height);

                    let c_color = contents
                        .caret_color
                        .or(visual.text_color)
                        .unwrap_or(Color::WHITE);
                    let visual = self.visual_properties.get(id).unwrap_or(&default_visual);

                    // 通常の Solid 矩形 (mode == 0.0) としてキャレット Quad を最前面に配置
                    let caret_instance = QuadInstance {
                        rect: caret_rect,
                        transform: packed_transform,
                        transform_origin: [0.5, 0.5],
                        color: c_color,
                        corner_radius: CornerRadius::ZERO,
                        border_width: EdgeInsets::ZERO,
                        border_color: Color::TRANSPARENT,
                        border_lengths: EdgeInsets::ZERO,
                        // 前面装飾スキップ
                        opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), -1.0, 0.0, 0.0],
                        uv_max: [0.0; 2],
                        uv_min: [0.0; 2],
                        gradient_end_color: Color::TRANSPARENT,
                        gradient_angle: 0.0,
                        _padding: 0.0,
                        shadow_color: Color::TRANSPARENT,
                        shadow_params: [0.0; 4],
                    };

                    current_instances.push(caret_instance);
                    current_ids.push(id);
                }
            }
        }

        // 走査終了後、最後に残ったバッチをフラッシュ
        if !current_instances.is_empty() {
            batches.push(DrawBatch {
                scissor_rect: last_clip.unwrap_or(LayoutRect::ZERO),
                instances: current_instances,
                entity_ids: current_ids,
                batch_type: current_batch_type,
            });
        }

        RenderData { batches }
    }

    /// 現在、アクティブに動いているトランジション（wgpuアニメーション）があるか判定します
    pub fn has_active_animations(&self) -> bool {
        // トランジション（CSS transition）のアクティブ判定
        let has_transitions = !self.active_transitions.is_empty()
            && self
                .active_transitions
                .values()
                .any(|list| !list.is_empty());

        // キーフレームアニメーション（CSS animation）のアクティブ判定
        let has_keyframes = !self.active_animations.is_empty()
            && self.active_animations.values().any(|list| !list.is_empty());

        // 3フォーカスされたインプットがあり、キャレット点滅が有効な間は描画ループを駆動
        let has_blinking_input = self
            .interaction_states
            .focused
            .and_then(|id| self.input_contents.get(id))
            .map(|c| c.has_caret && c.is_blink)
            .unwrap_or(false);

        // 一時的表示スクロールバーのフェード進行中は描画更新ループを継続
        let has_active_transient_scrollbar = self.scrollbar_styles.values().any(|sb_state| {
            sb_state.style.display == ScrollbarDisplay::Transient
                && sb_state
                    .last_scroll_time
                    .map(|t| t.elapsed() < Duration::from_millis(1500))
                    .unwrap_or(false)
        });

        has_transitions || has_keyframes || has_blinking_input || has_active_transient_scrollbar
    }

    /// 毎フレームの描画前に呼び出され、すべてのアクティブなトランジションを 1 Tick 進めます
    pub fn tick_transitions(&mut self) {
        let now = Instant::now();

        // (1.0 / 120.0 秒 = 約 8,333,333 ナノ秒)
        const FRAME_TIME_120FPS: Duration = Duration::from_nanos(8_333_333);
        if let Some(last) = self.last_tick_time
            && now.duration_since(last) < FRAME_TIME_120FPS
        {
            return;
        }

        // 実行制限を通過したため、基準時刻を更新して処理を継続
        self.last_tick_time = Some(now);

        // 借用チェッカーを回避するため、一時的にマップを take して更新する
        let mut active_map = std::mem::take(&mut self.active_transitions);

        // 完了して空になった要素のIDを記録する一時配列
        let mut to_remove = Vec::new();

        for (id, transitions) in active_map.iter_mut() {
            let mut i = 0;
            while i < transitions.len() {
                let t_state = &mut transitions[i];

                // start_time が None なら、このフレームの時刻 now を格納しその値を取り出す。
                let start_time = *t_state.start_time.get_or_insert(now);
                let elapsed = now.duration_since(start_time);

                // 進行度 (0.0 ～ 1.0)
                let progress = (elapsed.as_secs_f32() / t_state.duration.as_secs_f32()).min(1.0);
                let eased_t = t_state.curve.evaluate(progress);

                // Lerpによる新しい値の決定
                let current_val = t_state.start_value.lerp(&t_state.end_value, eased_t);

                // SoA（Context のアクティブなプロパティ）に補間された値を書き戻す
                match current_val {
                    TransitionValue::Color(c) => {
                        if let Some(v) = self.visual_properties.get_mut(id) {
                            if t_state.property_list == PropertyList::BackgroundColor {
                                v.bg_color = Some(c);
                            } else if t_state.property_list == PropertyList::BorderColor {
                                v.border_color = Some(c);
                            }
                        }
                        self.mark_render_dirty(id);
                    }
                    TransitionValue::Opacity(o) => {
                        if let Some(v) = self.visual_properties.get_mut(id) {
                            v.opacity = Some(o);
                        }
                        self.mark_render_dirty(id);
                    }
                    TransitionValue::Transform(m) => {
                        if let Some(v) = self.visual_properties.get_mut(id) {
                            v.transform = Some(m);
                        }
                        self.mark_render_dirty(id);
                    }
                    TransitionValue::CornerRadius(cr) => {
                        if let Some(v) = self.visual_properties.get_mut(id) {
                            v.corner_radius = Some(cr);
                        }
                        self.mark_render_dirty(id);
                    }
                    TransitionValue::Width(w) => {
                        if let Some(layout) = self.basic_layouts.get_mut(id) {
                            layout.size.width = Val::Px(w); // ピクセル値で上書き
                        }
                        self.mark_layout_dirty(id); // レイアウト再計算をマーク

                        // キャッシュを毎フレーム強制バイパスさせるためにマスクを再セット
                        if let Some(mask) = self.active_masks.get_mut(id) {
                            mask.set(STATE_QUEUED_LAYOUT);
                        }
                    }
                    // 縦幅（Height）の毎フレームアニメーション補間
                    TransitionValue::Height(h) => {
                        if let Some(layout) = self.basic_layouts.get_mut(id) {
                            layout.size.height = Val::Px(h);
                        }
                        self.mark_layout_dirty(id);

                        if let Some(mask) = self.active_masks.get_mut(id) {
                            mask.set(STATE_QUEUED_LAYOUT);
                        }
                    }
                    // 影（BoxShadow）の毎フレームの書き戻し処理
                    TransitionValue::BoxShadow(shadow) => {
                        if let Some(v) = self.visual_properties.get_mut(id) {
                            v.shadow_params = Some(shadow);
                            v.shadow_color = Some(shadow.color);
                        }
                        self.mark_render_dirty(id);
                    }
                }

                // アニメーション完了判定
                if progress >= 1.0 {
                    transitions.remove(i);
                } else {
                    i += 1;
                }

                // トランジションが空になった要素をマーク
                if transitions.is_empty() {
                    to_remove.push(id);
                }
            }
        }

        // 空になったエントリをマップから完全削除（クリーンアップ）
        for id in to_remove {
            active_map.remove(id);
        }

        self.active_transitions = active_map;
    }

    /// 必要に応じてトランジションを起動、または上書き（逆再生含む）します
    pub(crate) fn trigger_transition_if_needed(
        &mut self,
        id: EntityId,
        property_list: PropertyList,
        start_value: TransitionValue,
        end_value: TransitionValue,
    ) -> bool {
        // 1. その要素に、このプロパティに対するトランジション設定が定義されているか検証
        if let Some(visual) = self.base_visual_properties.get(id) {
            // transitions ベクタの中から、一致する PropertyList を探す
            if let Some(t) = visual
                .transitions
                .iter()
                .find(|t| t.property_list == property_list || t.property_list == PropertyList::Size)
            {
                let now = Instant::now();
                if let Some(entry) = self.active_transitions.entry(id) {
                    let active_list = entry.or_insert_with(Vec::new);

                    // 2. 割り込み処理の解決（すでに同じプロパティのアニメーションが走っているか）
                    let actual_start = if let Some(existing) = active_list
                        .iter_mut()
                        .find(|et| et.property_list == property_list)
                    {
                        // すでに同じ目的地に向かってアニメーション中の場合は、
                        // 割り込みを一切行わず、そのまま既存アニメーションを走らせる
                        if existing.end_value == end_value {
                            return true;
                        }

                        // すでに駆動中の場合は、その現在の補間位置をリアルタイム計算する
                        // ※ start_time が None の場合（登録されたが一度も tick されていない場合）は
                        // 経過時間 0 として進捗 progress を 0.0 にする
                        let elapsed = existing
                            .start_time
                            .map(|st| now.duration_since(st))
                            .unwrap_or(Duration::ZERO);
                        let progress =
                            (elapsed.as_secs_f32() / existing.duration.as_secs_f32()).min(1.0);
                        let eased_t = existing.curve.evaluate(progress);

                        // 中間位置の算出（これが新しいアニメーションの開始点になる）
                        let current_interposed_val =
                            existing.start_value.lerp(&existing.end_value, eased_t);

                        // 既存のアニメーション状態をリセットし、現在地点から新しい目標値（end_value）へ向かうように上書き
                        existing.start_time = None;
                        existing.start_value = current_interposed_val;
                        existing.end_value = end_value;
                        existing.duration = t.duration;
                        existing.curve = t.curve;

                        return true; // 既存のアニメーションを上書き更新したため即時復帰
                    } else {
                        // 新規開始の場合は、渡された現在の開始値をそのまま採用
                        start_value
                    };

                    // 3. 新規トランジションをアクティブリストに登録
                    active_list.push(ActiveTransition {
                        property_list,
                        start_time: None,
                        duration: t.duration,
                        curve: t.curve,
                        start_value: actual_start,
                        end_value,
                    });

                    return true; // トランジションを正常に起動
                }
            }
        }
        false // トランジション設定がなかったため、即時適用パスへ
    }

    /// 状態の変更を検知し、アニメーション（トランジション）が必要な箇所を自動的に開始・制御します。
    pub(crate) fn resolve_element_style_state(&mut self, id: EntityId, allow_transition: bool) {
        let active_mask = self.active_masks[id];

        // ビジュアルプロパティ (bg_color, opacity等) の解決
        let has_base_visual = self.base_visual_properties.contains_key(id);
        let has_active_visual = self.visual_properties.contains_key(id);

        // 要素がホバーやプレス時の動的スタイルを登録しているか
        let has_interaction_styles = self.interaction_properties.contains_key(id);

        // スタイルを一切持たない要素は、ヒープアロケーションを避けるため完全にスキップ
        // 静的なベース装飾がなくても、ホバースタイル等を持っていれば確実にカスケード解決を通す
        if has_base_visual || has_active_visual || has_interaction_styles {
            // 不変参照から現在の描画用データを安全に取得 (Copy可能なプリミティブのみ)
            let current_bg = self
                .visual_properties
                .get(id)
                .and_then(|v| v.bg_color)
                .unwrap_or(Color::TRANSPARENT);
            let current_border = self
                .visual_properties
                .get(id)
                .and_then(|v| v.border_color)
                .unwrap_or(Color::TRANSPARENT);
            let current_opacity = self
                .visual_properties
                .get(id)
                .and_then(|v| v.opacity)
                .unwrap_or(1.0);
            let current_transform = self
                .visual_properties
                .get(id)
                .and_then(|v| v.transform)
                .unwrap_or(IDENTITY_MATRIX);
            let current_radius = self
                .visual_properties
                .get(id)
                .and_then(|v| v.corner_radius)
                .unwrap_or(CornerRadius::ZERO);
            let current_shadow = self
                .visual_properties
                .get(id)
                .and_then(|v| v.shadow_params)
                .unwrap_or(BoxShadow::none());

            let mut target_pointer_events = self
                .base_visual_properties
                .get(id)
                .and_then(|v| v.pointer_events);

            let mut target_cursor = self.base_visual_properties.get(id).and_then(|v| v.cursor);
            let mut target_resizable_cursor = self
                .base_visual_properties
                .get(id)
                .and_then(|v| v.resizable_cursor);

            // 目標値（Target）をクローンせずに参照経由で構築
            let mut target_bg = self.base_visual_properties.get(id).and_then(|v| v.bg_color);
            let mut target_border = self
                .base_visual_properties
                .get(id)
                .and_then(|v| v.border_color);
            let mut target_opacity = self.base_visual_properties.get(id).and_then(|v| v.opacity);
            let mut target_transform = self
                .base_visual_properties
                .get(id)
                .and_then(|v| v.transform);
            let mut target_radius = self
                .base_visual_properties
                .get(id)
                .and_then(|v| v.corner_radius);
            // 影（BoxShadow）の動的ターゲットを初期化
            let mut target_shadow_params = self
                .base_visual_properties
                .get(id)
                .and_then(|v| v.shadow_params);
            let mut target_shadow_color = self
                .base_visual_properties
                .get(id)
                .and_then(|v| v.shadow_color);
            let mut target_text_color = self
                .base_visual_properties
                .get(id)
                .and_then(|v| v.text_color);
            let mut target_select_bg = self
                .base_visual_properties
                .get(id)
                .and_then(|v| v.select_bg_color);
            let mut target_select_text = self
                .base_visual_properties
                .get(id)
                .and_then(|v| v.select_text_color);
            let mut target_border_lengths = self
                .base_visual_properties
                .get(id)
                .and_then(|v| v.border_lengths);
            let mut target_border_styles = self
                .base_visual_properties
                .get(id)
                .and_then(|v| v.border_styles);
            let mut target_border_alignments = self
                .base_visual_properties
                .get(id)
                .and_then(|v| v.border_alignments);

            // 疑似クラス（Hovered等）のマージをクローンなしで解決
            if let Some(interaction) = self.interaction_properties.get(id) {
                let cascade = [
                    (STATE_FOCUSED, &interaction.focused),
                    (STATE_SELECTED, &interaction.selected),
                    (STATE_ACTIVED, &interaction.actived),
                    (STATE_HOVERED, &interaction.hovered),
                    (STATE_PRESSED, &interaction.pressed),
                    (STATE_DISABLED, &interaction.disabled),
                    (STATE_DRAGGED, &interaction.dragged),
                    (STATE_DRAGGING, &interaction.dragging),
                    (STATE_DRAG_IN, &interaction.drag_in),
                    (STATE_DRAG_OVER, &interaction.drag_over),
                ];

                for (state, style_opt) in cascade {
                    if active_mask.has(state)
                        && let Some(style) = style_opt
                    {
                        let inner_vis = &style.inner.visual_property;
                        let inner_mask = style.inner.mask;

                        if inner_mask.has(STYLE_BG_COLOR) {
                            target_bg = inner_vis.bg_color;
                        }
                        if inner_mask.has(STYLE_BORDER_COLOR) {
                            target_border = inner_vis.border_color;
                        }
                        if inner_mask.has(STYLE_OPACITY) {
                            target_opacity = inner_vis.opacity;
                        }
                        if inner_mask.has(STYLE_TRANSFORM) {
                            target_transform = inner_vis.transform;
                        }
                        if inner_mask.has(STYLE_CORNER_RADIUS) {
                            target_radius = inner_vis.corner_radius;
                        }
                        if inner_mask.has(STYLE_POINTER_EVENTS) {
                            target_pointer_events = inner_vis.pointer_events;
                        }
                        if inner_mask.has(STYLE_BOX_SHADOW) {
                            if inner_vis.shadow_params.is_some() {
                                target_shadow_params = inner_vis.shadow_params;
                            }
                            if inner_vis.shadow_color.is_some() {
                                target_shadow_color = inner_vis.shadow_color;
                            }
                        }
                        if inner_mask.has(STYLE_TEXT_COLOR) {
                            target_text_color = inner_vis.text_color;
                        }
                        if inner_mask.has(STYLE_USER_SELECT) {
                            if inner_vis.select_bg_color.is_some() {
                                target_select_bg = inner_vis.select_bg_color;
                            }
                            if inner_vis.select_text_color.is_some() {
                                target_select_text = inner_vis.select_text_color;
                            }
                        }
                        if inner_mask.has(STYLE_BORDER) {
                            if inner_vis.border_lengths.is_some() {
                                target_border_lengths = inner_vis.border_lengths;
                            }
                            if inner_vis.border_styles.is_some() {
                                target_border_styles = inner_vis.border_styles;
                            }
                            if inner_vis.border_alignments.is_some() {
                                target_border_alignments = inner_vis.border_alignments;
                            }
                        }
                        if inner_mask.has(STYLE_CURSOR) {
                            target_cursor = inner_vis.cursor;
                        }
                        if inner_mask.has(STYLE_RESIZABLE) {
                            target_resizable_cursor = inner_vis.resizable_cursor;
                        }
                    }
                }
            }

            if active_mask.has(STYLE_INTERACTION_WITHIN)
                && let Some(interaction) = self.interaction_properties.get(id)
            {
                // 自身の mask にビットが立っている場合のみツリー再帰を走らせてマージ解決
                let cascade_within = [
                    (STATE_FOCUSED, &interaction.focused_within),
                    (STATE_SELECTED, &interaction.selected_within),
                    (STATE_ACTIVED, &interaction.actived_within),
                    (STATE_HOVERED, &interaction.hovered_within),
                    (STATE_PRESSED, &interaction.pressed_within),
                    (STATE_DISABLED, &interaction.disabled_within),
                    (STATE_DRAGGING, &interaction.dragged_within),
                    (STATE_DRAG_IN, &interaction.hovered_within),
                ];

                for (state, style_opt) in cascade_within {
                    // 子孫要素のいずれかがこの state_flag を満たしているか
                    if self.has_descendant_with_state(id, state)
                        && let Some(style) = style_opt
                    {
                        let inner_vis = &style.inner.visual_property;
                        let inner_mask = style.inner.mask;

                        if inner_mask.has(STYLE_BG_COLOR) {
                            target_bg = inner_vis.bg_color;
                        }
                        if inner_mask.has(STYLE_BORDER_COLOR) {
                            target_border = inner_vis.border_color;
                        }
                        if inner_mask.has(STYLE_OPACITY) {
                            target_opacity = inner_vis.opacity;
                        }
                        if inner_mask.has(STYLE_TRANSFORM) {
                            target_transform = inner_vis.transform;
                        }
                        if inner_mask.has(STYLE_CORNER_RADIUS) {
                            target_radius = inner_vis.corner_radius;
                        }
                        if inner_mask.has(STYLE_POINTER_EVENTS) {
                            target_pointer_events = inner_vis.pointer_events;
                        }
                        if inner_mask.has(STYLE_BOX_SHADOW) {
                            if inner_vis.shadow_params.is_some() {
                                target_shadow_params = inner_vis.shadow_params;
                            }
                            if inner_vis.shadow_color.is_some() {
                                target_shadow_color = inner_vis.shadow_color;
                            }
                        }
                        if inner_mask.has(STYLE_TEXT_COLOR) {
                            target_text_color = inner_vis.text_color;
                        }
                        if inner_mask.has(STYLE_BORDER) {
                            if inner_vis.border_lengths.is_some() {
                                target_border_lengths = inner_vis.border_lengths;
                            }
                            if inner_vis.border_styles.is_some() {
                                target_border_styles = inner_vis.border_styles;
                            }
                            if inner_vis.border_alignments.is_some() {
                                target_border_alignments = inner_vis.border_alignments;
                            }
                        }
                    }
                }

                // All（いずれかのインタラクションがあればON）の解決
                if let Some(ref style) = interaction.any_within
                    && self.has_descendant_with_any_active_state(id)
                {
                    let inner_vis = &style.inner.visual_property;
                    let inner_mask = style.inner.mask;

                    if inner_mask.has(STYLE_BG_COLOR) {
                        target_bg = inner_vis.bg_color;
                    }
                    if inner_mask.has(STYLE_BORDER_COLOR) {
                        target_border = inner_vis.border_color;
                    }
                    if inner_mask.has(STYLE_OPACITY) {
                        target_opacity = inner_vis.opacity;
                    }
                    if inner_mask.has(STYLE_TRANSFORM) {
                        target_transform = inner_vis.transform;
                    }
                    if inner_mask.has(STYLE_CORNER_RADIUS) {
                        target_radius = inner_vis.corner_radius;
                    }
                    if inner_mask.has(STYLE_POINTER_EVENTS) {
                        target_pointer_events = inner_vis.pointer_events;
                    }
                    if inner_mask.has(STYLE_BOX_SHADOW) {
                        if inner_vis.shadow_params.is_some() {
                            target_shadow_params = inner_vis.shadow_params;
                        }
                        if inner_vis.shadow_color.is_some() {
                            target_shadow_color = inner_vis.shadow_color;
                        }
                    }
                    if inner_mask.has(STYLE_TEXT_COLOR) {
                        target_text_color = inner_vis.text_color;
                    }
                    if inner_mask.has(STYLE_BORDER) {
                        if inner_vis.border_lengths.is_some() {
                            target_border_lengths = inner_vis.border_lengths;
                        }
                        if inner_vis.border_styles.is_some() {
                            target_border_styles = inner_vis.border_styles;
                        }
                        if inner_vis.border_alignments.is_some() {
                            target_border_alignments = inner_vis.border_alignments;
                        }
                    }
                }
            }

            // プレースホルダー表示状態
            let mut is_placeholder_active = false;
            if let Some(contents) = self.input_contents.get(id) {
                // 文字列が空、かつ IME 変換中でない場合はプレースホルダーと判定
                let has_no_ime = contents
                    .ime_state
                    .as_ref()
                    .map(|s| s.composition_text.is_empty())
                    .unwrap_or(true);
                if contents.text.0.get().is_empty() && has_no_ime {
                    is_placeholder_active = true;
                }
            }

            // 各プロパティの即時適用の変更を評価
            let target_bg_val = target_bg.unwrap_or(Color::TRANSPARENT);
            let bg_changed = current_bg != target_bg_val;

            let target_border_val = target_border.unwrap_or(Color::TRANSPARENT);
            let border_changed = current_border != target_border_val;

            let target_opacity_val = target_opacity.unwrap_or(1.0);
            let opacity_changed = (current_opacity - target_opacity_val).abs() > 0.001;

            let target_transform_val = target_transform.unwrap_or(IDENTITY_MATRIX);
            let transform_changed = current_transform != target_transform_val;

            let target_radius_val = target_radius.unwrap_or(CornerRadius::ZERO);
            let radius_changed = current_radius != target_radius_val;

            let target_shadow_val = target_shadow_params.unwrap_or(BoxShadow::none());
            let shadow_changed = current_shadow != target_shadow_val;

            // トランジション判定 (変更がある場合のみトリガー)
            let mut bg_triggered = false;
            if allow_transition && bg_changed && has_active_visual {
                bg_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::BackgroundColor,
                    TransitionValue::Color(current_bg),
                    TransitionValue::Color(target_bg_val),
                );
            }

            let mut border_triggered = false;
            if border_changed && has_active_visual {
                border_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::BorderColor,
                    TransitionValue::Color(current_border),
                    TransitionValue::Color(target_border_val),
                );
            }

            let mut opacity_triggered = false;
            if opacity_changed && has_active_visual {
                opacity_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::Opacity,
                    TransitionValue::Opacity(current_opacity),
                    TransitionValue::Opacity(target_opacity_val),
                );
            }

            let mut transform_triggered = false;
            if transform_changed {
                transform_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::Transform,
                    TransitionValue::Transform(current_transform),
                    TransitionValue::Transform(target_transform_val),
                );
            }

            let mut radius_triggered = false;
            if radius_changed && has_active_visual {
                radius_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::CornerRadius,
                    TransitionValue::CornerRadius(current_radius),
                    TransitionValue::CornerRadius(target_radius_val),
                );
            }

            let mut shadow_triggered = false;
            if shadow_changed && has_active_visual {
                shadow_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::BoxShadow,
                    TransitionValue::BoxShadow(current_shadow),
                    TransitionValue::BoxShadow(target_shadow_val),
                );
            }

            // アニメーションが起動した、または明示的にベースの描画プロパティがある場合のみ
            // 遅延評価（Lazy）でマップを確保し、書き込みを行う
            if bg_triggered
                || border_triggered
                || opacity_triggered
                || transform_triggered
                || radius_triggered
                || shadow_triggered
                || bg_changed
                || border_changed
                || opacity_changed
                || transform_changed
                || radius_changed
                || shadow_changed
                || self.base_visual_properties.contains_key(id)
            {
                if !self.visual_properties.contains_key(id) {
                    self.visual_properties.insert(id, Default::default());
                }
                let active_vis = self.visual_properties.get_mut(id).unwrap();

                if !bg_triggered {
                    active_vis.bg_color = target_bg;
                }
                if !border_triggered {
                    active_vis.border_color = target_border;
                }
                if !opacity_triggered {
                    active_vis.opacity = target_opacity;
                }
                if !transform_triggered {
                    active_vis.transform = target_transform;
                }
                if !radius_triggered {
                    active_vis.corner_radius = target_radius;
                }
                if is_placeholder_active {
                    // プレースホルダー時はフォーカスに関わらず、強制的に半透明の薄いグレー
                    active_vis.text_color = Some(Color::rgb_f32(0.5, 0.5, 0.5));
                } else {
                    active_vis.text_color = target_text_color; // 通常時、または疑似状態（Hover等）のテキストカラー
                }

                // 解決した影（target_shadow）をアクティブプロパティに代入
                // アニメーション非起動時のみ行うように修正
                if !shadow_triggered {
                    active_vis.shadow_params = target_shadow_params;
                    active_vis.shadow_color = target_shadow_color;
                }

                active_vis.border_lengths = target_border_lengths;
                active_vis.border_styles = target_border_styles;
                active_vis.border_alignments = target_border_alignments;

                // 解決された選択色をアクティブビジュアルに代入
                active_vis.select_bg_color = target_select_bg;
                active_vis.select_text_color = target_select_text;
                // 常に即時解決する静的プロパティ群
                active_vis.user_select = self
                    .base_visual_properties
                    .get(id)
                    .and_then(|v| v.user_select);

                active_vis.cursor = target_cursor;
                active_vis.resizable_cursor = target_resizable_cursor;

                // コールドプロパティの即時代入
                if let Some(target_vis) = self.base_visual_properties.get(id) {
                    active_vis.z_index = target_vis.z_index;
                    active_vis.backdrop = target_vis.backdrop;
                    active_vis.font_size = target_vis.font_size;
                    active_vis.font_family = target_vis.font_family.clone();
                    active_vis.font_weight = target_vis.font_weight;
                    active_vis.font_style = target_vis.font_style;
                    active_vis.bg_gradient = target_vis.bg_gradient;
                    active_vis.pointer_events = target_vis.pointer_events;
                    active_vis.transitions = target_vis.transitions.clone();
                    active_vis.keyframe_animations = target_vis.keyframe_animations.clone();
                }

                // 即時変更があったため、レンダラーへの転送 Dirty をマーク
                self.mark_render_dirty(id);
            }
        }

        //  (Width, Height) の解決
        let has_base_layout = self.base_basic_layouts.contains_key(id);
        let has_active_layout = self.basic_layouts.contains_key(id);

        // レイアウト変更のない要素は完全にスキップ
        if has_base_layout || has_active_layout {
            let active_layout = self.basic_layouts.get(id).cloned().unwrap_or_default();

            // BasicLayout は heap allocation を持たないフラットな構造（Copy同等）なので
            // cloned() によるクローンは極めて低コスト（数ナノ秒）です。
            let base_layout = self.base_basic_layouts.get(id).cloned().unwrap_or_default();
            let mut target_layout = base_layout;

            if let Some(interaction) = self.interaction_properties.get(id) {
                let cascade = [
                    (STATE_FOCUSED, &interaction.focused),
                    (STATE_SELECTED, &interaction.selected),
                    (STATE_ACTIVED, &interaction.actived),
                    (STATE_HOVERED, &interaction.hovered),
                    (STATE_PRESSED, &interaction.pressed),
                    (STATE_DISABLED, &interaction.disabled),
                    (STATE_DRAGGED, &interaction.dragged),
                    (STATE_DRAGGING, &interaction.dragging),
                    (STATE_DRAG_IN, &interaction.drag_in),
                    (STATE_DRAG_OVER, &interaction.drag_over),
                ];

                for (state, style_opt) in cascade {
                    if active_mask.has(state)
                        && let Some(style) = style_opt
                    {
                        target_layout.override_with(&style.inner.basic_layout, style.inner.mask);
                    }
                }
            }

            // 単位を親/ウィンドウアラインメントを考慮した物理ピクセル(f32)へ解決
            let target_w_px = self.resolve_val_to_px(id, target_layout.size.width, true);
            let current_w_px = self.resolve_val_to_px(id, active_layout.size.width, true);
            let target_h_px = self.resolve_val_to_px(id, target_layout.size.height, false);
            let current_h_px = self.resolve_val_to_px(id, active_layout.size.height, false);

            let mut width_triggered = false;
            let mut height_triggered = false;

            // Width または 一括 Size トランジション設定が定義されているか検証
            let can_trigger_width = self
                .visual_properties
                .get(id)
                .map(|v| {
                    v.transitions.iter().any(|t| {
                        t.property_list == PropertyList::Width
                            || t.property_list == PropertyList::Size
                    })
                })
                .unwrap_or(false);

            if allow_transition
                && can_trigger_width
                && has_active_layout
                && let (Some(cw), Some(tw)) = (current_w_px, target_w_px)
                && (cw - tw).abs() > 0.01
            // 浮動小数点誤差を無視
            {
                width_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::Width,
                    TransitionValue::Width(cw),
                    TransitionValue::Width(tw),
                );
            }

            // Height または 一括 Size トランジション設定が定義されているか検証
            let can_trigger_height = self
                .visual_properties
                .get(id)
                .map(|v| {
                    v.transitions.iter().any(|t| {
                        t.property_list == PropertyList::Height
                            || t.property_list == PropertyList::Size
                    })
                })
                .unwrap_or(false);

            if allow_transition
                && can_trigger_height
                && has_active_layout
                && let (Some(ch), Some(th)) = (current_h_px, target_h_px)
                && (ch - th).abs() > 0.01
            {
                height_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::Height,
                    TransitionValue::Height(ch),
                    TransitionValue::Height(th),
                );
            }

            // 遅延マウント
            if !self.basic_layouts.contains_key(id) {
                self.basic_layouts.insert(id, Default::default());
            }
            let active_layout_mut = self.basic_layouts.get_mut(id).unwrap();
            *active_layout_mut = target_layout;

            if width_triggered {
                active_layout_mut.size.width = Val::Px(current_w_px.unwrap());
            }
            if height_triggered {
                active_layout_mut.size.height = Val::Px(current_h_px.unwrap());
            }

            // 最終的に解決されたレイアウトを Taffy ツリーに即時同期させるため、
            // スタイル解決の末尾でレイアウトの Dirty マークを叩きます
            self.mark_layout_dirty(id);
        }

        // スタイル解決が完了した結果、自身に新しくキーフレームアニメーション定義が
        // 読み込まれていれば、自動的にそのアニメーションの再生を開始する
        self.trigger_keyframe_animations_if_needed(id);

        if let Some(effects) = self.element_effects.get(id) {
            let text_effects: Vec<EffectId> = effects
                .iter()
                .filter(|(cat, _)| *cat == EffectCategory::Text)
                .map(|(_, eff_id)| *eff_id)
                .collect();
            for eff_id in text_effects {
                crate::execute_effect(eff_id);
            }
        }
    }

    /// 単位（Px, Percent, Auto）を親要素のサイズまたはウィンドウ基準をベースに f32 (物理ピクセル) へ解決します。
    pub(crate) fn resolve_val_to_px(&self, id: EntityId, val: Val, is_width: bool) -> Option<f32> {
        match val {
            Val::Px(v) => Some(v),
            Val::Percent(p) => {
                // 親要素の確定サイズを優先取得
                let parent_size = if let Some(Some(parent_id)) = self.parents.get(id) {
                    self.rects
                        .get(*parent_id)
                        .map(|r| LayoutSize::new(r.width, r.height))
                } else {
                    None
                };

                // 親要素が未確定または存在しない場合は、最終ウィンドウ寸法を基準にする
                let ref_size = parent_size.or(self.last_window_size)?;
                let ref_val = if is_width {
                    ref_size.width
                } else {
                    ref_size.height
                };

                Some(ref_val * (p / 100.0))
            }
            Val::Auto => {
                // Auto の場合は前フレームで確定している Taffy のレイアウト結果を実数値の基準値とする
                self.rects
                    .get(id)
                    .map(|r| if is_width { r.width } else { r.height })
            }
        }
    }

    /// 毎フレームの描画前に呼び出され、すべてのアクティブなキーフレームアニメーションを 1 Tick 進めます
    pub fn tick_animations(&mut self) {
        let now = Instant::now();

        // 借用チェッカーを回避するため、一時的にマップを take して更新
        let mut active_map = std::mem::take(&mut self.active_animations);
        let mut to_remove = Vec::new();

        for (id, animations) in active_map.iter_mut() {
            let mut i = 0;
            while i < animations.len() {
                let anim = &mut animations[i];
                let elapsed = now.duration_since(anim.start_time);
                let elapsed_secs = elapsed.as_secs_f32();
                let duration_secs = anim.duration.as_secs_f32();

                // 1. 現在の周回回数（ループインデックス）の算出
                let current_iteration = (elapsed_secs / duration_secs).floor() as u32;

                // ループ制限に達しているかチェック
                let is_finished = match anim.iteration_count {
                    PlaybackCount::Count(max_count) => current_iteration >= max_count,
                    PlaybackCount::Infinite => false,
                };

                if is_finished {
                    // ループ終了：目標の最終値（end_value）で固定してアニメーションを破棄
                    self.apply_animation_value(id, anim.property, &anim.end_value);
                    animations.remove(i);
                    continue;
                }

                // 2. 現在のループ内での正規化進行度 (0.0 ～ 1.0) の計算
                let local_time = elapsed_secs % duration_secs;
                let progress = if duration_secs > 0.0 {
                    (local_time / duration_secs).min(1.0)
                } else {
                    1.0
                };
                let eased_t = anim.curve.evaluate(progress);

                // 3. 値の補間
                let current_val = anim.start_value.lerp(&anim.end_value, eased_t);

                // 4. SoA へ補間された動的スタイル値を上書き書き戻し
                self.apply_animation_value(id, anim.property, &current_val);

                // レンダラーへ再描画要求（ファストパス）
                self.mark_render_dirty(id);

                i += 1;
            }

            if animations.is_empty() {
                to_remove.push(id);
            }
        }

        // 空になったエントリをクリーンアップ
        for id in to_remove {
            active_map.remove(id);
        }
        self.active_animations = active_map;
    }

    /// 補間されたアニメーション値を SoA のアクティブプロパティへ安全に上書きします
    fn apply_animation_value(
        &mut self,
        id: EntityId,
        property: PropertyList,
        value: &TransitionValue,
    ) {
        if !self.visual_properties.contains_key(id) {
            self.visual_properties.insert(id, Default::default());
        }
        let v = self.visual_properties.get_mut(id).unwrap();

        match *value {
            TransitionValue::Color(c) => {
                if property == PropertyList::BackgroundColor {
                    v.bg_color = Some(c);
                } else if property == PropertyList::BorderColor {
                    v.border_color = Some(c);
                }
            }
            TransitionValue::Opacity(o) => {
                v.opacity = Some(o);
            }
            TransitionValue::Transform(m) => {
                v.transform = Some(m);
            }
            TransitionValue::CornerRadius(cr) => {
                v.corner_radius = Some(cr);
            }
            TransitionValue::Width(w) => {
                if let Some(layout) = self.basic_layouts.get_mut(id) {
                    layout.size.width = Val::Px(w);
                }
                self.mark_layout_dirty(id); // レイアウト再計算を要求（スローパス）
            }
            TransitionValue::Height(h) => {
                if let Some(layout) = self.basic_layouts.get_mut(id) {
                    layout.size.height = Val::Px(h);
                }
                self.mark_layout_dirty(id);
            }
            TransitionValue::BoxShadow(shadow) => {
                v.shadow_params = Some(shadow);
                v.shadow_color = Some(shadow.color);
            }
        }
    }

    /// 要素が持つ静的な `KeyframeAnimation` 定義に基づいて、
    /// CPU 側のアクティブアニメーション再生テーブルを自動起動します。
    pub(crate) fn trigger_keyframe_animations_if_needed(&mut self, id: EntityId) {
        if let Some(visual) = self.visual_properties.get(id) {
            if visual.keyframe_animations.is_empty() {
                return;
            }

            let now = Instant::now();

            // 借用回避のため定義を一度クローン
            let anims = visual.keyframe_animations.clone();

            if !self.active_animations.contains_key(id) {
                self.active_animations.insert(id, Vec::new());
            }
            let active_list = self.active_animations.get_mut(id).unwrap();

            for anim in anims {
                // すでに同じプロパティのアニメーションが駆動中なら重複起動をスルー
                if active_list.iter().any(|a| a.property == anim.property) {
                    continue;
                }

                // 初期値（開始値）と目標値（100%キーフレームに相当する値）を設定
                // ※ ここでは例として「回転 (Transform)」の場合、0度から360度へ向かう値を算出します。
                let (start_val, end_val) = match anim.property {
                    PropertyList::Transform => {
                        let start = TransitionValue::Transform(IDENTITY_MATRIX);
                        // Z軸を1周（2PI）回転させる行列を終点にする
                        let mut end_transform =
                            crate::Transform::new().rotate(std::f32::consts::PI * 2.0);
                        let end = TransitionValue::Transform(end_transform.matrix);
                        (start, end)
                    }
                    PropertyList::Opacity => {
                        (TransitionValue::Opacity(1.0), TransitionValue::Opacity(0.0)) // フェードアウト等
                    }
                    _ => continue, // 必要に応じて他プロパティも定義
                };

                active_list.push(ActiveAnimation {
                    property: anim.property,
                    start_time: now,
                    duration: anim.duration,
                    iteration_count: anim.iteration_count,
                    curve: anim.curve,
                    start_value: start_val,
                    end_value: end_val,
                });
            }
        }
    }

    /// マウス座標などが、要素の描画領域かつ表示枠内に収まっているかを判定。
    /// 階層的な早期枝刈りヒットテスト
    pub fn hit_test(&self, point: LayoutPoint) -> Option<EntityId> {
        // 各要素の実効 z_index を、親から子へカスケードして算出
        let mut effective_z_indices = SecondaryMap::with_capacity(self.active_entities.len());
        for &id in &self.flat_dfs_sequence {
            let self_z = self.visual_properties.get(id).and_then(|v| v.z_index);

            let parent_z = self
                .parents
                .get(id)
                .copied()
                .flatten()
                .and_then(|pid| effective_z_indices.get(pid).copied());

            let eff_z = self_z.or(parent_z).unwrap_or(0);
            effective_z_indices.insert(id, eff_z);
        }

        // 実効 z_index に基づいて active_entities を安定ソート
        let mut sorted_entities = self.active_entities.clone();
        sorted_entities.sort_by_key(|&id| effective_z_indices.get(id).copied().unwrap_or(0));

        for &id in sorted_entities.iter().rev() {
            // ドラッグ中かつゴースト化した元の実体要素、およびプレースホルダー要素はヒットテストを強制スルーさせる
            if Some(id) == self.interaction_states.dragged
                || self.active_masks[id].has(STATE_DRAG_OVER)
            {
                continue;
            }

            // 親などの overflow 等でクリップされている表示範囲外ならスキップ
            if let Some(clip) = self.clip_rects.get(id)
                && !clip.contains(point)
            {
                continue;
            }

            // pointer-events 設定の解決
            let pointer_events = self
                .visual_properties
                .get(id)
                .and_then(|v| v.pointer_events)
                .or_else(|| {
                    self.base_visual_properties
                        .get(id)
                        .and_then(|v| v.pointer_events)
                })
                .unwrap_or(PointerEvents::Auto);

            if pointer_events == PointerEvents::None {
                continue; // 透過設定
            }

            // 物理範囲にヒットしたかを検証
            if let Some(rect) = self.rects.get(id)
                && rect.contains(point)
            {
                return Some(id);
            }
        }
        None
    }

    /// 階層的な境界判定ヘルパー（非対象のブランチをまるごとスキップ）
    fn hit_test_recursive(&self, id: EntityId, point: LayoutPoint) -> Option<EntityId> {
        // 1. 親などの overflow: hidden 等でクリップされている表示範囲をチェック
        // クリップ領域外であれば、この要素もそのすべての子孫要素も画面上に見えていないため、走査を即座にスキップ（枝刈り）
        if let Some(clip) = self.clip_rects.get(id)
            && !clip.contains(point)
        {
            return None;
        }

        // 2. 子要素を逆順（前面優先）で再帰降下
        if let Some(children) = self.children.get(id) {
            let child_len = children.len();
            for i in (0..child_len).rev() {
                let child_id = children[i];
                if let Some(hit) = self.hit_test_recursive(child_id, point) {
                    return Some(hit);
                }
            }
        }

        // pointer_events: none の場合は、自分自身の矩形判定のみをスルーする (子要素は上を辿れるため除外しない)
        // visual_properties (動的) に無ければ base_visual_properties (静的) を見に行く
        let pointer_events = self
            .visual_properties
            .get(id)
            .and_then(|v| v.pointer_events)
            .or_else(|| {
                self.base_visual_properties
                    .get(id)
                    .and_then(|v| v.pointer_events)
            })
            .unwrap_or(PointerEvents::Auto);

        if pointer_events != PointerEvents::None
            && let Some(rect) = self.rects.get(id)
            && rect.contains(point)
        {
            return Some(id);
        }

        None
    }

    /// 指定された要素（target）が、ある親要素（parent）自身、またはその子孫であるかを判定します。
    pub fn is_descendant_of(&self, target: EntityId, parent: EntityId) -> bool {
        if target == parent {
            return true;
        }
        let mut curr = target;
        while let Some(Some(p)) = self.parents.get(curr) {
            if *p == parent {
                return true;
            }
            curr = *p;
        }
        false
    }

    /// 指定された動的状態（例: STATE_HOVERED）に切り替わる際、
    /// その要素に割り当てられている状態スタイルがレイアウトの再計算を必要とするか判定します。
    pub(crate) fn does_state_require_layout(&self, id: EntityId, state_flag: u128) -> bool {
        if let Some(interaction) = self.interaction_properties.get(id) {
            // 対象となる状態スタイルを取得
            let target_style = match state_flag {
                STATE_HOVERED => &interaction.hovered,
                STATE_FOCUSED => &interaction.focused,
                STATE_PRESSED => &interaction.pressed,
                STATE_DISABLED => &interaction.disabled,
                STATE_ACTIVED => &interaction.actived,
                STATE_SELECTED => &interaction.selected,
                STATE_DRAGGED => &interaction.dragged,
                STATE_DRAGGING => &interaction.dragging,
                STATE_DRAG_IN => &interaction.drag_in,
                STATE_DRAG_OVER => &interaction.drag_over,
                _ => &None,
            };

            // 指定された状態スタイルが存在する場合のみ、内部マスクを検証
            if let Some(style) = target_style {
                let mask = style.inner.mask;
                // 基本レイアウト、Flexレイアウト、またはGridレイアウト変更が含まれていれば true
                return mask.has_basic_layout() || mask.has_flex_layout() || mask.has_grid_layout();
            }
        }
        false
    }

    /// 子孫要素のインタラクション状態（state_flag）を走査します
    pub(crate) fn has_descendant_with_state(&self, parent: EntityId, state_flag: u128) -> bool {
        // ヒープアロケーションを防ぐため、スタック領域に16要素まで確保可能な SmallVec を用意
        let mut stack = SmallVec::<[EntityId; 16]>::new();

        if let Some(children) = self.children.get(parent) {
            for &child_id in children {
                stack.push(child_id);
            }
        }

        while let Some(child_id) = stack.pop() {
            if self.entities.contains_key(child_id)
                && let Some(mask) = self.active_masks.get(child_id)
                && mask.has(state_flag)
            {
                return true; // 状態が見つかれば、関数呼び出しを重ねることなく即時早期リターン
            }

            // 子要素があれば、非再帰スタックにプッシュして探索を継続
            if let Some(children) = self.children.get(child_id) {
                for &next_child in children {
                    stack.push(next_child);
                }
            }
        }

        false
    }

    /// いずれか一つのアクティブなユーザーインタラクションが子孫要素でONになっているか非再帰で走査します
    pub(crate) fn has_descendant_with_any_active_state(&self, parent: EntityId) -> bool {
        let mut stack = SmallVec::<[EntityId; 16]>::new();

        if let Some(children) = self.children.get(parent) {
            for &child_id in children {
                stack.push(child_id);
            }
        }

        while let Some(child_id) = stack.pop() {
            if self.entities.contains_key(child_id)
                && let Some(mask) = self.active_masks.get(child_id)
                && mask.has_active_interaction_property()
            {
                return true;
            }

            if let Some(children) = self.children.get(child_id) {
                for &next_child in children {
                    stack.push(next_child);
                }
            }
        }

        false
    }

    /// 各インタラクション状態（ステート）を更新し、レイアウト変更を伴うか自動的に判別して Dirty フラグを制御する共通ヘルパー
    #[inline(always)]
    fn update_state(&mut self, id: EntityId, state_flag: u128, active: bool) {
        if let Some(mask) = self.active_masks.get_mut(id) {
            let was_active = mask.has(state_flag);
            if was_active != active {
                // 1. ビットマスクの更新
                if active {
                    mask.set(state_flag);
                } else {
                    mask.unset(state_flag);
                }

                // 状態変化の発生時に、即座に動的なスタイルを解決する
                self.resolve_element_style_state(id, true);

                // STYLE_INTERACTION_WITHIN マスク判定による親先祖の早期バイパス
                let mut curr = id;
                while let Some(Some(parent_id)) = self.parents.get(curr).copied() {
                    if self.entities.contains_key(parent_id) {
                        let parent_mask = self.active_masks[parent_id];

                        // 先祖要素が one of the within スタイルを1つでも持っている場合のみ深く入る
                        if parent_mask.has(STYLE_INTERACTION_WITHIN) {
                            self.resolve_element_style_state(parent_id, true);

                            if self.does_state_require_layout(parent_id, state_flag) {
                                self.mark_layout_dirty(parent_id);
                            } else {
                                self.mark_render_dirty(parent_id);
                            }
                        }
                    }
                    curr = parent_id;
                }

                // 2. 残りの状態遷移イベントの自動トリガー解決
                if active {
                    match state_flag {
                        // 無効化（Disabled）状態が有効になった瞬間
                        STATE_DISABLED => {
                            let mut on_dis = self
                                .event_listeners
                                .get_mut(id)
                                .and_then(|l| l.on_disable.take());
                            if let Some(mut handler) = on_dis {
                                let _guard = crate::ActiveElementGuard::new(id);
                                handler(self);
                                if let Some(l) = self.event_listeners.get_mut(id) {
                                    l.on_disable = Some(handler);
                                }
                            }
                        }
                        // アクティブ（Actived：STATE_ACTIVED）状態が有効になった瞬間
                        STATE_ACTIVED => {
                            let mut on_act = self
                                .event_listeners
                                .get_mut(id)
                                .and_then(|l| l.on_active.take());
                            if let Some(mut handler) = on_act {
                                let _guard = crate::ActiveElementGuard::new(id);
                                handler(self);
                                if let Some(l) = self.event_listeners.get_mut(id) {
                                    l.on_active = Some(handler);
                                }
                            }
                        }
                        // セレクト（Selected：STATE_SELECTED）状態が有効になった瞬間
                        STATE_SELECTED => {
                            let mut on_sel = self
                                .event_listeners
                                .get_mut(id)
                                .and_then(|l| l.on_select.take());
                            if let Some(mut handler) = on_sel {
                                let _guard = crate::ActiveElementGuard::new(id);
                                handler(self);
                                if let Some(l) = self.event_listeners.get_mut(id) {
                                    l.on_select = Some(handler);
                                }
                            }
                        }
                        _ => {}
                    }
                }

                // 3. レイアウト再計算（スローパス）か描画更新（ファストパス）かを自動判定
                if self.does_state_require_layout(id, state_flag) {
                    self.mark_layout_dirty(id);
                } else {
                    self.mark_render_dirty(id);
                }
            }
        }
    }

    /// ホバー（Hovered：マウスホバー）状態を更新します。
    ///
    /// ホバースタイル内にレイアウト変更プロパティ（幅やマージン等）が含まれていれば自動的にレイアウト再計算が要求され、
    /// 色や不透明度の変化だけであれば最速の描画更新（ファストパス）として処理されます。
    #[inline]
    pub fn set_hovered(&mut self, id: EntityId, hovered: bool) {
        self.update_state(id, STATE_HOVERED, hovered);
    }

    /// フォーカス（Focused：キーボードタブフォーカス等）状態を更新します。
    #[inline]
    pub fn set_focused(&mut self, id: EntityId, focused: bool) {
        self.update_state(id, STATE_FOCUSED, focused);
    }

    /// プレス（Pressed：クリック押し下げ、タップ中）状態を更新します。
    #[inline]
    pub fn set_pressed(&mut self, id: EntityId, pressed: bool) {
        self.update_state(id, STATE_PRESSED, pressed);
    }

    /// 無効化（Disabled：ボタンの操作不可など）状態を更新します。
    #[inline]
    pub fn set_disabled(&mut self, id: EntityId, disabled: bool) {
        self.update_state(id, STATE_DISABLED, disabled);
    }

    /// アクティブ（Actived：タブのトグル選択中など）状態を更新します。
    #[inline]
    pub fn set_actived(&mut self, id: EntityId, actived: bool) {
        self.update_state(id, STATE_ACTIVED, actived);
    }

    /// セレクト（Selected：チェックボックス、リストなどの選択）状態を更新します。
    #[inline]
    pub fn set_selected(&mut self, id: EntityId, selected: bool) {
        self.update_state(id, STATE_SELECTED, selected);
    }

    /// ドラッグ（Dragged：スライダーノブやスプリッターのドラッグ中）状態を更新します。
    #[inline]
    pub fn set_dragged(&mut self, id: EntityId, dragged: bool) {
        self.update_state(id, STATE_DRAGGED, dragged);
    }

    /// 要素のドラッグ・ドロップ擬似状態（STATE_DRAGGING, STATE_DRAG_IN, STATE_DRAG_OVER）を制御します。
    #[inline]
    pub(crate) fn set_drag_state(&mut self, id: EntityId, flag: u128, active: bool) {
        self.update_state(id, flag, active);
    }

    /// 各スタイルの解決を1回のルックアップと1回のカスケード解決ループに統合
    pub(crate) fn resolve_active_layouts(
        &self,
        id: EntityId,
    ) -> (BasicLayout, FlexLayout, Option<GridLayout>) {
        let mut basic = self.basic_layouts.get(id).copied().unwrap_or_default();
        let mut flex = self.flex_layouts.get(id).copied().unwrap_or_default();
        let mut grid = self.grid_layouts.get(id).cloned();

        let active_mask = self.active_masks[id];

        // 幅・高さ・一括サイズに対して、現在トランジションアニメーションが駆動中であるかを走査
        let is_width_transitioning = self
            .active_transitions
            .get(id)
            .map(|list| {
                list.iter().any(|t| {
                    t.property_list == PropertyList::Width || t.property_list == PropertyList::Size
                })
            })
            .unwrap_or(false);
        let is_height_transitioning = self
            .active_transitions
            .get(id)
            .map(|list| {
                list.iter().any(|t| {
                    t.property_list == PropertyList::Height || t.property_list == PropertyList::Size
                })
            })
            .unwrap_or(false);

        // 状態マッピング解決のルックアップとループを1回に集約
        if let Some(interaction) = self.interaction_properties.get(id) {
            let cascade = [
                (STATE_FOCUSED, &interaction.focused),
                (STATE_SELECTED, &interaction.selected),
                (STATE_ACTIVED, &interaction.actived),
                (STATE_HOVERED, &interaction.hovered),
                (STATE_PRESSED, &interaction.pressed),
                (STATE_DISABLED, &interaction.disabled),
                (STATE_DRAGGING, &interaction.dragging),
                (STATE_DRAG_IN, &interaction.drag_in),
                (STATE_DRAG_OVER, &interaction.drag_over),
            ];

            for (state, style_opt) in cascade {
                if active_mask.has(state)
                    && let Some(style) = style_opt
                {
                    let mut mask = style.inner.mask;
                    if is_width_transitioning || is_height_transitioning {
                        mask.unset(STYLE_SIZE);
                    }

                    basic.override_with(&style.inner.basic_layout, style.inner.mask);
                    flex.override_with(&style.inner.flex_layout, style.inner.mask);

                    if style.inner.mask.has_grid_layout()
                        && let Some(ref hover_grid) = style.inner.grid_layout
                    {
                        grid = Some(hover_grid.clone());
                    }
                }
            }
        }

        (basic, flex, grid)
    }

    pub fn inject_pointer_move(&mut self, logical_pos: LayoutPoint) {
        let _context_guard = bind_context(self);

        let prev_pos = self.current_pointer_position;
        self.current_pointer_position = Some(logical_pos);

        // リサイズ中のドラッグ同期処理
        if let Some(state) = self.resizing_state.clone() {
            let id = state.entity_id;
            let delta_x = logical_pos.x - state.start_mouse_pos.x;
            let delta_y = logical_pos.y - state.start_mouse_pos.y;

            let start_rect = state.start_rect;
            let position = self
                .basic_layouts
                .get(id)
                .map(|l| l.position)
                .unwrap_or(Position::Relative);

            // 1-1. 最小サイズ・最大クランプ値の解決
            let (min_w, max_w, min_h, max_h) = {
                let basic = self.basic_layouts.get(id).copied().unwrap_or_default();

                let ref_w = start_rect.width;
                let ref_h = start_rect.height;

                // ボーダー厚みの計算
                let b_l = match basic.border.left {
                    Length::Px(v) => v,
                    Length::Percent(p) => ref_w * (p / 100.0),
                };
                let b_r = match basic.border.right {
                    Length::Px(v) => v,
                    Length::Percent(p) => ref_w * (p / 100.0),
                };
                let b_t = match basic.border.top {
                    Length::Px(v) => v,
                    Length::Percent(p) => ref_h * (p / 100.0),
                };
                let b_b = match basic.border.bottom {
                    Length::Px(v) => v,
                    Length::Percent(p) => ref_h * (p / 100.0),
                };

                // パディング厚みの計算
                let p_l = match basic.padding.left {
                    Length::Px(v) => v,
                    Length::Percent(p) => ref_w * (p / 100.0),
                };
                let p_r = match basic.padding.right {
                    Length::Px(v) => v,
                    Length::Percent(p) => ref_w * (p / 100.0),
                };
                let p_t = match basic.padding.top {
                    Length::Px(v) => v,
                    Length::Percent(p) => ref_h * (p / 100.0),
                };
                let p_b = match basic.padding.bottom {
                    Length::Px(v) => v,
                    Length::Percent(p) => ref_h * (p / 100.0),
                };

                // 枠線と余白を足した、物理的にこれ以上小さくできない限界サイズ
                let abs_min_w = b_l + b_r + p_l + p_r;
                let abs_min_h = b_t + b_b + p_t + p_b;

                // ユーザー指定の min_size / max_size を物理ピクセルに解決
                let user_min_w = match basic.min_size.width {
                    Val::Px(v) => v,
                    Val::Percent(_) => self
                        .resolve_val_to_px(id, basic.min_size.width, true)
                        .unwrap_or(0.0),
                    Val::Auto => 0.0, // Autoのときは最小値制約なし
                };
                let user_min_h = match basic.min_size.height {
                    Val::Px(v) => v,
                    Val::Percent(_) => self
                        .resolve_val_to_px(id, basic.min_size.height, false)
                        .unwrap_or(0.0),
                    Val::Auto => 0.0,
                };
                let user_max_w = match basic.max_size.width {
                    Val::Px(v) => v,
                    Val::Percent(_) => self
                        .resolve_val_to_px(id, basic.max_size.width, true)
                        .unwrap_or(f32::MAX),
                    Val::Auto => f32::MAX, // Autoのときは最大値制限なし
                };
                let user_max_h = match basic.max_size.height {
                    Val::Px(v) => v,
                    Val::Percent(_) => self
                        .resolve_val_to_px(id, basic.max_size.height, false)
                        .unwrap_or(f32::MAX),
                    Val::Auto => f32::MAX,
                };

                (
                    abs_min_w.max(user_min_w).max(10.0), // 最低限 10px は維持
                    user_max_w,
                    abs_min_h.max(user_min_h).max(10.0),
                    user_max_h,
                )
            };

            let mut new_w = start_rect.width;
            let mut new_h = start_rect.height;

            let mut delta_inset_top = 0.0;
            let mut delta_inset_left = 0.0;

            // 配置モードによるサイズ変更と位置補正の切り分け
            if position == Position::Absolute {
                // 絶対配置（Absolute）物理サイズ変更と、Top / Left 引っ張り時の Inset 同期移動補正を行う
                match state.direction {
                    ResizeDirection::Right => {
                        new_w = (start_rect.width + delta_x).clamp(min_w, max_w);
                    }
                    ResizeDirection::Bottom => {
                        new_h = (start_rect.height + delta_y).clamp(min_h, max_h);
                    }
                    ResizeDirection::BottomRight => {
                        new_w = (start_rect.width + delta_x).clamp(min_w, max_w);
                        new_h = (start_rect.height + delta_y).clamp(min_h, max_h);
                    }
                    ResizeDirection::Left => {
                        let potential_w = start_rect.width - delta_x;
                        new_w = potential_w.clamp(min_w, max_w);
                        // サイズが大きくなった分（start - new_w）だけ、正確に左端（left）を左（マイナス）へ補正
                        delta_inset_left = start_rect.width - new_w;
                    }
                    ResizeDirection::Top => {
                        let potential_h = start_rect.height - delta_y;
                        new_h = potential_h.clamp(min_h, max_h);
                        // サイズが大きくなった分だけ、正確に上端（top）を上（マイナス）へ補正
                        delta_inset_top = start_rect.height - new_h;
                    }
                    ResizeDirection::TopLeft => {
                        let potential_w = start_rect.width - delta_x;
                        new_w = potential_w.clamp(min_w, max_w);
                        delta_inset_left = start_rect.width - new_w;

                        let potential_h = start_rect.height - delta_y;
                        new_h = potential_h.clamp(min_h, max_h);
                        delta_inset_top = start_rect.height - new_h;
                    }
                    ResizeDirection::TopRight => {
                        new_w = (start_rect.width + delta_x).clamp(min_w, max_w);

                        let potential_h = start_rect.height - delta_y;
                        new_h = potential_h.clamp(min_h, max_h);
                        delta_inset_top = start_rect.height - new_h;
                    }
                    ResizeDirection::BottomLeft => {
                        let potential_w = start_rect.width - delta_x;
                        new_w = potential_w.clamp(min_w, max_w);
                        delta_inset_left = start_rect.width - new_w;

                        new_h = (start_rect.height + delta_y).clamp(min_h, max_h);
                    }
                }
            } else {
                // 相対配置（Relative）フローを崩さないため inset は変更せず、
                // 引っ張る方向（Top/Left時はマイナス乗算）に合わせてサイズ（幅・高さ）のみを増減させる
                match state.direction {
                    ResizeDirection::Right | ResizeDirection::Left => {
                        let factor = if state.direction == ResizeDirection::Left {
                            -1.0
                        } else {
                            1.0
                        };
                        new_w = (start_rect.width + delta_x * factor).clamp(min_w, max_w);
                    }
                    ResizeDirection::Bottom | ResizeDirection::Top => {
                        let factor = if state.direction == ResizeDirection::Top {
                            -1.0
                        } else {
                            1.0
                        };
                        new_h = (start_rect.height + delta_y * factor).clamp(min_h, max_h);
                    }
                    ResizeDirection::TopLeft => {
                        new_w = (start_rect.width - delta_x).clamp(min_w, max_w);
                        new_h = (start_rect.height - delta_y).clamp(min_h, max_h);
                    }
                    ResizeDirection::TopRight => {
                        new_w = (start_rect.width + delta_x).clamp(min_w, max_w);
                        new_h = (start_rect.height - delta_y).clamp(min_h, max_h);
                    }
                    ResizeDirection::BottomLeft => {
                        new_w = (start_rect.width - delta_x).clamp(min_w, max_w);
                        new_h = (start_rect.height + delta_y).clamp(min_h, max_h);
                    }
                    ResizeDirection::BottomRight => {
                        new_w = (start_rect.width + delta_x).clamp(min_w, max_w);
                        new_h = (start_rect.height + delta_y).clamp(min_h, max_h);
                    }
                }
            }

            // 基本サイズ情報を SoA のアクティブレイアウトへ書き込み
            if let Some(layout) = self.basic_layouts.get_mut(id) {
                layout.size.width = Val::Px(new_w);
                layout.size.height = Val::Px(new_h);

                // 絶対配置時のみ位置を動的に補正し、制約衝突を回避するため right / bottom を Auto 化
                if position == Position::Absolute {
                    if let Val::Px(start_top) = state.start_inset.top {
                        layout.inset.top = Val::Px(start_top + delta_inset_top);
                    }
                    if let Val::Px(start_left) = state.start_inset.left {
                        layout.inset.left = Val::Px(start_left + delta_inset_left);
                    }
                    layout.inset.right = Val::Auto;
                    layout.inset.bottom = Val::Auto;
                }
            }

            // base_basic_layouts にも同時に書き込み、解決処理（resolve）によるリセットを完全に防ぐ
            if let Some(layout) = self.base_basic_layouts.get_mut(id) {
                layout.size.width = Val::Px(new_w);
                layout.size.height = Val::Px(new_h);

                if position == Position::Absolute {
                    if let Val::Px(start_top) = state.start_inset.top {
                        layout.inset.top = Val::Px(start_top + delta_inset_top);
                    }
                    if let Val::Px(start_left) = state.start_inset.left {
                        layout.inset.left = Val::Px(start_left + delta_inset_left);
                    }
                    layout.inset.right = Val::Auto;
                    layout.inset.bottom = Val::Auto;
                }
            }

            // Taffy 測定キャッシュをバイパスし再計算をマーク
            self.mark_layout_dirty(id);
            self.mark_render_dirty(id);
            return; // リサイズドラッグ中は、通常のホバーやドラッグ判定を完全にスキップして早期リターン
        }

        let mut scrollbar_dragged = false;
        let mut active_drag_target: Option<(EntityId, bool, bool)> = None;

        for (id, state) in self.scrollbar_styles.iter() {
            if state.v_thumb_dragged {
                active_drag_target = Some((id, true, false));
                break;
            } else if state.h_thumb_dragged {
                active_drag_target = Some((id, false, true));
                break;
            }
        }

        if let Some((c_id, is_v, is_h)) = active_drag_target {
            let (sb_state, container_rect, scroll_size) = {
                let sb_state = self.scrollbar_styles.get(c_id).cloned().unwrap();
                let container_rect = self.rects.get(c_id).copied().unwrap_or(LayoutRect::ZERO);
                let scroll_size = self.get_scroll_size(c_id);
                (sb_state, container_rect, scroll_size)
            };

            let window_size = self.last_window_size.unwrap_or(LayoutSize::ZERO);

            let visible_width = if window_size.width > 0.0 {
                let left = container_rect.x.max(0.0);
                let right = (container_rect.x + container_rect.width).min(window_size.width);
                (right - left).max(0.0)
            } else {
                container_rect.width
            };

            let visible_height = if window_size.height > 0.0 {
                let top = container_rect.y.max(0.0);
                let bottom = (container_rect.y + container_rect.height).min(window_size.height);
                (bottom - top).max(0.0)
            } else {
                container_rect.height
            };

            if is_v {
                let track_id = sb_state.v_track_id.unwrap();
                let thumb_id = sb_state.v_thumb_id.unwrap();
                let track_rect = self.rects[track_id];
                let thumb_rect = self.rects[thumb_id];

                // サムのマージンを差し引く
                let mut margin_top = 0.0;
                let mut margin_bottom = 0.0;
                if let Some(ref thumb_style) = sb_state.style.v_thumb {
                    if let Val::Px(val) = thumb_style.inner.basic_layout.margin.top {
                        margin_top = val;
                    }
                    if let Val::Px(val) = thumb_style.inner.basic_layout.margin.bottom {
                        margin_bottom = val;
                    }
                }

                // 同期処理と同じく、マージンを含めた実際の有効可動域を正確に計算
                let track_range =
                    track_rect.height - thumb_rect.height - margin_top - margin_bottom;
                if track_range > 0.0 {
                    let dy = logical_pos.y - sb_state.drag_start_mouse.y;
                    let max_scroll_y = scroll_size.height - visible_height;

                    if max_scroll_y > 0.0 {
                        let ratio = max_scroll_y / track_range;
                        let target_scroll_y = sb_state.drag_start_offset.y + dy * ratio;

                        let current_x = self.scroll_offsets.get(c_id).map(|o| o.x).unwrap_or(0.0);
                        self.scroll_to(c_id, current_x, target_scroll_y);
                    }
                }
            } else if is_h {
                let track_id = sb_state.h_track_id.unwrap();
                let thumb_id = sb_state.h_thumb_id.unwrap();
                let track_rect = self.rects[track_id];
                let thumb_rect = self.rects[thumb_id];

                let mut margin_left = 0.0;
                let mut margin_right = 0.0;
                if let Some(ref thumb_style) = sb_state.style.h_thumb {
                    if let Val::Px(val) = thumb_style.inner.basic_layout.margin.left {
                        margin_left = val;
                    }
                    if let Val::Px(val) = thumb_style.inner.basic_layout.margin.right {
                        margin_right = val;
                    }
                }

                let track_range = track_rect.width - thumb_rect.width - margin_left - margin_right;
                if track_range > 0.0 {
                    let dx = logical_pos.x - sb_state.drag_start_mouse.x;
                    let max_scroll_x = scroll_size.width - visible_width;

                    if max_scroll_x > 0.0 {
                        let ratio = max_scroll_x / track_range;
                        let target_scroll_x = sb_state.drag_start_offset.x + dx * ratio;

                        let current_y = self.scroll_offsets.get(c_id).map(|o| o.y).unwrap_or(0.0);
                        self.scroll_to(c_id, target_scroll_x, current_y);
                    }
                }
            }

            self.mark_render_dirty(c_id);
            scrollbar_dragged = true;
        }

        // マウスボタン押し下げ中は、他の要素へのインタラクション漏洩を防ぐためヒット先を押し下げ要素に強制ロック
        let target_id = if let Some(pressed_id) = self.interaction_states.pressed {
            Some(pressed_id)
        } else {
            self.hit_test(logical_pos)
        };

        // 直前のリサイズホバー対象を退避
        let prev_resize_hover = self.active_resize_hover;
        // リサイズホバー情報を一旦リセット
        self.active_resize_hover = None;

        // ヒットした要素、およびその親先祖に向かってツリーを遡上
        let mut current_id = target_id;
        let mut found_resize_hover = None;

        while let Some(id) = current_id {
            if self.active_masks[id].has(STYLE_RESIZABLE) {
                let rect = self.rects[id];
                let resizable_flags = self
                    .basic_layouts
                    .get(id)
                    .map(|l| l.resizable)
                    .unwrap_or([false; 4]);

                // 境界外周に 6.0px のあそびを持たせてヒット判定
                let detect_border = 6.0f32;
                if let Some(dir) =
                    detect_resize_direction(rect, resizable_flags, logical_pos, detect_border)
                {
                    found_resize_hover = Some((id, dir));
                    break; // 最も前面寄りのリサイズ親要素を優先採用
                }
            }
            current_id = self.parents.get(id).copied().flatten();
        }

        if let Some((id, dir)) = found_resize_hover {
            self.active_resize_hover = Some((id, dir));

            if let Some(vis) = self.visual_properties.get_mut(id) {
                // 要素に resizable_cursor の個別指定があれば、方向に応じて該当カーソルを抽出
                let custom_cursor = if let Some(arr) = vis.resizable_cursor {
                    let idx = match dir {
                        ResizeDirection::Top | ResizeDirection::Bottom => 0, // Ns
                        ResizeDirection::Left | ResizeDirection::Right => 1, // Ew
                        ResizeDirection::TopRight | ResizeDirection::BottomLeft => 2, // Nesw
                        ResizeDirection::TopLeft | ResizeDirection::BottomRight => 3, // Nwse
                    };
                    arr[idx]
                } else {
                    None
                };

                // 独自指定があればそれを使い、無ければライブラリの自動マッピングを使用
                vis.cursor = Some(custom_cursor.unwrap_or_else(|| resize_direction_to_cursor(dir)));
            }
            self.mark_render_dirty(id);
        }

        // 枠線から外れた、または異なる要素に変わった場合
        if let Some((prev_id, _)) = prev_resize_hover {
            let now_id = self.active_resize_hover.map(|(id, _)| id);

            // 異なるホバー状態になった場合、旧要素のカーソル上書きを破棄し本来のスタイルに即時強制リセット
            if Some(prev_id) != now_id {
                // スタイルの再解決を叩き、上書きされていた vis.cursor を本来のカーソル（通常ホバー/ベース等）へ復旧
                self.resolve_element_style_state(prev_id, false);
                self.mark_render_dirty(prev_id);
            }
        }

        if let Some(pressed_id) = self.interaction_states.pressed {
            let user_select = self
                .visual_properties
                .get(pressed_id)
                .and_then(|v| v.user_select)
                .unwrap_or(UserSelect::None);

            if user_select == UserSelect::Text
                && let Some(start_pos) = self.selection_start_index.get(pressed_id).copied()
            {
                // プレースホルダー選択のドラッグ遮断
                if let Some(contents) = self.input_contents.get(pressed_id) {
                    let text_val = contents.text.0.get();
                    let is_placeholder = text_val.is_empty()
                        && contents
                            .ime_state
                            .as_ref()
                            .map(|s| s.composition_text.is_empty())
                            .unwrap_or(true);

                    if is_placeholder && !contents.placeholder_select {
                        return;
                    }
                }

                let rect = self.rects[pressed_id];
                let (basic, _, _) = self.resolve_active_layouts(pressed_id);
                let border_left = match basic.border.left {
                    Length::Px(v) => v,
                    _ => 0.0,
                };
                let padding_left = match basic.padding.left {
                    Length::Px(v) => v,
                    _ => 0.0,
                };
                let border_top = match basic.border.top {
                    Length::Px(v) => v,
                    _ => 0.0,
                };
                let padding_top = match basic.padding.top {
                    Length::Px(v) => v,
                    _ => 0.0,
                };

                let local_x = logical_pos.x - (rect.x + border_left + padding_left);
                let local_y = logical_pos.y - (rect.y + border_top + padding_top);

                if let Some(layout) = self.get_or_create_layout(pressed_id) {
                    let (current_index, is_trailing) =
                        self.text_engine.hit_test_point(&layout, local_x, local_y);
                    let final_index = if is_trailing {
                        current_index + 1
                    } else {
                        current_index
                    };

                    let range = if start_pos <= final_index {
                        // 順選択（右方向ドラッグ）
                        if let Some(contents) = self.input_contents.get_mut(pressed_id) {
                            contents.selection_reversed = false;
                        }
                        start_pos..final_index
                    } else {
                        // 逆選択（左方向ドラッグ）
                        if let Some(contents) = self.input_contents.get_mut(pressed_id) {
                            contents.selection_reversed = true;
                        }
                        final_index..start_pos
                    };

                    self.text_selections.insert(pressed_id, range.clone());

                    self.update_selection_rects(pressed_id);

                    if let Some(contents) = self.input_contents.get_mut(pressed_id) {
                        contents.selected_range = range;
                        crate::update_input_caret_position(self, pressed_id);
                    }
                    self.mark_render_dirty(pressed_id);
                }
            }
        }

        // ヒットテスト
        let target_id = self.hit_test(logical_pos);

        // ホバー（Enter/Leave）状態の解決
        if target_id != self.interaction_states.hovered {
            // 旧ホバー要素からマウスが去った
            if let Some(old_id) = self.interaction_states.hovered {
                self.set_hovered(old_id, false);

                let mut on_leave = self
                    .event_listeners
                    .get_mut(old_id)
                    .and_then(|l| l.on_mouse_leave.take());
                if let Some(mut handler) = on_leave {
                    let _guard = crate::ActiveElementGuard::new(old_id);
                    handler(self);
                    if let Some(l) = self.event_listeners.get_mut(old_id) {
                        l.on_mouse_leave = Some(handler);
                    }
                }
            }

            // 新ホバー要素にマウスが入った
            if let Some(new_id) = target_id {
                self.set_hovered(new_id, true);

                // on_mouse_enter
                let mut on_enter = self
                    .event_listeners
                    .get_mut(new_id)
                    .and_then(|l| l.on_mouse_enter.take());
                if let Some(mut handler) = on_enter {
                    let _guard = crate::ActiveElementGuard::new(new_id);
                    handler(self);
                    if let Some(l) = self.event_listeners.get_mut(new_id) {
                        l.on_mouse_enter = Some(handler);
                    }
                }

                // on_hover
                let mut on_hover = self
                    .event_listeners
                    .get_mut(new_id)
                    .and_then(|l| l.on_hover.take());
                if let Some(mut handler) = on_hover {
                    let _guard = crate::ActiveElementGuard::new(new_id);
                    handler(self);
                    if let Some(l) = self.event_listeners.get_mut(new_id) {
                        l.on_hover = Some(handler);
                    }
                }
            }

            self.interaction_states.hovered = target_id;
        }

        // カーソル移動イベントの伝播
        if let Some(target_id) = target_id {
            // on_cursor_moved
            let mut on_move = self
                .event_listeners
                .get_mut(target_id)
                .and_then(|l| l.on_cursor_moved.take());
            if let Some(mut handler) = on_move {
                let rect = self.rects[target_id];
                let relative_pos = LayoutPoint::new(logical_pos.x - rect.x, logical_pos.y - rect.y);
                let _guard = crate::ActiveElementGuard::new(target_id);
                handler(self, relative_pos);
                if let Some(l) = self.event_listeners.get_mut(target_id) {
                    l.on_cursor_moved = Some(handler);
                }
            }
        }

        // ドラッグイベントの伝播
        if let Some(pressed_id) = self.interaction_states.pressed
            && let Some(prev) = prev_pos
        {
            let delta = LayoutPoint::new(logical_pos.x - prev.x, logical_pos.y - prev.y);
            if delta.x != 0.0 || delta.y != 0.0 {
                self.set_dragged(pressed_id, true);
                self.interaction_states.dragged = Some(pressed_id);

                // D&D 設定（STYLE_DRAGGABLE）を持っている場合のセッションのキック
                if self.active_masks[pressed_id].has(STYLE_DRAGGABLE)
                    && self.active_drag_state.is_none()
                {
                    let drag_prop = self.drag_properties.get(pressed_id).copied().unwrap();
                    let start_rect = self.rects[pressed_id];

                    // 開始時のクリック位置と要素左上の相対的なズレを計算
                    let click_offset = LayoutPoint::new(
                        logical_pos.x - start_rect.x,
                        logical_pos.y - start_rect.y,
                    );

                    // ウィンドウの真のルート要素をライブラリ側で自己解決
                    let root_entity = self
                        .find_root_entity()
                        .expect("Root EntityId not found in Context");

                    // プレースホルダーアタッチ先親要素の決定
                    let (parent_id_opt, parent_rect, parent_border_left, parent_border_top) =
                        match drag_prop.placeholder_parent {
                            DragPlaceholderParent::Root => (
                                Some(root_entity),
                                self.rects
                                    .get(root_entity)
                                    .copied()
                                    .unwrap_or(LayoutRect::ZERO),
                                0.0,
                                0.0,
                            ),
                            DragPlaceholderParent::Custom(p_id) => {
                                let p_rect =
                                    self.rects.get(p_id).copied().unwrap_or(LayoutRect::ZERO);
                                let b_l = if let Some(l) = self.basic_layouts.get(p_id) {
                                    match l.border.left {
                                        Length::Px(v) => v,
                                        _ => 0.0,
                                    }
                                } else {
                                    0.0
                                };
                                let b_t = if let Some(l) = self.basic_layouts.get(p_id) {
                                    match l.border.top {
                                        Length::Px(v) => v,
                                        _ => 0.0,
                                    }
                                } else {
                                    0.0
                                };
                                (Some(p_id), p_rect, b_l, b_t)
                            }
                        };

                    // プレースホルダー（クローン）をアタッチ先親の直下へ spawn して生成
                    let placeholder_id = self.spawn(parent_id_opt);
                    if let Some(p_id) = parent_id_opt {
                        self.add_child(p_id, placeholder_id);
                    }

                    // 元要素のレイアウトおよびビジュアル情報をコピーして初期マウント
                    if let Some(basic) = self.base_basic_layouts.get(pressed_id).copied() {
                        self.base_basic_layouts.insert(placeholder_id, basic);
                        self.basic_layouts.insert(placeholder_id, basic);
                    }
                    if let Some(visual) = self.base_visual_properties.get(pressed_id).cloned() {
                        self.base_visual_properties
                            .insert(placeholder_id, visual.clone());
                        self.visual_properties.insert(placeholder_id, visual);
                    }
                    if let Some(interaction) = self.interaction_properties.get(pressed_id).cloned()
                    {
                        self.interaction_properties
                            .insert(placeholder_id, interaction);
                    }

                    // ドラッグ元の元の要素は非可視（または半透明）にするため STATE_DRAGGING 状態をセット
                    self.set_drag_state(pressed_id, STATE_DRAGGING, true);

                    // プレースホルダー側は absolute 配置化し、STATE_DRAG_OVER 状態をセット
                    self.set_drag_state(placeholder_id, STATE_DRAG_OVER, true);
                    if let Some(layout) = self.basic_layouts.get_mut(placeholder_id) {
                        layout.position = Position::Absolute;
                        layout.size.width = Val::Px(start_rect.width);
                        layout.size.height = Val::Px(start_rect.height);
                    }
                    if let Some(layout) = self.base_basic_layouts.get_mut(placeholder_id) {
                        layout.position = Position::Absolute;
                        layout.size.width = Val::Px(start_rect.width);
                        layout.size.height = Val::Px(start_rect.height);
                    }

                    // 元の要素が持つ本物の子要素トポロジーを、一時的にプレースホルダー配下へ自動アタッチ
                    if let Some(src_children) = self.children.get(pressed_id).cloned() {
                        for child_id in src_children {
                            // 子要素の親ポインタをプレースホルダーに付け替え
                            self.parents.insert(child_id, Some(placeholder_id));

                            // プレースホルダー側の子要素リストへ追加
                            if let Some(ph_children) = self.children.get_mut(placeholder_id) {
                                ph_children.push(child_id);
                            }

                            // Taffy 側の親子構造も、一時的にプレースホルダーに繋ぎ替え
                            if let Some(&src_node) = self.taffy_nodes.get(pressed_id)
                                && let Some(&ph_node) = self.taffy_nodes.get(placeholder_id)
                                && let Some(&child_node) = self.taffy_nodes.get(child_id)
                            {
                                let _ = self.taffy.remove_child(src_node, child_node);
                                let _ = self.taffy.add_child(ph_node, child_node);
                            }
                        }

                        // 元の要素の子要素リストは一時的にクリア（プレースホルダーに避難しているため）
                        if let Some(src_children_mut) = self.children.get_mut(pressed_id) {
                            src_children_mut.clear();
                        }
                        self.mark_layout_dirty(pressed_id);
                        self.mark_layout_dirty(placeholder_id);
                    }

                    // プレースホルダー自体はヒットテストを完全に透過させる
                    if let Some(vis) = self.visual_properties.get_mut(placeholder_id) {
                        vis.pointer_events = Some(PointerEvents::None);
                    }
                    if let Some(vis) = self.base_visual_properties.get_mut(placeholder_id) {
                        vis.pointer_events = Some(PointerEvents::None);
                    }
                    if let Some(mask) = self.active_masks.get_mut(placeholder_id) {
                        mask.set(STYLE_POINTER_EVENTS);
                    }

                    // プレースホルダーアタッチ前の、本当の元の親要素のIDを安全に記録
                    let original_parent = self.parents.get(pressed_id).copied().flatten();

                    // セッション開始
                    self.active_drag_state = Some(ActiveDragState {
                        source_entity: pressed_id,
                        placeholder_entity: placeholder_id,
                        current_drop_target: None,
                        start_mouse_pos: logical_pos,
                        start_rect,
                        click_offset,
                        original_parent,
                    });

                    // ドラッグ開始コールバックに、Original(pressed_id) と Placeholder(placeholder_id) の両ハンドルを渡して実行
                    let mut start_listener_opt = self
                        .event_listeners
                        .get_mut(pressed_id)
                        .and_then(|l| l.on_drag_start.take());
                    if let Some(mut listener) = start_listener_opt {
                        {
                            let _guard = crate::ActiveElementGuard::new(pressed_id);
                            listener(
                                self,
                                Element::from(pressed_id),
                                Element::from(placeholder_id),
                            );
                        }
                        if let Some(l) = self.event_listeners.get_mut(pressed_id) {
                            l.on_drag_start = Some(listener);
                        }
                    }
                }

                // on_drag
                let mut on_drag = self
                    .event_listeners
                    .get_mut(pressed_id)
                    .and_then(|l| l.on_drag.take());
                if let Some(mut handler) = on_drag {
                    let _guard = crate::ActiveElementGuard::new(pressed_id);
                    handler(self, delta);
                    if let Some(l) = self.event_listeners.get_mut(pressed_id) {
                        l.on_drag = Some(handler);
                    }
                }
            }
        }

        // D&D プレースホルダーの移動とドロップ先ホバー検知
        if let Some(mut drag_state) = self.active_drag_state.clone() {
            let src_id = drag_state.source_entity;
            let placeholder_id = drag_state.placeholder_entity;
            let drag_prop = self.drag_properties.get(src_id).copied().unwrap();

            // ウィンドウの真のルート要素をライブラリ側で自己解決
            let root_entity = self
                .find_root_entity()
                .expect("Root EntityId not found in Context");

            // 5-1. アタッチ先親コンテナ基準での相対ローカル座標を逆算して追従（Inset更新）
            let (parent_rect, b_l, b_t) = match drag_prop.placeholder_parent {
                DragPlaceholderParent::Root => (
                    self.rects
                        .get(root_entity)
                        .copied()
                        .unwrap_or(LayoutRect::ZERO),
                    0.0,
                    0.0,
                ),
                DragPlaceholderParent::Custom(p_id) => {
                    let p_rect = self.rects.get(p_id).copied().unwrap_or(LayoutRect::ZERO);
                    let b_l = if let Some(l) = self.basic_layouts.get(p_id) {
                        match l.border.left {
                            Length::Px(v) => v,
                            _ => 0.0,
                        }
                    } else {
                        0.0
                    };
                    let b_t = if let Some(l) = self.basic_layouts.get(p_id) {
                        match l.border.top {
                            Length::Px(v) => v,
                            _ => 0.0,
                        }
                    } else {
                        0.0
                    };
                    (p_rect, b_l, b_t)
                }
            };

            // マウスのドラッグ開始時クリックオフセットを用いて、ローカル Top-Left 座標を算出
            let local_x = logical_pos.x - (parent_rect.x + b_l) - drag_state.click_offset.x;
            let local_y = logical_pos.y - (parent_rect.y + b_t) - drag_state.click_offset.y;

            if let Some(layout) = self.basic_layouts.get_mut(placeholder_id) {
                layout.inset.left = Val::Px(local_x);
                layout.inset.top = Val::Px(local_y);
                layout.inset.right = Val::Auto;
                layout.inset.bottom = Val::Auto;
            }
            if let Some(layout) = self.base_basic_layouts.get_mut(placeholder_id) {
                layout.inset.left = Val::Px(local_x);
                layout.inset.top = Val::Px(local_y);
                layout.inset.right = Val::Auto;
                layout.inset.bottom = Val::Auto;
            }

            self.mark_layout_dirty(placeholder_id);
            self.mark_render_dirty(placeholder_id);

            // 5-2. 現在ホバー侵入中のドロップターゲット要素を検知
            let hit_id_opt = self.hit_test(logical_pos);
            let mut found_drop_target = None;

            if let Some(hit_id) = hit_id_opt {
                let mut current_id = Some(hit_id);
                while let Some(id) = current_id {
                    // ヒットした要素がドラッグ元（src_id）自身、またはその子孫である場合は
                    // ドロップ先として誤認されるのを完全に防ぐため、スルーしてさらに上の親を辿る
                    if id == src_id || self.is_descendant_of(id, src_id) {
                        current_id = self.parents.get(id).copied().flatten();
                        continue;
                    }

                    if id != placeholder_id && self.active_masks[id].has(STYLE_DROPPABLE) {
                        found_drop_target = Some(id);
                        break;
                    }
                    current_id = self.parents.get(id).copied().flatten();
                }
            }

            // ドロップ先のホバー切り替えイベントを解決（STATE_DRAG_IN の同期）
            if found_drop_target != drag_state.current_drop_target {
                if let Some(old_target) = drag_state.current_drop_target {
                    self.set_drag_state(old_target, STATE_DRAG_IN, false);
                }
                if let Some(new_target) = found_drop_target {
                    self.set_drag_state(new_target, STATE_DRAG_IN, true);
                }
                drag_state.current_drop_target = found_drop_target;
                self.active_drag_state = Some(drag_state.clone());
            }

            // コールバックを一時的に take して借用を分離した後に実行
            match drag_prop.drag_mode {
                DragPayload::Element => {
                    let mut listener_opt = self
                        .event_listeners
                        .get_mut(src_id)
                        .and_then(|l| l.on_entity_drag.take());
                    if let Some(mut listener) = listener_opt {
                        {
                            let _guard = crate::ActiveElementGuard::new(src_id);
                            listener(
                                self,
                                Element::from(src_id),
                                found_drop_target.map(Element::from),
                            );
                        }
                        // 再度元の場所へ戻す
                        if let Some(l) = self.event_listeners.get_mut(src_id) {
                            l.on_entity_drag = Some(listener);
                        }
                    }
                }
                DragPayload::EntityId => {
                    let mut listener_opt = self
                        .event_listeners
                        .get_mut(src_id)
                        .and_then(|l| l.on_id_drag.take());
                    if let Some(mut listener) = listener_opt {
                        {
                            let _guard = crate::ActiveElementGuard::new(src_id);
                            listener(self, src_id, found_drop_target);
                        }
                        if let Some(l) = self.event_listeners.get_mut(src_id) {
                            l.on_id_drag = Some(listener);
                        }
                    }
                }
            }
        }
    }

    pub fn inject_pointer_button(
        &mut self,
        button: MouseButton,
        state: ElementState,
        modifiers: Modifiers,
    ) {
        let _context_guard = bind_context(self);
        let current_hovered = self.interaction_states.hovered;

        match state {
            ElementState::Pressed => {
                if button == MouseButton::Left {
                    // リサイズドラッグの開始判定
                    if let Some((id, dir)) = self.active_resize_hover {
                        let rect = self.rects[id];
                        let position = self
                            .basic_layouts
                            .get(id)
                            .map(|l| l.position)
                            .unwrap_or(Position::Relative);

                        // 親要素の矩形を取得
                        // 親要素の矩形と、その「左・上ボーダーの厚み」を正確に取得する
                        let (parent_rect, parent_border_left, parent_border_top) =
                            if let Some(Some(parent_id)) = self.parents.get(id) {
                                let p_rect = self
                                    .rects
                                    .get(*parent_id)
                                    .copied()
                                    .unwrap_or(LayoutRect::ZERO);

                                let border_l =
                                    if let Some(layout) = self.basic_layouts.get(*parent_id) {
                                        match layout.border.left {
                                            Length::Px(v) => v,
                                            Length::Percent(p) => p_rect.width * (p / 100.0),
                                        }
                                    } else {
                                        0.0
                                    };
                                let border_t =
                                    if let Some(layout) = self.basic_layouts.get(*parent_id) {
                                        match layout.border.top {
                                            Length::Px(v) => v,
                                            Length::Percent(p) => p_rect.height * (p / 100.0),
                                        }
                                    } else {
                                        0.0
                                    };

                                (p_rect, border_l, border_t)
                            } else {
                                (LayoutRect::ZERO, 0.0, 0.0)
                            };

                        // 親コンテナのボーダー内側を基準点として物理相対位置を逆算
                        let local_x = rect.x - (parent_rect.x + parent_border_left);
                        let local_y = rect.y - (parent_rect.y + parent_border_top);

                        // 【解決】絶対配置の場合、開始時に Top-Left 基準に完全に正規化（コンバート）する
                        // これにより、もともと right / bottom 基準で配置されていた要素であっても、
                        // ドラッグ開始の瞬間に左上へ吹っ飛ぶ現象を完全に阻止します。
                        let mut start_inset = Rect {
                            top: Val::Px(0.0),
                            right: Val::Px(0.0),
                            bottom: Val::Px(0.0),
                            left: Val::Px(0.0),
                        };
                        if position == Position::Absolute {
                            start_inset.top = Val::Px(local_y);
                            start_inset.left = Val::Px(local_x);
                            start_inset.right = Val::Auto;
                            start_inset.bottom = Val::Auto;

                            // SoA 側も、この Top-Left 座標で即時上書きアップデート
                            if let Some(layout) = self.basic_layouts.get_mut(id) {
                                layout.inset = start_inset;
                            }
                            if let Some(layout) = self.base_basic_layouts.get_mut(id) {
                                layout.inset = start_inset;
                            }
                        } else {
                            // 相対配置時は、通常通りそのままのインセットを使用
                            start_inset = self
                                .basic_layouts
                                .get(id)
                                .map(|l| l.inset)
                                .unwrap_or(BasicLayout::default().inset);
                        }

                        let start_pos = self.current_pointer_position.unwrap_or(LayoutPoint::ZERO);

                        self.resizing_state = Some(ResizingState {
                            entity_id: id,
                            direction: dir,
                            start_mouse_pos: start_pos,
                            start_rect: rect,
                            start_inset,
                        });

                        // リサイズ中の要素は pressed とマーク（多重干渉防止）
                        self.interaction_states.pressed = Some(id);
                        self.mark_render_dirty(id);
                        return; // リサイズドラッグが開始されたため、通常のクリック・フォーカス処理を完全にバイパス
                    }
                }

                let mut clicked_scrollbar = false;

                if let Some(pointer_pos) = self.current_pointer_position
                    && let Some(target_id) = current_hovered
                {
                    // 1. ヒットした要素がサム、またはトラックであるかを判定
                    let mut parent_container = None;
                    let mut is_v_thumb = false;
                    let mut is_h_thumb = false;
                    let mut is_v_track = false;
                    let mut is_h_track = false;

                    for (c_id, sb_state) in self.scrollbar_styles.iter() {
                        if sb_state.v_thumb_id == Some(target_id) {
                            parent_container = Some(c_id);
                            is_v_thumb = true;
                            break;
                        } else if sb_state.h_thumb_id == Some(target_id) {
                            parent_container = Some(c_id);
                            is_h_thumb = true;
                            break;
                        } else if sb_state.v_track_id == Some(target_id) {
                            parent_container = Some(c_id);
                            is_v_track = true;
                            break;
                        } else if sb_state.h_track_id == Some(target_id) {
                            parent_container = Some(c_id);
                            is_h_track = true;
                            break;
                        }
                    }

                    if let Some(c_id) = parent_container {
                        clicked_scrollbar = true;

                        let (sb_state, container_rect, scroll_size) = {
                            let sb_state = self.scrollbar_styles.get(c_id).cloned().unwrap();
                            let container_rect =
                                self.rects.get(c_id).copied().unwrap_or(LayoutRect::ZERO);
                            let scroll_size = self.get_scroll_size(c_id);
                            (sb_state, container_rect, scroll_size)
                        };

                        let offset = self
                            .scroll_offsets
                            .get(c_id)
                            .copied()
                            .unwrap_or(LayoutPoint::ZERO);

                        if is_v_thumb || is_h_thumb {
                            // A. サムをクリックした場合：ドラッグを開始
                            if let Some(st) = self.scrollbar_styles.get_mut(c_id) {
                                if is_v_thumb {
                                    st.v_thumb_dragged = true;
                                } else {
                                    st.h_thumb_dragged = true;
                                }
                                st.drag_start_mouse = pointer_pos;
                                st.drag_start_offset = offset;
                            }
                            self.interaction_states.pressed = Some(target_id); // サム要素自体を pressed に設定
                            self.mark_render_dirty(target_id);
                        } else if is_v_track || is_h_track {
                            let window_size = self.last_window_size.unwrap_or(LayoutSize::ZERO);

                            let visible_width = if window_size.width > 0.0 {
                                let left = container_rect.x.max(0.0);
                                let right = (container_rect.x + container_rect.width)
                                    .min(window_size.width);
                                (right - left).max(0.0)
                            } else {
                                container_rect.width
                            };

                            let visible_height = if window_size.height > 0.0 {
                                let top = container_rect.y.max(0.0);
                                let bottom = (container_rect.y + container_rect.height)
                                    .min(window_size.height);
                                (bottom - top).max(0.0)
                            } else {
                                container_rect.height
                            };

                            // B. レールをクリックした場合：ダイレクトジャンプスクロールを実行
                            if is_v_track {
                                let track_rect = self.rects[target_id];
                                let thumb_rect = self.rects[sb_state.v_thumb_id.unwrap()];
                                let relative_y = pointer_pos.y - track_rect.y;

                                let track_range = track_rect.height - thumb_rect.height;
                                let scroll_ratio = if track_range > 0.0 {
                                    ((relative_y - thumb_rect.height * 0.5) / track_range)
                                        .clamp(0.0, 1.0)
                                } else {
                                    0.0
                                };

                                let target_y = scroll_ratio * (scroll_size.height - visible_height);
                                self.scroll_to(c_id, offset.x, target_y);

                                let new_offset = self
                                    .scroll_offsets
                                    .get(c_id)
                                    .copied()
                                    .unwrap_or(LayoutPoint::ZERO);
                                if let Some(st) = self.scrollbar_styles.get_mut(c_id) {
                                    st.v_thumb_dragged = true;
                                    st.drag_start_mouse = pointer_pos;
                                    st.drag_start_offset = new_offset;
                                }
                                self.interaction_states.pressed =
                                    Some(sb_state.v_thumb_id.unwrap());
                                self.mark_render_dirty(sb_state.v_thumb_id.unwrap());
                            } else {
                                let track_rect = self.rects[target_id];
                                let thumb_rect = self.rects[sb_state.h_thumb_id.unwrap()];
                                let relative_x = pointer_pos.x - track_rect.x;

                                let track_range = track_rect.width - thumb_rect.width;
                                let scroll_ratio = if track_range > 0.0 {
                                    ((relative_x - thumb_rect.width * 0.5) / track_range)
                                        .clamp(0.0, 1.0)
                                } else {
                                    0.0
                                };

                                let target_x = scroll_ratio * (scroll_size.width - visible_width);
                                self.scroll_to(c_id, target_x, offset.y);

                                let new_offset = self
                                    .scroll_offsets
                                    .get(c_id)
                                    .copied()
                                    .unwrap_or(LayoutPoint::ZERO);
                                if let Some(st) = self.scrollbar_styles.get_mut(c_id) {
                                    st.h_thumb_dragged = true;
                                    st.drag_start_mouse = pointer_pos;
                                    st.drag_start_offset = new_offset;
                                }
                                self.interaction_states.pressed =
                                    Some(sb_state.h_thumb_id.unwrap());
                                self.mark_render_dirty(sb_state.h_thumb_id.unwrap());
                            }
                        }
                    }
                }

                if clicked_scrollbar {
                    return; // 背後の一般子要素へのイベント透過を防止
                }

                if let Some(target_id) = current_hovered {
                    self.interaction_states.pressed = Some(target_id);
                    self.set_pressed(target_id, true);

                    let user_select = self
                        .visual_properties
                        .get(target_id)
                        .and_then(|v| v.user_select)
                        .unwrap_or(UserSelect::None);

                    let is_input = self.active_masks[target_id].has(COMP_INPUT_CONTENT);

                    if user_select == UserSelect::Text
                        && !is_input
                        && let Some(pointer_pos) = self.current_pointer_position
                    {
                        let rect = self.rects[target_id];
                        let (basic, _, _) = self.resolve_active_layouts(target_id);
                        let border_left = match basic.border.left {
                            Length::Px(v) => v,
                            _ => 0.0,
                        };
                        let padding_left = match basic.padding.left {
                            Length::Px(v) => v,
                            _ => 0.0,
                        };
                        let border_top = match basic.border.top {
                            Length::Px(v) => v,
                            _ => 0.0,
                        };
                        let padding_top = match basic.padding.top {
                            Length::Px(v) => v,
                            _ => 0.0,
                        };

                        let local_x = pointer_pos.x - (rect.x + border_left + padding_left);
                        let local_y = pointer_pos.y - (rect.y + border_top + padding_top);

                        if let Some(layout) = self.get_or_create_layout(target_id) {
                            let (clicked_index, is_trailing) =
                                self.text_engine.hit_test_point(&layout, local_x, local_y);
                            let final_index = if is_trailing {
                                clicked_index + 1
                            } else {
                                clicked_index
                            };

                            if modifiers.shift {
                                // 共通の Shift選択拡張
                                let anchor = self
                                    .selection_start_index
                                    .get(target_id)
                                    .copied()
                                    .unwrap_or(final_index);
                                if !self.selection_start_index.contains_key(target_id) {
                                    self.selection_start_index.insert(target_id, final_index);
                                }
                                let range = if anchor <= final_index {
                                    anchor..final_index
                                } else {
                                    final_index..anchor
                                };
                                self.text_selections.insert(target_id, range);
                                self.update_selection_rects(target_id);
                            } else {
                                // 共通の通常クリックリセット
                                self.selection_start_index.insert(target_id, final_index);
                                self.text_selections
                                    .insert(target_id, final_index..final_index);
                                self.selected_rects.remove(target_id);
                            }

                            self.mark_render_dirty(target_id);
                        }
                    }

                    // フォーカス可能要素のみにフォーカスを制限
                    let is_focusable = self.active_masks[target_id].has(COMP_INPUT_CONTENT)
                        || self.active_masks[target_id].has(COMP_WEBVIEW_CONTENT);

                    if is_focusable {
                        // フォーカスの自動切り替え
                        if self.interaction_states.focused != Some(target_id) {
                            if let Some(old_focus_id) = self.interaction_states.focused {
                                self.set_focused(old_focus_id, false);

                                // on_blur
                                let mut on_blur = self
                                    .event_listeners
                                    .get_mut(old_focus_id)
                                    .and_then(|l| l.on_blur.take());
                                if let Some(mut handler) = on_blur {
                                    let _guard = crate::ActiveElementGuard::new(old_focus_id);
                                    handler(self);
                                    if let Some(l) = self.event_listeners.get_mut(old_focus_id) {
                                        l.on_blur = Some(handler);
                                    }
                                }
                            }

                            // 新しいフォーカス可能要素にフォーカスを設定
                            self.set_focused(target_id, true);

                            // on_focus
                            let mut on_focus = self
                                .event_listeners
                                .get_mut(target_id)
                                .and_then(|l| l.on_focus.take());
                            if let Some(mut handler) = on_focus {
                                let _guard = crate::ActiveElementGuard::new(target_id);
                                handler(self);
                                if let Some(l) = self.event_listeners.get_mut(target_id) {
                                    l.on_focus = Some(handler);
                                }
                            }
                            self.interaction_states.focused = Some(target_id);
                        }
                    } else {
                        // フォーカス不可能な要素をクリックした場合は、
                        // 現在フォーカスされているインプットからフォーカスを完全に外し状態をクリアする
                        if let Some(old_focus_id) = self.interaction_states.focused {
                            self.set_focused(old_focus_id, false);

                            let mut on_blur = self
                                .event_listeners
                                .get_mut(old_focus_id)
                                .and_then(|l| l.on_blur.take());
                            if let Some(mut handler) = on_blur {
                                let _guard = crate::ActiveElementGuard::new(old_focus_id);
                                handler(self);
                                if let Some(l) = self.event_listeners.get_mut(old_focus_id) {
                                    l.on_blur = Some(handler);
                                }
                            }
                            self.interaction_states.focused = None;
                        }
                    }

                    // on_mouse_input
                    let mut on_input = self
                        .event_listeners
                        .get_mut(target_id)
                        .and_then(|l| l.on_mouse_input.take());
                    if let Some(mut handler) = on_input {
                        let _guard = crate::ActiveElementGuard::new(target_id);
                        handler(self, button, modifiers, state);
                        if let Some(l) = self.event_listeners.get_mut(target_id) {
                            l.on_mouse_input = Some(handler);
                        }
                    }
                }
            }
            ElementState::Released => {
                // リサイズドラッグの終了処理
                if let Some(state) = self.resizing_state.take() {
                    let id = state.entity_id;
                    self.interaction_states.pressed = None;

                    // 元のリサイズホバーカーソル表示を維持するために再検出をマーク
                    // リサイズ状態が解除された「この瞬間」に現在の座標で move を再キックし、
                    // すり抜けていた通常のホバー・離脱判定（Leave）を正確に評価させる
                    if let Some(pos) = self.current_pointer_position {
                        self.inject_pointer_move(pos);
                    }
                    self.mark_render_dirty(id);
                    return;
                }

                // D&D ドラッグ終了・ドロップ確定処理
                if let Some(drag_state) = self.active_drag_state.take() {
                    let src_id = drag_state.source_entity;
                    let placeholder_id = drag_state.placeholder_entity;
                    let drag_prop = self.drag_properties.get(src_id).copied().unwrap();

                    // 疑似クラス（STATE_DRAGGING, STATE_DRAG_IN）を解除
                    self.set_drag_state(src_id, STATE_DRAGGING, false);
                    if let Some(target_id) = drag_state.current_drop_target {
                        self.set_drag_state(target_id, STATE_DRAG_IN, false);
                    }

                    // プレースホルダー要素を親および Taffy から安全にデスポーン
                    // このタイミングではまだ despawn_internal せず最後に移動させます。
                    self.interaction_states.pressed = None;
                    self.interaction_states.dragged = None;

                    let drop_success = drag_state.current_drop_target;

                    // A. 実体移動（DragMode::Entity）の場合のツリートポロジー書き換え
                    if let Some(target_id) = drop_success
                        && drag_prop.drag_mode == DragPayload::Element
                        && let Some(drop_prop) = self.drop_properties.get(target_id).copied()
                    {
                        // 1. まずドラッグ元要素を現在の親の children リストから安全に引き抜いて削除
                        if let Some(src_parent_id) = drag_state.original_parent {
                            if let Some(src_children) = self.children.get_mut(src_parent_id) {
                                src_children.retain(|x| *x != src_id);
                            }
                            // 旧親側の Taffy 順序も再同期
                            self.resync_taffy_children_order(src_parent_id);
                            self.mark_layout_dirty(src_parent_id);
                        }

                        // ドラッグ元要素の配置（Position）の取得
                        let position = self
                            .basic_layouts
                            .get(src_id)
                            .map(|l| l.position)
                            .unwrap_or(Position::Relative);

                        if position == Position::Absolute {
                            // 【絶対配置（Absolute）】: 位置移動（補正）を伴うアタッチ
                            if drag_prop.update_position {
                                // 1. プレースホルダーの最終的な絶対画面座標を取得
                                let ph_abs_rect = self
                                    .rects
                                    .get(placeholder_id)
                                    .copied()
                                    .unwrap_or(LayoutRect::ZERO);

                                // 2. 新しい親（target_id）の絶対画面座標とボーダー厚みを取得
                                let target_rect = self
                                    .rects
                                    .get(target_id)
                                    .copied()
                                    .unwrap_or(LayoutRect::ZERO);
                                let (border_l, border_t) =
                                    if let Some(layout) = self.basic_layouts.get(target_id) {
                                        let b_l = match layout.border.left {
                                            Length::Px(v) => v,
                                            _ => 0.0,
                                        };
                                        let b_t = match layout.border.top {
                                            Length::Px(v) => v,
                                            _ => 0.0,
                                        };
                                        (b_l, b_t)
                                    } else {
                                        (0.0, 0.0)
                                    };

                                // 3. 新しい親を基準にした新しいローカル相対位置を逆算して割り出す
                                let new_inset_left = ph_abs_rect.x - (target_rect.x + border_l);
                                let new_inset_top = ph_abs_rect.y - (target_rect.y + border_t);

                                let new_inset = Rect {
                                    top: Val::Px(new_inset_top),
                                    right: Val::Auto,
                                    bottom: Val::Auto,
                                    left: Val::Px(new_inset_left),
                                };

                                if let Some(layout) = self.basic_layouts.get_mut(src_id) {
                                    layout.inset = new_inset;
                                }
                                if let Some(layout) = self.base_basic_layouts.get_mut(src_id) {
                                    layout.inset = new_inset;
                                }
                            }

                            // ドロップ先コンテナ（target_id）の末尾の子要素としてマウント
                            self.add_child(target_id, src_id);
                        } else {
                            // 【相対配置（Relative）】: マウス座標に基づいた子要素の動的並び替えアタッチ
                            if drag_prop.update_position {
                                let mouse_pos =
                                    self.current_pointer_position.unwrap_or(LayoutPoint::ZERO);
                                let insert_idx = calculate_insert_index(self, target_id, mouse_pos);

                                if let Some(parent_children) = self.children.get_mut(target_id) {
                                    // 算出されたインデックス位置へ挿入
                                    parent_children.insert(insert_idx, src_id);
                                }
                                self.parents.insert(src_id, Some(target_id));

                                // Taffy 側のノード順序を物理並び替え結果に沿って一括して再同期
                                self.resync_taffy_children_order(target_id);
                            } else {
                                // 自動更新オフの場合は末尾に通常アタッチ
                                self.add_child(target_id, src_id);
                            }
                            self.mark_layout_dirty(target_id);
                        }

                        self.is_structure_dirty = true;
                    }

                    // 避難していた本物の子要素トポロジーを、元の要素（src_id）の配下へ自動復元
                    if let Some(ph_children) = self.children.get(placeholder_id).cloned() {
                        for child_id in ph_children {
                            // 子要素の親ポインタを元の要素に書き戻し
                            self.parents.insert(child_id, Some(src_id));

                            // 元の要素の子要素リストへ復旧
                            if let Some(src_children) = self.children.get_mut(src_id) {
                                src_children.push(child_id);
                            }

                            // Taffy 側の親子構造も、元の要素に繋ぎ戻し
                            if let Some(&src_node) = self.taffy_nodes.get(src_id)
                                && let Some(&ph_node) = self.taffy_nodes.get(placeholder_id)
                                && let Some(&child_node) = self.taffy_nodes.get(child_id)
                            {
                                let _ = self.taffy.remove_child(ph_node, child_node);
                                let _ = self.taffy.add_child(src_node, child_node);
                            }
                        }

                        // プレースホルダー側は空にして破棄に備える
                        if let Some(ph_children_mut) = self.children.get_mut(placeholder_id) {
                            ph_children_mut.clear();
                        }
                        self.mark_layout_dirty(src_id);
                        self.mark_layout_dirty(placeholder_id);
                    }

                    // コールバックを一時的に take して借用を完全に切り離して実行する
                    match drag_prop.drag_mode {
                        DragPayload::Element => {
                            let mut listener_opt = self
                                .event_listeners
                                .get_mut(src_id)
                                .and_then(|l| l.on_entity_drop.take());
                            if let Some(mut listener) = listener_opt {
                                {
                                    let _guard = crate::ActiveElementGuard::new(src_id);
                                    listener(
                                        self,
                                        Element::from(src_id),
                                        drop_success.map(Element::from),
                                    );
                                }
                                if let Some(l) = self.event_listeners.get_mut(src_id) {
                                    l.on_entity_drop = Some(listener);
                                }
                            }
                        }
                        DragPayload::EntityId => {
                            let mut listener_opt = self
                                .event_listeners
                                .get_mut(src_id)
                                .and_then(|l| l.on_id_drop.take());
                            if let Some(mut listener) = listener_opt {
                                {
                                    let _guard = crate::ActiveElementGuard::new(src_id);
                                    listener(self, src_id, drop_success);
                                }
                                if let Some(l) = self.event_listeners.get_mut(src_id) {
                                    l.on_id_drop = Some(listener);
                                }
                            }
                        }
                    }

                    // 位置情報の安全な回収がすべて完了した、この最末尾で初めてプレースホルダーを破棄
                    self.despawn_internal(placeholder_id);

                    // 離脱直後に位置を再移動評価して、通常のホバーを正しく復元
                    if let Some(pos) = self.current_pointer_position {
                        self.inject_pointer_move(pos);
                    }

                    self.mark_render_dirty(src_id);
                    return; // 早期リターン
                }

                let mut dirty_ids = smallvec::SmallVec::<[EntityId; 4]>::new();
                for (id, state) in self.scrollbar_styles.iter_mut() {
                    if state.v_thumb_dragged || state.h_thumb_dragged {
                        state.v_thumb_dragged = false;
                        state.h_thumb_dragged = false;
                        dirty_ids.push(id);
                    }
                }

                for id in dirty_ids {
                    self.mark_render_dirty(id);
                }

                if let Some(pressed_id) = self.interaction_states.pressed {
                    self.set_pressed(pressed_id, false);
                    self.set_dragged(pressed_id, false);
                    self.interaction_states.dragged = None;

                    if let Some(contents) = self.input_contents.get_mut(pressed_id) {
                        contents.is_selecting = false;
                    }

                    // 1. on_mouse_input の発火（ボタンの種類を問わず常に呼ぶ）
                    let mut on_input = self
                        .event_listeners
                        .get_mut(pressed_id)
                        .and_then(|l| l.on_mouse_input.take());
                    if let Some(mut handler) = on_input {
                        let _guard = crate::ActiveElementGuard::new(pressed_id);
                        handler(self, button, modifiers, state);
                        if let Some(l) = self.event_listeners.get_mut(pressed_id) {
                            l.on_mouse_input = Some(handler);
                        }
                    }

                    // 2. 同一要素上で離された場合の各種クリック解決
                    if self.interaction_states.hovered == Some(pressed_id) {
                        match button {
                            // 左クリックの解決
                            MouseButton::Left => {
                                let mut on_click = self
                                    .event_listeners
                                    .get_mut(pressed_id)
                                    .and_then(|l| l.on_click.take());
                                if let Some(mut handler) = on_click {
                                    let _guard = crate::ActiveElementGuard::new(pressed_id);
                                    handler(self);
                                    if let Some(l) = self.event_listeners.get_mut(pressed_id) {
                                        l.on_click = Some(handler);
                                    }
                                }
                            }
                            // 右クリックの解決（追加）
                            MouseButton::Right => {
                                let mut on_right = self
                                    .event_listeners
                                    .get_mut(pressed_id)
                                    .and_then(|l| l.on_right_click.take());
                                if let Some(mut handler) = on_right {
                                    let _guard = crate::ActiveElementGuard::new(pressed_id);
                                    handler(self);
                                    if let Some(l) = self.event_listeners.get_mut(pressed_id) {
                                        l.on_right_click = Some(handler);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    self.interaction_states.pressed = None;
                }
            }
        }
    }

    /// ウィンドウ内の最上位ルート要素の EntityId を自律解決して返します。
    pub(crate) fn find_root_entity(&self) -> Option<EntityId> {
        // すでにフラットシーケンスが構築されていればその先頭、
        // 無ければ parents マップをスキャンして親が None の生存要素をフォールバック解決します
        self.flat_dfs_sequence.first().copied().or_else(|| {
            self.parents
                .iter()
                .find(|&(id, &parent_id_opt)| {
                    // 親が None かつ、要素 id 自体が slotmap (entities) に生存しているか
                    parent_id_opt.is_none() && self.entities.contains_key(id)
                })
                .map(|(id, _)| id)
        })
    }

    // ダブルクリック
    pub fn inject_pointer_double_click(&mut self, modifiers: Modifiers) {
        let _context_guard = bind_context(self);
        let current_hovered = self.interaction_states.hovered;

        if let Some(target_id) = current_hovered {
            let user_select = self
                .visual_properties
                .get(target_id)
                .and_then(|v| v.user_select)
                .unwrap_or(UserSelect::None);

            if user_select == UserSelect::Text
                && let Some(pointer_pos) = self.current_pointer_position
            {
                if let Some(contents) = self.input_contents.get(target_id) {
                    let text_val = contents.text.0.get();
                    let is_placeholder = text_val.is_empty()
                        && contents
                            .ime_state
                            .as_ref()
                            .map(|s| s.composition_text.is_empty())
                            .unwrap_or(true);

                    if is_placeholder && !contents.placeholder_select {
                        return;
                    }
                }

                let rect = self.rects[target_id];
                let (basic, _, _) = self.resolve_active_layouts(target_id);
                let border_left = match basic.border.left {
                    Length::Px(v) => v,
                    _ => 0.0,
                };
                let padding_left = match basic.padding.left {
                    Length::Px(v) => v,
                    _ => 0.0,
                };
                let border_top = match basic.border.top {
                    Length::Px(v) => v,
                    _ => 0.0,
                };
                let padding_top = match basic.padding.top {
                    Length::Px(v) => v,
                    _ => 0.0,
                };

                let local_x = pointer_pos.x - (rect.x + border_left + padding_left);
                let local_y = pointer_pos.y - (rect.y + border_top + padding_top);

                if let Some(layout) = self.get_or_create_layout(target_id) {
                    let (clicked_index, is_trailing) =
                        self.text_engine.hit_test_point(&layout, local_x, local_y);
                    let final_index = if is_trailing {
                        clicked_index + 1
                    } else {
                        clicked_index
                    };

                    if let Some(text) = self.text_contents.get(target_id) {
                        let text_u16: Vec<u16> = text.encode_utf16().collect();

                        // 高精度な文節境界を抽出
                        let range = crate::find_word_boundaries(&text_u16, final_index);

                        self.text_selections.insert(target_id, range.clone());
                        // アンカー開始を文節左端にセット
                        self.selection_start_index.insert(target_id, range.start);
                        self.update_selection_rects(target_id); // 選択矩形を更新

                        if let Some(contents) = self.input_contents.get_mut(target_id) {
                            contents.selected_range = range;
                            contents.selection_reversed = false; // キャレットは右端に配置
                            crate::update_input_caret_position(self, target_id);
                        }

                        self.mark_render_dirty(target_id);
                    }
                }
            }
        }
    }

    /// 外部で計算された論理ピクセルスクロール移動量 (scroll_x, scroll_y) を注入し、
    /// バブリングによる自動スクロール処理、またはユーザーイベントハンドラへの配送を行います。
    pub fn inject_mouse_wheel(&mut self, scroll_x: f32, scroll_y: f32) {
        let _context_guard = bind_context(self);

        let mut curr = self.interaction_states.hovered;
        let mut handled = false;

        // イベントバブリング: ホバー要素から親へ辿る
        while let Some(curr_id) = curr {
            // 個別に定義された `on_mouse_wheel` ハンドラがあれば最優先実行
            let mut on_wheel = self
                .event_listeners
                .get_mut(curr_id)
                .and_then(|l| l.on_mouse_wheel.take());

            if let Some(mut handler) = on_wheel {
                let _guard = crate::ActiveElementGuard::new(curr_id);
                handler(self, scroll_x, scroll_y);
                if let Some(l) = self.event_listeners.get_mut(curr_id) {
                    l.on_mouse_wheel = Some(handler);
                }
                handled = true; // イベントが消費されたため、これ以降のコンテナスクロールは行わない
                break;
            }

            // ユーザーハンドラがない場合、要素がスクロールコンテナであるか判定
            let mask = self.active_masks[curr_id];
            if mask.has(STYLE_OVERFLOW) {
                let (basic, _, _) = self.resolve_active_layouts(curr_id);

                let mut scrolled = false;

                // 縦方向スクロール
                if scroll_y != 0.0
                    && (basic.overflow.y == Overflow::Scroll
                        || basic.overflow.y == Overflow::Hidden)
                    && self.scroll_by(curr_id, 0.0, scroll_y)
                {
                    scrolled = true;
                }

                // 横方向スクロール
                if scroll_x != 0.0
                    && (basic.overflow.x == Overflow::Scroll
                        || basic.overflow.x == Overflow::Hidden)
                    && self.scroll_by(curr_id, scroll_x, 0.0)
                {
                    scrolled = true;
                }

                if scrolled {
                    handled = true;
                    break; // スクロールを実行したためバブリングを終了
                }
            }

            // 先祖へ伝播
            curr = self.parents.get(curr_id).copied().flatten();
        }
    }

    pub fn inject_keyboard_key(
        &mut self,
        key: VirtualKey,
        state: ElementState,
        modifiers: Modifiers,
    ) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.interaction_states.focused {
            // 内部で完結する全選択（Ctrl+A）のみを自動処理
            if state == ElementState::Pressed && modifiers.ctrl {
                let user_select = self
                    .visual_properties
                    .get(focused_id)
                    .and_then(|v| v.user_select)
                    .unwrap_or(UserSelect::None);

                if key == VirtualKey::A && user_select == UserSelect::Text {
                    if let Some(layout) = self.get_or_create_layout(focused_id)
                        && let Some(text) = self.text_contents.get(focused_id)
                    {
                        let u16_len = text.encode_utf16().count();
                        let full_range = 0..u16_len;

                        self.text_selections.insert(focused_id, full_range.clone());

                        self.update_selection_rects(focused_id);

                        if let Some(contents) = self.input_contents.get_mut(focused_id) {
                            contents.selected_range = full_range;
                            contents.selection_reversed = false;
                            crate::update_input_caret_position(self, focused_id);
                        }
                        self.mark_render_dirty(focused_id);
                    }
                    return;
                }
            }

            let mut on_key = self
                .event_listeners
                .get_mut(focused_id)
                .and_then(|l| l.on_keyboard_input.take());
            if let Some(mut handler) = on_key {
                let _guard = crate::ActiveElementGuard::new(focused_id);
                handler(self, key, modifiers, state);
                if let Some(l) = self.event_listeners.get_mut(focused_id) {
                    l.on_keyboard_input = Some(handler);
                }
            }
        }
    }

    pub fn inject_character(&mut self, c: char) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.interaction_states.focused {
            let mut on_char = self
                .event_listeners
                .get_mut(focused_id)
                .and_then(|l| l.on_char_input.take());
            if let Some(mut handler) = on_char {
                let _guard = crate::ActiveElementGuard::new(focused_id);
                handler(self, c);
                if let Some(l) = self.event_listeners.get_mut(focused_id) {
                    l.on_char_input = Some(handler);
                }
            }
        }
    }

    pub fn inject_ime(&mut self, ime_state: ImeState) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.interaction_states.focused {
            let mut on_ime = self
                .event_listeners
                .get_mut(focused_id)
                .and_then(|l| l.on_ime.take());
            if let Some(mut handler) = on_ime {
                let _guard = crate::ActiveElementGuard::new(focused_id);
                handler(self, ime_state);
                if let Some(l) = self.event_listeners.get_mut(focused_id) {
                    l.on_ime = Some(handler);
                }
            }
        }
    }

    pub fn inject_file_dropped(&mut self, paths: Vec<PathBuf>) {
        let _context_guard = bind_context(self);
        if let Some(target_id) = self.interaction_states.hovered {
            let mut on_drop = self
                .event_listeners
                .get_mut(target_id)
                .and_then(|l| l.on_file_dropped.take());
            if let Some(mut handler) = on_drop {
                let _guard = crate::ActiveElementGuard::new(target_id);
                handler(self, paths);
                if let Some(l) = self.event_listeners.get_mut(target_id) {
                    l.on_file_dropped = Some(handler);
                }
            }
        }
    }

    /// 現在の選択範囲（text_selections）に基づき、
    /// 描画用の物理選択矩形（selected_rects）を自動再計算して SoA キャッシュを更新します。
    pub(crate) fn update_selection_rects(&mut self, id: EntityId) {
        if let Some(range) = self.text_selections.get(id).cloned()
            && range.start < range.end
            && let Some(layout) = self.get_or_create_layout(id)
        {
            let mut hit_test_metrics = vec![DWRITE_HIT_TEST_METRICS::default(); 16];
            let mut actual_count: u32 = 0;
            let res = unsafe {
                layout.HitTestTextRange(
                    range.start as u32,
                    (range.end - range.start) as u32,
                    0.0,
                    0.0,
                    Some(&mut hit_test_metrics),
                    &mut actual_count,
                )
            };

            if res.is_ok() && actual_count as usize > hit_test_metrics.len() {
                hit_test_metrics.resize(actual_count as usize, DWRITE_HIT_TEST_METRICS::default());
                let _ = unsafe {
                    layout.HitTestTextRange(
                        range.start as u32,
                        (range.end - range.start) as u32,
                        0.0,
                        0.0,
                        Some(&mut hit_test_metrics),
                        &mut actual_count,
                    )
                };
            }

            let mut rects = Vec::with_capacity(actual_count as usize);
            (0..actual_count as usize).for_each(|m_idx| {
                let metric = &hit_test_metrics[m_idx];
                rects.push(LayoutRect::new(
                    metric.left,
                    metric.top,
                    metric.width,
                    metric.height,
                ));
            });
            self.selected_rects.insert(id, rects);
            return;
        }
        // 範囲が 0、または選択なしの時は自動クリーンアップ
        self.selected_rects.remove(id);
    }

    /// キャッシュされたレイアウトがあればそれを返し、無ければ安全に生成して保持します。
    pub(crate) fn get_or_create_layout(&self, id: EntityId) -> Option<IDWriteTextLayout> {
        if let Some(layout) = self.dwrite_layouts.borrow().get(id) {
            return Some(layout.clone());
        }

        let text = self.text_contents.get(id)?;
        let default_visual = VisualProperty::default();
        let visual = self.visual_properties.get(id).unwrap_or(&default_visual);
        let font_size = visual.font_size.unwrap_or(16.0);
        let font_family = visual.font_family.as_deref();
        let font_weight = visual.font_weight;
        let font_style = visual.font_style;

        let spans = self.text_spans.get(id).map(|s| s.as_slice()).unwrap_or(&[]);

        let layout = self.text_engine.create_layout(
            text,
            font_size,
            font_family,
            font_weight,
            font_style,
            None,
            spans,
        );

        self.dwrite_layouts.borrow_mut().insert(id, layout.clone());
        Some(layout)
    }

    /// テキスト変更やスタイル更新時にキャッシュを安全に破棄します。
    pub(crate) fn clear_layout_cache(&mut self, id: EntityId) {
        self.dwrite_layouts.borrow_mut().remove(id);
    }

    /// 現在フォーカスされている要素で範囲選択されている文字列を取得します。
    pub fn get_selected_text(&self) -> Option<String> {
        let focused_id = self.interaction_states.focused?;
        let user_select = self
            .visual_properties
            .get(focused_id)
            .and_then(|v| v.user_select)
            .unwrap_or(UserSelect::None);

        if user_select == UserSelect::Text {
            let range = self.text_selections.get(focused_id)?;
            if range.start < range.end {
                let text = self.text_contents.get(focused_id)?;
                let u16_text: Vec<u16> = text.encode_utf16().collect();
                let slice =
                    &u16_text[range.start.min(u16_text.len())..range.end.min(u16_text.len())];
                return String::from_utf16(slice).ok();
            }
        }
        None
    }

    /// 外部から提供されたテキストを、現在フォーカスされている入力要素にペーストします。
    pub fn inject_paste(&mut self, text: &str) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.interaction_states.focused
            && self.active_masks[focused_id].has(COMP_INPUT_CONTENT)
            && let Some(contents) = self.input_contents.get_mut(focused_id)
        {
            let text_val = contents.text.0.get();
            let range = contents.selected_range.clone();

            let u16_text: Vec<u16> = text_val.encode_utf16().collect();
            let mut left = u16_text[..range.start.min(u16_text.len())].to_vec();
            let right = u16_text[range.end.min(u16_text.len())..].to_vec();

            let mut pasted_u16: Vec<u16> = text.encode_utf16().collect();

            // ペーストテキストに対する数値制限フィルターの適用
            if contents.numeric_only {
                pasted_u16.retain(|&ch_u16| {
                    if let Ok(ch_char) = String::from_utf16(&[ch_u16])
                        && let Some(c) = ch_char.chars().next()
                    {
                        return c.is_numeric() || c == '.' || c == '-';
                    }

                    false
                });
            }

            // ペーストテキストに対する文字数制限の適用（制限限界位置で自動カット）
            if let Some(max) = contents.max_length {
                let current_after_range_deleted = u16_text.len()
                    - (range.end.min(u16_text.len()) - range.start.min(u16_text.len()));
                if current_after_range_deleted >= max {
                    return; // すでに限界文字数に達しているため無視
                }
                let allowed_len = max - current_after_range_deleted;
                if pasted_u16.len() > allowed_len {
                    pasted_u16.truncate(allowed_len); // 限界位置で足し合わせをカット
                }
            }

            left.extend_from_slice(&pasted_u16);
            left.extend_from_slice(&right);

            let new_text = String::from_utf16_lossy(&left);
            let new_caret = range.start + pasted_u16.len();

            // 変更履歴（Undo）をセーブ
            contents.record_undo(text_val.clone(), range.clone());

            contents.selected_range = new_caret..new_caret;
            self.text_selections
                .insert(focused_id, new_caret..new_caret);
            self.selected_rects.remove(focused_id);
            contents.text.1.set(new_text);

            crate::update_input_caret_position(self, focused_id);
            self.mark_render_dirty(focused_id);
        }
    }

    /// Undo (元に戻す) のインジェクション
    pub fn inject_undo(&mut self) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.interaction_states.focused
            && self.active_masks[focused_id].has(COMP_INPUT_CONTENT)
            && let Some(contents) = self.input_contents.get_mut(focused_id)
            && let Some((prev_text, prev_sel)) = contents.undo_stack.pop()
        {
            let current_text = contents.text.0.get();
            let current_sel = contents.selected_range.clone();
            contents.redo_stack.push((current_text, current_sel)); // 現在の状態を Redo 用にセーブ

            contents.selected_range = prev_sel.clone();
            self.text_selections.insert(focused_id, prev_sel);
            self.selected_rects.remove(focused_id);
            contents.text.1.set(prev_text);

            crate::update_input_caret_position(self, focused_id);
            self.mark_render_dirty(focused_id);
        }
    }

    /// Redo (やり直し) のインジェクション
    pub fn inject_redo(&mut self) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.interaction_states.focused
            && self.active_masks[focused_id].has(COMP_INPUT_CONTENT)
            && let Some(contents) = self.input_contents.get_mut(focused_id)
            && let Some((next_text, next_sel)) = contents.redo_stack.pop()
        {
            let current_text = contents.text.0.get();
            let current_sel = contents.selected_range.clone();
            contents.undo_stack.push((current_text, current_sel)); // 現在の状態を Undo 用に退避

            contents.selected_range = next_sel.clone();
            self.text_selections.insert(focused_id, next_sel);
            self.selected_rects.remove(focused_id);
            contents.text.1.set(next_text);

            crate::update_input_caret_position(self, focused_id);
            self.mark_render_dirty(focused_id);
        }
    }

    /// 切り取り (Ctrl+X) の実行と削除後のテキスト取得
    pub fn inject_cut(&mut self) -> Option<String> {
        let _context_guard = bind_context(self);
        let focused_id = self.interaction_states.focused?;
        let user_select = self
            .visual_properties
            .get(focused_id)
            .and_then(|v| v.user_select)
            .unwrap_or(UserSelect::None);

        if user_select == UserSelect::Text
            && let Some(range) = self.text_selections.get(focused_id).cloned()
            && range.start < range.end
            && let Some(text) = self.text_contents.get(focused_id)
        {
            let u16_text: Vec<u16> = text.encode_utf16().collect();
            let slice = &u16_text[range.start.min(u16_text.len())..range.end.min(u16_text.len())];
            let cut_text = String::from_utf16(slice).ok()?;

            // 対象が Input コントロールである場合のみ、切り取り削除上書きを実行
            if self.active_masks[focused_id].has(COMP_INPUT_CONTENT)
                && let Some(contents) = self.input_contents.get_mut(focused_id)
            {
                // 削除前の履歴セーブ
                let current_text = contents.text.0.get();
                let current_range = contents.selected_range.clone();
                contents.record_undo(current_text, current_range);

                let u16_input: Vec<u16> = contents.text.0.get().encode_utf16().collect();
                let mut left = u16_input[..range.start.min(u16_input.len())].to_vec();
                let right = u16_input[range.end.min(u16_input.len())..].to_vec();
                left.extend_from_slice(&right);

                let new_text = String::from_utf16_lossy(&left);
                contents.selected_range = range.start..range.start;
                self.text_selections
                    .insert(focused_id, range.start..range.start);
                self.selected_rects.remove(focused_id);
                contents.text.1.set(new_text);

                crate::update_input_caret_position(self, focused_id);
                self.mark_render_dirty(focused_id);
            }

            return Some(cut_text);
        }

        None
    }

    /// 指定された要素の子要素全体のスクロール領域を親ローカル座標系で算出します。
    pub fn get_scroll_size(&self, id: EntityId) -> LayoutSize {
        let mut max_x = 0.0f32;
        let mut max_y = 0.0f32;

        // 親要素自体のボーダー・パディング厚を取得
        let (basic, _, _) = self.resolve_active_layouts(id);
        let border_left = match basic.border.left {
            Length::Px(v) => v,
            _ => 0.0,
        };
        let border_top = match basic.border.top {
            Length::Px(v) => v,
            _ => 0.0,
        };
        let padding_left = match basic.padding.left {
            Length::Px(v) => v,
            _ => 0.0,
        };
        let padding_top = match basic.padding.top {
            Length::Px(v) => v,
            _ => 0.0,
        };

        let offset_x = border_left + padding_left;
        let offset_y = border_top + padding_top;

        // スクロールバー要素のIDを取得して除外対象にする
        let (v_track_opt, h_track_opt) = if let Some(sb_state) = self.scrollbar_styles.get(id) {
            (sb_state.v_track_id, sb_state.h_track_id)
        } else {
            (None, None)
        };

        if let Some(children_list) = self.children.get(id) {
            for &child_id in children_list {
                // スクロールバーのトラックはサイズ計算から除外
                if Some(child_id) == v_track_opt || Some(child_id) == h_track_opt {
                    continue;
                }

                // 絶対配置要素（スクロールバーのサムなど）もスクロール領域サイズ計算から除外
                let is_absolute = self
                    .basic_layouts
                    .get(child_id)
                    .map(|l| l.position == Position::Absolute)
                    .unwrap_or(false);
                if is_absolute {
                    continue;
                }

                if let Some(&rect) = self.rects.get(child_id) {
                    let parent_rect = self.rects.get(id).copied().unwrap_or(LayoutRect::ZERO);
                    let scroll_offset = self
                        .scroll_offsets
                        .get(id)
                        .copied()
                        .unwrap_or(LayoutPoint::ZERO);

                    // 親の左上（border+padding除外）を原点 (0,0) とした子要素の右下端
                    let local_right =
                        rect.x - parent_rect.x + scroll_offset.x + rect.width - offset_x;
                    let local_bottom =
                        rect.y - parent_rect.y + scroll_offset.y + rect.height - offset_y;

                    max_x = max_x.max(local_right);
                    max_y = max_y.max(local_bottom);
                }
            }
        }

        LayoutSize::new(max_x, max_y)
    }

    /// スクロールオフセットを目標位置へクランプした上で代入します。
    /// オフセットに変化が生じた場合は true を返し、レイアウトのDirtyマークを打ちます。
    pub fn scroll_to(&mut self, id: EntityId, mut x: f32, mut y: f32) -> bool {
        let rect = match self.rects.get(id).copied() {
            Some(r) => r,
            None => return false,
        };

        let scroll_size = self.get_scroll_size(id);
        let window_size = self.last_window_size.unwrap_or(LayoutSize::ZERO);

        // 親コンテナのボーダーおよびパディング厚を取得
        let (basic, _, _) = self.resolve_active_layouts(id);
        let border_right = match basic.border.right {
            Length::Px(v) => v,
            _ => 0.0,
        };
        let border_bottom = match basic.border.bottom {
            Length::Px(v) => v,
            _ => 0.0,
        };
        let border_left = match basic.border.left {
            Length::Px(v) => v,
            _ => 0.0,
        };
        let border_top = match basic.border.top {
            Length::Px(v) => v,
            _ => 0.0,
        };
        let padding_bottom = match basic.padding.bottom {
            Length::Px(v) => v,
            _ => 0.0,
        };
        let padding_right = match basic.padding.right {
            Length::Px(v) => v,
            _ => 0.0,
        };
        let padding_left = match basic.padding.left {
            Length::Px(v) => v,
            _ => 0.0,
        };
        let padding_top = match basic.padding.top {
            Length::Px(v) => v,
            _ => 0.0,
        };

        // ウィンドウ内に実際に収まっている有効なコンテナ表示サイズを算出
        let visible_width = if window_size.width > 0.0 {
            let left = rect.x.max(0.0);
            let right = (rect.x + rect.width).min(window_size.width);
            (right - left).max(0.0)
        } else {
            rect.width
        };

        let visible_height = if window_size.height > 0.0 {
            let top = rect.y.max(0.0);
            let bottom = (rect.y + rect.height).min(window_size.height);
            (bottom - top).max(0.0)
        } else {
            rect.height
        };

        // 全体サイズから境界（ボーダーとパディング）を差し引き内枠の有効表示可能サイズを正確に算出
        let content_w =
            (visible_width - border_left - border_right - padding_left - padding_right).max(0.0);
        let content_h =
            (visible_height - border_top - border_bottom - padding_top - padding_bottom).max(0.0);

        // コンテンツサイズと内枠表示領域サイズの差分として、正確な最大スクロール量を算出
        let max_scroll_x = (scroll_size.width - content_w).max(0.0);
        let max_scroll_y = (scroll_size.height - content_h).max(0.0);

        x = x.clamp(0.0, max_scroll_x);
        y = y.clamp(0.0, max_scroll_y);

        // スロットが存在しない場合はあらかじめ挿入して初期化
        if !self.scroll_offsets.contains_key(id) {
            self.scroll_offsets.insert(id, LayoutPoint::ZERO);
        }

        let current = self.scroll_offsets.get_mut(id).unwrap();
        if (current.x - x).abs() > 0.01 || (current.y - y).abs() > 0.01 {
            current.x = x;
            current.y = y;

            // スクロールバー状態の最終スクロール時刻を更新
            if let Some(sb_state) = self.scrollbar_styles.get_mut(id) {
                sb_state.last_scroll_time = Some(Instant::now());
            }

            // オフセット変化に伴い、子孫全体の絶対座標を再同期させる
            self.mark_layout_dirty(id);
            true
        } else {
            false
        }
    }

    /// スクロールコンテナのスタイル設定に連動し、
    /// トラック・サムに相当する要素（Element）を遅延生成して親子関係にアタッチします。
    pub(crate) fn ensure_scrollbar_elements(
        &mut self,
        id: EntityId,
        sb: &ScrollbarStyle,
        merge: bool,
    ) {
        if !self.scrollbar_styles.contains_key(id) {
            self.scrollbar_styles.insert(
                id,
                ScrollBarState {
                    style: sb.clone(),
                    v_track_id: None,
                    v_thumb_id: None,
                    h_track_id: None,
                    h_thumb_id: None,
                    v_thumb_hovered: false,
                    v_thumb_dragged: false,
                    h_thumb_hovered: false,
                    h_thumb_dragged: false,
                    drag_start_mouse: LayoutPoint::ZERO,
                    drag_start_offset: LayoutPoint::ZERO,
                    last_scroll_time: None,
                },
            );
        }

        let mut state = self.scrollbar_styles.get(id).cloned().unwrap();
        state.style = sb.clone();
        let mut changed = false;

        if sb.display != ScrollbarDisplay::None {
            // A. 縦スクロールバー (V-Track)
            let v_track = if let Some(v_track) = state.v_track_id {
                v_track
            } else {
                let v_track = self.spawn(Some(id));
                self.add_child(id, v_track);
                state.v_track_id = Some(v_track);
                changed = true;
                v_track
            };

            // トラックは常に絶対配置（コンテナの右端に固定）
            let track_style = sb
                .v_track
                .clone()
                .unwrap_or_default()
                .absolute()
                .z_index(9999)
                .width(sb.width)
                .inset((0.0, 0.0, 0.0, crate::auto()))
                .pointer_events_auto(); // イベントを透過させない

            let el = Element::from(v_track);
            el.style_internal(self, track_style, merge);

            // A-1. 縦つまみ (V-Thumb、V-Track の子要素としてアタッチ)
            let v_thumb = if let Some(v_thumb) = state.v_thumb_id {
                v_thumb
            } else {
                let v_thumb = self.spawn(Some(v_track));
                self.add_child(v_track, v_thumb);
                state.v_thumb_id = Some(v_thumb);
                changed = true;
                v_thumb
            };

            let mut thumb_width = sb.width;
            if let Some(ref thumb_style) = sb.v_thumb
                && let Val::Px(w) = thumb_style.inner.basic_layout.size.width
            {
                thumb_width = w.min(sb.width);
            }

            // サムは V-Track の絶対座標を原点とし、Y方向のみ absolute スライド
            let thumb_style = sb
                .v_thumb
                .clone()
                .unwrap_or_default()
                .absolute()
                .width(thumb_width)
                .inset((0.0, crate::auto(), crate::auto(), crate::auto()))
                .pointer_events_auto();

            let el = Element::from(v_thumb);
            el.style_internal(self, thumb_style, merge);

            // B. 横スクロールバー (H-Track)
            let h_track = if let Some(h_track) = state.h_track_id {
                h_track
            } else {
                let h_track = self.spawn(Some(id));
                self.add_child(id, h_track);
                state.h_track_id = Some(h_track);
                changed = true;
                h_track
            };

            let track_style = sb
                .h_track
                .clone()
                .unwrap_or_default()
                .absolute()
                .z_index(9999)
                .height(sb.width)
                .inset((crate::auto(), 0.0, 0.0, 0.0))
                .pointer_events_auto();

            let el = Element::from(h_track);
            el.style_internal(self, track_style, merge);

            // B-1. 横つまみ (H-Thumb、H-Track の子要素としてアタッチ)
            let h_thumb = if let Some(h_thumb) = state.h_thumb_id {
                h_thumb
            } else {
                let h_thumb = self.spawn(Some(h_track));
                self.add_child(h_track, h_thumb);
                state.h_thumb_id = Some(h_thumb);
                changed = true;
                h_thumb
            };

            let mut thumb_height = sb.width;
            if let Some(ref thumb_style) = sb.h_thumb
                && let Val::Px(h) = thumb_style.inner.basic_layout.size.height
            {
                thumb_height = h.min(sb.width);
            }

            let thumb_style = sb
                .h_thumb
                .clone()
                .unwrap_or_default()
                .absolute()
                .height(thumb_height)
                .inset((crate::auto(), crate::auto(), crate::auto(), 0.0))
                .pointer_events_auto();

            let el = Element::from(h_thumb);
            el.style_internal(self, thumb_style, merge);
        }

        if changed {
            *self.scrollbar_styles.get_mut(id).unwrap() = state;
            self.is_structure_dirty = true; // flat_dfs_sequence の更新契機
        }
    }

    /// 現在のスクロール位置から相対移動します。
    pub fn scroll_by(&mut self, id: EntityId, dx: f32, dy: f32) -> bool {
        let current = self
            .scroll_offsets
            .get(id)
            .copied()
            .unwrap_or(LayoutPoint::ZERO);
        self.scroll_to(id, current.x + dx, current.y + dy)
    }

    /// StyleTarget に応じた可変 BasicLayout を自動生成（Ensure）を解決した上で取得します
    pub(crate) fn get_basic_layout_mut(
        &mut self,
        id: EntityId,
        target: StyleTarget,
    ) -> Option<&mut BasicLayout> {
        match target {
            StyleTarget::Base => self.base_basic_layouts.get_mut(id),
            _ => {
                // interaction_properties SoA スロットの存在を保証
                if !self.interaction_properties.contains_key(id) {
                    self.interaction_properties
                        .insert(id, InteractionStyles::default());
                }
                let styles = self.interaction_properties.get_mut(id).unwrap();

                // すべての StyleTarget に対応する Option<ThisStyle> フィールドを完全に解決
                let style_ref = match target {
                    StyleTarget::Hovered => styles.hovered.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Focused => styles.focused.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Pressed => styles.pressed.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Disabled => styles.disabled.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Actived => styles.actived.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Selected => styles.selected.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Dragged => styles.dragged.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Dragging => styles.dragging.get_or_insert_with(ThisStyle::new),
                    StyleTarget::DragIn => styles.drag_in.get_or_insert_with(ThisStyle::new),
                    StyleTarget::DragOver => styles.drag_over.get_or_insert_with(ThisStyle::new),

                    StyleTarget::HoveredWithin => {
                        styles.hovered_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::FocusedWithin => {
                        styles.focused_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::PressedWithin => {
                        styles.pressed_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::DisabledWithin => {
                        styles.disabled_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::ActivedWithin => {
                        styles.actived_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::SelectedWithin => {
                        styles.selected_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::DraggedWithin => {
                        styles.dragged_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::AnyWithin => styles.any_within.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Base => unreachable!(),
                };

                // CoW (Arc::make_mut) 解決を安全に施した inner の可変参照を引き出す
                Some(&mut Arc::make_mut(&mut style_ref.inner).basic_layout)
            }
        }
    }

    /// StyleTarget に応じた可変 VisualProperty を自動生成（Ensure）を解決した上で取得します
    pub(crate) fn get_visual_property_mut(
        &mut self,
        id: EntityId,
        target: StyleTarget,
    ) -> Option<&mut VisualProperty> {
        match target {
            StyleTarget::Base => self.base_visual_properties.get_mut(id),
            _ => {
                if !self.interaction_properties.contains_key(id) {
                    self.interaction_properties
                        .insert(id, InteractionStyles::default());
                }
                let styles = self.interaction_properties.get_mut(id).unwrap();

                let style_ref = match target {
                    StyleTarget::Hovered => styles.hovered.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Focused => styles.focused.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Pressed => styles.pressed.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Disabled => styles.disabled.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Actived => styles.actived.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Selected => styles.selected.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Dragged => styles.dragged.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Dragging => styles.dragging.get_or_insert_with(ThisStyle::new),
                    StyleTarget::DragIn => styles.drag_in.get_or_insert_with(ThisStyle::new),
                    StyleTarget::DragOver => styles.drag_over.get_or_insert_with(ThisStyle::new),

                    StyleTarget::HoveredWithin => {
                        styles.hovered_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::FocusedWithin => {
                        styles.focused_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::PressedWithin => {
                        styles.pressed_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::DisabledWithin => {
                        styles.disabled_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::ActivedWithin => {
                        styles.actived_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::SelectedWithin => {
                        styles.selected_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::DraggedWithin => {
                        styles.dragged_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::AnyWithin => styles.any_within.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Base => unreachable!(),
                };

                Some(&mut Arc::make_mut(&mut style_ref.inner).visual_property)
            }
        }
    }

    /// StyleTarget に応じた可変 FlexLayout を自動生成（Ensure）を解決した上で取得します
    pub(crate) fn get_flex_layout_mut(
        &mut self,
        id: EntityId,
        target: StyleTarget,
    ) -> Option<&mut FlexLayout> {
        match target {
            StyleTarget::Base => self.flex_layouts.get_mut(id),
            _ => {
                if !self.interaction_properties.contains_key(id) {
                    self.interaction_properties
                        .insert(id, InteractionStyles::default());
                }
                let styles = self.interaction_properties.get_mut(id).unwrap();

                let style_ref = match target {
                    StyleTarget::Hovered => styles.hovered.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Focused => styles.focused.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Pressed => styles.pressed.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Disabled => styles.disabled.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Actived => styles.actived.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Selected => styles.selected.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Dragged => styles.dragged.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Dragging => styles.dragging.get_or_insert_with(ThisStyle::new),
                    StyleTarget::DragIn => styles.drag_in.get_or_insert_with(ThisStyle::new),
                    StyleTarget::DragOver => styles.drag_over.get_or_insert_with(ThisStyle::new),

                    StyleTarget::HoveredWithin => {
                        styles.hovered_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::FocusedWithin => {
                        styles.focused_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::PressedWithin => {
                        styles.pressed_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::DisabledWithin => {
                        styles.disabled_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::ActivedWithin => {
                        styles.actived_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::SelectedWithin => {
                        styles.selected_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::DraggedWithin => {
                        styles.dragged_within.get_or_insert_with(ThisStyle::new)
                    }
                    StyleTarget::AnyWithin => styles.any_within.get_or_insert_with(ThisStyle::new),
                    StyleTarget::Base => unreachable!(),
                };

                Some(&mut Arc::make_mut(&mut style_ref.inner).flex_layout)
            }
        }
    }

    /// 指定された要素が現在マウスホバーされているか判定します
    #[inline]
    pub fn is_hovered(&self, id: EntityId) -> bool {
        self.active_masks
            .get(id)
            .map(|m| m.has(STATE_HOVERED))
            .unwrap_or(false)
    }

    /// 指定された要素が現在キーボードフォーカスを得ているか判定します
    #[inline]
    pub fn is_focused(&self, id: EntityId) -> bool {
        self.active_masks
            .get(id)
            .map(|m| m.has(STATE_FOCUSED))
            .unwrap_or(false)
    }

    /// 指定された要素が現在マウスやタップで押し下げられているか判定します
    #[inline]
    pub fn is_pressed(&self, id: EntityId) -> bool {
        self.active_masks
            .get(id)
            .map(|m| m.has(STATE_PRESSED))
            .unwrap_or(false)
    }

    /// 指定された要素が無効化（操作不可）状態にあるか判定します
    #[inline]
    pub fn is_disabled(&self, id: EntityId) -> bool {
        self.active_masks
            .get(id)
            .map(|m| m.has(STATE_DISABLED))
            .unwrap_or(false)
    }

    /// 指定された要素が現在アクティブ（有効選択など）状態にあるか判定します
    #[inline]
    pub fn is_actived(&self, id: EntityId) -> bool {
        self.active_masks
            .get(id)
            .map(|m| m.has(STATE_ACTIVED))
            .unwrap_or(false)
    }

    /// 指定された要素が現在テキストまたはトグル選択されているか判定します
    #[inline]
    pub fn is_selected(&self, id: EntityId) -> bool {
        self.active_masks
            .get(id)
            .map(|m| m.has(STATE_SELECTED))
            .unwrap_or(false)
    }

    /// 指定された要素が現在ドラッグ操作中にあるか判定します
    #[inline]
    pub fn is_dragged(&self, id: EntityId) -> bool {
        self.active_masks
            .get(id)
            .map(|m| m.has(STATE_DRAGGED))
            .unwrap_or(false)
    }

    pub fn entity_id_focused(&self) -> Option<EntityId> {
        self.interaction_states.focused
    }

    pub fn entity_id_dragged(&self) -> Option<EntityId> {
        self.interaction_states.dragged
    }

    pub fn entity_id_hovered(&self) -> Option<EntityId> {
        self.interaction_states.hovered
    }

    pub fn entity_id_pressed(&self) -> Option<EntityId> {
        self.interaction_states.pressed
    }

    /// 指定した要素の画面上の絶対座標（LayoutRect）を取得します。
    pub fn rect(&self, handle: Element) -> Option<LayoutRect> {
        self.rects.get(handle.id).copied()
    }

    /// 指定した要素の画面上のクリップ境界（LayoutRect）を取得します。
    pub fn clip_rect(&self, handle: Element) -> Option<LayoutRect> {
        self.clip_rects.get(handle.id).copied()
    }

    /// 指定した要素の子要素一覧を取得します。
    pub fn children_list(&self, handle: Element) -> Option<Vec<Element>> {
        self.children
            .get(handle.id)
            .map(|c| c.iter().map(|&id| Element { id }).collect())
    }

    /// 画面上でアクティブ（有効）になっている要素の総数を取得します。
    pub fn active_entities_count(&self) -> usize {
        self.active_entities.len()
    }

    /// 現在ホバーされている要素から親ツリーを遡り、適用するべき物理的な CursorIcon を正確に解決します。
    pub fn resolve_cursor(&self, hovered_id: EntityId) -> CursorIcon {
        // 現在プレス中の要素（pressed）があればそれを最優先で探索の基点にする
        let start_id = self.interaction_states.pressed.unwrap_or(hovered_id);

        let mut curr = Some(start_id);
        let mut global_cursor = None;

        while let Some(id) = curr {
            let cursor_opt = self
                .visual_properties
                .get(id)
                .and_then(|v| v.cursor)
                .or_else(|| self.base_visual_properties.get(id).and_then(|v| v.cursor));

            if let Some(cursor) = cursor_opt {
                match cursor {
                    // Global バリアントを見つけた場合、より具体的な個別カーソルが見つかっていない場合のみ記録
                    CursorIcon::Global(global_icon) => {
                        if global_cursor.is_none() {
                            global_cursor = Some(global_icon);
                        }
                    }
                    // 通常の個別カーソルが見つかった場合はこれが最優先なので即時採用
                    // 親の Global の影響を遮断してDefault()に戻したい場合は、子要素側で Default() がヒットするため即時解決
                    normal_cursor => {
                        return normal_cursor;
                    }
                }
            }
            curr = self.parents.get(id).copied().flatten();
        }

        // 個別指定がなく、親のいずれかに Global カーソルが定義されていた場合はそれを採用
        if let Some(global) = global_cursor {
            match global {
                GlobalCursorIcon::Default(opt) => CursorIcon::Default(opt),
                GlobalCursorIcon::Pointer(opt) => CursorIcon::Pointer(opt),
                GlobalCursorIcon::Text(opt) => CursorIcon::Text(opt),
                GlobalCursorIcon::Grab(opt) => CursorIcon::Grab(opt),
                GlobalCursorIcon::Grabbing(opt) => CursorIcon::Grabbing(opt),
                GlobalCursorIcon::NotAllowed(opt) => CursorIcon::NotAllowed(opt),
                GlobalCursorIcon::ResizeNs(opt) => CursorIcon::ResizeNs(opt),
                GlobalCursorIcon::ResizeEw(opt) => CursorIcon::ResizeEw(opt),
                GlobalCursorIcon::ResizeNesw(opt) => CursorIcon::ResizeNesw(opt),
                GlobalCursorIcon::ResizeNwse(opt) => CursorIcon::ResizeNwse(opt),
            }
        } else {
            // 先祖に何の設定もない場合はデフォルトの矢印
            CursorIcon::Default(None)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResizeDirection {
    Top,
    Right,
    Bottom,
    Left,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone)]
pub(crate) struct ResizingState {
    pub(crate) entity_id: EntityId,
    pub(crate) direction: ResizeDirection,
    pub(crate) start_mouse_pos: LayoutPoint,
    pub(crate) start_rect: LayoutRect,
    pub(crate) start_inset: Rect<Val>,
}

/// リサイズ方向から対応するカーソル種別へ変換するヘルパー
fn resize_direction_to_cursor(dir: ResizeDirection) -> CursorIcon {
    match dir {
        ResizeDirection::Top | ResizeDirection::Bottom => CursorIcon::ResizeNs(None),
        ResizeDirection::Left | ResizeDirection::Right => CursorIcon::ResizeEw(None),
        ResizeDirection::TopRight | ResizeDirection::BottomLeft => CursorIcon::ResizeNesw(None),
        ResizeDirection::TopLeft | ResizeDirection::BottomRight => CursorIcon::ResizeNwse(None),
    }
}

/// マウス位置と要素の境界・リサイズ許可フラグから、該当するリサイズ方向を算出するヘルパー
fn detect_resize_direction(
    rect: LayoutRect,
    resizable: [bool; 4], // [top, right, bottom, left]
    pos: LayoutPoint,
    border: f32,
) -> Option<ResizeDirection> {
    let [t, r, b, l] = resizable;
    if !t && !r && !b && !l {
        return None;
    }

    // 境界線の外側（-border）から内側（+border）までのあそびの範囲を厳密に判定
    let on_t = t
        && (pos.y >= rect.y - border && pos.y <= rect.y + border)
        && (pos.x >= rect.x - border && pos.x <= rect.x + rect.width + border);

    let on_b = b
        && (pos.y >= rect.y + rect.height - border && pos.y <= rect.y + rect.height + border)
        && (pos.x >= rect.x - border && pos.x <= rect.x + rect.width + border);

    let on_l = l
        && (pos.x >= rect.x - border && pos.x <= rect.x + border)
        && (pos.y >= rect.y - border && pos.y <= rect.y + rect.height + border);

    let on_r = r
        && (pos.x >= rect.x + rect.width - border && pos.x <= rect.x + rect.width + border)
        && (pos.y >= rect.y - border && pos.y <= rect.y + rect.height + border);

    match (on_t, on_r, on_b, on_l) {
        (true, true, _, _) => Some(ResizeDirection::TopRight),
        (true, _, _, true) => Some(ResizeDirection::TopLeft),
        (_, true, true, _) => Some(ResizeDirection::BottomRight),
        (_, _, true, true) => Some(ResizeDirection::BottomLeft),
        (true, _, _, _) => Some(ResizeDirection::Top),
        (_, true, _, _) => Some(ResizeDirection::Right),
        (_, _, true, _) => Some(ResizeDirection::Bottom),
        (_, _, _, true) => Some(ResizeDirection::Left),
        _ => None,
    }
}

/// ドロップ先コンテナのフレックス方向（Row / Column）に基づいて、
/// マウスのドロップ座標がどの子要素の手前（インデックス）に位置するかを逆引き算出します。
fn calculate_insert_index(cx: &Context, parent_id: EntityId, logical_pos: LayoutPoint) -> usize {
    let mut insert_idx = 0;

    if let Some(children) = cx.children.get(parent_id) {
        let parent_flex = cx.flex_layouts.get(parent_id).copied().unwrap_or_default();
        let is_row = parent_flex.flex_direction == FlexDirection::Row
            || parent_flex.flex_direction == FlexDirection::RowReverse;

        for (idx, &child_id) in children.iter().enumerate() {
            if let Some(rect) = cx.rects.get(child_id) {
                if is_row {
                    let center_x = rect.x + rect.width * 0.5;
                    if logical_pos.x > center_x {
                        insert_idx = idx + 1;
                    }
                } else {
                    let center_y = rect.y + rect.height * 0.5;
                    if logical_pos.y > center_y {
                        insert_idx = idx + 1;
                    }
                }
            }
        }
    }

    insert_idx
}

// クリップボード API による UTF-16 読み書きヘルパー
unsafe fn win32_set_clipboard(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let text_u16: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    let size = text_u16.len() * 2;
    let h_mem = unsafe { GlobalAlloc(GMEM_MOVEABLE, size)? };
    let ptr = unsafe { GlobalLock(h_mem) };
    unsafe {
        std::ptr::copy_nonoverlapping(text_u16.as_ptr(), ptr as *mut u16, text_u16.len());
    }
    let _ = unsafe { GlobalUnlock(h_mem) };
    if unsafe { OpenClipboard(None).is_ok() } {
        let _ = unsafe { EmptyClipboard() };
        let _ = unsafe { SetClipboardData(13, Some(HANDLE(h_mem.0))) }; // 13 = CF_UNICODETEXT
        let _ = unsafe { CloseClipboard() };
    }
    Ok(())
}

unsafe fn win32_get_clipboard() -> Result<String, Box<dyn std::error::Error>> {
    let mut result = String::new();
    if unsafe { OpenClipboard(None).is_ok() } {
        let h_mem = unsafe { GetClipboardData(13)? };
        if !h_mem.is_invalid() {
            let ptr = unsafe { GlobalLock(HGLOBAL(h_mem.0)) };
            if !ptr.is_null() {
                let u16_ptr = ptr as *const u16;
                let mut len = 0;
                while unsafe { *u16_ptr.add(len) } != 0 {
                    len += 1;
                }
                let slice = unsafe { std::slice::from_raw_parts(u16_ptr, len) };
                result = String::from_utf16_lossy(slice);
                let _ = unsafe { GlobalUnlock(HGLOBAL(h_mem.0)) };
            }
        }
        let _ = unsafe { CloseClipboard() };
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
