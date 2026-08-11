use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    ActiveMasksSecondary, ActiveTransitionsSparseSecondary, BaseVisualPropertiesSecondary,
    BasicLayout, ChildrenSecondary, ComponentMask, ContentStore, Context, DirtyRenderEntitiesVec,
    Display, DwriteLayoutsSparseSecondary, EdgeInsets, EntityId, FlexLayout, GridLayout,
    InputContentsSparseSecondary, InteractionPropertiesSecondary, InteractionStyles, LayoutPoint,
    LayoutRect, LayoutSize, Length, OutputStore, ParentsSecondary, Position, PropertyList, Rect,
    RectsSecondary, RenderStore, ResizeDirection, ResizingState, STATE_ACTIVED, STATE_DISABLED,
    STATE_DND_DRAG_IN, STATE_DND_DRAG_OVER, STATE_DND_DRAGGING, STATE_FOCUSED,
    STATE_FOCUSED_VISIBLE, STATE_HOVERED, STATE_PRESSED, STATE_QUEUED_LAYOUT, STATE_SELECTED,
    STYLE_SIZE, ScrollOffsetsSecondary, ScrollbarDisplay, ScrollbarMode, ScrollbarStyle, Size,
    StyleTarget, SystemStore, TextContentsSparseSecondary, TextEngine, TextSpansSparseSecondary,
    ThisStyle, TopologyStore, Val, VisualPropertiesSecondary, WindowStore,
};
use slotmap::{SecondaryMap, SparseSecondaryMap};
use smallvec::SmallVec;
use taffy::TaffyTree;

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default)]
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
    pub(crate) last_scroll_time: Option<std::time::Instant>,
}

pub(crate) type BasicLayoutsSecondary = SecondaryMap<EntityId, BasicLayout>;
pub(crate) type BaseBasicLayoutsSecondary = SecondaryMap<EntityId, BasicLayout>;
pub(crate) type FlexLayoutsSecondary = SecondaryMap<EntityId, FlexLayout>;
pub(crate) type GridLayoutsSecondary = SparseSecondaryMap<EntityId, GridLayout>;
pub(crate) type ScrollbarStylesSecondary = SparseSecondaryMap<EntityId, ScrollBarState>;
pub(crate) type TaffyNodesSecondary = SecondaryMap<EntityId, taffy::NodeId>;
pub(crate) type TaffyTreeEntityId = taffy::TaffyTree<EntityId>;
pub(crate) type DirtyLayoutEntitiesVec = Vec<EntityId>;

#[allow(clippy::struct_field_names)]
pub struct LayoutStore {
    pub(crate) lay_basic: BasicLayoutsSecondary,
    pub(crate) lay_base_basic: BaseBasicLayoutsSecondary,
    pub(crate) lay_flex: FlexLayoutsSecondary,
    pub(crate) lay_grid: GridLayoutsSecondary,
    pub(crate) lay_scrollbar_styles: ScrollbarStylesSecondary,
    pub(crate) lay_taffy_nodes: TaffyNodesSecondary,
    pub(crate) lay_taffy: TaffyTreeEntityId,
    pub(crate) lay_dirty_entities: DirtyLayoutEntitiesVec,
}

impl Default for LayoutStore {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutStore {
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self {
            lay_basic: SecondaryMap::new(),
            lay_base_basic: SecondaryMap::new(),
            lay_flex: SecondaryMap::new(),
            lay_grid: SparseSecondaryMap::new(),
            lay_scrollbar_styles: SparseSecondaryMap::new(),
            lay_taffy_nodes: SecondaryMap::new(),
            lay_taffy: TaffyTree::new(),
            lay_dirty_entities: Vec::new(),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.lay_basic.clear();
        self.lay_base_basic.clear();
        self.lay_flex.clear();
        self.lay_grid.clear();
        self.lay_scrollbar_styles.clear();
        self.lay_taffy_nodes.clear();
        self.lay_taffy = TaffyTree::new();
        self.lay_dirty_entities.clear();
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.lay_basic.remove(id);
        self.lay_base_basic.remove(id);
        self.lay_flex.remove(id);
        self.lay_grid.remove(id);
        self.lay_scrollbar_styles.remove(id);
        self.lay_taffy_nodes.remove(id);
        self.lay_dirty_entities.retain(|&x| x != id);
    }
}

impl LayoutStore {
    /// 各スタイルの解決を1回のルックアップと1回のカスケード解決ループに統合
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve_active_layouts(
        id: EntityId,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSecondary,
        ren_interaction: &InteractionPropertiesSecondary,
        ren_visual: &VisualPropertiesSecondary,
        ren_active_transitions: &ActiveTransitionsSparseSecondary,
    ) -> (BasicLayout, FlexLayout, Option<GridLayout>) {
        let mut basic = lay_basic.get(id).copied().unwrap_or_default();
        let mut flex = lay_flex.get(id).copied().unwrap_or_default();
        let mut grid = lay_grid.get(id).cloned();

        let active_mask = topo_active_masks[id];

        // 幅・高さ・一括サイズに対して、現在トランジションアニメーションが駆動中であるかを走査
        let (is_width_transitioning, is_height_transitioning) =
            LayoutStore::is_transition_currently_running(id, ren_active_transitions);

        // 自身、または親先祖から focused / focus_visible のフォーカス関連スタイルを解決
        let [focused_style_resolved, focused_visible_style_resolved] =
            [STATE_FOCUSED, STATE_FOCUSED_VISIBLE].map(|state| {
                RenderStore::resolv_focus_style(
                    id,
                    ren_interaction,
                    ren_visual,
                    &active_mask,
                    topo_parents,
                    state,
                )
            });

        // 状態マッピング解決のルックアップとループを1回に集約
        LayoutStore::apply_interaction_styles(
            id,
            &active_mask,
            &mut basic,
            &mut flex,
            &mut grid,
            &[focused_style_resolved, focused_visible_style_resolved],
            (is_width_transitioning, is_height_transitioning),
            ren_interaction,
        );

        (basic, flex, grid)
    }

    /// ターゲット状態に応じた `BasicLayout` の可変参照を引き出す
    pub(crate) fn get_basic_layout_mut<'a>(
        id: EntityId,
        target: StyleTarget,
        lay_base_basic: &'a mut BaseBasicLayoutsSecondary,
        ren_interaction: &'a mut InteractionPropertiesSecondary,
    ) -> Option<&'a mut BasicLayout> {
        if target == StyleTarget::Base {
            lay_base_basic.get_mut(id)
        } else {
            // ren_interaction から該当疑似クラスを安全に解決
            let styles = ren_interaction.get_mut(id)?;
            let style_ref = styles.get_style_target_mut(target);
            Some(&mut Arc::make_mut(&mut style_ref.inner).basic_layout)
        }
    }

    pub(crate) fn get_flex_layout_mut<'a>(
        id: EntityId,
        target: StyleTarget,
        lay_flex: &'a mut SecondaryMap<EntityId, FlexLayout>,
        ren_interaction: &'a mut InteractionPropertiesSecondary,
    ) -> Option<&'a mut FlexLayout> {
        if target == StyleTarget::Base {
            lay_flex.get_mut(id)
        } else {
            if !ren_interaction.contains_key(id) {
                ren_interaction.insert(id, InteractionStyles::default());
            }
            let styles = ren_interaction.get_mut(id).unwrap();
            let style_ref = styles.get_style_target_mut(target);
            Some(&mut Arc::make_mut(&mut style_ref.inner).flex_layout)
        }
    }

    pub(crate) fn is_transition_currently_running(
        id: EntityId,
        ren_active_transitions: &ActiveTransitionsSparseSecondary,
    ) -> (bool, bool) {
        let Some(list) = ren_active_transitions.get(id) else {
            return (false, false);
        };

        let mut w = false;
        let mut h = false;
        for t in list {
            match t.property_list {
                PropertyList::Width => w = true,
                PropertyList::Height => h = true,
                PropertyList::Size => {
                    w = true;
                    h = true;
                }
                _ => {}
            }
            if w && h {
                break;
            }
        }
        (w, h)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_interaction_styles(
        id: EntityId,
        active_mask: &ComponentMask,
        basic: &mut BasicLayout,
        flex: &mut FlexLayout,
        grid: &mut Option<GridLayout>,
        focused_resolved: &[Option<ThisStyle>; 2],
        is_transitioning: (bool, bool),
        ren_interaction: &InteractionPropertiesSecondary,
    ) {
        let Some(interaction) = ren_interaction.get(id) else {
            return;
        };

        let cascade = [
            (STATE_FOCUSED, &focused_resolved[0]),
            (STATE_FOCUSED_VISIBLE, &focused_resolved[1]),
            (STATE_SELECTED, &interaction.selected),
            (STATE_ACTIVED, &interaction.actived),
            (STATE_HOVERED, &interaction.hovered),
            (STATE_PRESSED, &interaction.pressed),
            (STATE_DISABLED, &interaction.disabled),
            (STATE_DND_DRAGGING, &interaction.dragging),
            (STATE_DND_DRAG_IN, &interaction.drag_in),
            (STATE_DND_DRAG_OVER, &interaction.drag_over),
        ];

        for (state, style_opt) in cascade {
            if !active_mask.has(state) {
                continue;
            }
            let Some(style) = style_opt else {
                continue;
            };

            let mut mask = style.inner.mask;
            if is_transitioning.0 || is_transitioning.1 {
                mask.unset(STYLE_SIZE);
            }

            basic.override_with(&style.inner.basic_layout, mask);
            flex.override_with(&style.inner.flex_layout, mask);

            if mask.has_grid_layout()
                && let Some(ref hover_grid) = style.inner.grid_layout
            {
                *grid = Some(hover_grid.clone());
            }
        }
    }

    // Taffyスタイルを一括解決するヘルパー
    pub(crate) fn resolve_taffy_style(
        id: EntityId,
        basic: &BasicLayout,
        flex: &FlexLayout,
        grid: Option<&GridLayout>,
        lay_scrollbar_styles: &ScrollbarStylesSecondary,
    ) -> taffy::Style {
        let sb_style = lay_scrollbar_styles.get(id).map(|s| &s.style);

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
            align_items: flex.align_items.map(taffy::AlignItems::from),
            align_self: flex.align_self.map(taffy::AlignItems::from),
            justify_items: flex.justify_items.map(taffy::AlignItems::from),
            justify_self: flex.justify_self.map(taffy::AlignItems::from),
            align_content: flex.align_content.map(taffy::AlignContent::from),
            justify_content: flex.justify_content.map(taffy::AlignContent::from),
            gap: flex.gap.into(),
            flex_direction: flex.flex_direction.into(),
            flex_wrap: flex.flex_wrap.into(),
            flex_basis: flex.flex_basis.into(),
            flex_grow: flex.flex_grow,
            flex_shrink: flex.flex_shrink,
            scrollbar_width: if let Some(sb) = sb_style
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
            style.grid_template_rows.clone_from(&g.grid_template_rows);
            style
                .grid_template_columns
                .clone_from(&g.grid_template_columns);
            style.grid_auto_rows.clone_from(&g.grid_auto_rows);
            style.grid_auto_columns.clone_from(&g.grid_auto_columns);
            style.grid_auto_flow = g.grid_auto_flow.into();
            style.grid_template_areas.clone_from(&g.grid_template_areas);
            style
                .grid_template_column_names
                .clone_from(&g.grid_template_column_names);
            style
                .grid_template_row_names
                .clone_from(&g.grid_template_row_names);
            style.grid_row = g.grid_row.clone().into();
            style.grid_column = g.grid_column.clone().into();
        }
        style
    }

    /// 指定された要素の現在解決されている物理ボーダー（EdgeInsets）を取得します。
    #[inline]
    pub(crate) fn get_physical_border(rect: LayoutRect, border: Rect<Length>) -> EdgeInsets {
        EdgeInsets {
            top: LayoutStore::length_to_px(border.top, rect.height),
            right: LayoutStore::length_to_px(border.right, rect.width),
            bottom: LayoutStore::length_to_px(border.bottom, rect.height),
            left: LayoutStore::length_to_px(border.left, rect.width),
        }
    }

    /// 指定された要素の現在解決されている物理パディング（EdgeInsets）を取得します。
    #[inline]
    pub(crate) fn get_physical_padding(rect: LayoutRect, padding: Rect<Length>) -> EdgeInsets {
        EdgeInsets {
            top: LayoutStore::length_to_px(padding.top, rect.height),
            right: LayoutStore::length_to_px(padding.right, rect.width),
            bottom: LayoutStore::length_to_px(padding.bottom, rect.height),
            left: LayoutStore::length_to_px(padding.left, rect.width),
        }
    }

    #[inline]
    pub(crate) fn get_physical_border_padding(
        rect: LayoutRect,
        border: Rect<Length>,
        padding: Rect<Length>,
    ) -> (EdgeInsets, EdgeInsets) {
        let border = LayoutStore::get_physical_border(rect, border);
        let padding = LayoutStore::get_physical_padding(rect, padding);

        (border, padding)
    }

    /// 実際の可視サイズから、物理ボーダーとパディングの厚みを引いた内枠の有効表示可能サイズを算出します。
    #[inline]
    pub(crate) fn calculate_inner_content_size(
        visible_size: LayoutSize,
        border: EdgeInsets,
        padding: EdgeInsets,
    ) -> LayoutSize {
        let content_w =
            (visible_size.width - border.left - border.right - padding.left - padding.right)
                .max(0.0);
        let content_h =
            (visible_size.height - border.top - border.bottom - padding.top - padding.bottom)
                .max(0.0);

        LayoutSize::new(content_w, content_h)
    }

    #[inline]
    pub(crate) fn calculate_viewport_size(
        visible_size: LayoutRect,
        border: EdgeInsets,
        padding: EdgeInsets,
    ) -> LayoutSize {
        let content_w =
            (visible_size.width - border.left - border.right - padding.left - padding.right)
                .max(0.0);
        let content_h =
            (visible_size.height - border.top - border.bottom - padding.top - padding.bottom)
                .max(0.0);

        LayoutSize::new(content_w, content_h)
    }

    #[inline]
    fn length_to_px(length: Length, reference: f32) -> f32 {
        match length {
            Length::Px(v) => v,
            Length::Percent(p) => reference * (p / 100.0),
        }
    }

    // 全スクロールバー関連IDを一括抽出
    #[inline]
    pub(crate) fn scrollbar_el_ids(
        lay_scrollbar_styles: &SparseSecondaryMap<EntityId, ScrollBarState>,
    ) -> HashSet<EntityId> {
        lay_scrollbar_styles
            .values()
            .flat_map(|sb_state| {
                [
                    sb_state.v_track_id,
                    sb_state.v_thumb_id,
                    sb_state.h_track_id,
                    sb_state.h_thumb_id,
                ]
                .into_iter()
                .flatten()
            })
            .collect()
    }

    /// スクロールバー用要素のレイアウト情報を同期して更新。
    pub(crate) fn update_scrollbar_element_layout(
        id: EntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        size: Size<Val>,
        inset: Rect<Val>,
    ) {
        let layouts = [lay_basic.get_mut(id), lay_base_basic.get_mut(id)];

        for layout in layouts.into_iter().flatten() {
            layout.display = Display::Flex;
            layout.size = size;
            layout.inset = inset;
        }

        // affy 側のノードスタイルも Display::None にして同期
        let _ = lay_taffy.set_style(
            lay_taffy_nodes[id],
            taffy::Style {
                display: taffy::Display::None,
                ..Default::default()
            },
        );
    }

    /// スクロールバー用要素（TrackやThumb）のレイアウト、不透明度、Taffyスタイルへの反映を一括して同期更新します。
    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub(crate) fn update_scrollbar_element(
        id: EntityId,
        size: Size<Val>,
        inset: Rect<Val>,
        opacity: f32,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSecondary,
        lay_scrollbar_styles: &ScrollbarStylesSecondary,
        ren_visual: &mut VisualPropertiesSecondary,
        ren_base_visual: &mut BaseVisualPropertiesSecondary,
        ren_interaction: &InteractionPropertiesSecondary,
        ren_active_transitions: &ActiveTransitionsSparseSecondary,
    ) {
        LayoutStore::update_scrollbar_element_layout(
            id,
            lay_basic,
            lay_base_basic,
            lay_taffy_nodes,
            lay_taffy,
            size,
            inset,
        );
        RenderStore::update_scrollbar_element_opacity(id, ren_visual, ren_base_visual, opacity);

        let (basic, flex, grid) = LayoutStore::resolve_active_layouts(
            id,
            topo_active_masks,
            topo_parents,
            lay_basic,
            lay_flex,
            lay_grid,
            ren_interaction,
            ren_visual,
            ren_active_transitions,
        );

        LayoutStore::set_taffy_style(
            id,
            &basic,
            &flex,
            grid.as_ref(),
            lay_taffy,
            lay_taffy_nodes,
            lay_scrollbar_styles,
        );
    }

    /// 解決済みの基本スタイルを `TaffyTree` のノードへ同期して適用。
    #[inline]
    pub(crate) fn set_taffy_style(
        id: EntityId,
        basic: &BasicLayout,
        flex: &FlexLayout,
        grid: Option<&GridLayout>,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_scrollbar_styles: &ScrollbarStylesSecondary,
    ) {
        let taffy_style =
            LayoutStore::resolve_taffy_style(id, basic, flex, grid, lay_scrollbar_styles);
        let _ = lay_taffy.set_style(lay_taffy_nodes[id], taffy_style);
    }

    /// スクロールバー用要素をレイアウト上から安全に隠します。
    #[inline]
    pub(crate) fn hide_scrollbar_element(
        id: EntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
    ) {
        let layouts = [lay_basic.get_mut(id), lay_base_basic.get_mut(id)];

        for layout in layouts.into_iter().flatten() {
            layout.display = Display::None;
        }
    }

    pub(crate) fn local_rect_from_taffy(
        id: EntityId,
        lay_taffy: &TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
    ) -> LayoutRect {
        let Some(&taffy_node) = lay_taffy_nodes.get(id) else {
            return LayoutRect::ZERO;
        };

        let Ok(layout) = lay_taffy.layout(taffy_node) else {
            return LayoutRect::ZERO;
        };

        LayoutRect::new(
            layout.location.x,
            layout.location.y,
            layout.size.width,
            layout.size.height,
        )
    }

    /// 指定された親コンテナにアタッチされている `DComp` / Taffy 側のすべての子ノードの物理順序を
    /// 内部 `SoA` リスト（self.children）の順序に沿って再同期。
    pub(crate) fn resync_taffy_children_order(
        parent_id: EntityId,
        topo_children: &ChildrenSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
    ) {
        let Some(&parent_node) = lay_taffy_nodes.get(parent_id) else {
            return;
        };
        // 一旦現在登録されているすべての子ノードを Taffy 側から安全にデタッチ
        if let Ok(taffy_children) = lay_taffy.children(parent_node) {
            for child_node in taffy_children {
                let _ = lay_taffy.remove_child(parent_node, child_node);
            }
        }
        // 最新の並び替え順序リストの存在チェック
        let Some(children_list) = topo_children.get(parent_id) else {
            return;
        };

        // 最新の順序に従って、Taffy 側に再アタッチ
        for &child_id in children_list {
            let Some(&child_node) = lay_taffy_nodes.get(child_id) else {
                continue;
            };
            let _ = lay_taffy.add_child(parent_node, child_node);
        }
    }

    pub fn clear_layout_dirty(
        topo_active_masks: &mut ActiveMasksSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
    ) {
        for id in lay_dirty_entities.drain(..) {
            if let Some(mask) = topo_active_masks.get_mut(id) {
                mask.unset(STATE_QUEUED_LAYOUT);
            }
        }
        lay_dirty_entities.clear();
    }

    #[inline]
    pub(crate) fn mark_layout_dirty(
        id: EntityId,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_nodes: &TaffyNodesSecondary,
    ) {
        let mut curr = id;
        // Taffy 側の該当ノードのレイアウトキャッシュを無効化
        if let Some(&taffy_node) = lay_taffy_nodes.get(curr) {
            let _ = lay_taffy.mark_dirty(taffy_node);
        }

        loop {
            // マスクが存在する場合のみDirtyマーク
            if let Some(mask) = topo_active_masks.get_mut(curr) {
                // すでに登録済みなら多重登録を防ぐため探索を早期ブレイク
                if mask.has(STATE_QUEUED_LAYOUT) {
                    break;
                }
                mask.set(STATE_QUEUED_LAYOUT); // 自身を Dirty マーク
                lay_dirty_entities.push(curr);
            }

            // 親要素（先祖）をルートまで辿って Dirty フラグを連鎖伝播させる
            let Some(Some(parent_id)) = topo_parents.get(curr).copied() else {
                break;
            };
            curr = parent_id;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn sync_resizing_drag(
        logical_pos: LayoutPoint,
        state: &ResizingState,
        win_last_size: Option<&LayoutSize>,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_nodes: &TaffyNodesSecondary,
        ren_dirty_entities: &mut DirtyRenderEntitiesVec,
        out_rects: &RectsSecondary,
    ) {
        let id = state.entity_id;
        let delta_x = logical_pos.x - state.start_mouse_pos.x;
        let delta_y = logical_pos.y - state.start_mouse_pos.y;

        let start_rect = state.start_rect;
        let position = lay_basic.get(id).map(|l| l.position).unwrap_or_default();

        // 最小サイズ・最大クランプ値の解決
        let (min_w, max_w, min_h, max_h) = {
            let basic = lay_basic.get(id).copied().unwrap_or_default();

            let rect = OutputStore::rect(id, out_rects).unwrap_or_default();
            let (border, padding) =
                LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

            // 枠線と余白を足した、物理的にこれ以上小さくできない限界サイズ
            let abs_min_w = border.left + border.right + padding.left + padding.right;
            let abs_min_h = border.top + border.bottom + padding.top + padding.bottom;

            // 指定値を物理ピクセルに解決する
            let resolve_val = |val: Val, is_width: bool, fallback: f32| match val {
                Val::Px(v) => v,
                Val::Percent(_) => OutputStore::val_to_px(
                    id,
                    val,
                    is_width,
                    topo_parents,
                    out_rects,
                    win_last_size,
                )
                .unwrap_or(fallback),
                Val::Auto => fallback,
            };

            let user_min_w = resolve_val(basic.min_size.width, true, 0.0);
            let user_min_h = resolve_val(basic.min_size.height, false, 0.0);
            let user_max_w = resolve_val(basic.max_size.width, true, f32::MAX);
            let user_max_h = resolve_val(basic.max_size.height, false, f32::MAX);

            (
                abs_min_w.max(user_min_w).max(10.0), // 最低限 10px は維持
                user_max_w,
                abs_min_h.max(user_min_h).max(10.0),
                user_max_h,
            )
        };

        // ドラッグ方向に基づく係数マップ
        // 1.0 は引っ張り（サイズ増加）、-1.0 は押し込み（サイズ減少）
        let (h_factor, v_factor) = match state.direction {
            ResizeDirection::Left => (Some(-1.0), None),
            ResizeDirection::Right => (Some(1.0), None),
            ResizeDirection::Top => (None, Some(-1.0)),
            ResizeDirection::Bottom => (None, Some(1.0)),
            ResizeDirection::TopLeft => (Some(-1.0), Some(-1.0)),
            ResizeDirection::TopRight => (Some(1.0), Some(-1.0)),
            ResizeDirection::BottomLeft => (Some(-1.0), Some(1.0)),
            ResizeDirection::BottomRight => (Some(1.0), Some(1.0)),
        };

        let mut new_w = start_rect.width;
        let mut new_h = start_rect.height;
        let mut delta_inset_left = 0.0;
        let mut delta_inset_top = 0.0;

        if let Some(factor) = h_factor {
            new_w = (start_rect.width + delta_x * factor).clamp(min_w, max_w);
            // 絶対配置（Absolute）で、左側へサイズを伸ばした（縮めた）場合はインセットを同期補正
            if position == Position::Absolute && factor < 0.0 {
                delta_inset_left = start_rect.width - new_w;
            }
        }

        if let Some(factor) = v_factor {
            new_h = (start_rect.height + delta_y * factor).clamp(min_h, max_h);
            // 絶対配置（Absolute）で、上側へサイズを伸ばした（縮めた）場合はインセットを同期補正
            if position == Position::Absolute && factor < 0.0 {
                delta_inset_top = start_rect.height - new_h;
            }
        }

        let layouts = [lay_basic.get_mut(id), lay_base_basic.get_mut(id)];

        for layout in layouts.into_iter().flatten() {
            layout.size.width = Val::Px(new_w);
            layout.size.height = Val::Px(new_h);

            if position != Position::Absolute {
                continue;
            }

            if let Val::Px(start_top) = state.start_inset.top {
                layout.inset.top = Val::Px(start_top + delta_inset_top);
            }
            if let Val::Px(start_left) = state.start_inset.left {
                layout.inset.left = Val::Px(start_left + delta_inset_left);
            }
            layout.inset.right = Val::Auto;
            layout.inset.bottom = Val::Auto;
        }

        LayoutStore::mark_layout_dirty(
            id,
            topo_active_masks,
            topo_parents,
            lay_taffy,
            lay_dirty_entities,
            lay_taffy_nodes,
        );
        RenderStore::mark_render_dirty(id, topo_active_masks, ren_dirty_entities);
    }
}

// つまみ（Thumb）計算用の入力パラメータ
pub(crate) struct ExtractedThumb {
    initial_len: f32,
    min_len: f32,
    max_len: f32,
    margin_start: f32,
    margin_end: f32,
    pad_start: f32,
    pad_end: f32,
    cross_size_override: Option<f32>,
}

pub(crate) struct ScrollbarSyncContext<'a> {
    pub topo_active_masks: &'a ActiveMasksSecondary,
    pub topo_parents: &'a ParentsSecondary,
    pub lay_taffy_nodes: &'a TaffyNodesSecondary,
    pub lay_taffy: &'a mut TaffyTreeEntityId,
    pub lay_basic: &'a mut BasicLayoutsSecondary,
    pub lay_base_basic: &'a mut BaseBasicLayoutsSecondary,
    pub lay_flex: &'a FlexLayoutsSecondary,
    pub lay_grid: &'a GridLayoutsSecondary,
    pub lay_scrollbar_styles: &'a ScrollbarStylesSecondary,
    pub ren_active_transitions: &'a ActiveTransitionsSparseSecondary,
    pub ren_visual: &'a mut VisualPropertiesSecondary,
    pub ren_base_visual: &'a mut BaseVisualPropertiesSecondary,
    pub ren_interaction: &'a InteractionPropertiesSecondary,
}

impl ScrollbarSyncContext<'_> {
    #[inline]
    pub fn update_el(&mut self, el_id: EntityId, size: Size<Val>, rect: Rect<Val>, opacity: f32) {
        LayoutStore::update_scrollbar_element(
            el_id,
            size,
            rect,
            opacity,
            self.topo_active_masks,
            self.topo_parents,
            self.lay_taffy,
            self.lay_basic,
            self.lay_base_basic,
            self.lay_taffy_nodes,
            self.lay_flex,
            self.lay_grid,
            self.lay_scrollbar_styles,
            self.ren_visual,
            self.ren_base_visual,
            self.ren_interaction,
            self.ren_active_transitions,
        );
    }
    #[inline]
    pub fn hide_el(&mut self, el_id: EntityId) {
        LayoutStore::hide_scrollbar_element(el_id, self.lay_basic, self.lay_base_basic);
    }
}

impl LayoutStore {
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(crate) fn sync_scrollbar_styles(
        win_last_size: Option<LayoutSize>,
        sys_text_engine: &TextEngine,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        cont_input_contents: &InputContentsSparseSecondary,
        cont_text_contents: &TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_scrollbar_styles: &ScrollbarStylesSecondary,
        ren_visual: &mut VisualPropertiesSecondary,
        ren_base_visual: &mut BaseVisualPropertiesSecondary,
        ren_active_transitions: &ActiveTransitionsSparseSecondary,
        ren_interaction: &InteractionPropertiesSecondary,
        out_rects: &RectsSecondary,
        out_scroll_offsets: &ScrollOffsetsSecondary,
    ) {
        let scrollbar_ids: Vec<EntityId> = lay_scrollbar_styles.keys().collect();

        for id in scrollbar_ids {
            let sb_state = lay_scrollbar_styles.get(id).cloned().unwrap();
            let container_rect = out_rects[id];
            let scroll_size = OutputStore::get_scroll_size(
                id,
                topo_active_masks,
                cont_input_contents,
                sys_text_engine,
                cont_text_contents,
                ren_visual,
                cont_text_spans,
                sys_dwrite_layouts,
                lay_basic,
                lay_flex,
                lay_grid,
                ren_active_transitions,
                topo_parents,
                topo_children,
                ren_interaction,
                out_rects,
                lay_scrollbar_styles,
                out_scroll_offsets,
            );
            let current_scroll = out_scroll_offsets.get(id).copied().unwrap_or_default();

            let (basic, _, _) = LayoutStore::resolve_active_layouts(
                id,
                topo_active_masks,
                topo_parents,
                lay_basic,
                lay_flex,
                lay_grid,
                ren_interaction,
                ren_visual,
                ren_active_transitions,
            );
            let rect = OutputStore::rect(id, out_rects).unwrap_or_default();
            let (border, padding) =
                LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

            let visible_size = WindowStore::calculate_visible_size(win_last_size, container_rect);
            let content_size =
                LayoutStore::calculate_inner_content_size(visible_size, border, padding);

            let show_v = scroll_size.height > content_size.height;
            let show_h = scroll_size.width > content_size.width;

            // トラックデフォルト長の計算
            let track_h_default = LayoutStore::calculate_track_len(
                visible_size.height,
                border.top,
                border.bottom,
                show_h,
                sb_state.style.width,
            );
            let track_w = LayoutStore::calculate_track_len(
                visible_size.width,
                border.left,
                border.right,
                show_v,
                sb_state.style.width,
            );

            let mut ctx = ScrollbarSyncContext {
                lay_taffy_nodes,
                lay_taffy,
                lay_basic,
                lay_base_basic,
                lay_flex,
                lay_grid,
                topo_active_masks,
                ren_active_transitions,
                topo_parents,
                ren_interaction,
                ren_visual,
                ren_base_visual,
                lay_scrollbar_styles,
            };

            // 縦トラック (V-Track) の同期
            if let Some(v_track) = sb_state.v_track_id {
                let (visible, opacity) = LayoutStore::calculate_visibility_opacity(
                    sb_state.style.display,
                    sb_state.last_scroll_time,
                    show_v,
                );
                if visible {
                    let mut track_h = track_h_default;
                    if let Some(ref track_style) = sb_state.style.v_track
                        && let Val::Px(val) = track_style.inner.basic_layout.size.height
                    {
                        track_h = val;
                    }
                    ctx.update_el(
                        v_track,
                        Size::new(Val::Px(sb_state.style.width), Val::Px(track_h)),
                        Rect::new(Val::Px(0.0), Val::Px(0.0), Val::Auto, Val::Auto),
                        opacity,
                    );
                } else {
                    ctx.hide_el(v_track);
                }
            }

            // 縦つまみ (V-Thumb) の同期
            if let Some(v_thumb) = sb_state.v_thumb_id {
                let (visible, opacity) = LayoutStore::calculate_visibility_opacity(
                    sb_state.style.display,
                    sb_state.last_scroll_time,
                    show_v,
                );
                if visible {
                    let mut ext = ExtractedThumb {
                        initial_len: track_h_default,
                        min_len: 24.0,
                        max_len: track_h_default,
                        margin_start: 0.0,
                        margin_end: 0.0,
                        pad_start: 0.0,
                        pad_end: 0.0,
                        cross_size_override: None,
                    };

                    if let Some(ref thumb_style) = sb_state.style.v_thumb {
                        let layout = &thumb_style.inner.basic_layout;
                        if let Val::Px(val) = layout.size.height {
                            ext.initial_len = val;
                        }
                        if let Val::Px(val) = layout.min_size.height {
                            ext.min_len = val;
                        }
                        if let Val::Px(val) = layout.max_size.height {
                            ext.max_len = val;
                        }
                        if let Val::Px(val) = layout.margin.top {
                            ext.margin_start = val;
                        }
                        if let Val::Px(val) = layout.margin.bottom {
                            ext.margin_end = val;
                        }
                        if let Length::Px(val) = layout.padding.left {
                            ext.pad_start = val;
                        }
                        if let Length::Px(val) = layout.padding.right {
                            ext.pad_end = val;
                        }
                        if let Val::Px(val) = layout.size.width {
                            ext.cross_size_override = Some(val);
                        }
                    }

                    let (h, y, w, x) = LayoutStore::calculate_thumb_geometry(
                        track_h_default,
                        visible_size.height,
                        scroll_size.height,
                        current_scroll.y,
                        sb_state.style.width,
                        &ext,
                    );

                    ctx.update_el(
                        v_thumb,
                        Size::new(Val::Px(w), Val::Px(h)),
                        Rect::new(Val::Px(y), Val::Auto, Val::Auto, Val::Px(x)),
                        opacity,
                    );
                } else {
                    ctx.hide_el(v_thumb);
                }
            }

            // 横トラック (H-Track) の同期
            if let Some(h_track) = sb_state.h_track_id {
                let (visible, opacity) = LayoutStore::calculate_visibility_opacity(
                    sb_state.style.display,
                    sb_state.last_scroll_time,
                    show_h,
                );
                if visible {
                    ctx.update_el(
                        h_track,
                        Size::new(Val::Px(track_w), Val::Px(sb_state.style.width)),
                        Rect::new(Val::Auto, Val::Auto, Val::Px(0.0), Val::Px(0.0)),
                        opacity,
                    );
                } else {
                    ctx.hide_el(h_track);
                }
            }

            // 横つまみ (H-Thumb) の同期
            if let Some(h_thumb) = sb_state.h_thumb_id {
                let (visible, opacity) = LayoutStore::calculate_visibility_opacity(
                    sb_state.style.display,
                    sb_state.last_scroll_time,
                    show_h,
                );
                if visible {
                    let mut ext = ExtractedThumb {
                        initial_len: track_w,
                        min_len: 24.0,
                        max_len: track_w,
                        margin_start: 0.0,
                        margin_end: 0.0,
                        pad_start: 0.0,
                        pad_end: 0.0,
                        cross_size_override: None,
                    };

                    if let Some(ref thumb_style) = sb_state.style.h_thumb {
                        let layout = &thumb_style.inner.basic_layout;
                        if let Val::Px(val) = layout.size.width {
                            ext.initial_len = val;
                        }
                        if let Val::Px(val) = layout.min_size.width {
                            ext.min_len = val;
                        }
                        if let Val::Px(val) = layout.max_size.width {
                            ext.max_len = val;
                        }
                        if let Val::Px(val) = layout.margin.left {
                            ext.margin_start = val;
                        }
                        if let Val::Px(val) = layout.margin.right {
                            ext.margin_end = val;
                        }
                        if let Length::Px(val) = layout.padding.top {
                            ext.pad_start = val;
                        }
                        if let Length::Px(val) = layout.padding.bottom {
                            ext.pad_end = val;
                        }
                        if let Val::Px(val) = layout.size.height {
                            ext.cross_size_override = Some(val);
                        }
                    }

                    let (w, x, h, y) = LayoutStore::calculate_thumb_geometry(
                        track_w,
                        visible_size.width,
                        scroll_size.width,
                        current_scroll.x,
                        sb_state.style.width,
                        &ext,
                    );

                    ctx.update_el(
                        h_thumb,
                        Size::new(Val::Px(w), Val::Px(h)),
                        Rect::new(Val::Px(y), Val::Auto, Val::Auto, Val::Px(x)),
                        opacity,
                    );
                } else {
                    ctx.hide_el(h_thumb);
                }
            }
        }
    }

    // 表示状態と不透明度の計算
    #[inline]
    fn calculate_visibility_opacity(
        display: ScrollbarDisplay,
        last_scroll_time: Option<Instant>,
        show_bar: bool,
    ) -> (bool, f32) {
        if !show_bar || display == ScrollbarDisplay::None {
            return (false, 1.0);
        }
        match display {
            ScrollbarDisplay::Always | ScrollbarDisplay::Auto => (true, 1.0),
            ScrollbarDisplay::Transient => {
                let Some(last) = last_scroll_time else {
                    return (false, 1.0);
                };

                let elapsed = last.elapsed();
                if elapsed < Duration::from_secs(1) {
                    (true, 1.0)
                } else if elapsed < Duration::from_millis(1500) {
                    let opacity = 1.0 - (elapsed.as_secs_f32() - 1.0) / 0.5;
                    (true, opacity)
                } else {
                    (false, 1.0)
                }
            }
            ScrollbarDisplay::None => (false, 1.0),
        }
    }

    // トラック有効長の計算
    #[inline]
    fn calculate_track_len(
        visible_dim: f32,
        border_start: f32,
        border_end: f32,
        show_other: bool,
        scrollbar_width: f32,
    ) -> f32 {
        let extra = if show_other { scrollbar_width } else { 0.0 };
        (visible_dim - border_start - border_end - extra).max(0.0)
    }

    // つまみの物理サイズと位置の計算
    #[inline]
    fn calculate_thumb_geometry(
        track_len: f32,
        visible_len: f32,
        scroll_len: f32,
        current_scroll_val: f32,
        scrollbar_width: f32,
        ext: &ExtractedThumb,
    ) -> (f32, f32, f32, f32) {
        let view_ratio = if scroll_len > 0.0 {
            (visible_len / scroll_len).min(1.0)
        } else {
            1.0
        };

        let calculated_len = ext.initial_len * view_ratio;
        let thumb_len = calculated_len
            .max(ext.min_len)
            .min(ext.max_len)
            .min(track_len);

        let scroll_ratio = if scroll_len > visible_len {
            (current_scroll_val / (scroll_len - visible_len)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let max_thumb_pos = (track_len - thumb_len - ext.margin_start - ext.margin_end).max(0.0);
        let thumb_main_pos = max_thumb_pos * scroll_ratio;

        let thumb_cross_len = if let Some(w) = ext.cross_size_override {
            w.min(scrollbar_width)
        } else {
            scrollbar_width
        };

        let thumb_cross_pos = if ext.pad_end > 0.0 {
            scrollbar_width - thumb_cross_len - ext.pad_end
        } else if ext.pad_start > 0.0 {
            ext.pad_start
        } else {
            (scrollbar_width - thumb_cross_len) * 0.5
        };

        (thumb_len, thumb_main_pos, thumb_cross_len, thumb_cross_pos)
    }
}

impl Context {
    #[inline]
    pub(crate) fn mark_layout_dirty(&mut self, id: EntityId) {
        let LayoutStore {
            lay_taffy_nodes,
            lay_taffy,
            lay_dirty_entities,
            ..
        } = &mut self.layouts;
        let TopologyStore {
            topo_active_masks,
            topo_parents,
            ..
        } = &mut self.topology;

        LayoutStore::mark_layout_dirty(
            id,
            topo_active_masks,
            topo_parents,
            lay_taffy,
            lay_dirty_entities,
            lay_taffy_nodes,
        );
    }

    /// 各スタイルの解決を1回のルックアップと1回のカスケード解決ループに統合
    #[inline]
    pub(crate) fn resolve_active_layouts(
        &self,
        id: EntityId,
    ) -> (BasicLayout, FlexLayout, Option<GridLayout>) {
        let LayoutStore {
            lay_basic,
            lay_flex,
            lay_grid,
            ..
        } = &self.layouts;
        let TopologyStore {
            topo_active_masks,
            topo_parents,
            ..
        } = &self.topology;
        let RenderStore {
            ren_active_transitions,
            ren_interaction,
            ren_visual,
            ..
        } = &self.renders;

        LayoutStore::resolve_active_layouts(
            id,
            topo_active_masks,
            topo_parents,
            lay_basic,
            lay_flex,
            lay_grid,
            ren_interaction,
            ren_visual,
            ren_active_transitions,
        )
    }

    #[inline]
    pub(crate) fn get_basic_layout_mut(
        &mut self,
        id: EntityId,
        target: StyleTarget,
    ) -> Option<&mut BasicLayout> {
        let LayoutStore { lay_base_basic, .. } = &mut self.layouts;
        let RenderStore {
            ren_interaction, ..
        } = &mut self.renders;

        LayoutStore::get_basic_layout_mut(id, target, lay_base_basic, ren_interaction)
    }

    #[inline]
    pub(crate) fn get_flex_layout_mut(
        &mut self,
        id: EntityId,
        target: StyleTarget,
    ) -> Option<&mut FlexLayout> {
        let LayoutStore { lay_flex, .. } = &mut self.layouts;
        let RenderStore {
            ren_interaction, ..
        } = &mut self.renders;

        LayoutStore::get_flex_layout_mut(id, target, lay_flex, ren_interaction)
    }

    // Taffyスタイルを一括解決するヘルパー
    #[inline]
    pub(crate) fn resolve_taffy_style(
        &self,
        id: EntityId,
        basic: &BasicLayout,
        flex: &FlexLayout,
        grid: Option<&GridLayout>,
    ) -> taffy::Style {
        let LayoutStore {
            lay_scrollbar_styles,
            ..
        } = &self.layouts;

        LayoutStore::resolve_taffy_style(id, basic, flex, grid, lay_scrollbar_styles)
    }
    // 全スクロールバー関連IDを一括抽出
    #[inline]
    pub(crate) fn scrollbar_el_ids(&self) -> HashSet<EntityId> {
        LayoutStore::scrollbar_el_ids(&self.layouts.lay_scrollbar_styles)
    }

    #[inline]
    pub(crate) fn sync_scrollbar_styles(&mut self) {
        let TopologyStore {
            topo_parents,
            topo_children,
            topo_active_masks,
            ..
        } = &mut self.topology;

        let LayoutStore {
            lay_basic,
            lay_base_basic,
            lay_flex,
            lay_grid,
            lay_scrollbar_styles,
            lay_taffy_nodes,
            lay_taffy,
            ..
        } = &mut self.layouts;

        let RenderStore {
            ren_visual,
            ren_interaction,
            ren_base_visual,
            ren_dirty_entities,
            ren_active_transitions,
            ..
        } = &mut self.renders;

        let OutputStore {
            out_rects,
            out_clip_rects,
            out_scroll_offsets,
            out_prev_rects,
            out_prev_clip_rects,
            out_selected_rects,
            out_text_selections,
            out_selection_start_index,
        } = &self.outputs;

        let ContentStore {
            cont_text_contents,
            cont_text_spans,
            cont_input_contents,
            ..
        } = &self.contents;

        let SystemStore {
            sys_text_engine,
            sys_dwrite_layouts,
            ..
        } = &self.system;

        let WindowStore { win_last_size, .. } = &self.window;

        LayoutStore::sync_scrollbar_styles(
            *win_last_size,
            sys_text_engine,
            sys_dwrite_layouts,
            cont_input_contents,
            cont_text_contents,
            cont_text_spans,
            topo_active_masks,
            topo_parents,
            topo_children,
            lay_taffy,
            lay_basic,
            lay_base_basic,
            lay_flex,
            lay_grid,
            lay_taffy_nodes,
            lay_scrollbar_styles,
            ren_visual,
            ren_base_visual,
            ren_active_transitions,
            ren_interaction,
            out_rects,
            out_scroll_offsets,
        );
    }
}
