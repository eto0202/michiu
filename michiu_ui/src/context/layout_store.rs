use crate::{
    ActiveMasksSecondary, ActiveTransitionsSparseSecondary, BaseVisualPropertiesSecondary,
    BasicLayout, CapacityConfig, ChildrenSecondary, ComponentMask, ContentStore, Context,
    DirtyRenderEntitiesVec, Display, DwriteLayoutsSparseSecondary, EdgeInsets, EntityId,
    FlexLayout, GridLayout, InputContentsSparseSecondary, InteractionPropertiesSecondary,
    InteractionStyles, LayoutPoint, LayoutRect, LayoutSize, Length, NormalLayout, OutputStore,
    ParentsSecondary, Position, PropertyList, Rect, RectsSecondary, RenderStore, ResizingState,
    ScrollOffsetsSecondary, ScrollSizesSecondary, ScrollbarDisplay, ScrollbarMode, ScrollbarStyle,
    Size, StyleTarget, SystemStore, TextContentsSparseSecondary, TextEngine,
    TextSpansSparseSecondary, ThisStyle, TopologyStore, Val, VisualPropertiesSecondary,
    WindowStore,
};
use slotmap::{SecondaryMap, SparseSecondaryMap};
use smallvec::SmallVec;
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use taffy::TaffyTree;

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

pub(crate) type LayoutsSecondary = SecondaryMap<EntityId, NormalLayout>;
pub(crate) type BaseLayoutsSecondary = SecondaryMap<EntityId, NormalLayout>;
pub(crate) type ResolvedLayoutsSecondary = SecondaryMap<EntityId, NormalLayout>;

pub(crate) type TaffyTreeEntityId = taffy::TaffyTree<EntityId>;
pub(crate) type TaffyNodesSecondary = SecondaryMap<EntityId, taffy::NodeId>;
pub(crate) type DirtyLayoutEntitiesVec = Vec<EntityId>;
pub(crate) type BasicLayoutsSecondary = SecondaryMap<EntityId, BasicLayout>;
pub(crate) type FlexLayoutsSecondary = SecondaryMap<EntityId, FlexLayout>;
pub(crate) type GridLayoutsSparseSecondary = SparseSecondaryMap<EntityId, GridLayout>;
pub(crate) type BaseBasicLayoutsSecondary = SecondaryMap<EntityId, BasicLayout>;
pub(crate) type BaseFlexLayoutsSecondary = SecondaryMap<EntityId, FlexLayout>;
pub(crate) type ResolvedBasicSecondary = SecondaryMap<EntityId, BasicLayout>;
pub(crate) type ResolvedFlexSecondary = SecondaryMap<EntityId, FlexLayout>;
pub(crate) type ResolvedGridSparseSecondary = SparseSecondaryMap<EntityId, GridLayout>;
pub(crate) type ScrollbarStylesSecondary = SparseSecondaryMap<EntityId, ScrollBarState>;

pub struct LayoutStore {
    pub(crate) lay_dirty_entities: DirtyLayoutEntitiesVec,
    pub(crate) lay_taffy_tree: TaffyTreeEntityId,
    pub(crate) lay_taffy_nodes: TaffyNodesSecondary,
    pub(crate) lay_basic: BasicLayoutsSecondary,
    pub(crate) lay_flex: FlexLayoutsSecondary,
    pub(crate) lay_grid: GridLayoutsSparseSecondary,
    pub(crate) lay_base_basic: BaseBasicLayoutsSecondary,
    pub(crate) lay_base_flex: BaseFlexLayoutsSecondary,
    pub(crate) lay_resolved_basic: ResolvedBasicSecondary,
    pub(crate) lay_resolved_flex: ResolvedFlexSecondary,
    pub(crate) lay_resolved_grid: ResolvedGridSparseSecondary,
    pub(crate) lay_scrollbar_styles: ScrollbarStylesSecondary,
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
            lay_dirty_entities: Vec::new(),
            lay_taffy_tree: TaffyTree::new(),
            lay_taffy_nodes: SecondaryMap::new(),
            lay_basic: SecondaryMap::new(),
            lay_flex: SecondaryMap::new(),
            lay_grid: SparseSecondaryMap::new(),
            lay_base_basic: SecondaryMap::new(),
            lay_base_flex: SecondaryMap::new(),
            lay_resolved_basic: SecondaryMap::new(),
            lay_resolved_flex: SecondaryMap::new(),
            lay_resolved_grid: SparseSecondaryMap::new(),
            lay_scrollbar_styles: SparseSecondaryMap::new(),
        }
    }

    #[inline]
    #[must_use]
    pub fn with_capacity(c: &CapacityConfig) -> Self {
        Self {
            lay_dirty_entities: Vec::with_capacity(c.lay_dirty_entities),
            lay_taffy_tree: TaffyTree::with_capacity(c.lay_taffy_tree),
            lay_taffy_nodes: SecondaryMap::with_capacity(c.lay_taffy_nodes),
            lay_basic: SecondaryMap::with_capacity(c.lay_basic),
            lay_flex: SecondaryMap::with_capacity(c.lay_flex),
            lay_grid: SparseSecondaryMap::with_capacity(c.lay_grid),
            lay_base_basic: SecondaryMap::with_capacity(c.lay_base_basic),
            lay_base_flex: SecondaryMap::with_capacity(c.lay_base_flex),
            lay_resolved_basic: SecondaryMap::with_capacity(c.lay_resolved_basic),
            lay_resolved_flex: SecondaryMap::with_capacity(c.lay_resolved_flex),
            lay_resolved_grid: SparseSecondaryMap::with_capacity(c.lay_resolved_grid),
            lay_scrollbar_styles: SparseSecondaryMap::with_capacity(c.lay_scrollbar_styles),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.lay_dirty_entities.clear();
        self.lay_taffy_tree = TaffyTree::new();
        self.lay_taffy_nodes.clear();
        self.lay_basic.clear();
        self.lay_flex.clear();
        self.lay_grid.clear();
        self.lay_base_basic.clear();
        self.lay_base_flex.clear();
        self.lay_resolved_basic.clear();
        self.lay_resolved_flex.clear();
        self.lay_resolved_grid.clear();
        self.lay_scrollbar_styles.clear();
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.lay_dirty_entities.retain(|&x| x != id);
        self.lay_taffy_nodes.remove(id);
        self.lay_basic.remove(id);
        self.lay_flex.remove(id);
        self.lay_grid.remove(id);
        self.lay_base_basic.remove(id);
        self.lay_base_flex.remove(id);
        self.lay_resolved_basic.remove(id);
        self.lay_resolved_flex.remove(id);
        self.lay_resolved_grid.remove(id);
        self.lay_scrollbar_styles.remove(id);
    }
}

impl LayoutStore {
    /// 各スタイルの解決を1回のルックアップと1回のカスケード解決ループに統合
    #[inline]
    pub(crate) fn resolve_active_layouts(
        id: EntityId,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSparseSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
    ) -> (BasicLayout, FlexLayout, Option<GridLayout>) {
        let mut basic = lay_basic.get(id).copied().unwrap_or_default();
        let mut flex = lay_flex.get(id).copied().unwrap_or_default();
        let mut grid = lay_grid.get(id).cloned();

        let active_mask = topo_active_masks[id];

        // 幅・高さ・一括サイズに対して、現在トランジションアニメーションが駆動中であるかを走査
        let (is_width_transitioning, is_height_transitioning) =
            LayoutStore::is_transition_currently_running(id, rnd_active_transitions);

        // 自身、または親先祖から focused / focus_visible のフォーカス関連スタイルを解決
        let [focused_style_resolved, focused_visible_style_resolved] = [
            ComponentMask::STATE_FOCUSED,
            ComponentMask::STATE_FOCUSED_VISIBLE,
        ]
        .map(|state| {
            RenderStore::resolv_focus_style(
                id,
                &active_mask,
                state,
                topo_parents,
                rnd_visual,
                rnd_interaction,
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
            rnd_interaction,
        );

        (basic, flex, grid)
    }

    #[inline]
    pub(crate) fn update_resolved_active_layout_cache(
        id: EntityId,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_resolved_basic: &mut ResolvedBasicSecondary,
        lay_resolved_flex: &mut ResolvedFlexSecondary,
        lay_resolved_grid: &mut ResolvedGridSparseSecondary,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSparseSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
    ) {
        let (basic, flex, grid) = LayoutStore::resolve_active_layouts(
            id,
            topo_active_masks,
            topo_parents,
            lay_basic,
            lay_flex,
            lay_grid,
            rnd_visual,
            rnd_interaction,
            rnd_active_transitions,
        );

        lay_resolved_basic.insert(id, basic);
        lay_resolved_flex.insert(id, flex);
        if let Some(g) = grid {
            lay_resolved_grid.insert(id, g);
        } else {
            lay_resolved_grid.remove(id);
        }
    }
    /// ターゲット状態に応じた `BasicLayout` の可変参照を引き出す
    pub(crate) fn get_basic_layout_mut<'a>(
        id: EntityId,
        target: StyleTarget,
        lay_base_basic: &'a mut BaseBasicLayoutsSecondary,
        rnd_interaction: &'a mut InteractionPropertiesSecondary,
    ) -> Option<&'a mut BasicLayout> {
        if target == StyleTarget::Base {
            lay_base_basic.get_mut(id)
        } else {
            // rnd_interaction から該当疑似クラスを安全に解決
            let styles = rnd_interaction.get_mut(id)?;
            let style_ref = styles.get_style_target_mut(target);
            Some(&mut Arc::make_mut(&mut style_ref.inner).basic_layout)
        }
    }

    pub(crate) fn get_flex_layout_mut<'a>(
        id: EntityId,
        target: StyleTarget,
        lay_flex: &'a mut SecondaryMap<EntityId, FlexLayout>,
        rnd_interaction: &'a mut InteractionPropertiesSecondary,
    ) -> Option<&'a mut FlexLayout> {
        if target == StyleTarget::Base {
            lay_flex.get_mut(id)
        } else {
            if !rnd_interaction.contains_key(id) {
                rnd_interaction.insert(id, InteractionStyles::default());
            }
            let styles = rnd_interaction.get_mut(id).unwrap();
            let style_ref = styles.get_style_target_mut(target);
            Some(&mut Arc::make_mut(&mut style_ref.inner).flex_layout)
        }
    }

    pub(crate) fn is_transition_currently_running(
        id: EntityId,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
    ) -> (bool, bool) {
        let Some(list) = rnd_active_transitions.get(id) else {
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

    pub(crate) fn apply_interaction_styles(
        id: EntityId,
        active_mask: &ComponentMask,
        basic: &mut BasicLayout,
        flex: &mut FlexLayout,
        grid: &mut Option<GridLayout>,
        focused_resolved: &[Option<ThisStyle>; 2],
        is_transitioning: (bool, bool),
        rnd_interaction: &InteractionPropertiesSecondary,
    ) {
        let Some(interaction) = rnd_interaction.get(id) else {
            return;
        };

        let cascade = [
            (ComponentMask::STATE_FOCUSED, &focused_resolved[0]),
            (ComponentMask::STATE_FOCUSED_VISIBLE, &focused_resolved[1]),
            (ComponentMask::STATE_SELECTED, &interaction.selected),
            (ComponentMask::STATE_ACTIVED, &interaction.actived),
            (ComponentMask::STATE_HOVERED, &interaction.hovered),
            (ComponentMask::STATE_PRESSED, &interaction.pressed),
            (ComponentMask::STATE_DISABLED, &interaction.disabled),
            (ComponentMask::STATE_DND_DRAGGING, &interaction.dragging),
            (ComponentMask::STATE_DND_DRAG_IN, &interaction.drag_in),
            (ComponentMask::STATE_DND_DRAG_OVER, &interaction.drag_over),
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
                mask.unset(ComponentMask::STYLE_SIZE);
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
        size: Size<Val>,
        inset: Rect<Val>,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
    ) {
        let layouts = [lay_basic.get_mut(id), lay_base_basic.get_mut(id)];

        for layout in layouts.into_iter().flatten() {
            layout.display = Display::Flex;
            layout.size = size;
            layout.inset = inset;
        }
    }

    /// スクロールバー用要素（TrackやThumb）のレイアウト、不透明度、Taffyスタイルへの反映を一括して同期更新します。
    #[inline]
    pub(crate) fn update_scrollbar_element(
        id: EntityId,
        size: Size<Val>,
        inset: Rect<Val>,
        opacity: f32,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        lay_resolved_basic: &mut ResolvedBasicSecondary,
        lay_resolved_flex: &mut ResolvedFlexSecondary,
        lay_resolved_grid: &mut ResolvedGridSparseSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSparseSecondary,
        lay_scrollbar_styles: &ScrollbarStylesSecondary,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_base_visual: &mut BaseVisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
    ) {
        LayoutStore::update_scrollbar_element_layout(
            id,
            size,
            inset,
            lay_taffy_tree,
            lay_basic,
            lay_base_basic,
            lay_taffy_nodes,
        );
        RenderStore::update_scrollbar_element_opacity(id, opacity, rnd_visual, rnd_base_visual);

        // 変更されたスクロールバー要素のキャッシュを更新
        LayoutStore::update_resolved_active_layout_cache(
            id,
            topo_active_masks,
            topo_parents,
            lay_resolved_basic,
            lay_resolved_flex,
            lay_resolved_grid,
            lay_basic,
            lay_flex,
            lay_grid,
            rnd_visual,
            rnd_interaction,
            rnd_active_transitions,
        );

        let basic = lay_resolved_basic.get(id).copied().unwrap_or_default();
        let flex = lay_resolved_flex.get(id).copied().unwrap_or_default();
        let grid = lay_resolved_grid.get(id); // Grid実装時用

        LayoutStore::set_taffy_style(
            id,
            &basic,
            &flex,
            grid,
            lay_taffy_tree,
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
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_scrollbar_styles: &ScrollbarStylesSecondary,
    ) {
        let taffy_style =
            LayoutStore::resolve_taffy_style(id, basic, flex, grid, lay_scrollbar_styles);
        let _ = lay_taffy_tree.set_style(lay_taffy_nodes[id], taffy_style);
    }

    /// スクロールバー用要素をレイアウト上から安全に隠します。
    #[inline]
    pub(crate) fn hide_scrollbar_element(
        id: EntityId,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
    ) {
        let layouts = [lay_basic.get_mut(id), lay_base_basic.get_mut(id)];

        for layout in layouts.into_iter().flatten() {
            layout.display = Display::None;
        }

        // 非表示パスで、Taffy側ノードスタイルを確実に Display::None にして同期する
        if let Some(&node_id) = lay_taffy_nodes.get(id) {
            let _ = lay_taffy_tree.set_style(
                node_id,
                taffy::Style {
                    display: taffy::Display::None,
                    ..Default::default()
                },
            );
        }
    }

    #[inline]
    pub(crate) fn local_rect_from_taffy(
        id: EntityId,
        lay_taffy_tree: &TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
    ) -> LayoutRect {
        let Some(&taffy_node) = lay_taffy_nodes.get(id) else {
            return LayoutRect::ZERO;
        };

        let Ok(layout) = lay_taffy_tree.layout(taffy_node) else {
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
    #[inline]
    pub(crate) fn resync_taffy_children_order(
        parent_id: EntityId,
        topo_children: &ChildrenSecondary,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
    ) {
        let Some(&parent_node) = lay_taffy_nodes.get(parent_id) else {
            return;
        };
        // 一旦現在登録されているすべての子ノードを Taffy 側から安全にデタッチ
        if let Ok(taffy_children) = lay_taffy_tree.children(parent_node) {
            for child_node in taffy_children {
                let _ = lay_taffy_tree.remove_child(parent_node, child_node);
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
            let _ = lay_taffy_tree.add_child(parent_node, child_node);
        }
    }

    #[inline]
    pub fn clear_layout_dirty(
        topo_active_masks: &mut ActiveMasksSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
    ) {
        for id in lay_dirty_entities.drain(..) {
            if let Some(mask) = topo_active_masks.get_mut(id) {
                mask.unset(ComponentMask::STATE_QUEUED_LAYOUT);
            }
        }
        lay_dirty_entities.clear();
    }

    #[inline]
    pub(crate) fn mark_layout_dirty(
        id: EntityId,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
    ) {
        let mut curr = id;
        // Taffy 側の該当ノードのレイアウトキャッシュを無効化
        if let Some(&taffy_node) = lay_taffy_nodes.get(curr) {
            let _ = lay_taffy_tree.mark_dirty(taffy_node);
        }

        loop {
            // マスクが存在する場合のみDirtyマーク
            if let Some(mask) = topo_active_masks.get_mut(curr) {
                // すでに登録済みなら多重登録を防ぐため探索を早期ブレイク
                if mask.has(ComponentMask::STATE_QUEUED_LAYOUT) {
                    break;
                }
                mask.set(ComponentMask::STATE_QUEUED_LAYOUT); // 自身を Dirty マーク
                lay_dirty_entities.push(curr);
            }

            // 親要素（先祖）をルートまで辿って Dirty フラグを連鎖伝播させる
            let Some(Some(parent_id)) = topo_parents.get(curr).copied() else {
                break;
            };
            curr = parent_id;
        }
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
    pub lay_taffy_tree: &'a mut TaffyTreeEntityId,
    pub lay_basic: &'a mut BasicLayoutsSecondary,
    pub lay_base_basic: &'a mut BaseBasicLayoutsSecondary,
    pub lay_resolved_basic: &'a mut ResolvedBasicSecondary,
    pub lay_resolved_flex: &'a mut ResolvedFlexSecondary,
    pub lay_resolved_grid: &'a mut ResolvedGridSparseSecondary,
    pub lay_taffy_nodes: &'a TaffyNodesSecondary,
    pub lay_flex: &'a FlexLayoutsSecondary,
    pub lay_grid: &'a GridLayoutsSparseSecondary,
    pub lay_scrollbar_styles: &'a ScrollbarStylesSecondary,
    pub rnd_visual: &'a mut VisualPropertiesSecondary,
    pub rnd_base_visual: &'a mut BaseVisualPropertiesSecondary,
    pub rnd_interaction: &'a InteractionPropertiesSecondary,
    pub rnd_active_transitions: &'a ActiveTransitionsSparseSecondary,
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
            self.lay_taffy_tree,
            self.lay_basic,
            self.lay_base_basic,
            self.lay_resolved_basic,
            self.lay_resolved_flex,
            self.lay_resolved_grid,
            self.lay_taffy_nodes,
            self.lay_flex,
            self.lay_grid,
            self.lay_scrollbar_styles,
            self.rnd_visual,
            self.rnd_base_visual,
            self.rnd_interaction,
            self.rnd_active_transitions,
        );
    }
    #[inline]
    pub fn hide_el(&mut self, el_id: EntityId) {
        LayoutStore::hide_scrollbar_element(
            el_id,
            self.lay_taffy_tree,
            self.lay_basic,
            self.lay_base_basic,
            self.lay_taffy_nodes,
        );
    }
}

impl LayoutStore {
    pub(crate) fn sync_scrollbar_styles(
        win_last_size: Option<LayoutSize>,
        sys_text_engine: &TextEngine,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        cont_text_contents: &TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        cont_input_contents: &InputContentsSparseSecondary,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        lay_resolved_basic: &mut ResolvedBasicSecondary,
        lay_resolved_flex: &mut ResolvedFlexSecondary,
        lay_resolved_grid: &mut ResolvedGridSparseSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSparseSecondary,
        lay_scrollbar_styles: &ScrollbarStylesSecondary,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_base_visual: &mut BaseVisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        out_rects: &RectsSecondary,
        out_scroll_offsets: &ScrollOffsetsSecondary,
        out_scroll_sizes: &ScrollSizesSecondary,
    ) {
        let scrollbar_ids: Vec<EntityId> = lay_scrollbar_styles.keys().collect();

        for id in scrollbar_ids {
            let sb_state = lay_scrollbar_styles.get(id).cloned().unwrap();
            let container_rect = out_rects[id];
            let scroll_size = out_scroll_sizes.get(id).copied().unwrap_or_default();
            let current_scroll = out_scroll_offsets.get(id).copied().unwrap_or_default();

            let basic = lay_resolved_basic.get(id).copied().unwrap_or_default();
            let rect = out_rects.get(id).copied().unwrap_or_default();
            let (border, padding) =
                LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

            let visible_size = WindowStore::calculate_visible_size(container_rect, win_last_size);
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
                topo_active_masks,
                topo_parents,
                lay_taffy_tree,
                lay_basic,
                lay_base_basic,
                lay_resolved_basic,
                lay_resolved_flex,
                lay_resolved_grid,
                lay_taffy_nodes,
                lay_flex,
                lay_grid,
                lay_scrollbar_styles,
                rnd_visual,
                rnd_base_visual,
                rnd_interaction,
                rnd_active_transitions,
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
        LayoutStore::mark_layout_dirty(
            id,
            &mut self.topology.topo_active_masks,
            &self.topology.topo_parents,
            &mut self.layouts.lay_dirty_entities,
            &mut self.layouts.lay_taffy_tree,
            &self.layouts.lay_taffy_nodes,
        );
    }

    #[inline]
    pub(crate) fn get_basic_layout_mut(
        &mut self,
        id: EntityId,
        target: StyleTarget,
    ) -> Option<&mut BasicLayout> {
        LayoutStore::get_basic_layout_mut(
            id,
            target,
            &mut self.layouts.lay_base_basic,
            &mut self.renders.rnd_interaction,
        )
    }

    #[inline]
    pub(crate) fn get_flex_layout_mut(
        &mut self,
        id: EntityId,
        target: StyleTarget,
    ) -> Option<&mut FlexLayout> {
        LayoutStore::get_flex_layout_mut(
            id,
            target,
            &mut self.layouts.lay_flex,
            &mut self.renders.rnd_interaction,
        )
    }
}
