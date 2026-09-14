use crate::{
    ActiveEntitiesVec, ActiveMasksSecondary, BaseBasicLayoutsSecondary, BasicLayoutsSecondary,
    CapacityConfig, ChildrenSecondary, ComponentMask, Context, DebugStore, DirtyLayoutEntitiesVec,
    DirtyRenderEntitiesVec, Element, EntitiesSlot, EntityId, EventStore, FlexLayoutsSecondary,
    LayoutPoint, LayoutRect, LayoutStore, MichiuError, MichiuSoA, OptionTraceExt, ParentsSecondary,
    Pipeline, PointerEvents, Position, Rect, RectsSecondary, RenderStore, ResultTraceExt,
    SessionSpawnedVec, TaffyNodesSecondary, TaffyTreeEntityId, TopologyStore, Val,
    define_sparse_secondary, handle_on_dnd_drag_start, handle_on_dnd_entity_drag,
    handle_on_dnd_entity_drop, handle_on_dnd_id_drag, handle_on_dnd_id_drop, handle_on_drag,
};
use slotmap::SparseSecondaryMap;

/// プレースホルダーを挿入してマウントする親先祖の制御方法
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DndDragPlaceholderParent {
    Root,             // 自動的に最上位ルート要素の子としてアタッチ
    Custom(EntityId), // ユーザーが指定した特定の親コンテナの子としてアタッチ（範囲制限）
}

/// ドラッグ＆ドロップ動作の論理形式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DndDragPayload {
    /// Element 自体を移動する。
    /// ドロップ時に UI ツリーが自動的に更新される。
    Element,

    /// Element が持つ `EntityId` のみをドラッグデータとして転送する。
    /// UI ツリーは変更されず、アプリケーション側で並び替え等を行う。
    EntityId,
}

/// ドラッグ可能な要素が保持するスタイリング・動作設定
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DndDragProperty {
    pub placeholder_parent: DndDragPlaceholderParent,
    pub drag_mode: DndDragPayload,
    // ドラッグ終了時に自動的に配置（相対並び替え／絶対座標）を更新するか
    pub update_position: bool,
}

/// ドロップ受け入れ先での取り込み形式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DndDropTarget {
    Child,   // ドロップ先の子要素として取り込む
    Sibling, // ドロップ先の兄弟要素（隣接位置）として取り込む
}

/// ドロップ受け入れ先（ドロップゾーン）が保持するスタイリング・動作設定
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DndDropProperty {
    pub target: DndDropTarget,
    pub drag_mode: DndDragPayload,
}

#[derive(Debug, Clone)]
pub struct ActiveDragState {
    pub source_entity: EntityId,               // ドラッグ元の要素
    pub placeholder_entity: EntityId,          // ルートまたは親に浮かせているプレースホルダー
    pub current_drop_target: Option<EntityId>, // 現在ホバー侵入中のドロップターゲット要素
    pub start_mouse_pos: LayoutPoint,          // ドラッグ開始時のマウス座標
    pub start_rect: LayoutRect,                // ドラッグ元の初期サイズ・座標
    pub click_offset: LayoutPoint,             // ドラッグ開始時のマウスと要素左上端の相対的なズレ
    pub original_parent: Option<EntityId>,
}

/// プレースホルダーをアタッチする際の親要素の情報
#[derive(Debug, Clone)]
pub(crate) struct PlaceholderAttachment {
    pub(crate) parent_id: Option<EntityId>,
    pub(crate) rect: LayoutRect,
    pub(crate) border_left: f32,
    pub(crate) border_top: f32,
}

define_sparse_secondary!(pub struct DndDragPropertiesSparse(DndDragProperty));
define_sparse_secondary!(pub struct DndDropPropertiesSparse(DndDropProperty));

pub(crate) struct DndStore {
    pub(crate) dnd_drag_properties: DndDragPropertiesSparse,
    pub(crate) dnd_drop_properties: DndDropPropertiesSparse,
    pub(crate) dnd_active_drag_state: Option<ActiveDragState>,
}

impl Default for DndStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DndStore {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            dnd_drag_properties: DndDragPropertiesSparse(SparseSecondaryMap::new()),
            dnd_drop_properties: DndDropPropertiesSparse(SparseSecondaryMap::new()),
            dnd_active_drag_state: None,
        }
    }

    #[inline]
    #[must_use]
    pub fn with_capacity(c: &CapacityConfig) -> Self {
        Self {
            dnd_drag_properties: DndDragPropertiesSparse(SparseSecondaryMap::with_capacity(
                c.dnd_drag_properties,
            )),
            dnd_drop_properties: DndDropPropertiesSparse(SparseSecondaryMap::with_capacity(
                c.dnd_drop_properties,
            )),
            ..Default::default()
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.dnd_drag_properties.clear();
        self.dnd_drop_properties.clear();
        self.dnd_active_drag_state = None;
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.dnd_drag_properties.remove(id);
        self.dnd_drop_properties.remove(id);

        if let Some(ref state) = self.dnd_active_drag_state
            && (state.source_entity == id || state.placeholder_entity == id)
        {
            self.dnd_active_drag_state = None;
        }
    }
}

impl DndStore {
    fn resolve_dnd_placeholder_parent(
        root: EntityId,
        drag_prop: &DndDragProperty,
        lay_basic: &BasicLayoutsSecondary,
        out_rects: &RectsSecondary,
    ) -> PlaceholderAttachment {
        match drag_prop.placeholder_parent {
            DndDragPlaceholderParent::Root => PlaceholderAttachment {
                parent_id: Some(root),
                rect: *out_rects.at(root),
                border_left: 0.0,
                border_top: 0.0,
            },
            DndDragPlaceholderParent::Custom(p_id) => {
                let rect = *out_rects.at(p_id);
                let (border_left, border_top) = lay_basic
                    .find(p_id)
                    .map(|l| (l.border.left.to_px_or_zero(), l.border.top.to_px_or_zero()))
                    .unwrap_or_default();

                PlaceholderAttachment {
                    parent_id: Some(p_id),
                    rect,
                    border_left,
                    border_top,
                }
            }
        }
    }

    fn spawn_dnd_placeholder(
        root: EntityId,
        drag_prop: &DndDragProperty,
        topo_entities: &mut EntitiesSlot,
        topo_active_entities: &mut ActiveEntitiesVec,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &mut ParentsSecondary,
        topo_children: &mut ChildrenSecondary,
        topo_session_spawned: &mut SessionSpawnedVec,
        topo_is_structure_dirty: &mut bool,
        topo_is_sort_dirty: &mut bool,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &mut TaffyNodesSecondary,
        lay_basic: &BasicLayoutsSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        out_rects: &RectsSecondary,
        debug: &mut DebugStore,
    ) -> EntityId {
        let placeholder =
            DndStore::resolve_dnd_placeholder_parent(root, drag_prop, lay_basic, out_rects);

        let placeholder_id = TopologyStore::spawn(
            placeholder.parent_id,
            topo_entities,
            topo_active_entities,
            topo_active_masks,
            topo_parents,
            topo_children,
            topo_session_spawned,
            topo_is_structure_dirty,
            topo_is_sort_dirty,
            lay_taffy_tree,
            lay_taffy_nodes,
            rnd_dirty_entities,
            debug,
        );

        if let Some(p_id) = placeholder.parent_id {
            TopologyStore::add_child(
                p_id,
                placeholder_id,
                topo_active_masks,
                topo_parents,
                topo_children,
                topo_is_structure_dirty,
                topo_is_sort_dirty,
                lay_dirty_entities,
                lay_taffy_tree,
                lay_taffy_nodes,
                debug,
            );
        }

        placeholder_id
    }

    fn setup_placeholder_properties(
        cx: &mut Context,
        pressed_id: EntityId,
        placeholder_id: EntityId,
        start_rect: LayoutRect,
    ) {
        // 元要素のレイアウトおよびビジュアル情報をコピー
        let basic = *cx.layouts.lay_base_basic.at_mut(pressed_id);
        cx.layouts.lay_base_basic.insert(placeholder_id, basic);
        cx.layouts.lay_basic.insert(placeholder_id, basic);

        let visual = cx.renders.rnd_base_visual.at(pressed_id).clone();
        cx.renders
            .rnd_base_visual
            .insert(placeholder_id, visual.clone());
        cx.renders.rnd_visual.insert(placeholder_id, visual);

        let interaction = cx.renders.rnd_interaction.at(pressed_id).clone();
        cx.renders
            .rnd_interaction
            .insert(placeholder_id, interaction);

        // ドラッグ元とプレースホルダーの状態を同期
        Pipeline::update_state(cx, pressed_id, ComponentMask::STATE_DND_DRAGGING, true);
        Pipeline::update_state(cx, placeholder_id, ComponentMask::STATE_DND_DRAG_OVER, true);

        // プレースホルダー側を Absolute 配置化
        let basic = cx.layouts.lay_basic.get_mut(placeholder_id);
        let base_basic = cx.layouts.lay_base_basic.get_mut(placeholder_id);
        for layout in [basic, base_basic].into_iter().flatten() {
            layout.position = Position::Absolute;
            layout.size.width = Val::Px(start_rect.width);
            layout.size.height = Val::Px(start_rect.height);
        }

        // ヒットテストを透過
        let visual = cx.renders.rnd_visual.get_mut(placeholder_id);
        let base_visual = cx.renders.rnd_base_visual.get_mut(placeholder_id);
        for vis in [visual, base_visual].into_iter().flatten() {
            vis.pointer_events = Some(PointerEvents::None);
        }

        let mask = cx.topology.topo_active_masks.at_mut(placeholder_id);
        mask.set(ComponentMask::STYLE_POINTER_EVENTS);
    }

    fn transfer_children_to_placeholder(
        pressed_id: EntityId,
        placeholder_id: EntityId,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &mut ParentsSecondary,
        topo_children: &mut ChildrenSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
        debug: &mut DebugStore,
    ) {
        for child_id in topo_children.at(pressed_id).clone() {
            // 子要素の親ポインタをプレースホルダーに付け替え
            topo_parents.insert(child_id, Some(placeholder_id));

            // プレースホルダー側の子要素リストへ追加
            topo_children.at_mut(placeholder_id).push(child_id);

            // Taffy 側の親子構造も、一時的にプレースホルダーに繋ぎ替え
            let src_node = *lay_taffy_nodes.at(pressed_id);
            let ph_node = *lay_taffy_nodes.at(placeholder_id);
            let child_node = *lay_taffy_nodes.at(child_id);

            lay_taffy_tree
                .remove_child(src_node, child_node)
                .unwrap_or_trace(Some(child_id), debug);
            lay_taffy_tree
                .add_child(ph_node, child_node)
                .unwrap_or_trace(Some(child_id), debug);
        }

        // 元の要素の子要素リストは一時的にクリア（プレースホルダーに避難しているため）
        topo_children.at_mut(pressed_id).clear();

        // 元要素とプレースホルダー要素の両方をダーティマーク
        for id in [pressed_id, placeholder_id] {
            LayoutStore::mark_layout_dirty(
                id,
                topo_active_masks,
                topo_parents,
                lay_dirty_entities,
                lay_taffy_tree,
                lay_taffy_nodes,
                debug,
            );
        }
    }

    fn start_dnd_drag_session(cx: &mut Context, pressed_id: EntityId, logical_pos: LayoutPoint) {
        let drag_prop = *cx.states.dnd.dnd_drag_properties.at(pressed_id);
        let start_rect = *cx.outputs.out_rects.at(pressed_id);

        // 開始時のクリック位置と要素左上の相対的なズレを計算
        let click_offset =
            LayoutPoint::new(logical_pos.x - start_rect.x, logical_pos.y - start_rect.y);

        // ウィンドウのルート要素を自己解決
        let root = TopologyStore::find_root_entity(
            &cx.topology.topo_entities,
            &cx.topology.topo_parents,
            &cx.topology.topo_flat_dfs_sequence,
        )
        .unwrap_or_trace(None, &mut cx.debug, || MichiuError::RootEntityNotFound);

        // プレースホルダーをアタッチ先親の直下へ spawn して生成
        let placeholder_id = DndStore::spawn_dnd_placeholder(
            root,
            &drag_prop,
            &mut cx.topology.topo_entities,
            &mut cx.topology.topo_active_entities,
            &mut cx.topology.topo_active_masks,
            &mut cx.topology.topo_parents,
            &mut cx.topology.topo_children,
            &mut cx.topology.topo_session_spawned,
            &mut cx.topology.topo_is_structure_dirty,
            &mut cx.topology.topo_is_sort_dirty,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_taffy_tree,
            &mut cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_basic,
            &mut cx.renders.rnd_dirty_entities,
            &cx.outputs.out_rects,
            &mut cx.debug,
        );

        // プレースホルダーの初期スタイル・透過・状態情報をセットアップ
        DndStore::setup_placeholder_properties(cx, pressed_id, placeholder_id, start_rect);

        // 元の要素から子要素トポロジーをプレースホルダーへ移行
        DndStore::transfer_children_to_placeholder(
            pressed_id,
            placeholder_id,
            &mut cx.topology.topo_active_masks,
            &mut cx.topology.topo_parents,
            &mut cx.topology.topo_children,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_taffy_tree,
            &cx.layouts.lay_taffy_nodes,
            &mut cx.debug,
        );

        // プレースホルダーアタッチ前の、本当の元の親要素のIDを記録
        let original_parent = *cx.topology.topo_parents.at(pressed_id);

        // セッション開始
        cx.states.dnd.dnd_active_drag_state = Some(ActiveDragState {
            source_entity: pressed_id,
            placeholder_entity: placeholder_id,
            current_drop_target: None,
            start_mouse_pos: logical_pos,
            start_rect,
            click_offset,
            original_parent,
        });

        // ドラッグ開始コールバック
        handle_on_dnd_drag_start(
            cx,
            pressed_id,
            Element::from(pressed_id),
            Element::from(placeholder_id),
        );
    }

    pub(crate) fn propagate_dnd_drag_events(
        cx: &mut Context,
        prev_pos: Option<LayoutPoint>,
        logical_pos: LayoutPoint,
    ) {
        let Some(pressed_id) = cx.events.evt_interaction_states.pressed else {
            return;
        };
        let Some(prev) = prev_pos else {
            return;
        };

        let delta = LayoutPoint::new(logical_pos.x - prev.x, logical_pos.y - prev.y);
        if delta.x == 0.0 && delta.y == 0.0 {
            return;
        }

        Pipeline::update_state(cx, pressed_id, ComponentMask::STATE_DRAGGED, true);
        cx.events.evt_interaction_states.dragged = Some(pressed_id);

        // D&D 設定（STYLE_DRAGGABLE）を持っている場合のセッションのキック
        if cx
            .topology
            .topo_active_masks
            .at(pressed_id)
            .has(ComponentMask::STYLE_DND_DRAGGABLE)
            && cx.states.dnd.dnd_active_drag_state.is_none()
        {
            DndStore::start_dnd_drag_session(cx, pressed_id, logical_pos);
        }

        handle_on_drag(cx, pressed_id, delta);
    }

    fn calculate_dnd_relative_local(
        root: EntityId,
        drag_prop: &DndDragProperty,
        lay_basic: &BasicLayoutsSecondary,
        out_rects: &RectsSecondary,
    ) -> (LayoutRect, f32, f32) {
        let p = DndStore::resolve_dnd_placeholder_parent(root, drag_prop, lay_basic, out_rects);
        (p.rect, p.border_left, p.border_top)
    }

    pub(crate) fn update_inset_based_relative_local(
        root: EntityId,
        placeholder: EntityId,
        logical_pos: LayoutPoint,
        drag_prop: &DndDragProperty,
        drag_state: &ActiveDragState,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        out_rects: &RectsSecondary,
        debug: &mut DebugStore,
    ) {
        // アタッチ先親コンテナ基準での相対ローカル座標を逆算して追従（Inset更新）
        let (parent_rect, b_l, b_t) =
            DndStore::calculate_dnd_relative_local(root, drag_prop, lay_basic, out_rects);

        // マウスのドラッグ開始時クリックオフセットを用いて、ローカル Top-Left 座標を算出
        let local_x = logical_pos.x - (parent_rect.x + b_l) - drag_state.click_offset.x;
        let local_y = logical_pos.y - (parent_rect.y + b_t) - drag_state.click_offset.y;

        let basic = lay_basic.at_mut(placeholder);
        basic.inset.left = Val::Px(local_x);
        basic.inset.top = Val::Px(local_y);
        basic.inset.right = Val::Auto;
        basic.inset.bottom = Val::Auto;

        let base_basic = lay_base_basic.at_mut(placeholder);
        base_basic.inset.left = Val::Px(local_x);
        base_basic.inset.top = Val::Px(local_y);
        base_basic.inset.right = Val::Auto;
        base_basic.inset.bottom = Val::Auto;

        LayoutStore::mark_layout_dirty(
            placeholder,
            topo_active_masks,
            topo_parents,
            lay_dirty_entities,
            lay_taffy_tree,
            lay_taffy_nodes,
            debug,
        );
        RenderStore::mark_render_dirty(placeholder, topo_active_masks, rnd_dirty_entities);
    }

    pub(crate) fn detect_drop_target_during_intrusion(
        src_id: EntityId,
        hit_id: Option<EntityId>,
        placeholder: EntityId,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
    ) -> Option<EntityId> {
        let hit_id = hit_id?;
        // ヒットした要素がドラッグ元自身、またはその子孫である場合は、
        // 自身のサブツリーをすべてスキップするためにドラッグ元の親から探索を開始
        let is_descendant = TopologyStore::is_descendant_of(hit_id, src_id, topo_parents);
        let mut current_id = if hit_id == src_id || is_descendant {
            *topo_parents.at(src_id)
        } else {
            Some(hit_id)
        };

        while let Some(id) = current_id {
            let is_dnd = topo_active_masks
                .at(id)
                .has(ComponentMask::STYLE_DND_DROPPABLE);

            if id != placeholder && is_dnd {
                return Some(id); // ドロップ先を見つけたら即座に返す
            }
            current_id = *topo_parents.at(id);
        }

        None
    }

    pub(crate) fn sync_state_drag_in(cx: &mut Context, found_drop_target: Option<EntityId>) {
        let Some(mut drag_state) = cx.states.dnd.dnd_active_drag_state.take() else {
            return;
        };

        if found_drop_target == drag_state.current_drop_target {
            cx.states.dnd.dnd_active_drag_state = Some(drag_state);
            return;
        }

        if let Some(old_target) = drag_state.current_drop_target {
            Pipeline::update_state(cx, old_target, ComponentMask::STATE_DND_DRAG_IN, false);
        }
        if let Some(new_target) = found_drop_target {
            Pipeline::update_state(cx, new_target, ComponentMask::STATE_DND_DRAG_IN, true);
        }

        drag_state.current_drop_target = found_drop_target;
        cx.states.dnd.dnd_active_drag_state = Some(drag_state);
    }

    pub(crate) fn callback_drag_prop(
        cx: &mut Context,
        src_id: EntityId,
        found_drop_target: Option<EntityId>,
        drag_prop: &DndDragProperty,
    ) {
        match drag_prop.drag_mode {
            DndDragPayload::Element => {
                handle_on_dnd_entity_drag(
                    cx,
                    src_id,
                    Element::from(src_id),
                    found_drop_target.map(Element::from),
                );
            }
            DndDragPayload::EntityId => {
                handle_on_dnd_id_drag(cx, src_id, src_id, found_drop_target);
            }
        }
    }

    #[inline]
    fn remove_dragged_elemet(
        src_id: EntityId,
        drag_state: &ActiveDragState,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_children: &mut ChildrenSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
        debug: &mut DebugStore,
    ) {
        let Some(src_parent_id) = drag_state.original_parent else {
            return;
        };

        topo_children.at_mut(src_parent_id).retain(|x| *x != src_id);

        // 旧親側の Taffy 順序も再同期
        LayoutStore::resync_taffy_children_order(
            src_parent_id,
            topo_children,
            lay_taffy_tree,
            lay_taffy_nodes,
            debug,
        );
        LayoutStore::mark_layout_dirty(
            src_parent_id,
            topo_active_masks,
            topo_parents,
            lay_dirty_entities,
            lay_taffy_tree,
            lay_taffy_nodes,
            debug,
        );
    }

    fn dnd_rewrite_tree_topology(
        src_id: EntityId,
        target_id: EntityId,
        holder: EntityId,
        drag_prop: &DndDragProperty,
        evt_current_pointer_position: Option<LayoutPoint>,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &mut ParentsSecondary,
        topo_children: &mut ChildrenSecondary,
        topo_is_structure_dirty: &mut bool,
        topo_is_sort_dirty: &mut bool,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &mut TaffyNodesSecondary,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        out_rects: &RectsSecondary,
        debug: &mut DebugStore,
    ) {
        let basic = lay_basic.at_mut(src_id);

        // ドラッグ元要素の配置
        if basic.position == Position::Absolute {
            // 絶対配置: 位置移動（補正）を伴うアタッチ
            if drag_prop.update_position {
                // プレースホルダーの最終的な絶対画面座標を取得
                let ph_abs_rect = *out_rects.at(holder);
                // 新しい親（target_id）の絶対画面座標とボーダー厚みを取得
                let target_rect = *out_rects.at(target_id);

                let border = LayoutStore::get_physical_border(target_rect, basic.border);

                // 新しい親を基準にした新しいローカル相対位置を逆算して割り出す
                let new_inset_left = ph_abs_rect.x - (target_rect.x + border.left);
                let new_inset_top = ph_abs_rect.y - (target_rect.y + border.top);

                let new_inset = Rect {
                    top: Val::Px(new_inset_top),
                    right: Val::Auto,
                    bottom: Val::Auto,
                    left: Val::Px(new_inset_left),
                };

                basic.inset = new_inset;
                lay_base_basic.at_mut(src_id).inset = new_inset;
            }

            // ドロップ先コンテナ（target_id）の末尾の子要素としてマウント
            TopologyStore::add_child(
                target_id,
                src_id,
                topo_active_masks,
                topo_parents,
                topo_children,
                topo_is_structure_dirty,
                topo_is_sort_dirty,
                lay_dirty_entities,
                lay_taffy_tree,
                lay_taffy_nodes,
                debug,
            );

            return;
        }
        // 相対配置: マウス座標に基づいた子要素の動的並び替えアタッチ
        if drag_prop.update_position {
            let mouse_pos = evt_current_pointer_position.unwrap_or_default();
            let insert_idx = TopologyStore::calculate_insert_index(
                target_id,
                mouse_pos,
                topo_children,
                lay_flex,
                out_rects,
            );

            // 算出されたインデックス位置へ挿入
            topo_children.at_mut(target_id).insert(insert_idx, src_id);
            topo_parents.insert(src_id, Some(target_id));

            // Taffy 側のノード順序を物理並び替え結果に沿って一括して再同期
            LayoutStore::resync_taffy_children_order(
                target_id,
                topo_children,
                lay_taffy_tree,
                lay_taffy_nodes,
                debug,
            );
        } else {
            // 自動更新オフの場合は末尾に通常アタッチ
            TopologyStore::add_child(
                target_id,
                src_id,
                topo_active_masks,
                topo_parents,
                topo_children,
                topo_is_structure_dirty,
                topo_is_sort_dirty,
                lay_dirty_entities,
                lay_taffy_tree,
                lay_taffy_nodes,
                debug,
            );
        }
        LayoutStore::mark_layout_dirty(
            target_id,
            topo_active_masks,
            topo_parents,
            lay_dirty_entities,
            lay_taffy_tree,
            lay_taffy_nodes,
            debug,
        );
    }

    /// ドラッグ＆ドロップの終了・ドロップ確定処理
    pub(crate) fn handle_dnd_drop(cx: &mut Context, drag_state: &ActiveDragState) {
        let src_id = drag_state.source_entity;
        let holder = drag_state.placeholder_entity;

        let Some(drag_prop) = cx.states.dnd.dnd_drag_properties.find(src_id).copied() else {
            return;
        };

        // 疑似クラスの解除
        Pipeline::update_state(cx, src_id, ComponentMask::STATE_DND_DRAGGING, false);
        if let Some(target_id) = drag_state.current_drop_target {
            Pipeline::update_state(cx, target_id, ComponentMask::STATE_DND_DRAG_IN, false);
        }

        cx.events.evt_interaction_states.pressed = None;
        cx.events.evt_interaction_states.dragged = None;

        let drop_success = drag_state.current_drop_target;

        // トポロジー書き換え（要素移動時のみ）
        if let Some(target_id) = drop_success
            && drag_prop.drag_mode == DndDragPayload::Element
            && cx.states.dnd.dnd_drop_properties.contains(target_id)
        {
            DndStore::remove_dragged_elemet(
                src_id,
                drag_state,
                &mut cx.topology.topo_active_masks,
                &mut cx.topology.topo_children,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_taffy_tree,
                &cx.layouts.lay_taffy_nodes,
                &mut cx.debug,
            );
            DndStore::dnd_rewrite_tree_topology(
                src_id,
                target_id,
                holder,
                &drag_prop,
                cx.events.evt_current_pointer_position,
                &mut cx.topology.topo_active_masks,
                &mut cx.topology.topo_parents,
                &mut cx.topology.topo_children,
                &mut cx.topology.topo_is_structure_dirty,
                &mut cx.topology.topo_is_sort_dirty,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_taffy_tree,
                &mut cx.layouts.lay_taffy_nodes,
                &mut cx.layouts.lay_basic,
                &mut cx.layouts.lay_base_basic,
                &cx.layouts.lay_flex,
                &cx.outputs.out_rects,
                &mut cx.debug,
            );
            cx.topology.topo_is_structure_dirty = true;
            cx.topology.topo_is_sort_dirty = true;
        }

        // 子要素のツリー構造復元
        TopologyStore::restore_child(
            src_id,
            holder,
            &mut cx.topology.topo_parents,
            &mut cx.topology.topo_children,
            &mut cx.layouts.lay_taffy_tree,
            &mut cx.layouts.lay_taffy_nodes,
            &mut cx.debug,
        );
        cx.topology.topo_children.at_mut(holder).clear();

        for id in [src_id, holder] {
            LayoutStore::mark_layout_dirty(
                id,
                &mut cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_taffy_tree,
                &cx.layouts.lay_taffy_nodes,
                &mut cx.debug,
            );
        }

        match drag_prop.drag_mode {
            DndDragPayload::Element => {
                handle_on_dnd_entity_drop(
                    cx,
                    src_id,
                    Element::from(src_id),
                    drop_success.map(Element::from),
                );
            }
            DndDragPayload::EntityId => {
                handle_on_dnd_id_drop(cx, src_id, src_id, drop_success);
            }
        }

        // プレースホルダー破棄
        TopologyStore::despawn_internal(
            holder,
            &mut cx.window,
            &mut cx.system,
            &mut cx.reactive,
            &mut cx.events,
            &mut cx.contents,
            &mut cx.topology,
            &mut cx.states,
            &mut cx.layouts,
            &mut cx.renders,
            &mut cx.outputs,
            &mut cx.debug,
        );

        if let Some(pos) = cx.events.evt_current_pointer_position {
            EventStore::inject_pointer_move(cx, pos);
        }

        RenderStore::mark_render_dirty(
            src_id,
            &mut cx.topology.topo_active_masks,
            &mut cx.renders.rnd_dirty_entities,
        );
    }
}
