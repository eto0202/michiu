pub mod dnd;
pub mod resize;
pub use dnd::*;
pub use resize::*;

use crate::{
    ActiveAnimationsSparseSecondary, ActiveEntitiesVec, ActiveFocusTrigger, ActiveMasksSecondary,
    ActiveTransitionsSparseSecondary, BaseBasicLayoutsSecondary, BaseVisualPropertiesSecondary,
    BasicLayout, BasicLayoutsSecondary, CapacityConfig, ChildrenSecondary, ClipRectsSecondary,
    ComponentMask, ContentStore, Context, CursorIcon, DfsIndicesSecondary, DirtyLayoutEntitiesVec,
    DirtyRenderEntitiesVec, DwriteLayoutsSparseSecondary, EffectiveZindicesSecondary, Element,
    ElementEffectsSecondary, ElementState, EntitiesSlot, EntityId, EventListeners,
    FlatDfsSequenceVec, FlexLayout, FlexLayoutsSecondary, FocusTrigger, Focusable, GridLayout,
    GridLayoutsSparseSecondary, InputContents, InputContentsSparseSecondary,
    InteractionPropertiesSecondary, LayoutPoint, LayoutRect, LayoutSize, LayoutStore, Length,
    Modifiers, MouseButton, OutputStore, Overflow, ParentsSecondary, Pipeline, PointerEvents,
    Position, ReactiveStore, Rect, RectsSecondary, RenderStore, ResolvedBasicSecondary,
    ResolvedFlexSecondary, ResolvedGridSparseSecondary, ScrollOffsetsSecondary,
    ScrollSizesSecondary, ScrollbarStylesSecondary, SelectedRectsSparseSecondary,
    SelectionStartIndexSparseSecondary, SessionSpawnedVec, SortedEntitiesVec, SystemStore,
    TaffyNodesSecondary, TaffyTreeEntityId, TextAlign, TextContentsSparseSecondary, TextEngine,
    TextSelectionsSparseSecondary, TextSpansSparseSecondary, TopoSortCacheVec, TopologyStore,
    UserSelect, Val, VirtualKey, VisualPropertiesSecondary, WindowStore, bind_context,
    handle_on_active, handle_on_blur, handle_on_click, handle_on_cursor_moved, handle_on_disable,
    handle_on_dnd_drag_start, handle_on_dnd_entity_drag, handle_on_dnd_entity_drop,
    handle_on_dnd_id_drag, handle_on_dnd_id_drop, handle_on_drag, handle_on_focus, handle_on_hover,
    handle_on_keyboard_input, handle_on_mouse_enter, handle_on_mouse_input, handle_on_mouse_leave,
    handle_on_mouse_wheel, handle_on_right_click, handle_on_select,
};
use slotmap::{SecondaryMap, SparseSecondaryMap};
use smallvec::SmallVec;
use std::{borrow::Cow, path::PathBuf};
use windows::Win32::Graphics::DirectWrite::IDWriteTextLayout;

/// 実行時にウィンドウ内で現在アクティブ（排他的）になっている、各状態の対象要素（EntityId）を管理します。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct InteractionStates {
    pub hovered: Option<EntityId>,
    pub focused: Option<EntityId>,
    pub pressed: Option<EntityId>,
    pub dragged: Option<EntityId>,
}

impl InteractionStates {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn clear_entity(&mut self, id: EntityId) {
        if self.hovered == Some(id) {
            self.hovered = None;
        }
        if self.focused == Some(id) {
            self.focused = None;
        }
        if self.pressed == Some(id) {
            self.pressed = None;
        }
        if self.dragged == Some(id) {
            self.dragged = None;
        }
    }
}

pub(crate) type EventListenersSparseSecondary = SparseSecondaryMap<EntityId, EventListeners>;

pub struct EventStore {
    pub(crate) dnd: DndStore,
    pub(crate) resize: ResizeStore,
    pub(crate) evt_listeners: EventListenersSparseSecondary,
    pub(crate) evt_interaction_states: InteractionStates,
    pub(crate) evt_current_pointer_position: Option<LayoutPoint>,
}

impl Default for EventStore {
    fn default() -> Self {
        Self::new()
    }
}

impl EventStore {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            dnd: DndStore::new(),
            resize: ResizeStore::new(),
            evt_listeners: SparseSecondaryMap::new(),
            evt_interaction_states: InteractionStates::new(),
            evt_current_pointer_position: None,
        }
    }

    #[inline]
    #[must_use]
    pub fn with_capacity(c: &CapacityConfig) -> Self {
        Self {
            dnd: DndStore::with_capacity(c),
            evt_listeners: SparseSecondaryMap::with_capacity(c.evt_listeners),
            ..Default::default()
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.dnd.clear();
        self.resize.clear();
        self.evt_listeners.clear();
        self.evt_interaction_states = InteractionStates::new();
        self.evt_current_pointer_position = None;
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.dnd.despawn(id);
        self.evt_listeners.remove(id);
        self.evt_interaction_states.clear_entity(id);
    }
}

impl EventStore {
    pub(crate) fn pressed_local_point(
        id: EntityId,
        logical_pos: LayoutPoint,
        dw_layout: Option<&IDWriteTextLayout>,
        sys_text_engine: &TextEngine,
        cont_input_contents: &InputContentsSparseSecondary,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparseSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        out_rects: &RectsSecondary,
        out_scroll_offsets: &ScrollOffsetsSecondary,
    ) -> LayoutPoint {
        let rect = out_rects.get(id).copied().unwrap_or_default();

        let basic = lay_resolved_basic.get(id).copied().unwrap_or_default();
        let flex = lay_resolved_flex.get(id).copied().unwrap_or_default();
        let _ = lay_resolved_grid.get(id).cloned().unwrap_or_default(); // TODO: Grid実装時用

        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

        let scroll = out_scroll_offsets.get(id).copied().unwrap_or_default();

        let (text_size, is_multiline) = if let Some(contents) = cont_input_contents.get(id) {
            let size = contents
                .last_layout
                .map_or(LayoutSize::ZERO, |r| LayoutSize::new(r.width, r.height));
            (size, contents.is_multiline)
        } else if let Some(dw_layout) = dw_layout {
            (sys_text_engine.get_layout_size(dw_layout), false)
        } else {
            (LayoutSize::ZERO, false)
        };

        let align_offset = OutputStore::calc_align_offset(
            rect,
            border,
            padding,
            text_size,
            flex.text_align,
            flex.align_items,
            is_multiline,
        );

        let local_x =
            logical_pos.x - (rect.x + border.left + padding.left + align_offset.x) + scroll.x;
        let local_y =
            logical_pos.y - (rect.y + border.top + padding.top + align_offset.y) + scroll.y;

        LayoutPoint {
            x: local_x,
            y: local_y,
        }
    }

    pub(crate) fn resolve_hover_state(cx: &mut Context, target_id: Option<EntityId>) {
        let old_id = cx.events.evt_interaction_states.hovered;

        if old_id == target_id {
            return;
        }

        cx.events.evt_interaction_states.hovered = target_id;

        // 旧ホバー要素からマウスが去った
        if let Some(old_id) = old_id {
            EventStore::update_state(cx, old_id, ComponentMask::STATE_HOVERED, false);
            handle_on_mouse_leave(cx, old_id);
        }

        // 新ホバー要素にマウスが入った
        if let Some(new_id) = target_id {
            EventStore::update_state(cx, new_id, ComponentMask::STATE_HOVERED, true);
            handle_on_mouse_enter(cx, new_id);
            handle_on_hover(cx, new_id);
        }
    }

    /// 各インタラクション状態（ステート）を更新し、レイアウト変更を伴うか自動的に判別して Dirty フラグを制御する共通ヘルパー
    #[inline]
    pub(crate) fn update_state(cx: &mut Context, id: EntityId, state_flag: u128, actived: bool) {
        let mut was_active = false;
        let mut state_changed = false;

        let Some(mask) = cx.topology.topo_active_masks.get_mut(id) else {
            return;
        };

        was_active = mask.has(state_flag);
        if was_active == actived {
            return;
        }

        state_changed = true;

        if actived {
            mask.set(state_flag);
        } else {
            mask.unset(state_flag);
        }

        let mut resolve_element = |cx: &mut Context, id: EntityId| {
            RenderStore::resolve_element_style_state(
                id,
                true,
                cx.window.win_last_size.as_ref(),
                &cx.system.sys_dwrite_layouts,
                &cx.reactive.react_element_effects,
                &cx.contents.cont_input_contents,
                &mut cx.topology.topo_active_masks,
                &mut cx.topology.topo_is_sort_dirty,
                &cx.topology.topo_entities,
                &cx.topology.topo_parents,
                &cx.topology.topo_children,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_taffy_tree,
                &mut cx.layouts.lay_basic,
                &cx.layouts.lay_taffy_nodes,
                &cx.layouts.lay_base_basic,
                &mut cx.renders.rnd_dirty_entities,
                &mut cx.renders.rnd_visual,
                &mut cx.renders.rnd_active_transitions,
                &mut cx.renders.rnd_active_animations,
                &cx.renders.rnd_base_visual,
                &cx.renders.rnd_interaction,
                &cx.outputs.out_rects,
            );

            // この要素のアクティブレイアウトキャッシュを差分更新
            LayoutStore::update_resolved_active_layout_cache(
                id,
                &cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_resolved_basic,
                &mut cx.layouts.lay_resolved_flex,
                &mut cx.layouts.lay_resolved_grid,
                &cx.layouts.lay_basic,
                &cx.layouts.lay_flex,
                &cx.layouts.lay_grid,
                &cx.renders.rnd_visual,
                &cx.renders.rnd_interaction,
                &cx.renders.rnd_active_transitions,
            );
        };

        let mark_dirty = |cx: &mut Context, id: EntityId| {
            if RenderStore::does_state_require_layout(id, state_flag, &cx.renders.rnd_interaction) {
                LayoutStore::mark_layout_dirty(
                    id,
                    &mut cx.topology.topo_active_masks,
                    &cx.topology.topo_parents,
                    &mut cx.layouts.lay_dirty_entities,
                    &mut cx.layouts.lay_taffy_tree,
                    &cx.layouts.lay_taffy_nodes,
                );
            }
            RenderStore::mark_render_dirty(
                id,
                &mut cx.topology.topo_active_masks,
                &mut cx.renders.rnd_dirty_entities,
            );
        };

        // 状態変化の発生時に即座に動的なスタイルを解決する
        resolve_element(cx, id);

        // 親から子方向へのスタイル解決の伝播
        if let Some(child) = cx.topology.topo_children.get(id).cloned() {
            for child_id in child {
                let has_parent = cx
                    .topology
                    .topo_active_masks
                    .get(child_id)
                    .is_some_and(|m| m.has(ComponentMask::STYLE_INTERACTION_PARENT));

                if has_parent {
                    resolve_element(cx, child_id);
                    mark_dirty(cx, child_id);
                }
            }
        }

        // STYLE_INTERACTION_WITHIN マスク判定による親先祖の早期バイパス
        let mut curr = id;
        while let Some(parent_id) = cx.topology.topo_parents.get(curr).copied().flatten() {
            if cx.topology.topo_entities.contains_key(parent_id) {
                let has_within = cx
                    .topology
                    .topo_active_masks
                    .get(parent_id)
                    .is_some_and(|m| m.has(ComponentMask::STYLE_INTERACTION_WITHIN));

                // 先祖要素が within スタイルを持っている場合のみそのスタイル評価を実行
                if has_within {
                    resolve_element(cx, parent_id);
                    mark_dirty(cx, parent_id);
                }
            }
            curr = parent_id;
        }

        // 状態変化による本要素のレイアウト汚染チェック
        mark_dirty(cx, id);

        if !state_changed {
            return;
        }

        // 残りの状態遷移イベントの解決
        if actived {
            match state_flag {
                ComponentMask::STATE_DISABLED => handle_on_disable(cx, id),
                ComponentMask::STATE_ACTIVED => handle_on_active(cx, id),
                ComponentMask::STATE_SELECTED => handle_on_select(cx, id),

                _ => {}
            }
        }
    }

    #[inline]
    pub(crate) fn propagate_cursor_move_events(
        cx: &mut Context,
        target_id: Option<EntityId>,
        logical_pos: LayoutPoint,
    ) {
        let Some(id) = target_id else {
            return;
        };

        let has_listener = cx
            .events
            .evt_listeners
            .get(id)
            .is_some_and(|l| l.on_cursor_moved.is_some());

        if has_listener {
            let rect = &cx.outputs.out_rects.get(id).copied().unwrap_or_default();
            let relative_pos = LayoutPoint::new(logical_pos.x - rect.x, logical_pos.y - rect.y);
            handle_on_cursor_moved(cx, id, relative_pos);
        }
    }

    #[inline]
    pub(crate) fn calculate_text_selection(
        start_pos: usize,
        local: LayoutPoint,
        dw_layout: &IDWriteTextLayout,
        sys_text_engine: &TextEngine,
    ) -> (std::ops::Range<usize>, bool) {
        let (current_index, is_trailing) =
            sys_text_engine.hit_test_point(dw_layout, local.x, local.y);
        let final_index = if is_trailing {
            current_index + 1
        } else {
            current_index
        };

        if start_pos <= final_index {
            (start_pos..final_index, false)
        } else {
            (final_index..start_pos, true)
        }
    }

    pub(crate) fn handle_text_selection_click(
        id: EntityId,
        start_pos: usize,
        local: LayoutPoint,
        win_scale_factor: f32,
        win_last_size: Option<LayoutSize>,
        sys_text_engine: &TextEngine,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        cont_text_contents: &mut TextContentsSparseSecondary,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparseSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        out_scroll_offsets: &mut ScrollOffsetsSecondary,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_selected_rects: &mut SelectedRectsSparseSecondary,
        out_rects: &RectsSecondary,
        out_scroll_sizes: &ScrollSizesSecondary,
    ) {
        let Some(dw_layout) = SystemStore::get_or_create_layout(
            id,
            sys_text_engine,
            sys_dwrite_layouts,
            cont_text_contents,
            cont_text_spans,
            lay_resolved_basic,
            rnd_visual,
            out_rects,
        ) else {
            return;
        };

        let (range, is_reversed) =
            EventStore::calculate_text_selection(start_pos, local, &dw_layout, sys_text_engine);

        out_text_selections.insert(id, range.clone());

        OutputStore::update_selection_rects(
            id,
            &dw_layout,
            out_selected_rects,
            out_text_selections,
        );

        if let Some(contents) = cont_input_contents.get_mut(id) {
            contents.selection_reversed = is_reversed;
            contents.selected_range = range;

            EventStore::apply_input_update(
                id,
                InputOp::MousePress,
                win_scale_factor,
                win_last_size,
                sys_text_engine,
                sys_dwrite_layouts,
                cont_text_contents,
                cont_input_contents,
                cont_text_spans,
                topo_active_masks,
                topo_parents,
                lay_dirty_entities,
                lay_taffy_tree,
                lay_scrollbar_styles,
                lay_taffy_nodes,
                lay_resolved_basic,
                lay_resolved_flex,
                lay_resolved_grid,
                rnd_dirty_entities,
                rnd_visual,
                rnd_base_visual,
                rnd_interaction,
                rnd_active_transitions,
                out_scroll_offsets,
                out_text_selections,
                out_selected_rects,
                out_rects,
                out_scroll_sizes,
            );
        } else {
            RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);
        }
    }

    pub(crate) fn inject_pointer_move_internal(cx: &mut Context, logical_pos: LayoutPoint) {
        let prev_pos = cx.events.evt_current_pointer_position;
        cx.events.evt_current_pointer_position = Some(logical_pos);

        // リサイズ中のドラッグ同期処理
        if let Some(ref state) = cx.events.resize.res_resizing_state {
            ResizeStore::sync_resizing_drag(
                logical_pos,
                state,
                cx.window.win_last_size.as_ref(),
                &mut cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_taffy_tree,
                &mut cx.layouts.lay_basic,
                &mut cx.layouts.lay_base_basic,
                &cx.layouts.lay_taffy_nodes,
                &mut cx.renders.rnd_dirty_entities,
                &cx.outputs.out_rects,
            );
            return; // リサイズドラッグ中は、通常のホバーやドラッグ判定を完全にスキップして早期リターン
        }

        OutputStore::sync_scrollbar_drag(
            logical_pos,
            cx.window.win_last_size,
            &mut cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_taffy_tree,
            &mut cx.layouts.lay_scrollbar_styles,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_resolved_basic,
            &mut cx.renders.rnd_dirty_entities,
            &cx.renders.rnd_visual,
            &cx.renders.rnd_interaction,
            &cx.renders.rnd_active_transitions,
            &mut cx.outputs.out_scroll_offsets,
            &cx.outputs.out_rects,
            &cx.outputs.out_scroll_sizes,
        );

        // ヒットテストのキャッシュ
        let hit_id = Pipeline::hit_test(cx, logical_pos);

        // マウスボタン押し下げ中は、他の要素へのインタラクション漏洩を防ぐためヒット先を押し下げ要素に強制ロック
        let target_id = cx.events.evt_interaction_states.pressed.or(hit_id);

        // 直前のリサイズホバー対象を退避
        let prev_resize_hover = cx.events.resize.res_active_resize_hover;
        // リサイズホバー情報を一旦リセット
        cx.events.resize.res_active_resize_hover = None;

        // ヒットした要素、およびその親先祖に向かってツリーを遡上
        let (current_id, found_resize_hover) = ResizeStore::found_resize_hover(
            target_id,
            logical_pos,
            &cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &cx.layouts.lay_basic,
            &cx.outputs.out_rects,
        );

        if let Some((id, dir)) = found_resize_hover {
            cx.events.resize.res_active_resize_hover = Some((id, dir));
            ResizeStore::apply_resizable_cursor_style(id, dir, &mut cx.renders.rnd_visual);
            RenderStore::mark_render_dirty(
                id,
                &mut cx.topology.topo_active_masks,
                &mut cx.renders.rnd_dirty_entities,
            );
        }

        // 枠線から外れた、または異なる要素に変わった場合
        if let Some((prev_id, _)) = prev_resize_hover {
            let now_id = cx.events.resize.res_active_resize_hover.map(|(id, _)| id);

            // 異なるホバー状態になった場合、旧要素のカーソル上書きを破棄し本来のスタイルに即時強制リセット
            if Some(prev_id) != now_id {
                // スタイルの再解決を叩き、上書きされていた vis.cursor を本来のカーソル（通常ホバー/ベース等）へ復旧
                RenderStore::resolve_element_style_state(
                    prev_id,
                    false,
                    cx.window.win_last_size.as_ref(),
                    &cx.system.sys_dwrite_layouts,
                    &cx.reactive.react_element_effects,
                    &cx.contents.cont_input_contents,
                    &mut cx.topology.topo_active_masks,
                    &mut cx.topology.topo_is_sort_dirty,
                    &cx.topology.topo_entities,
                    &cx.topology.topo_parents,
                    &cx.topology.topo_children,
                    &mut cx.layouts.lay_dirty_entities,
                    &mut cx.layouts.lay_taffy_tree,
                    &mut cx.layouts.lay_basic,
                    &cx.layouts.lay_taffy_nodes,
                    &cx.layouts.lay_base_basic,
                    &mut cx.renders.rnd_dirty_entities,
                    &mut cx.renders.rnd_visual,
                    &mut cx.renders.rnd_active_transitions,
                    &mut cx.renders.rnd_active_animations,
                    &cx.renders.rnd_base_visual,
                    &cx.renders.rnd_interaction,
                    &cx.outputs.out_rects,
                );
                RenderStore::mark_render_dirty(
                    prev_id,
                    &mut cx.topology.topo_active_masks,
                    &mut cx.renders.rnd_dirty_entities,
                );
            }
        }

        if let Some(pressed_id) = cx.events.evt_interaction_states.pressed {
            let user_select = cx
                .renders
                .rnd_visual
                .get(pressed_id)
                .and_then(|v| v.user_select)
                .unwrap_or_default();

            if user_select == UserSelect::Text
                && let Some(start_pos) = cx
                    .outputs
                    .out_selection_start_index
                    .get(pressed_id)
                    .copied()
            {
                // プレースホルダー選択のドラッグ遮断
                if let Some(contents) = cx.contents.cont_input_contents.get(pressed_id) {
                    let is_placeholder = contents.text.0.get().is_empty();
                    let is_ime = contents
                        .ime_state
                        .as_ref()
                        .is_none_or(|s| s.composition_text.is_empty());

                    if is_placeholder && is_ime && !contents.placeholder_select {
                        return;
                    }
                }

                let dw_layout = SystemStore::get_or_create_layout(
                    pressed_id,
                    &cx.system.sys_text_engine,
                    &cx.system.sys_dwrite_layouts,
                    &cx.contents.cont_text_contents,
                    &cx.contents.cont_text_spans,
                    &cx.layouts.lay_resolved_basic,
                    &cx.renders.rnd_visual,
                    &cx.outputs.out_rects,
                );

                let local = EventStore::pressed_local_point(
                    pressed_id,
                    logical_pos,
                    dw_layout.as_ref(),
                    &cx.system.sys_text_engine,
                    &cx.contents.cont_input_contents,
                    &cx.topology.topo_active_masks,
                    &cx.topology.topo_parents,
                    &cx.layouts.lay_resolved_basic,
                    &cx.layouts.lay_resolved_flex,
                    &cx.layouts.lay_resolved_grid,
                    &cx.renders.rnd_interaction,
                    &cx.renders.rnd_active_transitions,
                    &cx.renders.rnd_visual,
                    &cx.outputs.out_rects,
                    &cx.outputs.out_scroll_offsets,
                );
                EventStore::handle_text_selection_click(
                    pressed_id,
                    start_pos,
                    local,
                    cx.window.win_scale_factor,
                    cx.window.win_last_size,
                    &cx.system.sys_text_engine,
                    &cx.system.sys_dwrite_layouts,
                    &mut cx.contents.cont_text_contents,
                    &mut cx.contents.cont_input_contents,
                    &cx.contents.cont_text_spans,
                    &mut cx.topology.topo_active_masks,
                    &cx.topology.topo_parents,
                    &cx.topology.topo_children,
                    &mut cx.layouts.lay_dirty_entities,
                    &mut cx.layouts.lay_taffy_tree,
                    &mut cx.layouts.lay_scrollbar_styles,
                    &cx.layouts.lay_taffy_nodes,
                    &cx.layouts.lay_resolved_basic,
                    &cx.layouts.lay_resolved_flex,
                    &cx.layouts.lay_resolved_grid,
                    &mut cx.renders.rnd_dirty_entities,
                    &mut cx.renders.rnd_visual,
                    &cx.renders.rnd_base_visual,
                    &cx.renders.rnd_interaction,
                    &cx.renders.rnd_active_transitions,
                    &mut cx.outputs.out_scroll_offsets,
                    &mut cx.outputs.out_text_selections,
                    &mut cx.outputs.out_selected_rects,
                    &cx.outputs.out_rects,
                    &cx.outputs.out_scroll_sizes,
                );
            }
        }

        // ホバー（Enter/Leave）状態の解決
        if hit_id != cx.events.evt_interaction_states.hovered {
            EventStore::resolve_hover_state(cx, hit_id);
        }

        // カーソル移動イベントの伝播
        EventStore::propagate_cursor_move_events(cx, hit_id, logical_pos);

        // ドラッグイベントの伝播
        DndStore::propagate_dnd_drag_events(cx, prev_pos, logical_pos);

        // D&D プレースホルダーの移動とドロップ先ホバー検知
        let Some(ref drag_state) = cx.events.dnd.dnd_active_drag_state else {
            return;
        };

        // ウィンドウのルート要素を解決
        let Some(root) = TopologyStore::find_root_entity(
            &cx.topology.topo_entities,
            &cx.topology.topo_parents,
            &cx.topology.topo_flat_dfs_sequence,
        ) else {
            return; // TODO: エラー処理
        };

        let src_id = drag_state.source_entity;
        let placeholder_id = drag_state.placeholder_entity;

        let drag_prop = cx
            .events
            .dnd
            .dnd_drag_properties
            .get(src_id)
            .copied()
            .unwrap();

        // アタッチ先親コンテナ基準での相対ローカル座標を逆算して追従
        DndStore::update_inset_based_relative_local(
            root,
            placeholder_id,
            logical_pos,
            &drag_prop,
            drag_state,
            &mut cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_taffy_tree,
            &mut cx.layouts.lay_basic,
            &mut cx.layouts.lay_base_basic,
            &cx.layouts.lay_taffy_nodes,
            &mut cx.renders.rnd_dirty_entities,
            &cx.outputs.out_rects,
        );

        // 現在ホバー侵入中のドロップターゲット要素を検知
        let found_drop_target = DndStore::detect_drop_target_during_intrusion(
            src_id,
            hit_id,
            placeholder_id,
            &cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
        );

        // ドロップ先のホバー切り替えイベントを解決（STATE_DRAG_IN の同期）
        DndStore::sync_state_drag_in(cx, found_drop_target);

        DndStore::callback_drag_prop(cx, src_id, found_drop_target, &drag_prop);
    }

    /// 指定要素またはその親階層において、フォーカスの略奪を防止すべきか判定
    #[inline]
    fn should_prevent_focus_steal(cx: &Context, target_id: EntityId) -> bool {
        let mut curr = Some(target_id);
        while let Some(curr_id) = curr {
            let Some(mask) = cx.topology.topo_active_masks.get(curr_id) else {
                break;
            };

            if mask.has(ComponentMask::STYLE_PREVENT_FOCUS_STEAL)
                && curr_id == target_id
                && cx
                    .renders
                    .rnd_visual
                    .get(curr_id)
                    .and_then(|v| v.prevent_focus_steal)
                    .unwrap_or(false)
            {
                return true;
            }

            if mask.has(ComponentMask::STYLE_PREVENT_FOCUS_STEAL_WITHIN)
                && cx
                    .renders
                    .rnd_visual
                    .get(curr_id)
                    .and_then(|v| v.prevent_focus_steal_within)
                    .unwrap_or(false)
            {
                return true;
            }

            curr = cx.topology.topo_parents.get(curr_id).copied().flatten();
        }
        false
    }

    pub(crate) fn restrict_focusable_element(
        id: EntityId,
        topo_active_masks: &ActiveMasksSecondary,
        rnd_visual: &VisualPropertiesSecondary,
    ) -> bool {
        let focusable = rnd_visual.get(id).and_then(|v| v.focusable).or_else(|| {
            let mask = topo_active_masks.get(id).copied().unwrap_or_default();
            if mask.has_input_content() || mask.has_webveiw2_content() {
                Some(Focusable::Inherit(FocusTrigger::Both)) // 未指定時はキーボードフォーカス
            } else {
                None
            }
        });

        focusable.is_some_and(|f| match f {
            Focusable::SelfStyle(trigger) | Focusable::Inherit(trigger) => {
                trigger == FocusTrigger::Mouse || trigger == FocusTrigger::Both
            }
            Focusable::None => false,
        })
    }

    fn handle_pointer_pressed(cx: &mut Context, button: MouseButton, modifiers: Modifiers) {
        let current_hovered = cx.events.evt_interaction_states.hovered;

        // リサイズドラッグの開始判定（左クリック時のみ）
        if button == MouseButton::Left
            && let Some((id, dir)) = cx.events.resize.res_active_resize_hover
        {
            ResizeStore::state_pressed_resize_drag(
                id,
                dir,
                &mut cx.events.evt_interaction_states,
                &mut cx.events.resize.res_resizing_state,
                cx.events.evt_current_pointer_position,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_basic,
                &mut cx.layouts.lay_base_basic,
                &cx.outputs.out_rects,
            );
            RenderStore::mark_render_dirty(
                id,
                &mut cx.topology.topo_active_masks,
                &mut cx.renders.rnd_dirty_entities,
            );
            return; // リサイズ開始時は以降の処理を完全にスキップ
        }

        // スクロールバーのクリック判定
        if let Some(pointer_pos) = cx.events.evt_current_pointer_position
            && let Some(target_id) = current_hovered
        {
            let clicked_scrollbar = EventStore::hit_decision_element_scrollbar(
                target_id,
                pointer_pos,
                cx.window.win_last_size,
                &cx.system.sys_text_engine,
                &cx.system.sys_dwrite_layouts,
                &mut cx.events.evt_interaction_states,
                &cx.contents.cont_text_contents,
                &cx.contents.cont_text_spans,
                &cx.contents.cont_input_contents,
                &mut cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &cx.topology.topo_children,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_taffy_tree,
                &mut cx.layouts.lay_scrollbar_styles,
                &cx.layouts.lay_taffy_nodes,
                &cx.layouts.lay_resolved_basic,
                &mut cx.renders.rnd_dirty_entities,
                &cx.renders.rnd_visual,
                &cx.renders.rnd_interaction,
                &cx.renders.rnd_active_transitions,
                &mut cx.outputs.out_scroll_offsets,
                &cx.outputs.out_rects,
                &cx.outputs.out_scroll_sizes,
            );

            if clicked_scrollbar {
                return; // スクロールバー上の場合は背後への透過を防ぐ
            }
        }

        // 一般要素のプレス
        let Some(target_id) = current_hovered else {
            return;
        };

        cx.events.evt_interaction_states.pressed = Some(target_id);
        EventStore::update_state(cx, target_id, ComponentMask::STATE_PRESSED, true);

        // テキスト選択処理
        let user_select = cx
            .renders
            .rnd_visual
            .get(target_id)
            .and_then(|v| v.user_select)
            .unwrap_or_default();
        let is_input = cx
            .topology
            .topo_active_masks
            .get(target_id)
            .is_some_and(ComponentMask::has_input_content);

        if user_select == UserSelect::Text
            && !is_input
            && let Some(pointer_pos) = cx.events.evt_current_pointer_position
        {
            EventStore::handle_user_select_text(
                target_id,
                pointer_pos,
                modifiers.shift,
                &cx.system.sys_text_engine,
                &cx.system.sys_dwrite_layouts,
                &cx.contents.cont_text_contents,
                &cx.contents.cont_text_spans,
                &cx.contents.cont_input_contents,
                &mut cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &cx.layouts.lay_resolved_basic,
                &cx.layouts.lay_resolved_flex,
                &cx.layouts.lay_resolved_grid,
                &mut cx.renders.rnd_dirty_entities,
                &cx.renders.rnd_visual,
                &cx.renders.rnd_interaction,
                &cx.renders.rnd_active_transitions,
                &mut cx.outputs.out_text_selections,
                &mut cx.outputs.out_selection_start_index,
                &mut cx.outputs.out_selected_rects,
                &cx.outputs.out_rects,
                &cx.outputs.out_scroll_offsets,
            );
        }

        // フォーカスの解決
        if !EventStore::should_prevent_focus_steal(cx, target_id) {
            let is_focusable = EventStore::restrict_focusable_element(
                target_id,
                &cx.topology.topo_active_masks,
                &cx.renders.rnd_visual,
            );
            if is_focusable {
                EventStore::auto_focus_switch_by_trigger(cx, target_id, ActiveFocusTrigger::Mouse);
            } else {
                EventStore::handle_remove_focus(cx);
            }
        }

        // ユーザーイベントの発火
        handle_on_mouse_input(cx, target_id, button, modifiers, ElementState::Pressed);
    }

    #[inline]
    pub(crate) fn get_scrollbar_dirty_ids(
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
    ) -> SmallVec<[EntityId; 4]> {
        let mut dirty_ids = SmallVec::<[EntityId; 4]>::new();

        for (id, state) in lay_scrollbar_styles {
            if state.v_thumb_dragged || state.h_thumb_dragged {
                state.v_thumb_dragged = false;
                state.h_thumb_dragged = false;
                dirty_ids.push(id);
            }
        }
        dirty_ids
    }

    #[inline]
    fn handle_pointer_released(cx: &mut Context, button: MouseButton, modifiers: Modifiers) {
        // リサイズドラッグの終了処理
        if let Some(state) = cx.events.resize.res_resizing_state.take() {
            let id = state.entity_id;
            cx.events.evt_interaction_states.pressed = None;

            if let Some(pos) = cx.events.evt_current_pointer_position {
                EventStore::inject_pointer_move_internal(cx, pos);
            }
            RenderStore::mark_render_dirty(
                id,
                &mut cx.topology.topo_active_masks,
                &mut cx.renders.rnd_dirty_entities,
            );
            return;
        }

        // D&D ドラッグ終了・ドロップ確定処理
        if let Some(drag_state) = cx.events.dnd.dnd_active_drag_state.take() {
            DndStore::handle_dnd_drop(cx, &drag_state);
            return;
        }

        // スクロールバーの表示更新
        let dirty_ids = EventStore::get_scrollbar_dirty_ids(&mut cx.layouts.lay_scrollbar_styles);
        for id in dirty_ids {
            RenderStore::mark_render_dirty(
                id,
                &mut cx.topology.topo_active_masks,
                &mut cx.renders.rnd_dirty_entities,
            );
        }

        // 通常要素のリリース
        if let Some(pressed_id) = cx.events.evt_interaction_states.pressed {
            EventStore::update_state(cx, pressed_id, ComponentMask::STATE_PRESSED, false);
            EventStore::update_state(cx, pressed_id, ComponentMask::STATE_DRAGGED, false);
            cx.events.evt_interaction_states.dragged = None;

            if let Some(contents) = cx.contents.cont_input_contents.get_mut(pressed_id) {
                contents.is_selecting = false;
            }

            // マウスリリースイベントの発火
            handle_on_mouse_input(cx, pressed_id, button, modifiers, ElementState::Released);

            // 同一要素上で離された場合のクリックイベント解決
            if cx.events.evt_interaction_states.hovered == Some(pressed_id) {
                match button {
                    MouseButton::Left => handle_on_click(cx, pressed_id),
                    MouseButton::Right => handle_on_right_click(cx, pressed_id),
                    _ => {}
                }
            }

            cx.events.evt_interaction_states.pressed = None;
        }
    }

    #[inline]
    pub(crate) fn inject_pointer_button_internal(
        cx: &mut Context,
        button: MouseButton,
        state: ElementState,
        modifiers: Modifiers,
    ) {
        let current_hovered = cx.events.evt_interaction_states.hovered;

        match state {
            ElementState::Pressed => EventStore::handle_pointer_pressed(cx, button, modifiers),
            ElementState::Released => EventStore::handle_pointer_released(cx, button, modifiers),
        }
    }

    pub fn inject_pointer_double_click_internal(cx: &mut Context, modifiers: Modifiers) {
        let current_hovered = cx.events.evt_interaction_states.hovered;

        let Some(target_id) = current_hovered else {
            return;
        };

        let user_select = cx
            .renders
            .rnd_visual
            .get(target_id)
            .and_then(|v| v.user_select)
            .unwrap_or_default();
        if user_select != UserSelect::Text {
            return;
        }

        let Some(pointer_pos) = cx.events.evt_current_pointer_position else {
            return;
        };

        if let Some(contents) = cx.contents.cont_input_contents.get(target_id) {
            let text_val = contents.text.0.get();
            let is_placeholder = text_val.is_empty()
                && contents
                    .ime_state
                    .as_ref()
                    .is_none_or(|s| s.composition_text.is_empty());

            if is_placeholder && !contents.placeholder_select {
                return;
            }
        }

        let Some(text) = cx.contents.cont_text_contents.get(target_id) else {
            return;
        };

        let Some(dw_layout) = SystemStore::get_or_create_layout(
            target_id,
            &cx.system.sys_text_engine,
            &cx.system.sys_dwrite_layouts,
            &cx.contents.cont_text_contents,
            &cx.contents.cont_text_spans,
            &cx.layouts.lay_resolved_basic,
            &cx.renders.rnd_visual,
            &cx.outputs.out_rects,
        ) else {
            return;
        };

        let rect = cx
            .outputs
            .out_rects
            .get(target_id)
            .copied()
            .unwrap_or_default();
        let basic = &cx
            .layouts
            .lay_resolved_basic
            .get(target_id)
            .copied()
            .unwrap_or_default();
        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

        let local_x = pointer_pos.x - (rect.x + border.left + padding.left);
        let local_y = pointer_pos.y - (rect.y + border.top + padding.top);

        let (clicked_index, is_trailing) = cx
            .system
            .sys_text_engine
            .hit_test_point(&dw_layout, local_x, local_y);

        let final_index = if is_trailing {
            clicked_index + 1
        } else {
            clicked_index
        };

        let text_u16: Vec<u16> = text.encode_utf16().collect();

        // 文節境界を抽出
        let range = InputContents::find_word_boundaries(&text_u16, final_index);

        cx.outputs
            .out_text_selections
            .insert(target_id, range.clone());
        // アンカー開始を文節左端にセット
        cx.outputs
            .out_selection_start_index
            .insert(target_id, range.start);
        // 選択矩形を更新
        OutputStore::update_selection_rects(
            target_id,
            &dw_layout,
            &mut cx.outputs.out_selected_rects,
            &cx.outputs.out_text_selections,
        );

        if let Some(contents) = cx.contents.cont_input_contents.get_mut(target_id) {
            contents.selected_range = range;
            contents.selection_reversed = false; // キャレットは右端に配置
            EventStore::apply_input_update(
                target_id,
                InputOp::MousePress,
                cx.window.win_scale_factor,
                cx.window.win_last_size,
                &cx.system.sys_text_engine,
                &cx.system.sys_dwrite_layouts,
                &mut cx.contents.cont_text_contents,
                &mut cx.contents.cont_input_contents,
                &cx.contents.cont_text_spans,
                &mut cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_taffy_tree,
                &mut cx.layouts.lay_scrollbar_styles,
                &cx.layouts.lay_taffy_nodes,
                &cx.layouts.lay_resolved_basic,
                &cx.layouts.lay_resolved_flex,
                &cx.layouts.lay_resolved_grid,
                &mut cx.renders.rnd_dirty_entities,
                &mut cx.renders.rnd_visual,
                &cx.renders.rnd_base_visual,
                &cx.renders.rnd_interaction,
                &cx.renders.rnd_active_transitions,
                &mut cx.outputs.out_scroll_offsets,
                &mut cx.outputs.out_text_selections,
                &mut cx.outputs.out_selected_rects,
                &cx.outputs.out_rects,
                &cx.outputs.out_scroll_sizes,
            );
        } else {
            RenderStore::mark_render_dirty(
                target_id,
                &mut cx.topology.topo_active_masks,
                &mut cx.renders.rnd_dirty_entities,
            );
        }
    }

    pub fn inject_mouse_wheel_internal(cx: &mut Context, scroll_x: f32, scroll_y: f32) {
        let mut curr = cx.events.evt_interaction_states.hovered;

        // ホバー要素から親へ辿る
        while let Some(curr_id) = curr {
            // 個別に定義された `on_mouse_wheel` ハンドラがあれば最優先実行
            let has_listener = cx
                .events
                .evt_listeners
                .get(curr_id)
                .is_some_and(|l| l.on_mouse_wheel.is_some());

            if has_listener {
                handle_on_mouse_wheel(cx, curr_id, scroll_x, scroll_y);
                break; // イベントが消費されたため、これ以上の伝播やコンテナスクロールは行わない
            }

            // ユーザーハンドラがない場合、要素がスクロールコンテナであるか判定
            let has_overflow = cx
                .topology
                .topo_active_masks
                .get(curr_id)
                .is_some_and(|m| m.has(ComponentMask::STYLE_OVERFLOW));
            if has_overflow {
                let basic = &cx
                    .layouts
                    .lay_resolved_basic
                    .get(curr_id)
                    .copied()
                    .unwrap_or_default();

                // スクロール可能な軸の移動量
                let dy = if scroll_y != 0.0
                    && (basic.overflow.y == Overflow::Scroll
                        || basic.overflow.y == Overflow::Hidden)
                {
                    scroll_y
                } else {
                    0.0
                };

                let dx = if scroll_x != 0.0
                    && (basic.overflow.x == Overflow::Scroll
                        || basic.overflow.x == Overflow::Hidden)
                {
                    scroll_x
                } else {
                    0.0
                };

                if (dx != 0.0 || dy != 0.0)
                    && OutputStore::scroll_by(
                        curr_id,
                        dx,
                        dy,
                        cx.window.win_last_size,
                        &mut cx.topology.topo_active_masks,
                        &cx.topology.topo_parents,
                        &mut cx.layouts.lay_dirty_entities,
                        &mut cx.layouts.lay_taffy_tree,
                        &mut cx.layouts.lay_scrollbar_styles,
                        &cx.layouts.lay_taffy_nodes,
                        &cx.layouts.lay_resolved_basic,
                        &cx.renders.rnd_visual,
                        &cx.renders.rnd_interaction,
                        &cx.renders.rnd_active_transitions,
                        &mut cx.outputs.out_scroll_offsets,
                        &cx.outputs.out_rects,
                        &cx.outputs.out_scroll_sizes,
                    )
                {
                    break; // スクロールを実行したためバブリングを終了
                }
            }

            // 先祖へ伝播
            curr = cx.topology.topo_parents.get(curr_id).copied().flatten();
        }
    }

    /// 指定されたテキスト要素の内容をすべて選択状態に
    pub(crate) fn handle_select_all(
        id: EntityId,
        win_scale_factor: f32,
        win_last_size: Option<LayoutSize>,
        sys_text_engine: &TextEngine,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        cont_text_contents: &mut TextContentsSparseSecondary,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparseSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        out_scroll_offsets: &mut ScrollOffsetsSecondary,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_selected_rects: &mut SelectedRectsSparseSecondary,
        out_rects: &RectsSecondary,
        out_scroll_sizes: &ScrollSizesSecondary,
    ) {
        let Some(dw_layout) = SystemStore::get_or_create_layout(
            id,
            sys_text_engine,
            sys_dwrite_layouts,
            cont_text_contents,
            cont_text_spans,
            lay_resolved_basic,
            rnd_visual,
            out_rects,
        ) else {
            return;
        };

        let Some(text) = cont_text_contents.get(id) else {
            return;
        };

        let u16_len = text.encode_utf16().count();
        let full_range = 0..u16_len;

        out_text_selections.insert(id, full_range.clone());

        OutputStore::update_selection_rects(
            id,
            &dw_layout,
            out_selected_rects,
            out_text_selections,
        );

        if let Some(contents) = cont_input_contents.get_mut(id) {
            contents.selected_range = full_range;
            contents.selection_reversed = false;
            EventStore::apply_input_update(
                id,
                InputOp::SelectAll,
                win_scale_factor,
                win_last_size,
                sys_text_engine,
                sys_dwrite_layouts,
                cont_text_contents,
                cont_input_contents,
                cont_text_spans,
                topo_active_masks,
                topo_parents,
                lay_dirty_entities,
                lay_taffy_tree,
                lay_scrollbar_styles,
                lay_taffy_nodes,
                lay_resolved_basic,
                lay_resolved_flex,
                lay_resolved_grid,
                rnd_dirty_entities,
                rnd_visual,
                rnd_base_visual,
                rnd_interaction,
                rnd_active_transitions,
                out_scroll_offsets,
                out_text_selections,
                out_selected_rects,
                out_rects,
                out_scroll_sizes,
            );
        } else {
            RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);
        }
    }

    pub(crate) fn inject_keyboard_key_internal(
        cx: &mut Context,
        key: VirtualKey,
        state: ElementState,
        modifiers: Modifiers,
    ) {
        // Tabキー押下時は個別のフォーカス対象へのイベント配信前に巡回処理を実行
        if state == ElementState::Pressed && key == VirtualKey::TAB {
            EventStore::cycle_keyboard_focus_internal(cx, modifiers.shift);
            return;
        }

        let Some(focused_id) = cx.events.evt_interaction_states.focused else {
            return;
        };

        // フォーカス中に Enter または Space が押されたらクリックをエミュレート
        if state == ElementState::Pressed && (key == VirtualKey::RETURN || key == VirtualKey::SPACE)
        {
            let has_input_contents = cx
                .topology
                .topo_active_masks
                .get(focused_id)
                .is_some_and(ComponentMask::has_input_content);

            if !has_input_contents {
                handle_on_click(cx, focused_id);
                return;
            }
        }

        // 内部で完結する全選択（Ctrl+A）のみを自動処理
        if state == ElementState::Pressed && modifiers.ctrl && key == VirtualKey::A {
            let user_select = cx
                .renders
                .rnd_visual
                .get(focused_id)
                .and_then(|v| v.user_select)
                .unwrap_or_default();
            if user_select == UserSelect::Text {
                EventStore::handle_select_all(
                    focused_id,
                    cx.window.win_scale_factor,
                    cx.window.win_last_size,
                    &cx.system.sys_text_engine,
                    &cx.system.sys_dwrite_layouts,
                    &mut cx.contents.cont_text_contents,
                    &mut cx.contents.cont_input_contents,
                    &cx.contents.cont_text_spans,
                    &mut cx.topology.topo_active_masks,
                    &cx.topology.topo_parents,
                    &cx.topology.topo_children,
                    &mut cx.layouts.lay_dirty_entities,
                    &mut cx.layouts.lay_taffy_tree,
                    &mut cx.layouts.lay_scrollbar_styles,
                    &cx.layouts.lay_taffy_nodes,
                    &cx.layouts.lay_resolved_basic,
                    &cx.layouts.lay_resolved_flex,
                    &cx.layouts.lay_resolved_grid,
                    &mut cx.renders.rnd_dirty_entities,
                    &mut cx.renders.rnd_visual,
                    &cx.renders.rnd_base_visual,
                    &cx.renders.rnd_interaction,
                    &cx.renders.rnd_active_transitions,
                    &mut cx.outputs.out_scroll_offsets,
                    &mut cx.outputs.out_text_selections,
                    &mut cx.outputs.out_selected_rects,
                    &cx.outputs.out_rects,
                    &cx.outputs.out_scroll_sizes,
                );
                return;
            }
        }
        // それ以外の通常のキー入力
        handle_on_keyboard_input(cx, focused_id, key, modifiers, state);
    }

    /// キーボードフォーカスを次の適格な要素へ巡回
    pub(crate) fn cycle_keyboard_focus_internal(cx: &mut Context, reverse: bool) {
        if cx.topology.topo_flat_dfs_sequence.is_empty() {
            return;
        }

        let len = cx.topology.topo_flat_dfs_sequence.len();
        // 現在フォーカスされている要素のインデックスを特定（無ければ探索方向の末端から開始）
        let current_focused = cx.events.evt_interaction_states.focused;

        let start_idx = current_focused
            .and_then(|id| {
                cx.topology
                    .topo_flat_dfs_sequence
                    .iter()
                    .position(|&x| x == id)
            })
            .unwrap_or(if reverse { len - 1 } else { 0 });

        let mut target_id = None;

        for i in 1..len {
            let idx = if reverse {
                (start_idx + len - i) % len
            } else {
                (start_idx + i) % len
            };

            let Some(candidate_id) = cx.topology.topo_flat_dfs_sequence.get(idx).copied() else {
                continue;
            };

            if RenderStore::is_keyboard_focusable(
                candidate_id,
                &cx.topology.topo_entities,
                &cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &cx.layouts.lay_basic,
                &cx.renders.rnd_visual,
            ) {
                target_id = Some(candidate_id);
                break;
            }
        }

        // フォーカス対象が見つかった場合のみ状態の遷移処理
        let Some(candidate_id) = target_id else {
            return;
        };

        // 旧フォーカスの解除
        if let Some(old_id) = cx.events.evt_interaction_states.focused {
            EventStore::set_focused_by_trigger(cx, old_id, false, ActiveFocusTrigger::Keyboard);
        }

        // 新フォーカスの設定
        EventStore::set_focused_by_trigger(cx, candidate_id, true, ActiveFocusTrigger::Keyboard);
        cx.events.evt_interaction_states.focused = Some(candidate_id);

        RenderStore::mark_render_dirty(
            candidate_id,
            &mut cx.topology.topo_active_masks,
            &mut cx.renders.rnd_dirty_entities,
        );
    }

    pub(crate) fn inject_paste_internal(cx: &mut Context, text: &str) {
        let Some(focused_id) = cx.events.evt_interaction_states.focused else {
            return;
        };
        if !cx
            .topology
            .topo_active_masks
            .get(focused_id)
            .is_some_and(ComponentMask::has_input_content)
        {
            return;
        }
        let Some(contents) = cx.contents.cont_input_contents.get_mut(focused_id) else {
            return;
        };

        OutputStore::handle_paste(
            focused_id,
            text,
            contents,
            &mut cx.outputs.out_text_selections,
            &mut cx.outputs.out_selected_rects,
        );
        OutputStore::update_input_caret_position(
            focused_id,
            cx.window.win_scale_factor,
            cx.window.win_last_size,
            &cx.system.sys_text_engine,
            &cx.system.sys_dwrite_layouts,
            &mut cx.contents.cont_text_contents,
            &mut cx.contents.cont_input_contents,
            &cx.contents.cont_text_spans,
            &mut cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_taffy_tree,
            &mut cx.layouts.lay_scrollbar_styles,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
            &cx.layouts.lay_resolved_grid,
            &mut cx.renders.rnd_visual,
            &cx.renders.rnd_base_visual,
            &cx.renders.rnd_interaction,
            &cx.renders.rnd_active_transitions,
            &mut cx.outputs.out_scroll_offsets,
            &mut cx.outputs.out_text_selections,
            &cx.outputs.out_rects,
            &cx.outputs.out_scroll_sizes,
        );
        EventStore::apply_input_update(
            focused_id,
            InputOp::Paste,
            cx.window.win_scale_factor,
            cx.window.win_last_size,
            &cx.system.sys_text_engine,
            &cx.system.sys_dwrite_layouts,
            &mut cx.contents.cont_text_contents,
            &mut cx.contents.cont_input_contents,
            &cx.contents.cont_text_spans,
            &mut cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_taffy_tree,
            &mut cx.layouts.lay_scrollbar_styles,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
            &cx.layouts.lay_resolved_grid,
            &mut cx.renders.rnd_dirty_entities,
            &mut cx.renders.rnd_visual,
            &cx.renders.rnd_base_visual,
            &cx.renders.rnd_interaction,
            &cx.renders.rnd_active_transitions,
            &mut cx.outputs.out_scroll_offsets,
            &mut cx.outputs.out_text_selections,
            &mut cx.outputs.out_selected_rects,
            &cx.outputs.out_rects,
            &cx.outputs.out_scroll_sizes,
        );
    }

    pub(crate) fn inject_undo_internal(cx: &mut Context) {
        let Some(focused_id) = cx.events.evt_interaction_states.focused else {
            return;
        };
        if !cx
            .topology
            .topo_active_masks
            .get(focused_id)
            .is_some_and(ComponentMask::has_input_content)
        {
            return;
        }
        let Some(contents) = cx.contents.cont_input_contents.get_mut(focused_id) else {
            return;
        };
        let Some((prev_text, prev_sel)) = contents.undo_stack.pop() else {
            return;
        };

        OutputStore::handle_undo(
            focused_id,
            prev_sel.clone(),
            prev_text,
            contents,
            &mut cx.outputs.out_text_selections,
            &mut cx.outputs.out_selected_rects,
        );

        cx.outputs
            .out_selection_start_index
            .insert(focused_id, prev_sel.start);

        EventStore::apply_input_update(
            focused_id,
            InputOp::Undo,
            cx.window.win_scale_factor,
            cx.window.win_last_size,
            &cx.system.sys_text_engine,
            &cx.system.sys_dwrite_layouts,
            &mut cx.contents.cont_text_contents,
            &mut cx.contents.cont_input_contents,
            &cx.contents.cont_text_spans,
            &mut cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_taffy_tree,
            &mut cx.layouts.lay_scrollbar_styles,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
            &cx.layouts.lay_resolved_grid,
            &mut cx.renders.rnd_dirty_entities,
            &mut cx.renders.rnd_visual,
            &cx.renders.rnd_base_visual,
            &cx.renders.rnd_interaction,
            &cx.renders.rnd_active_transitions,
            &mut cx.outputs.out_scroll_offsets,
            &mut cx.outputs.out_text_selections,
            &mut cx.outputs.out_selected_rects,
            &cx.outputs.out_rects,
            &cx.outputs.out_scroll_sizes,
        );
    }

    pub(crate) fn inject_redo_internal(cx: &mut Context) {
        let Some(focused_id) = cx.events.evt_interaction_states.focused else {
            return;
        };
        if !cx
            .topology
            .topo_active_masks
            .get(focused_id)
            .is_some_and(ComponentMask::has_input_content)
        {
            return;
        }
        let Some(contents) = cx.contents.cont_input_contents.get_mut(focused_id) else {
            return;
        };
        let Some((next_text, next_sel)) = contents.redo_stack.pop() else {
            return;
        };

        OutputStore::handle_redo(
            focused_id,
            next_sel.clone(),
            next_text,
            contents,
            &mut cx.outputs.out_text_selections,
            &mut cx.outputs.out_selected_rects,
        );
        cx.outputs
            .out_selection_start_index
            .insert(focused_id, next_sel.start);

        EventStore::apply_input_update(
            focused_id,
            InputOp::Redo,
            cx.window.win_scale_factor,
            cx.window.win_last_size,
            &cx.system.sys_text_engine,
            &cx.system.sys_dwrite_layouts,
            &mut cx.contents.cont_text_contents,
            &mut cx.contents.cont_input_contents,
            &cx.contents.cont_text_spans,
            &mut cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_taffy_tree,
            &mut cx.layouts.lay_scrollbar_styles,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
            &cx.layouts.lay_resolved_grid,
            &mut cx.renders.rnd_dirty_entities,
            &mut cx.renders.rnd_visual,
            &cx.renders.rnd_base_visual,
            &cx.renders.rnd_interaction,
            &cx.renders.rnd_active_transitions,
            &mut cx.outputs.out_scroll_offsets,
            &mut cx.outputs.out_text_selections,
            &mut cx.outputs.out_selected_rects,
            &cx.outputs.out_rects,
            &cx.outputs.out_scroll_sizes,
        );
    }

    pub(crate) fn inject_cut_internal(cx: &mut Context) -> Option<Cow<'static, str>> {
        let focused_id = cx.events.evt_interaction_states.focused?;
        let user_select = cx
            .renders
            .rnd_visual
            .get(focused_id)
            .and_then(|v| v.user_select)
            .unwrap_or_default();

        if user_select != UserSelect::Text {
            return None;
        }

        let range = cx.outputs.out_text_selections.get(focused_id)?;

        // 空の選択範囲の場合
        if range.start >= range.end {
            return None;
        }

        let text = cx.contents.cont_text_contents.get(focused_id)?;

        // 選択されたUTF-16テキストの切り出し
        let u16_text: Vec<u16> = text.encode_utf16().collect();
        let slice = &u16_text[range.start.min(u16_text.len())..range.end.min(u16_text.len())];
        let cut_text = String::from_utf16(slice).ok()?;

        // 対象が Input コントロールである場合のみ書き換え
        let is_input = cx
            .topology
            .topo_active_masks
            .get(focused_id)
            .is_some_and(ComponentMask::has_input_content);

        // Input 用のコンテンツが実際に存在する場合のみ実行
        if is_input && let Some(contents) = cx.contents.cont_input_contents.get_mut(focused_id) {
            OutputStore::inject_cut_internal(
                focused_id,
                range.clone(),
                contents,
                &mut cx.outputs.out_text_selections,
                &mut cx.outputs.out_selected_rects,
            );
            EventStore::apply_input_update(
                focused_id,
                InputOp::Cut,
                cx.window.win_scale_factor,
                cx.window.win_last_size,
                &cx.system.sys_text_engine,
                &cx.system.sys_dwrite_layouts,
                &mut cx.contents.cont_text_contents,
                &mut cx.contents.cont_input_contents,
                &cx.contents.cont_text_spans,
                &mut cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_taffy_tree,
                &mut cx.layouts.lay_scrollbar_styles,
                &cx.layouts.lay_taffy_nodes,
                &cx.layouts.lay_resolved_basic,
                &cx.layouts.lay_resolved_flex,
                &cx.layouts.lay_resolved_grid,
                &mut cx.renders.rnd_dirty_entities,
                &mut cx.renders.rnd_visual,
                &cx.renders.rnd_base_visual,
                &cx.renders.rnd_interaction,
                &cx.renders.rnd_active_transitions,
                &mut cx.outputs.out_scroll_offsets,
                &mut cx.outputs.out_text_selections,
                &mut cx.outputs.out_selected_rects,
                &cx.outputs.out_rects,
                &cx.outputs.out_scroll_sizes,
            );
        }
        // Input・非Inputに関わらず切り出されたテキストを返す
        Some(cut_text.into())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScrollbarComponent {
    VThumb,
    HThumb,
    VTrack,
    HTrack,
}

impl EventStore {
    pub(crate) fn hit_decision_element_scrollbar(
        target_id: EntityId,
        pointer_pos: LayoutPoint,
        win_last_size: Option<LayoutSize>,
        sys_text_engine: &TextEngine,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        evt_interaction_states: &mut InteractionStates,
        cont_text_contents: &TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        cont_input_contents: &InputContentsSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        out_scroll_offsets: &mut ScrollOffsetsSecondary,
        out_rects: &RectsSecondary,
        out_scroll_sizes: &ScrollSizesSecondary,
    ) -> bool {
        let Some((c_id, component)) = lay_scrollbar_styles.iter().find_map(|(c_id, sb_state)| {
            if sb_state.v_thumb_id == Some(target_id) {
                Some((c_id, ScrollbarComponent::VThumb))
            } else if sb_state.h_thumb_id == Some(target_id) {
                Some((c_id, ScrollbarComponent::HThumb))
            } else if sb_state.v_track_id == Some(target_id) {
                Some((c_id, ScrollbarComponent::VTrack))
            } else if sb_state.h_track_id == Some(target_id) {
                Some((c_id, ScrollbarComponent::HTrack))
            } else {
                None
            }
        }) else {
            return false;
        };

        // 親スクロールコンテナ
        let sb_state = lay_scrollbar_styles.get(c_id).cloned().unwrap();
        let container_rect = out_rects.get(c_id).copied().unwrap_or_default();
        let scroll_size = out_scroll_sizes.get(c_id).copied().unwrap_or_default();
        let offset = out_scroll_offsets.get(c_id).copied().unwrap_or_default();

        match component {
            ScrollbarComponent::VThumb | ScrollbarComponent::HThumb => {
                // サムをクリックした場合：ドラッグを開始
                if let Some(st) = lay_scrollbar_styles.get_mut(c_id) {
                    if component == ScrollbarComponent::VThumb {
                        st.v_thumb_dragged = true;
                    } else {
                        st.h_thumb_dragged = true;
                    }
                    st.drag_start_mouse = pointer_pos;
                    st.drag_start_offset = offset;
                }
                evt_interaction_states.pressed = Some(target_id);
                RenderStore::mark_render_dirty(target_id, topo_active_masks, rnd_dirty_entities);
            }
            ScrollbarComponent::VTrack | ScrollbarComponent::HTrack => {
                // レールをクリックした場合：ダイレクトジャンプスクロールを実行
                let is_vertical = component == ScrollbarComponent::VTrack;
                let thumb_id = if is_vertical {
                    sb_state.v_thumb_id
                } else {
                    sb_state.h_thumb_id
                };

                let track_rect = out_rects.get(target_id).copied().unwrap_or_default();
                let thumb_rect = thumb_id
                    .and_then(|i| out_rects.get(i).copied())
                    .unwrap_or_default();
                let visible_size =
                    WindowStore::calculate_visible_size(container_rect, win_last_size);

                // 縦・横の計算用パラメータ
                let (pointer_coord, track_coord, track_len, thumb_len, scroll_total, visible_total) =
                    if is_vertical {
                        (
                            pointer_pos.y,
                            track_rect.y,
                            track_rect.height,
                            thumb_rect.height,
                            scroll_size.height,
                            visible_size.height,
                        )
                    } else {
                        (
                            pointer_pos.x,
                            track_rect.x,
                            track_rect.width,
                            thumb_rect.width,
                            scroll_size.width,
                            visible_size.width,
                        )
                    };

                let relative_pos = pointer_coord - track_coord;
                let track_range = track_len - thumb_len;
                let scroll_ratio = if track_range > 0.0 {
                    ((relative_pos - thumb_len * 0.5) / track_range).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                let target_val = scroll_ratio * (scroll_total - visible_total);
                let (target_x, target_y) = if is_vertical {
                    (offset.x, target_val)
                } else {
                    (target_val, offset.y)
                };

                OutputStore::scroll_to(
                    c_id,
                    target_x,
                    target_y,
                    win_last_size,
                    topo_active_masks,
                    topo_parents,
                    lay_dirty_entities,
                    lay_taffy_tree,
                    lay_scrollbar_styles,
                    lay_taffy_nodes,
                    lay_resolved_basic,
                    rnd_visual,
                    rnd_interaction,
                    rnd_active_transitions,
                    out_scroll_offsets,
                    out_rects,
                    out_scroll_sizes,
                );

                let new_offset = out_scroll_offsets.get(c_id).copied().unwrap_or_default();
                if let Some(st) = lay_scrollbar_styles.get_mut(c_id) {
                    if is_vertical {
                        st.v_thumb_dragged = true;
                    } else {
                        st.h_thumb_dragged = true;
                    }
                    st.drag_start_mouse = pointer_pos;
                    st.drag_start_offset = new_offset;
                }

                evt_interaction_states.pressed = thumb_id;
                if let Some(tid) = thumb_id {
                    RenderStore::mark_render_dirty(tid, topo_active_masks, rnd_dirty_entities);
                }
            }
        }
        true
    }

    pub(crate) fn handle_user_select_text(
        id: EntityId,
        pointer_pos: LayoutPoint,
        pressed_shift: bool,
        sys_text_engine: &TextEngine,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        cont_text_contents: &TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        cont_input_contents: &InputContentsSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparseSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_selection_start_index: &mut SelectionStartIndexSparseSecondary,
        out_selected_rects: &mut SelectedRectsSparseSecondary,
        out_rects: &RectsSecondary,
        out_scroll_offsets: &ScrollOffsetsSecondary,
    ) {
        let Some(dw_layout) = SystemStore::get_or_create_layout(
            id,
            sys_text_engine,
            sys_dwrite_layouts,
            cont_text_contents,
            cont_text_spans,
            lay_resolved_basic,
            rnd_visual,
            out_rects,
        ) else {
            return;
        };

        let local = EventStore::pressed_local_point(
            id,
            pointer_pos,
            Some(&dw_layout),
            sys_text_engine,
            cont_input_contents,
            topo_active_masks,
            topo_parents,
            lay_resolved_basic,
            lay_resolved_flex,
            lay_resolved_grid,
            rnd_interaction,
            rnd_active_transitions,
            rnd_visual,
            out_rects,
            out_scroll_offsets,
        );
        let (clicked_index, is_trailing) =
            sys_text_engine.hit_test_point(&dw_layout, local.x, local.y);
        let final_index = if is_trailing {
            clicked_index + 1
        } else {
            clicked_index
        };

        if pressed_shift {
            // 共通の Shift選択拡張
            let anchor = out_selection_start_index
                .get(id)
                .copied()
                .unwrap_or(final_index);
            if !out_selection_start_index.contains_key(id) {
                out_selection_start_index.insert(id, final_index);
            }
            let range = if anchor <= final_index {
                anchor..final_index
            } else {
                final_index..anchor
            };
            out_text_selections.insert(id, range);
            OutputStore::update_selection_rects(
                id,
                &dw_layout,
                out_selected_rects,
                out_text_selections,
            );
        } else {
            // 共通の通常クリックリセット
            out_selection_start_index.insert(id, final_index);
            out_text_selections.insert(id, final_index..final_index);
            out_selected_rects.remove(id);
        }

        RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);
    }

    /// 入力トリガー源を考慮してフォーカス状態を更新します。
    #[inline]
    pub(crate) fn set_focused_by_trigger(
        cx: &mut Context,
        id: EntityId,
        focused: bool,
        trigger: ActiveFocusTrigger,
    ) {
        EventStore::update_state(cx, id, ComponentMask::STATE_FOCUSED, focused);
        let show_visible = focused && (trigger == ActiveFocusTrigger::Keyboard);
        EventStore::update_state(cx, id, ComponentMask::STATE_FOCUSED_VISIBLE, show_visible);
    }

    pub(crate) fn auto_focus_switch_by_trigger(
        cx: &mut Context,
        id: EntityId,
        trigger: ActiveFocusTrigger,
    ) {
        // 同一要素をクリックした場合はフォーカス可視化の同期のみ
        if cx.events.evt_interaction_states.focused == Some(id) {
            EventStore::set_focused_by_trigger(cx, id, true, trigger);
            return;
        }

        if let Some(old_focus_id) = cx.events.evt_interaction_states.focused {
            EventStore::set_focused_by_trigger(cx, old_focus_id, false, trigger);

            // 古いフォーカス要素の選択範囲とハイライト矩形をクリア
            OutputStore::clear_selection_highlight_rect(
                old_focus_id,
                &mut cx.topology.topo_active_masks,
                &mut cx.contents.cont_input_contents,
                &mut cx.contents.cont_text_spans,
                &mut cx.outputs.out_text_selections,
                &mut cx.outputs.out_selected_rects,
            );
            // 進行中の IME コンポジションを強制的に確定させ候補窓を閉じる
            SystemStore::force_complete_ime_composition();

            handle_on_blur(cx, old_focus_id);
        }

        // 新しいフォーカス可能要素にフォーカスを設定
        EventStore::set_focused_by_trigger(cx, id, true, trigger);

        // 新しいフォーカス先が is_ime(false) の場合は IME 関連付けを解除
        let is_input = cx.topology.topo_active_masks[id].has_input_content();
        if is_input && let Some(contents) = cx.contents.cont_input_contents.get(id) {
            SystemStore::unassociate_ime(contents, &mut cx.window.win_default_himc);
        } else {
            // インプット以外の場合は IME をデフォルト状態に戻す
            SystemStore::reset_ime_default_state(cx.window.win_default_himc.as_ref());
        }

        cx.events.evt_interaction_states.focused = Some(id);

        handle_on_focus(cx, id);
    }

    #[inline]
    pub(crate) fn handle_remove_focus(cx: &mut Context) {
        let Some(old_focus_id) = cx.events.evt_interaction_states.focused else {
            return;
        };

        // 先にフォーカス状態を解除しておく
        // コールバック内で再フォーカスされても上書きしないため
        cx.events.evt_interaction_states.focused = None;

        EventStore::set_focused_by_trigger(cx, old_focus_id, false, ActiveFocusTrigger::Mouse);

        // 古いフォーカス要素の選択範囲とハイライト矩形をクリア
        OutputStore::clear_selection_highlight_rect(
            old_focus_id,
            &mut cx.topology.topo_active_masks,
            &mut cx.contents.cont_input_contents,
            &mut cx.contents.cont_text_spans,
            &mut cx.outputs.out_text_selections,
            &mut cx.outputs.out_selected_rects,
        );
        // 進行中の IME コンポジションを強制的に確定させ候補窓を閉じる
        SystemStore::force_complete_ime_composition();

        // IME をデフォルトの有効化状態に戻す
        SystemStore::reset_ime_default_state(cx.window.win_default_himc.as_ref());

        handle_on_blur(cx, old_focus_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputOp {
    // 初期化用
    Init,
    // シグナル監視エフェクト同期用
    TextEffect,
    MousePress,
    ArrowMove,
    SelectAll,
    CharTyped,
    Backspace,
    Delete,
    ImeUpdated,
    Paste,
    Cut,
    Undo,
    Redo,
}

impl EventStore {
    pub(crate) fn apply_input_update(
        id: EntityId,
        op: InputOp,
        win_scale_factor: f32,
        win_last_size: Option<LayoutSize>,
        sys_text_engine: &TextEngine,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        cont_text_contents: &mut TextContentsSparseSecondary,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparseSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        out_scroll_offsets: &mut ScrollOffsetsSecondary,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_selected_rects: &mut SelectedRectsSparseSecondary,
        out_rects: &RectsSecondary,
        out_scroll_sizes: &ScrollSizesSecondary,
    ) {
        // 直前までハイライトが描画されていたか
        let has_selection_before = out_selected_rects.contains_key(id);

        // 現在（操作後）に範囲選択されているか
        let has_selection_after = cont_input_contents
            .get(id)
            .is_some_and(|c| c.selected_range.start != c.selected_range.end);

        // 選択範囲の描画を更新
        if (has_selection_before || has_selection_after)
            && let Some(layout) = SystemStore::get_or_create_layout(
                id,
                sys_text_engine,
                sys_dwrite_layouts,
                cont_text_contents,
                cont_text_spans,
                lay_resolved_basic,
                rnd_visual,
                out_rects,
            )
        {
            OutputStore::update_selection_rects(
                id,
                &layout,
                out_selected_rects,
                out_text_selections,
            );
        }

        match op {
            // コンテンツのサイズに変動がない（Taffyレイアウトの再計算が不要）操作
            InputOp::MousePress | InputOp::ArrowMove | InputOp::SelectAll => {
                OutputStore::update_input_caret_position(
                    id,
                    win_scale_factor,
                    win_last_size,
                    sys_text_engine,
                    sys_dwrite_layouts,
                    cont_text_contents,
                    cont_input_contents,
                    cont_text_spans,
                    topo_active_masks,
                    topo_parents,
                    lay_dirty_entities,
                    lay_taffy_tree,
                    lay_scrollbar_styles,
                    lay_taffy_nodes,
                    lay_resolved_basic,
                    lay_resolved_flex,
                    lay_resolved_grid,
                    rnd_visual,
                    rnd_base_visual,
                    rnd_interaction,
                    rnd_active_transitions,
                    out_scroll_offsets,
                    out_text_selections,
                    out_rects,
                    out_scroll_sizes,
                );
                RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);
            }

            // Taffy計算前に表示用テキストの同期が必須
            // 未確定文字の伸縮時はシグナルが更新されないため ImeUpdated を含める
            InputOp::Init | InputOp::TextEffect | InputOp::ImeUpdated => {
                OutputStore::update_input_caret_position(
                    id,
                    win_scale_factor,
                    win_last_size,
                    sys_text_engine,
                    sys_dwrite_layouts,
                    cont_text_contents,
                    cont_input_contents,
                    cont_text_spans,
                    topo_active_masks,
                    topo_parents,
                    lay_dirty_entities,
                    lay_taffy_tree,
                    lay_scrollbar_styles,
                    lay_taffy_nodes,
                    lay_resolved_basic,
                    lay_resolved_flex,
                    lay_resolved_grid,
                    rnd_visual,
                    rnd_base_visual,
                    rnd_interaction,
                    rnd_active_transitions,
                    out_scroll_offsets,
                    out_text_selections,
                    out_rects,
                    out_scroll_sizes,
                );
                TopologyStore::mark_dirty(
                    id,
                    topo_active_masks,
                    topo_parents,
                    lay_dirty_entities,
                    lay_taffy_tree,
                    lay_taffy_nodes,
                    rnd_dirty_entities,
                );
            }

            // シグナルエフェクト側で update_input_caret_position が走るが、
            // 不具合によってエフェクト自体がスキップされることも考慮してフラグを立てる
            InputOp::CharTyped
            | InputOp::Backspace
            | InputOp::Delete
            | InputOp::Paste
            | InputOp::Cut
            | InputOp::Undo
            | InputOp::Redo => {
                TopologyStore::mark_dirty(
                    id,
                    topo_active_masks,
                    topo_parents,
                    lay_dirty_entities,
                    lay_taffy_tree,
                    lay_taffy_nodes,
                    rnd_dirty_entities,
                );
            }
        }
    }
}

impl Context {
    #[inline]
    pub(crate) fn apply_input_update(&mut self, id: EntityId, op: InputOp) {
        EventStore::apply_input_update(
            id,
            op,
            self.window.win_scale_factor,
            self.window.win_last_size,
            &self.system.sys_text_engine,
            &self.system.sys_dwrite_layouts,
            &mut self.contents.cont_text_contents,
            &mut self.contents.cont_input_contents,
            &self.contents.cont_text_spans,
            &mut self.topology.topo_active_masks,
            &self.topology.topo_parents,
            &mut self.layouts.lay_dirty_entities,
            &mut self.layouts.lay_taffy_tree,
            &mut self.layouts.lay_scrollbar_styles,
            &self.layouts.lay_taffy_nodes,
            &self.layouts.lay_resolved_basic,
            &self.layouts.lay_resolved_flex,
            &self.layouts.lay_resolved_grid,
            &mut self.renders.rnd_dirty_entities,
            &mut self.renders.rnd_visual,
            &self.renders.rnd_base_visual,
            &self.renders.rnd_interaction,
            &self.renders.rnd_active_transitions,
            &mut self.outputs.out_scroll_offsets,
            &mut self.outputs.out_text_selections,
            &mut self.outputs.out_selected_rects,
            &self.outputs.out_rects,
            &self.outputs.out_scroll_sizes,
        );
    }
}
