pub mod scrollbar;

pub use scrollbar::*;

use crate::{
    ActiveMasksSecondary, ActiveTransitionsSparse, BasicLayout, CapacityConfig, ChildrenSecondary,
    ComponentMask, Context, DebugStore, EdgeInsets, EntityId, FlexDirection, FlexLayout,
    GridLayout, InteractionPropertiesSecondary, InteractionStyles, LayoutRect, Length, MichiuSoA,
    ParentsSecondary, PropertyList, Rect, RenderStore, StyleTarget, TaffyResultTraceExt, TextAlign,
    ThisStyle, VisualPropertiesSecondary, define_secondary, define_sparse_secondary, define_vec,
};
use slotmap::{SecondaryMap, SparseSecondaryMap};
use std::sync::Arc;
use taffy::TaffyTree;

define_secondary!(pub struct TaffyNodesSecondary(taffy::NodeId));
define_secondary!(pub struct BasicLayoutsSecondary(BasicLayout));
define_secondary!(pub struct FlexLayoutsSecondary(FlexLayout));
define_secondary!(pub struct BaseBasicLayoutsSecondary(BasicLayout));
define_secondary!(pub struct BaseFlexLayoutsSecondary(FlexLayout));
define_secondary!(pub struct ResolvedBasicSecondary(BasicLayout));
define_secondary!(pub struct ResolvedFlexSecondary(FlexLayout));

impl FlexLayoutsSecondary {
    #[inline]
    pub(crate) fn flex_direction(&self, id: EntityId) -> FlexDirection {
        self.find(id).map(|f| f.flex_direction).unwrap_or_default()
    }
}

impl ResolvedFlexSecondary {
    #[inline]
    pub(crate) fn text_algin(&self, id: EntityId) -> TextAlign {
        self.find(id).map(|f| f.text_align).unwrap_or_default()
    }
}

define_sparse_secondary!(pub struct GridLayoutsSparse(GridLayout));
define_sparse_secondary!(pub struct ResolvedGridSparse(GridLayout));

define_vec!(pub struct DirtyLayoutEntitiesVec(EntityId));

pub type TaffyTreeEntityId = taffy::TaffyTree<EntityId>;

pub struct LayoutStore {
    pub(crate) scrollbar: ScrollbarStore,
    pub(crate) lay_dirty_entities: DirtyLayoutEntitiesVec,
    pub(crate) lay_taffy_tree: TaffyTreeEntityId,
    pub(crate) lay_taffy_nodes: TaffyNodesSecondary,
    pub(crate) lay_basic: BasicLayoutsSecondary,
    pub(crate) lay_flex: FlexLayoutsSecondary,
    pub(crate) lay_grid: GridLayoutsSparse,
    pub(crate) lay_base_basic: BaseBasicLayoutsSecondary,
    pub(crate) lay_base_flex: BaseFlexLayoutsSecondary,
    pub(crate) lay_resolved_basic: ResolvedBasicSecondary,
    pub(crate) lay_resolved_flex: ResolvedFlexSecondary,
    pub(crate) lay_resolved_grid: ResolvedGridSparse,
}

impl Default for LayoutStore {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutStore {
    #[must_use]
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            scrollbar: ScrollbarStore::new(),
            lay_dirty_entities: DirtyLayoutEntitiesVec(Vec::new()),
            lay_taffy_tree: TaffyTree::new(),
            lay_taffy_nodes: TaffyNodesSecondary(SecondaryMap::new()),
            lay_basic: BasicLayoutsSecondary(SecondaryMap::new()),
            lay_flex: FlexLayoutsSecondary(SecondaryMap::new()),
            lay_grid: GridLayoutsSparse(SparseSecondaryMap::new()),
            lay_base_basic: BaseBasicLayoutsSecondary(SecondaryMap::new()),
            lay_base_flex: BaseFlexLayoutsSecondary(SecondaryMap::new()),
            lay_resolved_basic: ResolvedBasicSecondary(SecondaryMap::new()),
            lay_resolved_flex: ResolvedFlexSecondary(SecondaryMap::new()),
            lay_resolved_grid: ResolvedGridSparse(SparseSecondaryMap::new()),
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn with_capacity(c: &CapacityConfig) -> Self {
        Self {
            scrollbar: ScrollbarStore::with_capacity(c),
            lay_dirty_entities: DirtyLayoutEntitiesVec(Vec::with_capacity(c.lay_dirty_entities)),
            lay_taffy_tree: TaffyTree::with_capacity(c.lay_taffy_tree),
            lay_taffy_nodes: TaffyNodesSecondary(SecondaryMap::with_capacity(c.lay_taffy_nodes)),
            lay_basic: BasicLayoutsSecondary(SecondaryMap::with_capacity(c.lay_basic)),
            lay_flex: FlexLayoutsSecondary(SecondaryMap::with_capacity(c.lay_flex)),
            lay_grid: GridLayoutsSparse(SparseSecondaryMap::with_capacity(c.lay_grid)),
            lay_base_basic: BaseBasicLayoutsSecondary(SecondaryMap::with_capacity(
                c.lay_base_basic,
            )),
            lay_base_flex: BaseFlexLayoutsSecondary(SecondaryMap::with_capacity(c.lay_base_flex)),
            lay_resolved_basic: ResolvedBasicSecondary(SecondaryMap::with_capacity(
                c.lay_resolved_basic,
            )),
            lay_resolved_flex: ResolvedFlexSecondary(SecondaryMap::with_capacity(
                c.lay_resolved_flex,
            )),
            lay_resolved_grid: ResolvedGridSparse(SparseSecondaryMap::with_capacity(
                c.lay_resolved_grid,
            )),
        }
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.scrollbar.clear();
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
    }

    #[inline]
    pub(crate) fn despawn(&mut self, id: EntityId) {
        self.scrollbar.despawn(id);
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
    }
}

impl LayoutStore {
    /// 各スタイルの解決を1回のルックアップと1回のカスケード解決ループに統合
    #[track_caller]
    #[inline]
    pub(crate) fn resolve_active_layouts(
        id: EntityId,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSparse,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparse,
        debug: &mut DebugStore,
    ) -> (BasicLayout, FlexLayout, Option<GridLayout>) {
        let mut basic = lay_basic.find_or_default(id, debug);
        let mut flex = lay_flex.find_or_default(id, debug);
        let mut grid = lay_grid.find(id).cloned();

        let active_mask = topo_active_masks.at(id);

        // 幅・高さ・一括サイズに対して、現在トランジションアニメーションが駆動中であるかを走査
        let is_transitioning =
            LayoutStore::is_transition_currently_running(id, rnd_active_transitions);

        // 自身、または親先祖から focused / focus_visible のフォーカス関連スタイルを解決
        let [focused_style_resolved, focused_visible_style_resolved] = [
            ComponentMask::STATE_FOCUSED,
            ComponentMask::STATE_FOCUSED_VISIBLE,
        ]
        .map(|state| {
            RenderStore::resolv_focus_style(
                id,
                active_mask,
                state,
                topo_parents,
                rnd_visual,
                rnd_interaction,
            )
        });

        // 状態マッピング解決のルックアップとループを1回に集約
        LayoutStore::apply_interaction_styles(
            id,
            active_mask,
            &mut basic,
            &mut flex,
            &mut grid,
            &[focused_style_resolved, focused_visible_style_resolved],
            is_transitioning,
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
        lay_resolved_grid: &mut ResolvedGridSparse,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSparse,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparse,
        debug: &mut DebugStore,
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
            debug,
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
    ) -> &'a mut BasicLayout {
        if target == StyleTarget::Base {
            lay_base_basic.at_mut(id)
        } else {
            if !rnd_interaction.contains_key(id) {
                rnd_interaction.insert(id, InteractionStyles::default());
            }
            // 上で入れたばっかだから Some のはず
            let styles = rnd_interaction.at_mut(id);
            let style_ref = styles.get_style_target_mut(target);
            &mut Arc::make_mut(&mut style_ref.inner).basic_layout
        }
    }

    pub(crate) fn get_flex_layout_mut<'a>(
        id: EntityId,
        target: StyleTarget,
        lay_flex: &'a mut FlexLayoutsSecondary,
        rnd_interaction: &'a mut InteractionPropertiesSecondary,
    ) -> &'a mut FlexLayout {
        if target == StyleTarget::Base {
            lay_flex.at_mut(id)
        } else {
            if !rnd_interaction.contains_key(id) {
                rnd_interaction.insert(id, InteractionStyles::default());
            }
            // 上で入れたばっかだから Some のはず
            let styles = rnd_interaction.at_mut(id);
            let style_ref = styles.get_style_target_mut(target);
            &mut Arc::make_mut(&mut style_ref.inner).flex_layout
        }
    }

    pub(crate) fn is_transition_currently_running(
        id: EntityId,
        rnd_active_transitions: &ActiveTransitionsSparse,
    ) -> (bool, bool) {
        let Some(list) = rnd_active_transitions.find(id) else {
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
        let Some(interaction) = rnd_interaction.find(id) else {
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
        bar_styles: &ScrollbarStylesSparse,
    ) -> taffy::Style {
        let sb_style = bar_styles.find(id).map(|s| &s.style);

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

    #[inline]
    fn length_to_px(length: Length, reference: f32) -> f32 {
        match length {
            Length::Px(v) => v,
            Length::Percent(p) => reference * (p / 100.0),
        }
    }

    /// 解決済みの基本スタイルを `TaffyTree` のノードへ同期して適用。
    #[track_caller]
    #[inline]
    pub(crate) fn set_taffy_style(
        id: EntityId,
        basic: &BasicLayout,
        flex: &FlexLayout,
        grid: Option<&GridLayout>,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
        bar_styles: &ScrollbarStylesSparse,
        debug: &mut DebugStore,
    ) {
        let taffy_style = LayoutStore::resolve_taffy_style(id, basic, flex, grid, bar_styles);
        let nodes = *lay_taffy_nodes.at(id);
        lay_taffy_tree
            .set_style(nodes, taffy_style)
            .unwrap_or_trace(Some(id), debug);
    }

    #[track_caller]
    #[inline]
    pub(crate) fn local_rect_from_taffy(
        id: EntityId,
        lay_taffy_tree: &TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
    ) -> LayoutRect {
        let taffy_node = *lay_taffy_nodes.at(id);

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
    #[track_caller]
    #[inline]
    pub(crate) fn resync_taffy_children_order(
        parent_id: EntityId,
        topo_children: &ChildrenSecondary,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
        debug: &mut DebugStore,
    ) {
        let parent_node = *lay_taffy_nodes.at(parent_id);
        // 一旦現在登録されているすべての子ノードを Taffy 側から安全にデタッチ
        if let Ok(taffy_children) = lay_taffy_tree.children(parent_node) {
            for child_node in taffy_children {
                lay_taffy_tree
                    .remove_child(parent_node, child_node)
                    .unwrap_or_trace(Some(parent_id), debug);
            }
        }

        // 最新の並び替え順序リストの存在チェック
        let children_list = topo_children.at(parent_id);
        // 最新の順序に従って、Taffy 側に再アタッチ
        for &child_id in children_list {
            let child_node = *lay_taffy_nodes.at(child_id);
            lay_taffy_tree
                .add_child(parent_node, child_node)
                .unwrap_or_trace(Some(child_id), debug);
        }
    }

    #[inline]
    pub(crate) fn clear_layout_dirty(
        topo_active_masks: &mut ActiveMasksSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
    ) {
        for id in lay_dirty_entities.drain(..) {
            if let Some(mask) = topo_active_masks.find_mut(id) {
                mask.unset(ComponentMask::STATE_QUEUED_LAYOUT);
            }
        }
    }

    #[track_caller]
    #[inline]
    pub(crate) fn mark_layout_dirty(
        id: EntityId,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
        debug: &mut DebugStore,
    ) {
        let mut curr = id;
        // Taffy 側の該当ノードのレイアウトキャッシュを無効化
        let taffy_node = *lay_taffy_nodes.at(curr);
        lay_taffy_tree
            .mark_dirty(taffy_node)
            .unwrap_or_trace(Some(curr), debug);

        loop {
            // マスクが存在する場合のみDirtyマーク
            if let Some(mask) = topo_active_masks.find_mut(curr) {
                // すでに登録済みなら多重登録を防ぐため探索を早期ブレイク
                if mask.has(ComponentMask::STATE_QUEUED_LAYOUT) {
                    break;
                }
                mask.set(ComponentMask::STATE_QUEUED_LAYOUT); // 自身を Dirty マーク
                lay_dirty_entities.push(curr);
            }

            // 親要素（先祖）をルートまで辿って Dirty フラグを連鎖伝播させる
            let Some(parent_id) = topo_parents.at(curr) else {
                break;
            };
            curr = *parent_id;
        }
    }

    #[inline]
    pub fn base_basic_mut(&mut self) -> &mut BaseBasicLayoutsSecondary {
        &mut self.lay_base_basic
    }

    #[inline]
    pub fn base_flex_mut(&mut self) -> &mut BaseFlexLayoutsSecondary {
        &mut self.lay_base_flex
    }

    #[inline]
    pub fn basic_mut(&mut self) -> &mut BasicLayoutsSecondary {
        &mut self.lay_basic
    }

    #[inline]
    pub fn dirty_entities_mut(&mut self) -> &mut DirtyLayoutEntitiesVec {
        &mut self.lay_dirty_entities
    }

    #[inline]
    pub fn flex_mut(&mut self) -> &mut FlexLayoutsSecondary {
        &mut self.lay_flex
    }

    #[inline]
    pub fn grid_mut(&mut self) -> &mut GridLayoutsSparse {
        &mut self.lay_grid
    }
    #[inline]
    pub fn resolved_basic_mut(&mut self) -> &mut ResolvedBasicSecondary {
        &mut self.lay_resolved_basic
    }

    #[inline]
    pub fn resolved_flex_mut(&mut self) -> &mut ResolvedFlexSecondary {
        &mut self.lay_resolved_flex
    }

    #[inline]
    pub fn resolved_grid_mut(&mut self) -> &mut ResolvedGridSparse {
        &mut self.lay_resolved_grid
    }

    #[inline]
    pub fn taffy_nodes_mut(&mut self) -> &mut TaffyNodesSecondary {
        &mut self.lay_taffy_nodes
    }

    #[inline]
    pub fn taffy_tree_mut(&mut self) -> &mut TaffyTree<EntityId> {
        &mut self.lay_taffy_tree
    }

    #[inline]
    pub fn bar_styles_mut(&mut self) -> &mut ScrollbarStylesSparse {
        &mut self.scrollbar.bar_styles
    }
}

impl Context {
    #[inline]
    pub(crate) fn get_basic_layout_mut(
        &mut self,
        id: EntityId,
        target: StyleTarget,
    ) -> &mut BasicLayout {
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
    ) -> &mut FlexLayout {
        LayoutStore::get_flex_layout_mut(
            id,
            target,
            &mut self.layouts.lay_flex,
            &mut self.renders.rnd_interaction,
        )
    }
}

#[cfg(test)]
mod tests;
