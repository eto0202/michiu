use std::collections::HashSet;

use crate::*;
use slotmap::{SecondaryMap, SparseSecondaryMap};
use taffy::TaffyTree;

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
        &self,
        id: EntityId,
        active_masks: &SecondaryMap<EntityId, ComponentMask>,
        parents: &SecondaryMap<EntityId, Option<EntityId>>,
        renders: &RenderStore,
    ) -> (BasicLayout, FlexLayout, Option<GridLayout>) {
        let mut basic = self.basic_layouts.get(id).copied().unwrap_or_default();
        let mut flex = self.flex_layouts.get(id).copied().unwrap_or_default();
        let mut grid = self.grid_layouts.get(id).cloned();

        let active_mask = active_masks[id];

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
                    let mut curr = parents.get(id).copied().flatten();
                    let mut found_parent_focused_style = None;
                    while let Some(curr_id) = curr {
                        if let Some(parent_interaction) =
                            renders.interaction_properties.get(curr_id)
                            && let Some(ref parent_f_style) = parent_interaction.focused
                        {
                            found_parent_focused_style = Some(parent_f_style.clone());
                            break;
                        }
                        curr = parents.get(curr_id).copied().flatten();
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
        &self,
        id: EntityId,
        layouts: (&BasicLayout, &FlexLayout, Option<&GridLayout>),
    ) -> taffy::Style {
        let (basic, flex, grid) = (layouts.0, layouts.1, layouts.2);
        let sb_style = self.scrollbar_styles.get(id).map(|s| &s.style);

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
        &self,
        id: EntityId,
        basic: &BasicLayout,
        outputs: &OutputStore,
    ) -> EdgeInsets {
        let rect = outputs.rects.get(id).copied().unwrap_or(LayoutRect::ZERO);

        EdgeInsets {
            top: self.resolve_length_to_px(basic.border.top, rect.height),
            right: self.resolve_length_to_px(basic.border.right, rect.width),
            bottom: self.resolve_length_to_px(basic.border.bottom, rect.height),
            left: self.resolve_length_to_px(basic.border.left, rect.width),
        }
    }

    /// 指定された要素の現在解決されている物理パディング（EdgeInsets）を取得します。
    pub(crate) fn get_physical_padding(
        &self,
        id: EntityId,
        basic: &BasicLayout,
        outputs: &OutputStore,
    ) -> EdgeInsets {
        let rect = outputs.rects.get(id).copied().unwrap_or(LayoutRect::ZERO);

        EdgeInsets {
            top: self.resolve_length_to_px(basic.padding.top, rect.height),
            right: self.resolve_length_to_px(basic.padding.right, rect.width),
            bottom: self.resolve_length_to_px(basic.padding.bottom, rect.height),
            left: self.resolve_length_to_px(basic.padding.left, rect.width),
        }
    }

    #[inline]
    fn resolve_length_to_px(&self, length: Length, reference: f32) -> f32 {
        match length {
            Length::Px(v) => v,
            Length::Percent(p) => reference * (p / 100.0),
        }
    }

    // 全スクロールバー関連IDを一括抽出
    #[inline]
    pub(crate) fn scrollbar_el_ids(&self) -> HashSet<EntityId> {
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
        scrollbar_el_ids
    }

    /// スクロールバー用要素（TrackやThumb）のレイアウト情報（解決値と静的ベース値）をアトミックに同時同期して更新します。
    pub(crate) fn update_scrollbar_element_layout(
        &mut self,
        id: EntityId,
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
        if let Some(layout) = self.basic_layouts.get_mut(id) {
            apply(layout);
        }
        // 2. RenderStore 側のベース静的値（base_basic_layouts）を同時更新
        if let Some(layout) = renders.base_basic_layouts.get_mut(id) {
            apply(layout);
        }

        // affy 側のノードスタイルも Display::None にして即時同期
        let node = self.taffy_nodes[id];
        let _ = self.taffy.set_style(
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
        &mut self,
        id: EntityId,
        layouts: (&BasicLayout, &FlexLayout, Option<&GridLayout>),
    ) {
        let taffy_style = self.resolve_taffy_style(id, layouts);
        let node = self.taffy_nodes[id];
        let _ = self.taffy.set_style(node, taffy_style);
    }

    /// スクロールバー用要素をレイアウト上から安全に隠します。
    #[inline]
    pub(crate) fn hide_scrollbar_element(&mut self, id: EntityId, renders: &mut RenderStore) {
        let hide = |layout: &mut BasicLayout| {
            layout.display = Display::None;
        };
        if let Some(layout) = self.basic_layouts.get_mut(id) {
            hide(layout);
        }
        if let Some(layout) = renders.base_basic_layouts.get_mut(id) {
            hide(layout);
        }
    }
}
