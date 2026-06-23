#![allow(unused)]
use crate::{
    ActiveTransition, AlignContent, AlignItems, AlignSelf, BasicLayout, BoxShadow, BoxSizing,
    Clear, Color, CornerRadius, CursorIcon, Direction, Display, DrawBatch, EdgeInsets, EffectId,
    Element, ElementState, EventListeners, FlexDirection, FlexLayout, FlexWrap, Float,
    GridAutoFlow, GridLayout, GridLine, GridPlacement, IDENTITY_MATRIX, ImageSource, ImeState,
    InteractionStates, InteractionStyles, JustifyContent, LayoutOverflow, LayoutPoint, LayoutRect,
    LayoutSize, Length, Modifiers, MouseButton, MovieProperty, MovieSource, Position, QuadInstance,
    ReadSignal, Rect, RenderData, SignalId, Size, TextAlign, TextEngine, TransitionValue, UiaValue,
    Val, VirtualKey, VisualProperty, WebView2Contents, WriteSignal, bind_context, bitmap::*,
    with_context,
};
use slotmap::{SecondaryMap, SlotMap, SparseSecondaryMap, new_key_type};
use smallvec::SmallVec;
use std::{
    borrow::Cow,
    marker::PhantomData,
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
};
use taffy::TaffyTree;

new_key_type! {
    /// UI内の各要素（Entity）を識別する一意な世代管理ID
    pub struct EntityId;
}

// Taffyスタイルを一括解決するヘルパー
pub(crate) fn resolve_taffy_style(
    basic: &BasicLayout,
    flex: &FlexLayout,
    grid: Option<&GridLayout>,
) -> taffy::Style {
    let mut style: taffy::Style = taffy::Style {
        display: basic.display.into(),
        box_sizing: basic.box_sizing.into(),
        direction: basic.direction.into(),
        overflow: basic.overflow.into(),
        scrollbar_width: basic.scrollbar_width,
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
        self.inner.send(Box::new(f)).map_err(|_| ())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EffectCategory {
    None,
    Style,
    Text,
    Image,
    Movie,
    WebView2,
    Contents,
    UiaName,
    UiaAutomationId,
}

// TODO: Drop トレイト実装
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

    // 前回値キャッシュ用のダブルバッファ
    pub(crate) prev_rects: SecondaryMap<EntityId, LayoutRect>,
    pub(crate) prev_clip_rects: SecondaryMap<EntityId, LayoutRect>,

    // 8. インタラクション・イベント追跡用ランタイム状態
    /// 現在のポインタの物理座標（ドラッグの移動量算出などに使用）
    pub(crate) current_pointer_position: Option<LayoutPoint>,
    /// 対称的に整理された、グローバルなインタラクション対象状態
    pub(crate) interaction_states: InteractionStates,
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
    pub(crate) subscribers: SecondaryMap<SignalId, Vec<EffectId>>,
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

    // 各要素の WebView2 の詳細な設定・URL情報（コールドデータ）
    pub(crate) webview_contents: SparseSecondaryMap<EntityId, WebView2Contents>,
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
            image_sources: SparseSecondaryMap::new(),
            movie_properties: SparseSecondaryMap::new(),
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
            // シグナルストレージの初期化
            signals: SlotMap::with_key(),
            subscribers: SecondaryMap::new(),
            effects: SlotMap::with_key(),
            element_effects: SecondaryMap::new(),
            task_receiver: rx,
            task_sender: TaskSender { inner: tx },
            text_engine: TextEngine::new(),
            active_transitions: SparseSecondaryMap::new(),
            webview_contents: SparseSecondaryMap::new(),
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

    /// Context インスタンスから直接シグナルを生成します。
    /// これにより build_ui の外側（メインスレッド上）でもシグナルを定義できます。
    pub fn create_signal<T: Send + 'static>(
        &mut self,
        initial_value: T,
    ) -> (ReadSignal<T>, WriteSignal<T>) {
        let id = self.signals.insert(Box::new(initial_value));
        self.subscribers.insert(id, Vec::new());
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
    pub(crate) fn process_main_thread_tasks(&mut self) {
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

    /// 親子関係の追加と、永続Taffy構造のリアルタイム同期
    pub(crate) fn add_child(&mut self, parent: EntityId, child: EntityId) {
        self.parents.insert(child, Some(parent));
        if let Some(children_list) = self.children.get_mut(parent) {
            children_list.push(child);
        }

        // Taffyツリーの親子関係を永続的に更新
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
        self.image_sources.clear();
        self.movie_properties.clear();
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
        self.webview_contents.clear();
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
            if let Some(effects) = self.element_effects.remove(id) {
                for (_, effect_id) in effects {
                    self.effects.remove(effect_id);
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
            self.image_sources.remove(id);
            self.movie_properties.remove(id);
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
            self.webview_contents.remove(id);

            self.is_structure_dirty = true;
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
        if let Some(mask) = self.active_masks.get_mut(id) {
            // すでにレイアウトキューに登録済み（STATE_QUEUED_LAYOUT がオン）なら早期リターン
            if !mask.has(STATE_QUEUED_LAYOUT) {
                mask.set(STATE_QUEUED_LAYOUT); // フラグをオンにして多重登録を防ぐ
                self.dirty_layout_entities.push(id);
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
        // ウィンドウサイズの変更検知
        let window_resized = if self.last_window_size != Some(window_size) {
            self.last_window_size = Some(window_size);
            true
        } else {
            false
        };

        // 構造変更がなく、スタイル変更（レイアウト変更要求）もなく、ウィンドウサイズも変わっていないなら、
        // Taffy計算も、ダブルバッファスワップも、4.6万回のループもすべてスキップして即時帰還する。
        if self.dirty_layout_entities.is_empty() && !self.is_structure_dirty && !window_resized {
            return;
        }

        if self.is_structure_dirty {
            self.rebuild_flat_dfs_sequence(root);
        }

        // 1. Taffy永続ツリーへの差分同期
        let dirty_entities = self.dirty_layout_entities.clone();
        for id in &dirty_entities {
            let (basic, flex, grid) = self.resolve_active_layouts(*id);
            let taffy_style = resolve_taffy_style(&basic, &flex, grid.as_ref());
            let taffy_node = self.taffy_nodes[*id];

            self.taffy.set_style(taffy_node, taffy_style).unwrap();
        }

        self.dirty_layout_entities.clear();

        // 2. Taffy のレイアウト再計算
        if let Some(&root_node) = self.taffy_nodes.get(root) {
            // 計測関数をクロージャとして定義
            let measure_func = |known_dims: taffy::Size<Option<f32>>,
                                available_space: taffy::Size<taffy::AvailableSpace>,
                                _node_id: taffy::NodeId,
                                context: Option<&mut EntityId>,
                                _style: &taffy::Style|
             -> taffy::Size<f32> {
                if let Some(&id) = context.as_deref() {
                    // テキスト内容を持っているかチェック
                    // (クロージャの外側の self (= Context) は直接キャプチャできないため、
                    //  一時的に bind_context されているスレッドローカル経由で取得)
                    return with_context(|cx| {
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

                            let max_width = match available_space.width {
                                taffy::AvailableSpace::Definite(w) => Some(w),
                                _ => None,
                            };

                            // DirectWrite を使用して正確なサイズを計測
                            let size = cx.text_engine.measure_text(
                                text,
                                font_size,
                                font_family,
                                font_weight,
                                font_style,
                                max_width,
                            );

                            taffy::Size {
                                width: known_dims.width.unwrap_or(size.width),
                                height: known_dims.height.unwrap_or(size.height),
                            }
                        } else {
                            taffy::Size::ZERO
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

        // 2. Taffyのレイアウト再計算
        let initial_clip = LayoutRect::new(0.0, 0.0, window_size.width, window_size.height);
        let flat_len = self.flat_dfs_sequence.len();

        // std::mem::swap で前回値の参照先を瞬時に切り替え
        std::mem::swap(&mut self.rects, &mut self.prev_rects);
        std::mem::swap(&mut self.clip_rects, &mut self.prev_clip_rects);

        // 新しい rects / clip_rects を書き込むためにクリア（バッファ領域は再利用される）
        self.rects.clear();
        self.clip_rects.clear();

        // 1次元非再帰・静的キャッシュバイパスループ
        for i in 0..flat_len {
            let id = self.flat_dfs_sequence[i];
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
                let parent_scroll = self
                    .scroll_offsets
                    .get(parent_id)
                    .copied()
                    .unwrap_or(LayoutPoint::ZERO);

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

            // サイズが0.0の要素や画面外の要素の描画スキップ処理は、将来レンダラー側（wgpu等）に委譲。
            self.rects.insert(id, abs_rect);
            let mask = self.active_masks[id];
            let current_clip = if mask.has(STYLE_OVERFLOW) {
                parent_clip.intersect(&abs_rect)
            } else {
                parent_clip
            };
            self.clip_rects.insert(id, current_clip);

            // 常に1次元DFS順でアクティブ要素リストに登録する（テストおよびイベント伝播の整合性を確保）
            self.active_entities.push(id);
        }

        // 状態クリア
        for &id in &dirty_entities {
            if let Some(mask) = self.active_masks.get_mut(id) {
                mask.unset(STATE_QUEUED_LAYOUT);
            }
        }
        self.dirty_layout_entities.clear();
    }

    /// 現在の全アクティブ要素から、wgpu 用の描画バッチを生成します
    pub fn collect_render_data(&self) -> RenderData {
        let mut batches = Vec::new();
        let mut current_instances = Vec::new();
        let mut current_ids = Vec::new();
        let mut last_clip = None;

        // 静的なデフォルト値（一度だけ確保して使い回す）
        let default_visual = VisualProperty::default();

        for &id in &self.active_entities {
            let rect = self.rects[id];
            if rect.width <= 0.0 || rect.height <= 0.0 {
                continue;
            }

            // この要素がCompositorに昇格する（STYLE_ANIMATIONSを持つ）場合、
            // メインのフラットな描画レイヤー（wgpu_visual）からは除外する
            if self.active_masks[id].has(STYLE_ANIMATIONS) {
                continue;
            }

            let clip = self.clip_rects[id];

            // 初回の初期化を安全にキャッチし、異なるクリップ境界の時に新しいバッチを作成する
            if let Some(prev_clip) = last_clip {
                if clip != prev_clip {
                    if !current_instances.is_empty() {
                        batches.push(DrawBatch {
                            scissor_rect: prev_clip,
                            instances: std::mem::take(&mut current_instances),
                            entity_ids: std::mem::take(&mut current_ids),
                        });
                    }
                    last_clip = Some(clip);
                }
            } else {
                // 最初の要素（root）の時点で、確実にその要素のクリップ矩形で初期化します
                last_clip = Some(clip);
            }

            let (basic, _, _) = self.resolve_active_layouts(id);
            // 直アクセス [id] を廃止し、安全な .get() と unwrap_or() に変更
            let visual = self.visual_properties.get(id).unwrap_or(&default_visual);

            // テキスト要素の場合は「テキストの色」、それ以外は「背景色」を color にセットする
            let color = if self.active_masks[id].has(COMP_TEXT_CONTENT) {
                visual.text_color.unwrap_or(Color::BLACK) // デフォルトは黒
            } else {
                visual.bg_color.unwrap_or(Color::TRANSPARENT) // デフォルトは透明
            };

            // グラデーションの解決
            let (gradient_end_color, gradient_angle, mode) = match visual.bg_gradient {
                Some(g) => (g.end_color, g.angle, 1.0f32), // グラデーションモード（1u）
                None => (color, 0.0, 0.0f32),              // 単色モード（0u）
            };

            // Length型 (Px / Percent) からのピクセル安全抽出用ヘルパー
            let get_px = |len: Length| match len {
                Length::Px(v) => v,
                Length::Percent(_) => 0.0,
            };

            let origin = visual
                .transform_origin
                .map(|p| [p.x, p.y])
                .unwrap_or([0.5, 0.5]); // デフォルトは中心

            // SoAからGPU用インスタンスデータへ変換
            let instance = QuadInstance {
                rect,
                transform: visual.transform.unwrap_or(IDENTITY_MATRIX),
                transform_origin: origin,
                color,
                corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
                border_width: EdgeInsets {
                    top: basic.border.top.into(),
                    right: basic.border.right.into(),
                    bottom: basic.border.bottom.into(),
                    left: basic.border.left.into(),
                },
                border_color: visual.border_color.unwrap_or(Color::TRANSPARENT),
                opacity_and_mode: [visual.opacity.unwrap_or(1.0), mode, 0.0, 0.0],
                uv_max: [0.0; 2],
                uv_min: [0.0; 2],
                gradient_end_color,
                gradient_angle,
                _padding: 0.0,
            };

            current_instances.push(instance);
            current_ids.push(id);
        }

        if !current_instances.is_empty() {
            batches.push(DrawBatch {
                scissor_rect: last_clip.unwrap_or(LayoutRect::ZERO),
                instances: current_instances,
                entity_ids: current_ids,
            });
        }

        RenderData { batches }
    }

    /// 現在、アクティブに動いているトランジション（wgpuアニメーション）があるか判定します
    pub fn has_active_animations(&self) -> bool {
        // マップが空、または登録されているすべてのベクタが空であるかを安全にチェック
        !self.active_transitions.is_empty()
            && self
                .active_transitions
                .values()
                .any(|list| !list.is_empty())
    }

    /// 毎フレームの描画前に呼び出され、すべてのアクティブなトランジションを 1 Tick 進めます
    pub fn tick_transitions(&mut self) {
        let now = std::time::Instant::now();

        // 借用チェッカーを回避するため、一時的にマップを take して更新する
        let mut active_map = std::mem::take(&mut self.active_transitions);

        // 完了して空になった要素のIDを記録する一時配列
        let mut to_remove = Vec::new();

        for (id, transitions) in active_map.iter_mut() {
            let mut i = 0;
            while i < transitions.len() {
                let t_state = &mut transitions[i];
                let elapsed = now.duration_since(t_state.start_time);

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
                    }
                    // 縦幅（Height）の毎フレームアニメーション補間
                    TransitionValue::Height(h) => {
                        if let Some(layout) = self.basic_layouts.get_mut(id) {
                            layout.size.height = Val::Px(h);
                        }
                        self.mark_layout_dirty(id);
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
        if let Some(visual) = self.visual_properties.get(id) {
            // transitions ベクタの中から、一致する PropertyList を探す
            if let Some(t) = visual
                .transitions
                .iter()
                .find(|t| t.property_list == property_list)
            {
                let now = std::time::Instant::now();
                if let Some(entry) = self.active_transitions.entry(id) {
                    let active_list = entry.or_insert_with(Vec::new);

                    // 2. 割り込み処理の解決（すでに同じプロパティのアニメーションが走っているか）
                    let actual_start = if let Some(existing) = active_list
                        .iter_mut()
                        .find(|et| et.property_list == property_list)
                    {
                        // すでに駆動中の場合は、その「現在の補間位置」をリアルタイム計算する
                        let elapsed = now.duration_since(existing.start_time);
                        let progress =
                            (elapsed.as_secs_f32() / existing.duration.as_secs_f32()).min(1.0);
                        let eased_t = existing.curve.evaluate(progress);

                        // 中間位置の算出（これが新しいアニメーションの開始点になる）
                        let current_interposed_val =
                            existing.start_value.lerp(&existing.end_value, eased_t);

                        // 既存のアニメーション状態をリセットし、現在地点から新しい目標値（end_value）へ向かうように上書き
                        existing.start_time = now;
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
                        start_time: now,
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
    pub(crate) fn resolve_element_style_state(&mut self, id: EntityId) {
        let active_mask = self.active_masks[id];

        // ─── 1. ビジュアルプロパティ (bg_color, opacity等) の解決 ───
        let has_base_visual = self.base_visual_properties.contains_key(id);
        let has_active_visual = self.visual_properties.contains_key(id);

        // ★ 早期リターン 1: スタイルを一切持たない要素は、ヒープアロケーションを避けるため完全にスキップ
        if has_base_visual || has_active_visual {
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

            // 疑似クラス（Hovered等）のマージをクローンなしで解決
            if let Some(interaction) = self.interaction_properties.get(id) {
                let cascade = [
                    (STATE_FOCUSED, &interaction.focused),
                    (STATE_SELECTED, &interaction.selected),
                    (STATE_ACTIVED, &interaction.actived),
                    (STATE_HOVERED, &interaction.hovered),
                    (STATE_PRESSED, &interaction.pressed),
                    (STATE_DISABLED, &interaction.disabled),
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
                    }
                }
            }

            // トランジション判定 (変更がある場合のみトリガーを試行)
            let target_bg_val = target_bg.unwrap_or(Color::TRANSPARENT);
            let mut bg_triggered = false;
            if current_bg != target_bg_val {
                bg_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::BackgroundColor,
                    TransitionValue::Color(current_bg),
                    TransitionValue::Color(target_bg_val),
                );
            }

            let target_border_val = target_border.unwrap_or(Color::TRANSPARENT);
            let mut border_triggered = false;
            if current_border != target_border_val {
                border_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::BorderColor,
                    TransitionValue::Color(current_border),
                    TransitionValue::Color(target_border_val),
                );
            }

            let target_opacity_val = target_opacity.unwrap_or(1.0);
            let mut opacity_triggered = false;
            if current_opacity != target_opacity_val {
                opacity_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::Opacity,
                    TransitionValue::Opacity(current_opacity),
                    TransitionValue::Opacity(target_opacity_val),
                );
            }

            let target_transform_val = target_transform.unwrap_or(IDENTITY_MATRIX);
            let mut transform_triggered = false;
            if current_transform != target_transform_val {
                transform_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::Transform,
                    TransitionValue::Transform(current_transform),
                    TransitionValue::Transform(target_transform_val),
                );
            }

            let target_radius_val = target_radius.unwrap_or(CornerRadius::ZERO);
            let mut radius_triggered = false;
            if current_radius != target_radius_val {
                radius_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::CornerRadius,
                    TransitionValue::CornerRadius(current_radius),
                    TransitionValue::CornerRadius(target_radius_val),
                );
            }

            // アニメーションが起動した、または明示的にベースの描画プロパティがある場合のみ
            // 遅延評価（Lazy）でマップを確保し、書き込みを行う
            if bg_triggered
                || border_triggered
                || opacity_triggered
                || transform_triggered
                || radius_triggered
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

                // コールドプロパティの即時代入
                if let Some(target_vis) = self.base_visual_properties.get(id) {
                    active_vis.box_shadow = target_vis.box_shadow;
                    active_vis.clip_path = target_vis.clip_path.clone();
                    active_vis.z_index = target_vis.z_index;
                    active_vis.cursor = target_vis.cursor;
                    active_vis.filter = target_vis.filter.clone();
                    active_vis.text_color = target_vis.text_color;
                    active_vis.font_size = target_vis.font_size;
                    active_vis.transitions = target_vis.transitions.clone();
                    active_vis.keyframe_animations = target_vis.keyframe_animations.clone();
                }
            }
        }

        // ─── 2. レイアウトプロパティ (Width, Height) の解決 ───
        let has_base_layout = self.base_basic_layouts.contains_key(id);
        let has_active_layout = self.basic_layouts.contains_key(id);

        // ★ 早期リターン 2: レイアウト変更のない要素は完全にスキップ
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
                ];

                for (state, style_opt) in cascade {
                    if active_mask.has(state)
                        && let Some(style) = style_opt
                    {
                        target_layout.override_with(&style.inner.basic_layout, style.inner.mask);
                    }
                }
            }

            let target_w = match target_layout.size.width {
                Val::Px(v) => Some(v),
                _ => None,
            };
            let current_w = match active_layout.size.width {
                Val::Px(v) => Some(v),
                _ => None,
            };
            let target_h = match target_layout.size.height {
                Val::Px(v) => Some(v),
                _ => None,
            };
            let current_h = match active_layout.size.height {
                Val::Px(v) => Some(v),
                _ => None,
            };

            let mut width_triggered = false;
            let mut height_triggered = false;

            if let (Some(cw), Some(tw)) = (current_w, target_w)
                && cw != tw
            {
                width_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::Width,
                    TransitionValue::Width(cw),
                    TransitionValue::Width(tw),
                );
            }

            if let (Some(ch), Some(th)) = (current_h, target_h)
                && ch != th
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
                active_layout_mut.size.width = Val::Px(current_w.unwrap());
            }
            if height_triggered {
                active_layout_mut.size.height = Val::Px(current_h.unwrap());
            }

            // 最終的に解決されたレイアウトを Taffy ツリーに即時同期させるため、
            // スタイル解決の末尾でレイアウトの Dirty マークを叩きます
            self.mark_layout_dirty(id);
        }
    }

    /// マウス座標などが、要素の描画領域かつ表示枠（クリップ枠）内に収まっているかを判定。
    /// 階層的な早期枝刈り（Culling）ヒットテスト
    pub fn hit_test(&self, point: LayoutPoint) -> Option<EntityId> {
        // ルート要素群（親を持たない要素）をフラット配列から抽出
        let mut roots = SmallVec::<[EntityId; 8]>::new();
        let flat_len = self.flat_dfs_sequence.len();
        for i in 0..flat_len {
            let id = self.flat_dfs_sequence[i];
            if self.parents.get(id).copied().flatten().is_none() {
                roots.push(id);
            }
        }

        // Painter's Algorithm（前面が優先）に従い、ルート要素を逆順から再帰降下チェック
        let roots_len = roots.len();
        for i in (0..roots_len).rev() {
            let root_id = roots[i];
            if let Some(hit) = self.hit_test_recursive(root_id, point) {
                return Some(hit);
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

        // 3. 子がヒットしなければ、最後に自分自身をチェック
        if let Some(rect) = self.rects.get(id)
            && rect.contains(point)
        {
            return Some(id);
        }

        None
    }

    /// 指定された動的状態（例: STATE_HOVERED）に切り替わる際、
    /// その要素に割り当てられている状態スタイルがレイアウトの再計算を必要とするか判定します。
    pub(crate) fn does_state_require_layout(&self, id: EntityId, state_flag: u64) -> bool {
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

    /// 各インタラクション状態（ステート）を更新し、レイアウト変更を伴うか自動的に判別して Dirty フラグを制御する共通ヘルパー
    #[inline(always)]
    fn update_state(&mut self, id: EntityId, state_flag: u64, active: bool) {
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
                self.resolve_element_style_state(id);

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
    pub(crate) fn set_hovered(&mut self, id: EntityId, hovered: bool) {
        self.update_state(id, STATE_HOVERED, hovered);
    }

    /// フォーカス（Focused：キーボードタブフォーカス等）状態を更新します。
    pub(crate) fn set_focused(&mut self, id: EntityId, focused: bool) {
        self.update_state(id, STATE_FOCUSED, focused);
    }

    /// プレス（Pressed：クリック押し下げ、タップ中）状態を更新します。
    pub(crate) fn set_pressed(&mut self, id: EntityId, pressed: bool) {
        self.update_state(id, STATE_PRESSED, pressed);
    }

    /// 無効化（Disabled：ボタンの操作不可など）状態を更新します。
    pub(crate) fn set_disabled(&mut self, id: EntityId, disabled: bool) {
        self.update_state(id, STATE_DISABLED, disabled);
    }

    /// アクティブ（Actived：タブのトグル選択中など）状態を更新します。
    pub(crate) fn set_actived(&mut self, id: EntityId, actived: bool) {
        self.update_state(id, STATE_ACTIVED, actived);
    }

    /// セレクト（Selected：チェックボックス、リストなどの選択）状態を更新します。
    pub(crate) fn set_selected(&mut self, id: EntityId, selected: bool) {
        self.update_state(id, STATE_SELECTED, selected);
    }

    /// ドラッグ（Dragged：スライダーノブやスプリッターのドラッグ中）状態を更新します。
    pub(crate) fn set_dragged(&mut self, id: EntityId, dragged: bool) {
        self.update_state(id, STATE_DRAGGED, dragged);
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

        // 状態マッピング解決のルックアップとループを1回に集約
        if let Some(interaction) = self.interaction_properties.get(id) {
            let cascade = [
                (STATE_FOCUSED, &interaction.focused),
                (STATE_SELECTED, &interaction.selected),
                (STATE_ACTIVED, &interaction.actived),
                (STATE_HOVERED, &interaction.hovered),
                (STATE_PRESSED, &interaction.pressed),
                (STATE_DISABLED, &interaction.disabled),
            ];

            for (state, style_opt) in cascade {
                if active_mask.has(state)
                    && let Some(style) = style_opt
                {
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

                // on_drag
                let mut on_drag = self
                    .event_listeners
                    .get_mut(pressed_id)
                    .and_then(|l| l.on_drag.take());
                if let Some(mut handler) = on_drag {
                    handler(self, delta);
                    if let Some(l) = self.event_listeners.get_mut(pressed_id) {
                        l.on_drag = Some(handler);
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
                if let Some(target_id) = current_hovered {
                    self.interaction_states.pressed = Some(target_id);
                    self.set_pressed(target_id, true);

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
                                handler(self);
                                if let Some(l) = self.event_listeners.get_mut(old_focus_id) {
                                    l.on_blur = Some(handler);
                                }
                            }
                        }

                        self.set_focused(target_id, true);

                        // on_focus
                        let mut on_focus = self
                            .event_listeners
                            .get_mut(target_id)
                            .and_then(|l| l.on_focus.take());
                        if let Some(mut handler) = on_focus {
                            handler(self);
                            if let Some(l) = self.event_listeners.get_mut(target_id) {
                                l.on_focus = Some(handler);
                            }
                        }
                        self.interaction_states.focused = Some(target_id);
                    }

                    // on_mouse_input
                    let mut on_input = self
                        .event_listeners
                        .get_mut(target_id)
                        .and_then(|l| l.on_mouse_input.take());
                    if let Some(mut handler) = on_input {
                        handler(self, button, modifiers, state);
                        if let Some(l) = self.event_listeners.get_mut(target_id) {
                            l.on_mouse_input = Some(handler);
                        }
                    }
                }
            }
            ElementState::Released => {
                if let Some(pressed_id) = self.interaction_states.pressed {
                    self.set_pressed(pressed_id, false);
                    self.set_dragged(pressed_id, false);
                    self.interaction_states.dragged = None;

                    // 1. on_mouse_input の発火（ボタンの種類を問わず常に呼ぶ）
                    let mut on_input = self
                        .event_listeners
                        .get_mut(pressed_id)
                        .and_then(|l| l.on_mouse_input.take());
                    if let Some(mut handler) = on_input {
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

    pub fn inject_mouse_wheel(&mut self, delta: f32) {
        let _context_guard = bind_context(self);
        if let Some(target_id) = self.interaction_states.hovered {
            let mut on_wheel = self
                .event_listeners
                .get_mut(target_id)
                .and_then(|l| l.on_mouse_wheel.take());
            if let Some(mut handler) = on_wheel {
                handler(self, delta);
                if let Some(l) = self.event_listeners.get_mut(target_id) {
                    l.on_mouse_wheel = Some(handler);
                }
            }
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
            let mut on_key = self
                .event_listeners
                .get_mut(focused_id)
                .and_then(|l| l.on_keyboard_input.take());
            if let Some(mut handler) = on_key {
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
                handler(self, paths);
                if let Some(l) = self.event_listeners.get_mut(target_id) {
                    l.on_file_dropped = Some(handler);
                }
            }
        }
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
}

#[cfg(test)]
mod tests;
