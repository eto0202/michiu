use std::{collections::HashSet, time::Duration};

use crate::*;
use slotmap::{SecondaryMap, SparseSecondaryMap};
use smallvec::SmallVec;
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

pub(crate) type BasicLayoutsSecondary = SecondaryMap<EntityId, BasicLayout>;
pub(crate) type BaseBasicLayoutsSecondary = SecondaryMap<EntityId, BasicLayout>;
pub(crate) type FlexLayoutsSecondary = SecondaryMap<EntityId, FlexLayout>;
pub(crate) type GridLayoutsSecondary = SparseSecondaryMap<EntityId, GridLayout>;
pub(crate) type ScrollbarStylesSecondary = SparseSecondaryMap<EntityId, ScrollBarState>;
pub(crate) type TaffyNodesSecondary = SecondaryMap<EntityId, taffy::NodeId>;
pub(crate) type TaffyTreeEntityId = taffy::TaffyTree<EntityId>;
pub(crate) type DirtyLayoutEntitiesVec = Vec<EntityId>;

pub struct LayoutStore {
    pub(crate) basic_layouts: BasicLayoutsSecondary,
    pub(crate) base_basic_layouts: BaseBasicLayoutsSecondary,
    pub(crate) flex_layouts: FlexLayoutsSecondary,
    pub(crate) grid_layouts: GridLayoutsSecondary,
    pub(crate) scrollbar_styles: ScrollbarStylesSecondary,
    pub(crate) taffy_nodes: TaffyNodesSecondary,
    pub(crate) taffy: TaffyTreeEntityId,
    pub(crate) dirty_layout_entities: DirtyLayoutEntitiesVec,
}

impl Default for LayoutStore {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutStore {
    #[inline]
    pub fn new() -> Self {
        Self {
            basic_layouts: SecondaryMap::new(),
            base_basic_layouts: SecondaryMap::new(),
            flex_layouts: SecondaryMap::new(),
            grid_layouts: SparseSecondaryMap::new(),
            scrollbar_styles: SparseSecondaryMap::new(),
            taffy_nodes: SecondaryMap::new(),
            taffy: TaffyTree::new(),
            dirty_layout_entities: Vec::new(),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.basic_layouts.clear();
        self.base_basic_layouts.clear();
        self.flex_layouts.clear();
        self.grid_layouts.clear();
        self.scrollbar_styles.clear();
        self.taffy_nodes.clear();
        self.taffy = TaffyTree::new();
        self.dirty_layout_entities.clear();
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.basic_layouts.remove(id);
        self.base_basic_layouts.remove(id);
        self.flex_layouts.remove(id);
        self.grid_layouts.remove(id);
        self.scrollbar_styles.remove(id);
        self.taffy_nodes.remove(id);
        self.dirty_layout_entities.retain(|&x| x != id);
    }
}

impl LayoutStore {
    /// 各スタイルの解決を1回のルックアップと1回のカスケード解決ループに統合
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve_active_layouts(
        id: EntityId,
        basic_layouts: &BasicLayoutsSecondary,
        flex_layouts: &FlexLayoutsSecondary,
        grid_layouts: &GridLayoutsSecondary,
        active_masks: &ActiveMasksSecondary,
        active_transitions: &ActiveTransitionsSparseSecondary,
        parents: &ParentsSecondary,
        interaction_properties: &InteractionPropertiesSecondary,
        visual_properties: &VisualPropertiesSecondary,
    ) -> (BasicLayout, FlexLayout, Option<GridLayout>) {
        let mut basic = basic_layouts.get(id).copied().unwrap_or_default();
        let mut flex = flex_layouts.get(id).copied().unwrap_or_default();
        let mut grid = grid_layouts.get(id).cloned();

        let active_mask = active_masks[id];

        // 幅・高さ・一括サイズに対して、現在トランジションアニメーションが駆動中であるかを走査
        let (is_width_transitioning, is_height_transitioning) =
            LayoutStore::is_transition_currently_running(id, active_transitions);

        // 自身、または親先祖から focused / focus_visible のフォーカス関連スタイルを解決
        let [focused_style_resolved, focused_visible_style_resolved] =
            [STATE_FOCUSED, STATE_FOCUSED_VISIBLE].map(|state| {
                RenderStore::resolv_focus_style(
                    id,
                    interaction_properties,
                    visual_properties,
                    &active_mask,
                    parents,
                    state,
                )
            });

        // 状態マッピング解決のルックアップとループを1回に集約
        LayoutStore::apply_interaction_styles(
            id,
            interaction_properties,
            [focused_style_resolved, focused_visible_style_resolved],
            &active_mask,
            (is_width_transitioning, is_height_transitioning),
            &mut basic,
            &mut flex,
            &mut grid,
        );

        (basic, flex, grid)
    }

    pub(crate) fn is_transition_currently_running(
        id: EntityId,
        active_transitions: &ActiveTransitionsSparseSecondary,
    ) -> (bool, bool) {
        let Some(list) = active_transitions.get(id) else {
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
        interaction_properties: &InteractionPropertiesSecondary,
        focused_resolved: [Option<ThisStyle>; 2],
        active_mask: &ComponentMask,
        is_transitioning: (bool, bool),
        basic: &mut BasicLayout,
        flex: &mut FlexLayout,
        grid: &mut Option<GridLayout>,
    ) {
        let Some(interaction) = interaction_properties.get(id) else {
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
            (STATE_DRAGGING, &interaction.dragging),
            (STATE_DRAG_IN, &interaction.drag_in),
            (STATE_DRAG_OVER, &interaction.drag_over),
        ];

        for (state, style_opt) in cascade {
            if active_mask.has(state)
                && let Some(style) = style_opt
            {
                let mut mask = style.inner.mask;
                if is_transitioning.0 || is_transitioning.1 {
                    mask.unset(STYLE_SIZE);
                }

                basic.override_with(&style.inner.basic_layout, style.inner.mask);
                flex.override_with(&style.inner.flex_layout, style.inner.mask);

                if style.inner.mask.has_grid_layout()
                    && let Some(ref hover_grid) = style.inner.grid_layout
                {
                    *grid = Some(hover_grid.clone());
                }
            }
        }
    }

    // Taffyスタイルを一括解決するヘルパー
    pub(crate) fn resolve_taffy_style(
        id: EntityId,
        scrollbar_styles: &ScrollbarStylesSecondary,
        basic: &BasicLayout,
        flex: &FlexLayout,
        grid: Option<&GridLayout>,
    ) -> taffy::Style {
        let sb_style = scrollbar_styles.get(id).map(|s| &s.style);

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
    fn length_to_px(length: Length, reference: f32) -> f32 {
        match length {
            Length::Px(v) => v,
            Length::Percent(p) => reference * (p / 100.0),
        }
    }

    // 全スクロールバー関連IDを一括抽出
    #[inline]
    pub(crate) fn scrollbar_el_ids(
        scrollbar_styles: &SparseSecondaryMap<EntityId, ScrollBarState>,
    ) -> HashSet<EntityId> {
        scrollbar_styles
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
        basic_layouts: &mut BasicLayoutsSecondary,
        base_basic_layouts: &mut BaseBasicLayoutsSecondary,
        taffy_nodes: &TaffyNodesSecondary,
        taffy: &mut TaffyTreeEntityId,
        size: Size<Val>,
        inset: Rect<Val>,
    ) {
        let layouts = [basic_layouts.get_mut(id), base_basic_layouts.get_mut(id)];

        for layout in layouts.into_iter().flatten() {
            layout.display = Display::Flex;
            layout.size = size;
            layout.inset = inset;
        }

        // affy 側のノードスタイルも Display::None にして同期
        let _ = taffy.set_style(
            taffy_nodes[id],
            taffy::Style {
                display: taffy::Display::None,
                ..Default::default()
            },
        );
    }

    /// 解決済みの基本スタイルを TaffyTree のノードへ同期して適用。
    #[inline]
    pub(crate) fn set_taffy_style(
        id: EntityId,
        scrollbar_styles: &ScrollbarStylesSecondary,
        taffy_nodes: &TaffyNodesSecondary,
        taffy: &mut TaffyTreeEntityId,
        basic: &BasicLayout,
        flex: &FlexLayout,
        grid: Option<&GridLayout>,
    ) {
        let taffy_style = LayoutStore::resolve_taffy_style(id, scrollbar_styles, basic, flex, grid);
        let _ = taffy.set_style(taffy_nodes[id], taffy_style);
    }

    /// スクロールバー用要素をレイアウト上から安全に隠します。
    #[inline]
    pub(crate) fn hide_scrollbar_element(
        id: EntityId,
        basic_layouts: &mut BasicLayoutsSecondary,
        base_basic_layouts: &mut BaseBasicLayoutsSecondary,
    ) {
        let layouts = [basic_layouts.get_mut(id), base_basic_layouts.get_mut(id)];

        for layout in layouts.into_iter().flatten() {
            layout.display = Display::None;
        }
    }

    pub(crate) fn local_rect_from_taffy(id: EntityId, layouts: &LayoutStore) -> LayoutRect {
        let Some(&taffy_node) = layouts.taffy_nodes.get(id) else {
            return LayoutRect::ZERO;
        };

        if let Ok(layout) = layouts.taffy.layout(taffy_node) {
            LayoutRect::new(
                layout.location.x,
                layout.location.y,
                layout.size.width,
                layout.size.height,
            )
        } else {
            LayoutRect::ZERO
        }
    }

    /// 指定された親コンテナにアタッチされている DComp / Taffy 側のすべての子ノードの物理順序を
    /// 内部 SoA リスト（self.children）の順序に沿って再同期。
    pub(crate) fn resync_taffy_children_order(
        parent_id: EntityId,
        taffy_nodes: &TaffyNodesSecondary,
        taffy: &mut TaffyTreeEntityId,
        children: &ChildrenSecondary,
    ) {
        let Some(&parent_node) = taffy_nodes.get(parent_id) else {
            return;
        };
        // 一旦現在登録されているすべての子ノードを Taffy 側から安全にデタッチ
        if let Ok(taffy_children) = taffy.children(parent_node) {
            for child_node in taffy_children {
                let _ = taffy.remove_child(parent_node, child_node);
            }
        }
        // 最新の並び替え順序リストの順に従って、Taffy 側に再アタッチ
        if let Some(children_list) = children.get(parent_id).cloned() {
            for child_id in children_list {
                if let Some(&child_node) = taffy_nodes.get(child_id) {
                    let _ = taffy.add_child(parent_node, child_node);
                }
            }
        }
    }

    pub fn clear_layout_dirty(layouts: &mut LayoutStore, topology: &mut TopologyStore) {
        for id in layouts.dirty_layout_entities.drain(..) {
            if let Some(mask) = topology.active_masks.get_mut(id) {
                mask.unset(STATE_QUEUED_LAYOUT);
            }
        }
        layouts.dirty_layout_entities.clear();
    }

    #[inline]
    pub(crate) fn mark_layout_dirty(
        id: EntityId,
        taffy_nodes: &TaffyNodesSecondary,
        taffy: &mut TaffyTreeEntityId,
        active_masks: &mut ActiveMasksSecondary,
        dirty_layout_entities: &mut DirtyLayoutEntitiesVec,
        parents: &ParentsSecondary,
    ) {
        let mut curr = id;
        // Taffy 側の該当ノードのレイアウトキャッシュを無効化
        if let Some(&taffy_node) = taffy_nodes.get(curr) {
            let _ = taffy.mark_dirty(taffy_node);
        }

        loop {
            if let Some(mask) = active_masks.get_mut(curr) {
                // すでにレイアウトキューに登録済み（STATE_QUEUED_LAYOUT がオン）なら
                // 多重登録を防ぎつつ、それより上の親はすでに Dirty 化されているため探索を早期ブレイク
                if !mask.has(STATE_QUEUED_LAYOUT) {
                    mask.set(STATE_QUEUED_LAYOUT); // 自身を Dirty マーク
                    dirty_layout_entities.push(curr);
                } else {
                    break;
                }
            }

            // 親要素（先祖）をルートまで辿って Dirty フラグを連鎖伝播させる
            if let Some(Some(parent_id)) = parents.get(curr).copied() {
                curr = parent_id;
            } else {
                break;
            }
        }
    }
}

impl Context {
    #[inline]
    pub(crate) fn mark_layout_dirty(&mut self, id: EntityId) {
        let LayoutStore {
            taffy_nodes,
            taffy,
            dirty_layout_entities,
            ..
        } = &mut self.layouts;
        let TopologyStore {
            active_masks,
            parents,
            ..
        } = &mut self.topology;

        LayoutStore::mark_layout_dirty(
            id,
            taffy_nodes,
            taffy,
            active_masks,
            dirty_layout_entities,
            parents,
        );
    }
    /// 実際の可視サイズから、物理ボーダーとパディングの厚みを引いた内枠の有効表示可能サイズを算出します。
    #[inline]
    pub(crate) fn calculate_inner_content_size(
        &self,
        visible_size: LayoutSize,
        border: EdgeInsets,
        padding: EdgeInsets,
    ) -> LayoutSize {
        LayoutStore::calculate_inner_content_size(visible_size, border, padding)
    }

    /// 各スタイルの解決を1回のルックアップと1回のカスケード解決ループに統合
    #[inline]
    pub(crate) fn resolve_active_layouts(
        &self,
        id: EntityId,
    ) -> (BasicLayout, FlexLayout, Option<GridLayout>) {
        let LayoutStore {
            basic_layouts,
            flex_layouts,
            grid_layouts,
            ..
        } = &self.layouts;
        let TopologyStore {
            active_masks,
            parents,
            ..
        } = &self.topology;
        let RenderStore {
            active_transitions,
            interaction_properties,
            visual_properties,
            ..
        } = &self.renders;

        LayoutStore::resolve_active_layouts(
            id,
            basic_layouts,
            flex_layouts,
            grid_layouts,
            active_masks,
            active_transitions,
            parents,
            interaction_properties,
            visual_properties,
        )
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
            scrollbar_styles, ..
        } = &self.layouts;

        LayoutStore::resolve_taffy_style(id, scrollbar_styles, basic, flex, grid)
    }
    // 全スクロールバー関連IDを一括抽出
    #[inline]
    pub(crate) fn scrollbar_el_ids(&self) -> HashSet<EntityId> {
        LayoutStore::scrollbar_el_ids(&self.layouts.scrollbar_styles)
    }

    /// スクロールバー用要素（TrackやThumb）のレイアウト、不透明度、Taffyスタイルへの反映を一括して同期更新します。
    #[inline]
    pub(crate) fn update_scrollbar_element(
        &mut self,
        id: EntityId,
        size: Size<Val>,
        inset: Rect<Val>,
        opacity: f32,
    ) {
        let LayoutStore {
            basic_layouts,
            base_basic_layouts,
            flex_layouts,
            grid_layouts,
            scrollbar_styles,
            taffy_nodes,
            taffy,
            ..
        } = &mut self.layouts;

        let TopologyStore {
            parents,
            active_masks,
            ..
        } = &self.topology;

        let RenderStore {
            active_transitions,
            interaction_properties,
            visual_properties,
            base_visual_properties,
            ..
        } = &mut self.renders;

        LayoutStore::update_scrollbar_element_layout(
            id,
            basic_layouts,
            base_basic_layouts,
            taffy_nodes,
            taffy,
            size,
            inset,
        );
        RenderStore::update_scrollbar_element_opacity(
            id,
            visual_properties,
            base_visual_properties,
            opacity,
        );

        let (basic, flex, grid) = LayoutStore::resolve_active_layouts(
            id,
            basic_layouts,
            flex_layouts,
            grid_layouts,
            active_masks,
            active_transitions,
            parents,
            interaction_properties,
            visual_properties,
        );

        LayoutStore::set_taffy_style(
            id,
            scrollbar_styles,
            taffy_nodes,
            taffy,
            &basic,
            &flex,
            grid.as_ref(),
        );
    }

    #[inline]
    pub(crate) fn local_rect_from_taffy(&self, id: EntityId) -> LayoutRect {
        LayoutStore::local_rect_from_taffy(id, &self.layouts)
    }

    /// 指定された親コンテナにアタッチされている DComp / Taffy 側のすべての子ノードの物理順序を
    /// 内部 SoA リスト（self.children）の順序に沿って一括して再同期）します。
    #[inline]
    pub(crate) fn resync_taffy_children_order(&mut self, parent_id: EntityId) {
        let LayoutStore {
            taffy_nodes, taffy, ..
        } = &mut self.layouts;
        let TopologyStore { children, .. } = &mut self.topology;

        LayoutStore::resync_taffy_children_order(parent_id, taffy_nodes, taffy, children);
    }

    pub(crate) fn sync_resizing_drag(&mut self, logical_pos: LayoutPoint, state: ResizingState) {
        let id = state.entity_id;
        let delta_x = logical_pos.x - state.start_mouse_pos.x;
        let delta_y = logical_pos.y - state.start_mouse_pos.y;

        let start_rect = state.start_rect;
        let position = self
            .layouts
            .basic_layouts
            .get(id)
            .map(|l| l.position)
            .unwrap_or(Position::Relative);

        // 1-1. 最小サイズ・最大クランプ値の解決
        let (min_w, max_w, min_h, max_h) = {
            let basic = self
                .layouts
                .basic_layouts
                .get(id)
                .copied()
                .unwrap_or_default();

            let ref_w = start_rect.width;
            let ref_h = start_rect.height;

            let rect = self.rect(id).unwrap_or_default();
            let (border, padding) =
                LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

            // 枠線と余白を足した、物理的にこれ以上小さくできない限界サイズ
            let abs_min_w = border.left + border.right + padding.left + padding.right;
            let abs_min_h = border.top + border.bottom + padding.top + padding.bottom;

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
        if let Some(layout) = self.layouts.basic_layouts.get_mut(id) {
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
        if let Some(layout) = self.layouts.base_basic_layouts.get_mut(id) {
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
    }

    /// スクロールバー用要素をレイアウト上から安全に隠します。
    #[inline]
    pub(crate) fn hide_scrollbar_element(&mut self, id: EntityId) {
        let LayoutStore {
            basic_layouts,
            base_basic_layouts,
            ..
        } = &mut self.layouts;

        LayoutStore::hide_scrollbar_element(id, basic_layouts, base_basic_layouts);
    }

    pub(crate) fn sync_scrollbar_styles(&mut self) {
        let scrollbar_ids: Vec<EntityId> = self.layouts.scrollbar_styles.keys().collect();
        for id in scrollbar_ids {
            let sb_state = self.layouts.scrollbar_styles.get(id).cloned().unwrap();
            let container_rect = self.outputs.rects[id];
            let scroll_size = self.get_scroll_size(id);
            let current_scroll = self
                .outputs
                .scroll_offsets
                .get(id)
                .copied()
                .unwrap_or(LayoutPoint::ZERO);

            // 親コンテナのボーダーおよびパディング厚を取得
            let (basic, _, _) = self.resolve_active_layouts(id);
            let rect = self.rect(id).unwrap_or_default();
            let (border, padding) =
                LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

            // ウィンドウ境界によるクランプ可視サイズの算出
            let visible_size = self.calculate_visible_size(container_rect);
            // 枠線と余白を引いた内枠コンテンツサイズの算出
            let content_size = self.calculate_inner_content_size(visible_size, border, padding);

            // 内枠の有効表示領域と、同じく内枠基準の scroll_size を精密に比較する
            let show_v_bar = scroll_size.height > content_size.height;
            let show_h_bar = scroll_size.width > content_size.width;

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
                        (visible_size.height
                            - border.top
                            - border.bottom
                            - (if show_h_bar {
                                sb_state.style.width
                            } else {
                                0.0
                            }))
                        .max(0.0)
                    };

                    let track_right = 0.0;

                    // v_track の幅を Val::Auto に上書きしたため、Taffy 側で known_dims.width が None（Auto）になる
                    // Taffy 側の measure_func は  outputs.rects から値を取得しようと試みる
                    // スクロールバー要素は 1 回目のパスで走査スルーされているため、この時点では outputs.rects に座標が登録されていない
                    // 結果としてサイズ 0.0 が返り、Track の幅が 0.0 に潰れて不可視になっていた
                    // Val::Auto ではなく Val::Px(sb_state.style.width) に修正して SoA 上に実サイズを維持
                    self.update_scrollbar_element(
                        v_track,
                        Size::new(Val::Px(sb_state.style.width), Val::Px(track_h)),
                        Rect::new(Val::Px(0.0), Val::Px(track_right), Val::Auto, Val::Auto),
                        v_track_opacity,
                    );
                } else {
                    self.hide_scrollbar_element(v_track);
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

                if v_thumb_visible {
                    let track_h = (visible_size.height
                        - border.top
                        - border.bottom
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
                        (visible_size.height / scroll_size.height).min(1.0)
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

                    let scroll_ratio = if scroll_size.height > visible_size.height {
                        (current_scroll.y / (scroll_size.height - visible_size.height))
                            .clamp(0.0, 1.0)
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

                    self.update_scrollbar_element(
                        v_thumb,
                        Size::new(Val::Px(thumb_width), Val::Px(thumb_height)),
                        Rect::new(Val::Px(thumb_y), Val::Auto, Val::Auto, Val::Px(thumb_x)),
                        v_thumb_opacity,
                    );
                } else {
                    self.hide_scrollbar_element(v_thumb);
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

                if h_track_visible {
                    let track_w = (visible_size.width
                        - border.left
                        - border.right
                        - (if show_v_bar {
                            sb_state.style.width
                        } else {
                            0.0
                        }))
                    .max(0.0);

                    let track_bottom = 0.0;

                    self.update_scrollbar_element(
                        h_track,
                        Size::new(Val::Px(track_w), Val::Px(sb_state.style.width)),
                        Rect::new(Val::Auto, Val::Auto, Val::Px(track_bottom), Val::Px(0.0)),
                        h_track_opacity,
                    );
                } else {
                    self.hide_scrollbar_element(h_track);
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

                if h_thumb_visible {
                    let track_w = (visible_size.width
                        - border.left
                        - border.right
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
                        (visible_size.width / scroll_size.width).min(1.0)
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

                    let scroll_ratio = if scroll_size.width > visible_size.width {
                        (current_scroll.x / (scroll_size.width - visible_size.width))
                            .clamp(0.0, 1.0)
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

                    self.update_scrollbar_element(
                        h_thumb,
                        Size::new(Val::Px(thumb_width), Val::Px(thumb_height)),
                        Rect::new(Val::Px(thumb_y), Val::Auto, Val::Auto, Val::Px(thumb_x)),
                        h_thumb_opacity,
                    );
                } else {
                    self.hide_scrollbar_element(h_thumb);
                }
            }
        }
    }
}
