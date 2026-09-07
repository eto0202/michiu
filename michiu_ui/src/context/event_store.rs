use crate::{
    ActiveAnimationsSparseSecondary, ActiveEntitiesVec, ActiveFocusTrigger, ActiveMasksSecondary,
    ActiveTransitionsSparseSecondary, BaseBasicLayoutsSecondary, BaseVisualPropertiesSecondary,
    BasicLayout, BasicLayoutsSecondary, ByteIndex, CapacityConfig, ChildrenSecondary,
    ClipRectsSecondary, ComponentMask, ContentStore, Context, CursorIcon, DfsIndicesSecondary,
    DirtyLayoutEntitiesVec, DirtyRenderEntitiesVec, DndStore, EffectiveZindicesSecondary, Element,
    ElementEffectsSecondary, ElementState, EntitiesSlot, EntityId, EventListeners,
    FlatDfsSequenceVec, FlexLayout, FlexLayoutsSecondary, FocusStore, GridLayout,
    GridLayoutsSparseSecondary, InputContents, InputContentsSparseSecondary, InputOp,
    InteractionPropertiesSecondary, LayoutPoint, LayoutRect, LayoutSize, LayoutStore, Length,
    MichiuString, Modifiers, MouseButton, OutputStore, Overflow, ParentsSecondary, Pipeline,
    PointerEvents, Position, RangeExt, ReactiveStore, Rect, RectsSecondary, RenderStore,
    ResizeStore, ResolvedBasicSecondary, ResolvedFlexSecondary, ResolvedGridSparseSecondary,
    ScrollOffsetsSecondary, ScrollSizesSecondary, ScrollStore, ScrollbarStore,
    ScrollbarStylesSecondary, SelectedRectsSparseSecondary, SelectionStartIndexSparseSecondary,
    SessionSpawnedVec, SortedEntitiesVec, SystemStore, TaffyNodesSecondary, TaffyTreeEntityId,
    TextAlign, TextBufferSparseSecondary, TextContentsSparseSecondary, TextEditStore, TextEngine,
    TextSelectionsSparseSecondary, TextSpansSparseSecondary, TopoSortCacheVec, TopologyStore,
    UserSelect, UsizeRangeExt, Val, VirtualKey, VisualPropertiesSecondary, WindowStore,
    bind_context, handle_on_active, handle_on_blur, handle_on_click, handle_on_cursor_moved,
    handle_on_disable, handle_on_dnd_drag_start, handle_on_dnd_entity_drag,
    handle_on_dnd_entity_drop, handle_on_dnd_id_drag, handle_on_dnd_id_drop, handle_on_drag,
    handle_on_focus, handle_on_hover, handle_on_keyboard_input, handle_on_mouse_enter,
    handle_on_mouse_input, handle_on_mouse_leave, handle_on_mouse_wheel, handle_on_right_click,
    handle_on_select,
};
use slotmap::{SecondaryMap, SparseSecondaryMap};
use smallvec::SmallVec;
use std::{borrow::Cow, ops::Range, path::PathBuf};
use windows::Win32::Graphics::DirectWrite::IDWriteTextLayout;

/// 実行時にウィンドウ内で現在アクティブ（排他的）になっている、各状態の対象要素（EntityId）を管理します。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct ActiveInteractionStates {
    pub hovered: Option<EntityId>,
    pub focused: Option<EntityId>,
    pub pressed: Option<EntityId>,
    pub dragged: Option<EntityId>,
}

impl ActiveInteractionStates {
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
    pub(crate) evt_listeners: EventListenersSparseSecondary,
    pub(crate) evt_interaction_states: ActiveInteractionStates,
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
            evt_listeners: SparseSecondaryMap::new(),
            evt_interaction_states: ActiveInteractionStates::new(),
            evt_current_pointer_position: None,
        }
    }

    #[inline]
    #[must_use]
    pub fn with_capacity(c: &CapacityConfig) -> Self {
        Self {
            evt_listeners: SparseSecondaryMap::with_capacity(c.evt_listeners),
            ..Default::default()
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.evt_listeners.clear();
        self.evt_interaction_states = ActiveInteractionStates::new();
        self.evt_current_pointer_position = None;
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.evt_listeners.remove(id);
        self.evt_interaction_states.clear_entity(id);
    }
}

impl EventStore {
    fn resolve_hover_state(cx: &mut Context, target_id: Option<EntityId>) {
        let old_id = cx.events.evt_interaction_states.hovered;

        if old_id == target_id {
            return;
        }

        cx.events.evt_interaction_states.hovered = target_id;

        // 旧ホバー要素からマウスが去った
        if let Some(old_id) = old_id {
            Pipeline::update_state(cx, old_id, ComponentMask::STATE_HOVERED, false);
            handle_on_mouse_leave(cx, old_id);
        }

        // 新ホバー要素にマウスが入った
        if let Some(new_id) = target_id {
            Pipeline::update_state(cx, new_id, ComponentMask::STATE_HOVERED, true);
            handle_on_mouse_enter(cx, new_id);
            handle_on_hover(cx, new_id);
        }
    }

    #[inline]
    fn propagate_cursor_move_events(
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

    pub(crate) fn inject_pointer_move(cx: &mut Context, logical_pos: LayoutPoint) {
        let prev_pos = cx.events.evt_current_pointer_position;
        cx.events.evt_current_pointer_position = Some(logical_pos);

        // リサイズ中のドラッグ同期処理
        if let Some(ref state) = cx.states.resize.res_resizing_state {
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

        ScrollStore::sync_scrollbar_drag(
            logical_pos,
            cx.window.win_last_size,
            &mut cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_taffy_tree,
            &mut cx.layouts.scrollbar.bar_styles,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_resolved_basic,
            &mut cx.renders.rnd_dirty_entities,
            &cx.renders.rnd_visual,
            &cx.renders.rnd_interaction,
            &cx.renders.rnd_active_transitions,
            &mut cx.states.scroll.sc_offsets,
            &cx.outputs.out_rects,
            &cx.states.scroll.sc_sizes,
        );

        // ヒットテストのキャッシュ
        let hit_id = TopologyStore::hit_test(
            logical_pos,
            cx.window.win_last_size,
            &cx.events.evt_interaction_states,
            &mut cx.topology.topo_active_masks,
            &mut cx.topology.topo_dfs_indices,
            &mut cx.topology.topo_effective_z_indices,
            &mut cx.topology.topo_sorted_entities,
            &mut cx.topology.topo_sort_cache,
            &mut cx.topology.topo_is_sort_dirty,
            &cx.topology.topo_active_entities,
            &cx.topology.topo_parents,
            &cx.topology.topo_flat_dfs_sequence,
            &cx.renders.rnd_visual,
            &cx.renders.rnd_base_visual,
            &mut cx.outputs.out_clip_rects,
            &cx.outputs.out_rects,
        );

        // マウスボタン押し下げ中は、他の要素へのインタラクション漏洩を防ぐためヒット先を押し下げ要素に強制ロック
        let target_id = cx.events.evt_interaction_states.pressed.or(hit_id);

        // 直前のリサイズホバー対象を退避
        let prev_resize_hover = cx.states.resize.res_active_resize_hover;
        // リサイズホバー情報を一旦リセット
        cx.states.resize.res_active_resize_hover = None;

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
            cx.states.resize.res_active_resize_hover = Some((id, dir));
            ResizeStore::apply_resizable_cursor_style(id, dir, &mut cx.renders.rnd_visual);

            RenderStore::mark_render_dirty(
                id,
                &mut cx.topology.topo_active_masks,
                &mut cx.renders.rnd_dirty_entities,
            );
        }

        // 枠線から外れた、または異なる要素に変わった場合
        if let Some((prev_id, _)) = prev_resize_hover {
            let now_id = cx.states.resize.res_active_resize_hover.map(|(id, _)| id);

            // 異なるホバー状態になった場合、旧要素のカーソル上書きを破棄し本来のスタイルに即時強制リセット
            if Some(prev_id) != now_id {
                // スタイルの再解決を叩き、上書きされていた vis.cursor を本来のカーソル（通常ホバー/ベース等）へ復旧
                RenderStore::resolve_element_style_state(
                    prev_id,
                    false,
                    cx.window.win_last_size.as_ref(),
                    &cx.system.sys_text_buffers,
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
                    .states
                    .edit
                    .edit_selection_start_index
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

                let engine = SystemStore::get_or_create_layout(
                    pressed_id,
                    &mut cx.system.sys_text_engine,
                    &cx.system.sys_text_buffers,
                    &cx.contents.cont_text_contents,
                    &cx.contents.cont_text_spans,
                    &cx.layouts.lay_resolved_basic,
                    &cx.layouts.lay_resolved_flex,
                    &cx.renders.rnd_visual,
                    &cx.outputs.out_rects,
                );
                let local = OutputStore::pressed_local_point(
                    pressed_id,
                    logical_pos,
                    engine.as_ref(),
                    &mut cx.system.sys_text_engine,
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
                    &cx.states.scroll.sc_offsets,
                );

                TextEditStore::handle_text_selection_click(
                    pressed_id,
                    start_pos.into(),
                    local,
                    cx.window.win_scale_factor,
                    cx.window.win_last_size,
                    &mut cx.system.sys_text_engine,
                    &cx.system.sys_text_buffers,
                    &mut cx.contents.cont_text_contents,
                    &mut cx.contents.cont_input_contents,
                    &cx.contents.cont_text_spans,
                    &mut cx.topology.topo_active_masks,
                    &cx.topology.topo_parents,
                    &cx.topology.topo_children,
                    &mut cx.layouts.lay_dirty_entities,
                    &mut cx.layouts.lay_taffy_tree,
                    &mut cx.layouts.scrollbar.bar_styles,
                    &cx.layouts.lay_taffy_nodes,
                    &cx.layouts.lay_resolved_basic,
                    &cx.layouts.lay_resolved_flex,
                    &cx.layouts.lay_resolved_grid,
                    &mut cx.renders.rnd_dirty_entities,
                    &mut cx.renders.rnd_visual,
                    &cx.renders.rnd_base_visual,
                    &cx.renders.rnd_interaction,
                    &cx.renders.rnd_active_transitions,
                    &mut cx.states.scroll.sc_offsets,
                    &mut cx.states.edit.edit_selections,
                    &mut cx.states.edit.edit_selected_rects,
                    &cx.outputs.out_rects,
                    &cx.states.scroll.sc_sizes,
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
        let Some(ref drag_state) = cx.states.dnd.dnd_active_drag_state else {
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
            .states
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

    fn handle_pointer_pressed(cx: &mut Context, button: MouseButton, modifiers: Modifiers) {
        let current_hovered = cx.events.evt_interaction_states.hovered;

        // リサイズドラッグの開始判定（左クリック時のみ）
        if button == MouseButton::Left
            && let Some((id, dir)) = cx.states.resize.res_active_resize_hover
        {
            ResizeStore::state_pressed_resize_drag(
                id,
                dir,
                &mut cx.events.evt_interaction_states,
                &mut cx.states.resize.res_resizing_state,
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
            let clicked_scrollbar = ScrollbarStore::hit_decision_element_scrollbar(
                target_id,
                pointer_pos,
                cx.window.win_last_size,
                &mut cx.events.evt_interaction_states,
                &mut cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_taffy_tree,
                &mut cx.layouts.scrollbar.bar_styles,
                &cx.layouts.lay_taffy_nodes,
                &cx.layouts.lay_resolved_basic,
                &mut cx.renders.rnd_dirty_entities,
                &cx.renders.rnd_visual,
                &cx.renders.rnd_interaction,
                &cx.renders.rnd_active_transitions,
                &mut cx.states.scroll.sc_offsets,
                &cx.outputs.out_rects,
                &cx.states.scroll.sc_sizes,
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
        Pipeline::update_state(cx, target_id, ComponentMask::STATE_PRESSED, true);

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
            TextEditStore::handle_user_select_text(
                target_id,
                pointer_pos,
                modifiers.shift,
                &mut cx.system.sys_text_engine,
                &cx.system.sys_text_buffers,
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
                &mut cx.states.edit.edit_selections,
                &mut cx.states.edit.edit_selection_start_index,
                &mut cx.states.edit.edit_selected_rects,
                &cx.outputs.out_rects,
                &cx.states.scroll.sc_offsets,
            );
        }

        // フォーカスの解決
        if !FocusStore::should_prevent_focus_steal(cx, target_id) {
            let is_focusable = FocusStore::restrict_focusable_element(
                target_id,
                &cx.topology.topo_active_masks,
                &cx.renders.rnd_visual,
            );
            if is_focusable {
                FocusStore::auto_focus_switch_by_trigger(cx, target_id, ActiveFocusTrigger::Mouse);
            } else {
                FocusStore::handle_remove_focus(cx);
            }
        }

        // ユーザーイベントの発火
        handle_on_mouse_input(cx, target_id, button, modifiers, ElementState::Pressed);
    }

    #[inline]
    fn handle_pointer_released(cx: &mut Context, button: MouseButton, modifiers: Modifiers) {
        // リサイズドラッグの終了処理
        if let Some(state) = cx.states.resize.res_resizing_state.take() {
            let id = state.entity_id;
            cx.events.evt_interaction_states.pressed = None;

            if let Some(pos) = cx.events.evt_current_pointer_position {
                EventStore::inject_pointer_move(cx, pos);
            }
            RenderStore::mark_render_dirty(
                id,
                &mut cx.topology.topo_active_masks,
                &mut cx.renders.rnd_dirty_entities,
            );
            return;
        }

        // D&D ドラッグ終了・ドロップ確定処理
        if let Some(drag_state) = cx.states.dnd.dnd_active_drag_state.take() {
            DndStore::handle_dnd_drop(cx, &drag_state);
            return;
        }

        // スクロールバーの表示更新
        let dirty_ids =
            ScrollbarStore::get_scrollbar_dirty_ids(&mut cx.layouts.scrollbar.bar_styles);
        for id in dirty_ids {
            RenderStore::mark_render_dirty(
                id,
                &mut cx.topology.topo_active_masks,
                &mut cx.renders.rnd_dirty_entities,
            );
        }

        // 通常要素のリリース
        if let Some(pressed_id) = cx.events.evt_interaction_states.pressed {
            Pipeline::update_state(cx, pressed_id, ComponentMask::STATE_PRESSED, false);
            Pipeline::update_state(cx, pressed_id, ComponentMask::STATE_DRAGGED, false);
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
    pub(crate) fn inject_pointer_button(
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

    pub(crate) fn inject_pointer_double_click(cx: &mut Context, modifiers: Modifiers) {
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
            let text_val = contents.to_michiu();
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

        let Some(buffer) = SystemStore::get_or_create_layout(
            target_id,
            &mut cx.system.sys_text_engine,
            &cx.system.sys_text_buffers,
            &cx.contents.cont_text_contents,
            &cx.contents.cont_text_spans,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
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

        let (clicked_index, _) = cx
            .system
            .sys_text_engine
            .hit_test_point(&buffer, LayoutPoint::new(local_x, local_y));

        let range = text.find_word_boundaries(clicked_index);

        cx.states
            .edit
            .edit_selections
            .insert(target_id, range.clone().to_usize_range());
        // アンカー開始を文節左端にセット
        cx.states
            .edit
            .edit_selection_start_index
            .insert(target_id, range.start.0);
        // 選択矩形を更新
        TextEditStore::update_selection_rects(
            target_id,
            &buffer,
            &mut cx.states.edit.edit_selected_rects,
            &cx.states.edit.edit_selections,
            &cx.contents.cont_input_contents,
        );

        if let Some(contents) = cx.contents.cont_input_contents.get_mut(target_id) {
            contents.selected_range = range;
            contents.selection_reversed = false; // キャレットは右端に配置
            TextEditStore::apply_input_update(
                target_id,
                InputOp::MousePress,
                cx.window.win_scale_factor,
                cx.window.win_last_size,
                &mut cx.system.sys_text_engine,
                &cx.system.sys_text_buffers,
                &mut cx.contents.cont_text_contents,
                &mut cx.contents.cont_input_contents,
                &cx.contents.cont_text_spans,
                &mut cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_taffy_tree,
                &mut cx.layouts.scrollbar.bar_styles,
                &cx.layouts.lay_taffy_nodes,
                &cx.layouts.lay_resolved_basic,
                &cx.layouts.lay_resolved_flex,
                &cx.layouts.lay_resolved_grid,
                &mut cx.renders.rnd_dirty_entities,
                &mut cx.renders.rnd_visual,
                &cx.renders.rnd_base_visual,
                &cx.renders.rnd_interaction,
                &cx.renders.rnd_active_transitions,
                &mut cx.states.scroll.sc_offsets,
                &mut cx.states.edit.edit_selections,
                &mut cx.states.edit.edit_selected_rects,
                &cx.outputs.out_rects,
                &cx.states.scroll.sc_sizes,
            );
        } else {
            RenderStore::mark_render_dirty(
                target_id,
                &mut cx.topology.topo_active_masks,
                &mut cx.renders.rnd_dirty_entities,
            );
        }
    }

    pub(crate) fn inject_mouse_wheel(cx: &mut Context, scroll_x: f32, scroll_y: f32) {
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
                    && ScrollStore::scroll_by(
                        curr_id,
                        dx,
                        dy,
                        cx.window.win_last_size,
                        &mut cx.topology.topo_active_masks,
                        &cx.topology.topo_parents,
                        &mut cx.layouts.lay_dirty_entities,
                        &mut cx.layouts.lay_taffy_tree,
                        &mut cx.layouts.scrollbar.bar_styles,
                        &cx.layouts.lay_taffy_nodes,
                        &cx.layouts.lay_resolved_basic,
                        &cx.renders.rnd_visual,
                        &cx.renders.rnd_interaction,
                        &cx.renders.rnd_active_transitions,
                        &mut cx.states.scroll.sc_offsets,
                        &cx.outputs.out_rects,
                        &cx.states.scroll.sc_sizes,
                    )
                {
                    break; // スクロールを実行したためバブリングを終了
                }
            }

            // 先祖へ伝播
            curr = cx.topology.topo_parents.get(curr_id).copied().flatten();
        }
    }

    pub(crate) fn inject_keyboard_key(
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
                TextEditStore::handle_select_all(
                    focused_id,
                    cx.window.win_scale_factor,
                    cx.window.win_last_size,
                    &mut cx.system.sys_text_engine,
                    &cx.system.sys_text_buffers,
                    &mut cx.contents.cont_text_contents,
                    &mut cx.contents.cont_input_contents,
                    &cx.contents.cont_text_spans,
                    &mut cx.topology.topo_active_masks,
                    &cx.topology.topo_parents,
                    &cx.topology.topo_children,
                    &mut cx.layouts.lay_dirty_entities,
                    &mut cx.layouts.lay_taffy_tree,
                    &mut cx.layouts.scrollbar.bar_styles,
                    &cx.layouts.lay_taffy_nodes,
                    &cx.layouts.lay_resolved_basic,
                    &cx.layouts.lay_resolved_flex,
                    &cx.layouts.lay_resolved_grid,
                    &mut cx.renders.rnd_dirty_entities,
                    &mut cx.renders.rnd_visual,
                    &cx.renders.rnd_base_visual,
                    &cx.renders.rnd_interaction,
                    &cx.renders.rnd_active_transitions,
                    &mut cx.states.scroll.sc_offsets,
                    &mut cx.states.edit.edit_selections,
                    &mut cx.states.edit.edit_selected_rects,
                    &cx.outputs.out_rects,
                    &cx.states.scroll.sc_sizes,
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

            if FocusStore::is_keyboard_focusable(
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
            FocusStore::set_focused_by_trigger(cx, old_id, false, ActiveFocusTrigger::Keyboard);
        }

        // 新フォーカスの設定
        FocusStore::set_focused_by_trigger(cx, candidate_id, true, ActiveFocusTrigger::Keyboard);
        cx.events.evt_interaction_states.focused = Some(candidate_id);

        RenderStore::mark_render_dirty(
            candidate_id,
            &mut cx.topology.topo_active_masks,
            &mut cx.renders.rnd_dirty_entities,
        );
    }

    fn handle_paste(
        focused_id: EntityId,
        text: &MichiuString,
        contents: &mut InputContents,
        edit_selections: &mut TextSelectionsSparseSecondary,
        edit_selected_rects: &mut SelectedRectsSparseSecondary,
    ) {
        let text_val = contents.to_michiu();
        let range = contents.selected_range.clone();

        // 文字数制限の計算
        let allowed_len = if let Some(max) = contents.max_length {
            let selected_char_count = text_val.slice(range.clone()).chars().count();
            let current_len_after_delete = text_val.char_count().0 - selected_char_count;

            if current_len_after_delete >= max.0 {
                return; // 枠が残っていないので貼り付け中断
            }
            max.0 - current_len_after_delete
        } else {
            usize::MAX
        };

        let mut pasted = String::new();
        let mut chars_added = 0;

        for c in text.chars() {
            // 数値制限フィルタ
            if contents.numeric_only && !c.is_numeric() && c != '.' && c != '-' {
                continue;
            }
            // 文字数制限カット
            if chars_added >= allowed_len {
                break;
            }
            pasted.push(c);
            chars_added += 1;
        }

        // 貼り付ける文字がなく、削除する選択範囲もない
        if pasted.is_empty() && range.start == range.end {
            return;
        }

        // 変更前の状態を Undo に退避
        contents.record_undo(text_val, range.clone());

        // MichiuString で置換
        let new_caret = contents.update_michiu(|m| m.replace_range(range, &pasted));

        let new_range = new_caret..new_caret;
        contents.selected_range = new_range.clone();
        edit_selections.insert(focused_id, new_range.to_usize_range());
        edit_selected_rects.remove(focused_id);
    }

    pub(crate) fn inject_paste(cx: &mut Context, text: &MichiuString) {
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

        EventStore::handle_paste(
            focused_id,
            text,
            contents,
            &mut cx.states.edit.edit_selections,
            &mut cx.states.edit.edit_selected_rects,
        );
        TextEditStore::update_input_caret_position(
            focused_id,
            cx.window.win_scale_factor,
            cx.window.win_last_size,
            &mut cx.system.sys_text_engine,
            &cx.system.sys_text_buffers,
            &mut cx.contents.cont_text_contents,
            &mut cx.contents.cont_input_contents,
            &cx.contents.cont_text_spans,
            &mut cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_taffy_tree,
            &mut cx.layouts.scrollbar.bar_styles,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
            &cx.layouts.lay_resolved_grid,
            &mut cx.renders.rnd_visual,
            &cx.renders.rnd_base_visual,
            &cx.renders.rnd_interaction,
            &cx.renders.rnd_active_transitions,
            &mut cx.states.scroll.sc_offsets,
            &mut cx.states.edit.edit_selections,
            &cx.outputs.out_rects,
            &cx.states.scroll.sc_sizes,
        );
        TextEditStore::apply_input_update(
            focused_id,
            InputOp::Paste,
            cx.window.win_scale_factor,
            cx.window.win_last_size,
            &mut cx.system.sys_text_engine,
            &cx.system.sys_text_buffers,
            &mut cx.contents.cont_text_contents,
            &mut cx.contents.cont_input_contents,
            &cx.contents.cont_text_spans,
            &mut cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_taffy_tree,
            &mut cx.layouts.scrollbar.bar_styles,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
            &cx.layouts.lay_resolved_grid,
            &mut cx.renders.rnd_dirty_entities,
            &mut cx.renders.rnd_visual,
            &cx.renders.rnd_base_visual,
            &cx.renders.rnd_interaction,
            &cx.renders.rnd_active_transitions,
            &mut cx.states.scroll.sc_offsets,
            &mut cx.states.edit.edit_selections,
            &mut cx.states.edit.edit_selected_rects,
            &cx.outputs.out_rects,
            &cx.states.scroll.sc_sizes,
        );
    }

    fn handle_undo(
        focused_id: EntityId,
        prev_sel: Range<ByteIndex>,
        prev_text: MichiuString,
        contents: &mut InputContents,
        edit_selections: &mut TextSelectionsSparseSecondary,
        edit_selected_rects: &mut SelectedRectsSparseSecondary,
    ) {
        contents.apply_undo(prev_text, prev_sel.clone());

        edit_selections.insert(focused_id, prev_sel.to_usize_range());
        edit_selected_rects.remove(focused_id);
    }

    pub(crate) fn inject_undo(cx: &mut Context) {
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

        EventStore::handle_undo(
            focused_id,
            prev_sel.clone(),
            prev_text,
            contents,
            &mut cx.states.edit.edit_selections,
            &mut cx.states.edit.edit_selected_rects,
        );

        cx.states
            .edit
            .edit_selection_start_index
            .insert(focused_id, prev_sel.start.0);

        TextEditStore::apply_input_update(
            focused_id,
            InputOp::Undo,
            cx.window.win_scale_factor,
            cx.window.win_last_size,
            &mut cx.system.sys_text_engine,
            &cx.system.sys_text_buffers,
            &mut cx.contents.cont_text_contents,
            &mut cx.contents.cont_input_contents,
            &cx.contents.cont_text_spans,
            &mut cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_taffy_tree,
            &mut cx.layouts.scrollbar.bar_styles,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
            &cx.layouts.lay_resolved_grid,
            &mut cx.renders.rnd_dirty_entities,
            &mut cx.renders.rnd_visual,
            &cx.renders.rnd_base_visual,
            &cx.renders.rnd_interaction,
            &cx.renders.rnd_active_transitions,
            &mut cx.states.scroll.sc_offsets,
            &mut cx.states.edit.edit_selections,
            &mut cx.states.edit.edit_selected_rects,
            &cx.outputs.out_rects,
            &cx.states.scroll.sc_sizes,
        );
    }

    fn handle_redo(
        focused_id: EntityId,
        next_sel: Range<ByteIndex>,
        next_text: MichiuString,
        contents: &mut InputContents,
        edit_selections: &mut TextSelectionsSparseSecondary,
        edit_selected_rects: &mut SelectedRectsSparseSecondary,
        edit_selection_start_index: &mut SelectionStartIndexSparseSecondary,
    ) {
        // InputContents 側の状態復元
        contents.apply_redo(next_text, next_sel.clone());

        edit_selections.insert(focused_id, next_sel.clone().to_usize_range());
        edit_selected_rects.remove(focused_id);
        edit_selection_start_index.insert(focused_id, next_sel.start.0);
    }

    pub(crate) fn inject_redo(cx: &mut Context) {
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

        EventStore::handle_redo(
            focused_id,
            next_sel.clone(),
            next_text,
            contents,
            &mut cx.states.edit.edit_selections,
            &mut cx.states.edit.edit_selected_rects,
            &mut cx.states.edit.edit_selection_start_index,
        );

        TextEditStore::apply_input_update(
            focused_id,
            InputOp::Redo,
            cx.window.win_scale_factor,
            cx.window.win_last_size,
            &mut cx.system.sys_text_engine,
            &cx.system.sys_text_buffers,
            &mut cx.contents.cont_text_contents,
            &mut cx.contents.cont_input_contents,
            &cx.contents.cont_text_spans,
            &mut cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_taffy_tree,
            &mut cx.layouts.scrollbar.bar_styles,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
            &cx.layouts.lay_resolved_grid,
            &mut cx.renders.rnd_dirty_entities,
            &mut cx.renders.rnd_visual,
            &cx.renders.rnd_base_visual,
            &cx.renders.rnd_interaction,
            &cx.renders.rnd_active_transitions,
            &mut cx.states.scroll.sc_offsets,
            &mut cx.states.edit.edit_selections,
            &mut cx.states.edit.edit_selected_rects,
            &cx.outputs.out_rects,
            &cx.states.scroll.sc_sizes,
        );
    }

    fn inject_cut_internal(
        focused_id: EntityId,
        range: Range<ByteIndex>,
        contents: &mut InputContents,
        edit_selections: &mut TextSelectionsSparseSecondary,
        edit_selected_rects: &mut SelectedRectsSparseSecondary,
    ) {
        // 削除前の履歴セーブ
        let current_text = contents.to_michiu();
        let current_range = contents.selected_range.clone();
        contents.record_undo(current_text, current_range);

        // remove_range() が内部で自動クランプして削除し、安全な新しいキャレット位置（始点）を返してくれる
        let new_caret = contents.update_michiu(|m| m.remove_range(range));

        // キャレット位置・選択状態の更新
        let new_range = new_caret..new_caret;
        contents.selected_range = new_range.clone();
        edit_selections.insert(focused_id, new_range.to_usize_range());
        edit_selected_rects.remove(focused_id);
    }

    pub(crate) fn inject_cut(cx: &mut Context) -> Option<MichiuString> {
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

        let range = cx.states.edit.edit_selections.get(focused_id)?;

        // 空の選択範囲の場合
        if range.start >= range.end {
            return None;
        }

        let text = cx.contents.cont_text_contents.get(focused_id)?;
        let cut_text = text.slice(range.clone().to_byte_range()).to_string();

        // 対象が Input コントロールである場合のみ書き換え
        let is_input = cx
            .topology
            .topo_active_masks
            .get(focused_id)
            .is_some_and(ComponentMask::has_input_content);

        // Input 用のコンテンツが実際に存在する場合のみ実行
        if is_input && let Some(contents) = cx.contents.cont_input_contents.get_mut(focused_id) {
            EventStore::inject_cut_internal(
                focused_id,
                range.clone().to_byte_range(),
                contents,
                &mut cx.states.edit.edit_selections,
                &mut cx.states.edit.edit_selected_rects,
            );
            TextEditStore::apply_input_update(
                focused_id,
                InputOp::Cut,
                cx.window.win_scale_factor,
                cx.window.win_last_size,
                &mut cx.system.sys_text_engine,
                &cx.system.sys_text_buffers,
                &mut cx.contents.cont_text_contents,
                &mut cx.contents.cont_input_contents,
                &cx.contents.cont_text_spans,
                &mut cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_taffy_tree,
                &mut cx.layouts.scrollbar.bar_styles,
                &cx.layouts.lay_taffy_nodes,
                &cx.layouts.lay_resolved_basic,
                &cx.layouts.lay_resolved_flex,
                &cx.layouts.lay_resolved_grid,
                &mut cx.renders.rnd_dirty_entities,
                &mut cx.renders.rnd_visual,
                &cx.renders.rnd_base_visual,
                &cx.renders.rnd_interaction,
                &cx.renders.rnd_active_transitions,
                &mut cx.states.scroll.sc_offsets,
                &mut cx.states.edit.edit_selections,
                &mut cx.states.edit.edit_selected_rects,
                &cx.outputs.out_rects,
                &cx.states.scroll.sc_sizes,
            );
        }
        // Input・非Inputに関わらず切り出されたテキストを返す
        Some(cut_text.into())
    }
}

impl EventStore {}

#[cfg(test)]
mod tests;
