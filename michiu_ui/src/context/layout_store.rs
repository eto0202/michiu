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

pub struct LayoutStore {
    pub(crate) basic_layouts: SecondaryMap<EntityId, BasicLayout>,
    pub(crate) flex_layouts: SecondaryMap<EntityId, FlexLayout>,
    pub(crate) grid_layouts: SparseSecondaryMap<EntityId, GridLayout>,
    pub(crate) scrollbar_styles: SparseSecondaryMap<EntityId, ScrollBarState>,
    pub(crate) taffy_nodes: SecondaryMap<EntityId, taffy::NodeId>,
    pub(crate) taffy: taffy::TaffyTree<EntityId>,
    pub(crate) flat_dfs_sequence: Vec<EntityId>,
    pub(crate) is_structure_dirty: bool,
    pub(crate) dirty_layout_entities: Vec<EntityId>,
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
            flex_layouts: SecondaryMap::new(),
            grid_layouts: SparseSecondaryMap::new(),
            scrollbar_styles: SparseSecondaryMap::new(),
            taffy_nodes: SecondaryMap::new(),
            taffy: TaffyTree::new(),
            flat_dfs_sequence: Vec::new(),
            is_structure_dirty: true,
            dirty_layout_entities: Vec::new(),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.basic_layouts.clear();
        self.flex_layouts.clear();
        self.grid_layouts.clear();
        self.scrollbar_styles.clear();
        self.taffy_nodes.clear();
        self.taffy = TaffyTree::new();
        self.flat_dfs_sequence.clear();
        self.is_structure_dirty = true;
        self.dirty_layout_entities.clear();
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.basic_layouts.remove(id);
        self.flex_layouts.remove(id);
        self.grid_layouts.remove(id);
        self.scrollbar_styles.remove(id);
        self.taffy_nodes.remove(id);
        self.dirty_layout_entities.retain(|&x| x != id);
        self.flat_dfs_sequence.retain(|&x| x != id);
    }
}

impl LayoutStore {
    /// 各スタイルの解決を1回のルックアップと1回のカスケード解決ループに統合
    pub(crate) fn resolve_active_layouts(
        id: EntityId,
        topology: &TopologyStore,
        layouts: &LayoutStore,
        renders: &RenderStore,
    ) -> (BasicLayout, FlexLayout, Option<GridLayout>) {
        let mut basic = layouts.basic_layouts.get(id).copied().unwrap_or_default();
        let mut flex = layouts.flex_layouts.get(id).copied().unwrap_or_default();
        let mut grid = layouts.grid_layouts.get(id).cloned();

        let active_mask = topology.active_masks[id];

        // 幅・高さ・一括サイズに対して、現在トランジションアニメーションが駆動中であるかを走査
        let is_width_transitioning = renders
            .active_transitions
            .get(id)
            .map(|list| {
                list.iter().any(|t| {
                    t.property_list == PropertyList::Width || t.property_list == PropertyList::Size
                })
            })
            .unwrap_or(false);
        let is_height_transitioning = renders
            .active_transitions
            .get(id)
            .map(|list| {
                list.iter().any(|t| {
                    t.property_list == PropertyList::Height || t.property_list == PropertyList::Size
                })
            })
            .unwrap_or(false);

        // 自身のフォーカススタイルが無い場合、親先祖要素が自身のために定義している focused スタイルを抽出
        let focus_style_resolved = if active_mask.has(STATE_FOCUSED) {
            if let Some(interaction) = renders.interaction_properties.get(id)
                && let Some(ref self_f_style) = interaction.focused
            {
                Some(self_f_style.clone()) // 自身に明確な focused 指定があれば最優先
            } else {
                let focus_mode = renders
                    .visual_properties
                    .get(id)
                    .and_then(|v| v.focusable)
                    .unwrap_or(Focusable::None);

                if matches!(focus_mode, Focusable::Inherit(_)) {
                    // 親先祖を上に辿り、最初に focused 疑似スタイルを定義している要素のその設定をそのまま借用する
                    let mut curr = topology.parents.get(id).copied().flatten();
                    let mut found_parent_focused_style = None;
                    while let Some(curr_id) = curr {
                        if let Some(parent_interaction) =
                            renders.interaction_properties.get(curr_id)
                            && let Some(ref parent_f_style) = parent_interaction.focused
                        {
                            found_parent_focused_style = Some(parent_f_style.clone());
                            break;
                        }
                        curr = topology.parents.get(curr_id).copied().flatten();
                    }
                    found_parent_focused_style
                } else {
                    None
                }
            }
        } else {
            None
        };

        // 状態マッピング解決のルックアップとループを1回に集約
        if let Some(interaction) = renders.interaction_properties.get(id) {
            let cascade = [
                (STATE_FOCUSED, &focus_style_resolved),
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

    // Taffyスタイルを一括解決するヘルパー
    pub(crate) fn resolve_taffy_style(
        id: EntityId,
        layouts: &LayoutStore,
        basic: &BasicLayout,
        flex: &FlexLayout,
        grid: Option<&GridLayout>,
    ) -> taffy::Style {
        let sb_style = layouts.scrollbar_styles.get(id).map(|s| &s.style);

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
    pub(crate) fn get_physical_border(
        id: EntityId,
        basic: &BasicLayout,
        outputs: &OutputStore,
    ) -> EdgeInsets {
        let rect = outputs.rects.get(id).copied().unwrap_or(LayoutRect::ZERO);

        EdgeInsets {
            top: LayoutStore::resolve_length_to_px(basic.border.top, rect.height),
            right: LayoutStore::resolve_length_to_px(basic.border.right, rect.width),
            bottom: LayoutStore::resolve_length_to_px(basic.border.bottom, rect.height),
            left: LayoutStore::resolve_length_to_px(basic.border.left, rect.width),
        }
    }

    /// 指定された要素の現在解決されている物理パディング（EdgeInsets）を取得します。
    pub(crate) fn get_physical_padding(
        id: EntityId,
        basic: &BasicLayout,
        outputs: &OutputStore,
    ) -> EdgeInsets {
        let rect = outputs.rects.get(id).copied().unwrap_or(LayoutRect::ZERO);

        EdgeInsets {
            top: LayoutStore::resolve_length_to_px(basic.padding.top, rect.height),
            right: LayoutStore::resolve_length_to_px(basic.padding.right, rect.width),
            bottom: LayoutStore::resolve_length_to_px(basic.padding.bottom, rect.height),
            left: LayoutStore::resolve_length_to_px(basic.padding.left, rect.width),
        }
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
    fn resolve_length_to_px(length: Length, reference: f32) -> f32 {
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
        let mut scrollbar_el_ids = HashSet::new();
        for sb_state in scrollbar_styles.values() {
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
        scrollbar_el_ids
    }

    /// スクロールバー用要素（TrackやThumb）のレイアウト情報（解決値と静的ベース値）をアトミックに同時同期して更新します。
    pub(crate) fn update_scrollbar_element_layout(
        id: EntityId,
        layouts: &mut LayoutStore,
        renders: &mut RenderStore,
        size: Size<Val>,
        inset: Rect<Val>,
    ) {
        let display = Display::Flex;

        let apply = |layout: &mut BasicLayout| {
            layout.display = display;
            layout.size = size;
            layout.inset = inset;
        };

        // 1. LayoutStore 側の解決値（basic_layouts）を更新
        if let Some(layout) = layouts.basic_layouts.get_mut(id) {
            apply(layout);
        }
        // 2. RenderStore 側のベース静的値（base_basic_layouts）を同時更新
        if let Some(layout) = renders.base_basic_layouts.get_mut(id) {
            apply(layout);
        }

        // affy 側のノードスタイルも Display::None にして即時同期
        let node = layouts.taffy_nodes[id];
        let _ = layouts.taffy.set_style(
            node,
            taffy::Style {
                display: taffy::Display::None,
                ..Default::default()
            },
        );
    }

    /// 解決済みの基本スタイルを TaffyTree のノードへ即時同期して適用します。
    #[inline]
    pub(crate) fn set_taffy_style(
        id: EntityId,
        layouts: &mut LayoutStore,
        basic: &BasicLayout,
        flex: &FlexLayout,
        grid: Option<&GridLayout>,
    ) {
        let taffy_style = LayoutStore::resolve_taffy_style(id, layouts, basic, flex, grid);
        let node = layouts.taffy_nodes[id];
        let _ = layouts.taffy.set_style(node, taffy_style);
    }

    /// スクロールバー用要素をレイアウト上から安全に隠します。
    #[inline]
    pub(crate) fn hide_scrollbar_element(
        id: EntityId,
        layouts: &mut LayoutStore,
        renders: &mut RenderStore,
    ) {
        let hide = |layout: &mut BasicLayout| {
            layout.display = Display::None;
        };
        if let Some(layout) = layouts.basic_layouts.get_mut(id) {
            hide(layout);
        }
        if let Some(layout) = renders.base_basic_layouts.get_mut(id) {
            hide(layout);
        }
    }

    /// 非再帰スタックによるフラットDFS配列の高速構築
    pub(crate) fn rebuild_flat_dfs_sequence(
        root: EntityId,
        layouts: &mut LayoutStore,
        topology: &TopologyStore,
    ) {
        layouts.flat_dfs_sequence.clear();

        // あらかじめ実用的なスタック深度を確保しておきメモリ再確保を削減
        let mut stack = Vec::with_capacity(32);
        stack.push(root);

        while let Some(id) = stack.pop() {
            layouts.flat_dfs_sequence.push(id);

            // 左側の子が先にポップされるように、右側（末尾）の子から逆順にスタックへプッシュ
            if let Some(children) = topology.children.get(id) {
                let len = children.len();
                for i in (0..len).rev() {
                    stack.push(children[i]);
                }
            }
        }

        layouts.is_structure_dirty = false;
    }

    pub(crate) fn local_rect_from_taffy(id: EntityId, layouts: &LayoutStore) -> LayoutRect {
        if let Some(&taffy_node) = layouts.taffy_nodes.get(id) {
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
        } else {
            LayoutRect::ZERO
        }
    }

    /// 指定された親コンテナにアタッチされている DComp / Taffy 側のすべての子ノードの物理順序を
    /// 内部 SoA リスト（self.children）の順序に沿って一括して再同期）します。
    pub(crate) fn resync_taffy_children_order(
        parent_id: EntityId,
        layouts: &mut LayoutStore,
        topology: &TopologyStore,
    ) {
        if let Some(&parent_node) = layouts.taffy_nodes.get(parent_id) {
            // 一旦現在登録されているすべての子ノードを Taffy 側から安全にデタッチ
            if let Ok(taffy_children) = layouts.taffy.children(parent_node) {
                for child_node in taffy_children {
                    let _ = layouts.taffy.remove_child(parent_node, child_node);
                }
            }
            // 最新の並び替え順序リストの順に従って、Taffy 側に再アタッチ
            if let Some(children_list) = topology.children.get(parent_id).cloned() {
                for child_id in children_list {
                    if let Some(&child_node) = layouts.taffy_nodes.get(child_id) {
                        let _ = layouts.taffy.add_child(parent_node, child_node);
                    }
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
}

impl Context {
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
        LayoutStore::resolve_active_layouts(id, &self.topology, &self.layouts, &self.renders)
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
        LayoutStore::resolve_taffy_style(id, &self.layouts, basic, flex, grid)
    }

    /// 指定された要素の現在解決されている物理ボーダー（EdgeInsets）を取得します。
    pub(crate) fn get_physical_border(&self, id: EntityId, basic: &BasicLayout) -> EdgeInsets {
        LayoutStore::get_physical_border(id, basic, &self.outputs)
    }

    /// 指定された要素の現在解決されている物理パディング（EdgeInsets）を取得します。
    pub(crate) fn get_physical_padding(&self, id: EntityId, basic: &BasicLayout) -> EdgeInsets {
        LayoutStore::get_physical_padding(id, basic, &self.outputs)
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
        LayoutStore::update_scrollbar_element_layout(
            id,
            &mut self.layouts,
            &mut self.renders,
            size,
            inset,
        );
        RenderStore::update_scrollbar_element_opacity(id, &mut self.renders, opacity);

        let (basic, flex, grid) =
            LayoutStore::resolve_active_layouts(id, &self.topology, &self.layouts, &self.renders);

        LayoutStore::set_taffy_style(id, &mut self.layouts, &basic, &flex, grid.as_ref());
    }

    /// 解決済みの基本スタイルを TaffyTree のノードへ即時同期して適用します。
    #[inline]
    pub(crate) fn set_taffy_style(
        &mut self,
        id: EntityId,
        basic: &BasicLayout,
        flex: &FlexLayout,
        grid: Option<&GridLayout>,
    ) {
        LayoutStore::set_taffy_style(id, &mut self.layouts, basic, flex, grid);
    }

    /// スクロールバー用要素をレイアウト上から安全に隠します。
    #[inline]
    pub(crate) fn hide_scrollbar_element(&mut self, id: EntityId) {
        LayoutStore::hide_scrollbar_element(id, &mut self.layouts, &mut self.renders);
    }

    /// 非再帰スタックによるフラットDFS配列の高速構築
    #[inline]
    pub(crate) fn rebuild_flat_dfs_sequence(&mut self, root: EntityId) {
        LayoutStore::rebuild_flat_dfs_sequence(root, &mut self.layouts, &self.topology);
    }

    #[inline]
    pub(crate) fn local_rect_from_taffy(&self, id: EntityId) -> LayoutRect {
        LayoutStore::local_rect_from_taffy(id, &self.layouts)
    }

    /// 指定された親コンテナにアタッチされている DComp / Taffy 側のすべての子ノードの物理順序を
    /// 内部 SoA リスト（self.children）の順序に沿って一括して再同期）します。
    #[inline]
    pub(crate) fn resync_taffy_children_order(&mut self, parent_id: EntityId) {
        LayoutStore::resync_taffy_children_order(parent_id, &mut self.layouts, &self.topology);
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

            let b = self.get_physical_border(id, &basic);
            let p = self.get_physical_padding(id, &basic);

            // 枠線と余白を足した、物理的にこれ以上小さくできない限界サイズ
            let abs_min_w = b.left + b.right + p.left + p.right;
            let abs_min_h = b.top + b.bottom + p.top + p.bottom;

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
        if let Some(layout) = self.renders.base_basic_layouts.get_mut(id) {
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

            let border = self.get_physical_border(id, &basic);
            let padding = self.get_physical_padding(id, &basic);

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
