use std::path::PathBuf;

use crate::{
    ActiveAnimationsSparseSecondary, ActiveEntitiesVec, ActiveFocusTrigger, ActiveMasksSecondary,
    ActiveTransitionsSparseSecondary, BaseBasicLayoutsSecondary, BaseVisualPropertiesSecondary,
    BasicLayout, BasicLayoutsSecondary, ChildrenSecondary, ClipRectsSecondary, ComponentMask,
    ContentStore, Context, CursorIcon, DfsIndicesSecondary, DirtyLayoutEntitiesVec,
    DirtyRenderEntitiesVec, DndDragPayload, DndDragPlaceholderParent, DndDragProperty,
    DndDropProperty, DwriteLayoutsSparseSecondary, EffectiveZindicesSecondary, Element,
    ElementEffectsSecondary, ElementState, EntitiesSlot, EntityId, EventListeners,
    FlatDfsSequenceVec, FlexLayout, FlexLayoutsSecondary, FocusTrigger, Focusable, GridLayout,
    GridLayoutsSparseSecondary, InputContents, InputContentsSparseSecondary,
    InteractionPropertiesSecondary, InteractionStates, LayoutPoint, LayoutRect, LayoutSize,
    LayoutStore, Length, Modifiers, MouseButton, OutputStore, Overflow, ParentsSecondary,
    PointerEvents, Position, ReactiveStore, Rect, RectsSecondary, RenderStore,
    ResolvedBasicSecondary, ResolvedFlexSecondary, ResolvedGridSparseSecondary, STATE_ACTIVED,
    STATE_DISABLED, STATE_DND_DRAG_IN, STATE_DND_DRAG_OVER, STATE_DND_DRAGGING, STATE_DRAGGED,
    STATE_FOCUSED, STATE_FOCUSED_VISIBLE, STATE_HOVERED, STATE_PRESSED, STATE_SELECTED,
    STYLE_DND_DRAGGABLE, STYLE_DND_DROPPABLE, STYLE_INTERACTION_PARENT, STYLE_INTERACTION_WITHIN,
    STYLE_OVERFLOW, STYLE_POINTER_EVENTS, STYLE_PREVENT_FOCUS_STEAL,
    STYLE_PREVENT_FOCUS_STEAL_WITHIN, STYLE_RESIZABLE, ScrollOffsetsSecondary,
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
use windows::Win32::Graphics::DirectWrite::IDWriteTextLayout;

#[derive(Debug, Clone)]
pub(crate) struct ActiveDndDragState {
    pub(crate) source_entity: EntityId,      // ドラッグ元の要素
    pub(crate) placeholder_entity: EntityId, // ルートまたは親に浮かせているプレースホルダー
    pub(crate) current_drop_target: Option<EntityId>, // 現在ホバー侵入中のドロップターゲット要素
    pub(crate) start_mouse_pos: LayoutPoint, // ドラッグ開始時のマウス座標
    pub(crate) start_rect: LayoutRect,       // ドラッグ元の初期サイズ・座標
    pub(crate) click_offset: LayoutPoint,    // ドラッグ開始時のマウスと要素左上端の相対的なズレ
    pub(crate) original_parent: Option<EntityId>,
}

/// プレースホルダーをアタッチする際の親要素の情報
pub(crate) struct PlaceholderAttachment {
    pub(crate) parent_id: Option<EntityId>,
    pub(crate) rect: LayoutRect,
    pub(crate) border_left: f32,
    pub(crate) border_top: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResizeDirection {
    Top,
    Right,
    Bottom,
    Left,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone)]
pub(crate) struct ResizingState {
    pub(crate) entity_id: EntityId,
    pub(crate) direction: ResizeDirection,
    pub(crate) start_mouse_pos: LayoutPoint,
    pub(crate) start_rect: LayoutRect,
    pub(crate) start_inset: Rect<Val>,
}

pub(crate) type EventListenersSparseSecondary = SparseSecondaryMap<EntityId, EventListeners>;
pub(crate) type ActiveResizeHoverOption = Option<(EntityId, ResizeDirection)>;
pub(crate) type DndDragPropertiesSparseSecondary = SparseSecondaryMap<EntityId, DndDragProperty>;
pub(crate) type DndDropPropertiesSparseSecondary = SparseSecondaryMap<EntityId, DndDropProperty>;

pub struct EventStore {
    pub(crate) evt_listeners: EventListenersSparseSecondary,
    pub evt_interaction_states: InteractionStates,
    pub(crate) evt_current_pointer_position: Option<LayoutPoint>,
    pub(crate) evt_resizing_state: Option<ResizingState>,
    pub(crate) evt_active_resize_hover: ActiveResizeHoverOption,
    pub(crate) evt_dnd_drag_properties: DndDragPropertiesSparseSecondary,
    pub(crate) evt_dnd_drop_properties: DndDropPropertiesSparseSecondary,
    pub(crate) evt_active_dnd_drag_state: Option<ActiveDndDragState>,
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
            evt_interaction_states: InteractionStates::new(),
            evt_current_pointer_position: None,
            evt_resizing_state: None,
            evt_active_resize_hover: None,
            evt_dnd_drag_properties: SparseSecondaryMap::new(),
            evt_dnd_drop_properties: SparseSecondaryMap::new(),
            evt_active_dnd_drag_state: None,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.evt_listeners.clear();
        self.evt_interaction_states = InteractionStates::new();
        self.evt_current_pointer_position = None;
        self.evt_resizing_state = None;
        self.evt_active_resize_hover = None;
        self.evt_dnd_drag_properties.clear();
        self.evt_dnd_drop_properties.clear();
        self.evt_active_dnd_drag_state = None;
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.evt_listeners.remove(id);
        self.evt_interaction_states.clear_entity(id);
        self.evt_dnd_drag_properties.remove(id);
        self.evt_dnd_drop_properties.remove(id);

        if let Some(ref state) = self.evt_active_dnd_drag_state
            && (state.source_entity == id || state.placeholder_entity == id)
        {
            self.evt_active_dnd_drag_state = None;
        }
    }
}

impl EventStore {
    pub(crate) fn resolve_dnd_placeholder_parent(
        root: EntityId,
        drag_prop: &DndDragProperty,
        lay_basic: &BasicLayoutsSecondary,
        out_rects: &RectsSecondary,
    ) -> PlaceholderAttachment {
        match drag_prop.placeholder_parent {
            DndDragPlaceholderParent::Root => PlaceholderAttachment {
                parent_id: Some(root),
                rect: out_rects.get(root).copied().unwrap_or_default(),
                border_left: 0.0,
                border_top: 0.0,
            },
            DndDragPlaceholderParent::Custom(p_id) => {
                let p_rect = out_rects.get(p_id).copied().unwrap_or_default();
                let (b_l, b_t) = lay_basic
                    .get(p_id)
                    .map(|l| {
                        (
                            match l.border.left {
                                Length::Px(v) => v,
                                Length::Percent(_) => 0.0,
                            },
                            match l.border.top {
                                Length::Px(v) => v,
                                Length::Percent(_) => 0.0,
                            },
                        )
                    })
                    .unwrap_or_default();

                PlaceholderAttachment {
                    parent_id: Some(p_id),
                    rect: p_rect,
                    border_left: b_l,
                    border_top: b_t,
                }
            }
        }
    }

    pub(crate) fn apply_resizable_cursor_style(
        id: EntityId,
        dir: ResizeDirection,
        rnd_visual: &mut VisualPropertiesSecondary,
    ) {
        let Some(vis) = rnd_visual.get_mut(id) else {
            return;
        };
        // 方向に対応する配列インデックス
        let idx = match dir {
            ResizeDirection::Top | ResizeDirection::Bottom => 0, // Ns
            ResizeDirection::Left | ResizeDirection::Right => 1, // Ew
            ResizeDirection::TopRight | ResizeDirection::BottomLeft => 2, // Nesw
            ResizeDirection::TopLeft | ResizeDirection::BottomRight => 3, // Nwse
        };

        // 独自指定があればそれを引き、なければデフォルトをフォールバックして解決
        let cursor = vis
            .resizable_cursor
            .and_then(|arr| arr[idx])
            .unwrap_or_else(|| EventStore::resize_direction_to_cursor(dir));

        vis.cursor = Some(cursor);
    }

    pub(crate) fn found_resize_hover(
        target_id: Option<EntityId>,
        logical_pos: LayoutPoint,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_basic: &BasicLayoutsSecondary,
        out_rects: &RectsSecondary,
    ) -> (Option<EntityId>, Option<(EntityId, ResizeDirection)>) {
        let mut current_id = target_id;
        let mut found_resize_hover = None;
        while let Some(id) = current_id {
            if topo_active_masks[id].has(STYLE_RESIZABLE) {
                let rect = out_rects.get(id).copied().unwrap_or_default();
                let resizable_flags = lay_basic.get(id).map_or([false; 4], |l| l.resizable);

                // 境界外周に 6.0px のあそびを持たせてヒット判定
                let detect_border = 6.0f32;
                let direction = EventStore::detect_resize_direction(
                    rect,
                    resizable_flags,
                    logical_pos,
                    detect_border,
                );
                if let Some(dir) = direction {
                    found_resize_hover = Some((id, dir));
                    break; // 最も前面寄りのリサイズ親要素を優先採用
                }
            }
            current_id = topo_parents.get(id).copied().flatten();
        }
        (current_id, found_resize_hover)
    }

    /// リサイズ方向から対応するカーソル種別へ変換するヘルパー
    pub(crate) fn resize_direction_to_cursor(dir: ResizeDirection) -> CursorIcon {
        match dir {
            ResizeDirection::Top | ResizeDirection::Bottom => CursorIcon::ResizeNs(None),
            ResizeDirection::Left | ResizeDirection::Right => CursorIcon::ResizeEw(None),
            ResizeDirection::TopRight | ResizeDirection::BottomLeft => CursorIcon::ResizeNesw(None),
            ResizeDirection::TopLeft | ResizeDirection::BottomRight => CursorIcon::ResizeNwse(None),
        }
    }

    /// マウス位置と要素の境界・リサイズ許可フラグから、該当するリサイズ方向を算出するヘルパー
    pub(crate) fn detect_resize_direction(
        rect: LayoutRect,
        resizable: [bool; 4], // [top, right, bottom, left]
        pos: LayoutPoint,
        border: f32,
    ) -> Option<ResizeDirection> {
        let [t, r, b, l] = resizable;
        if !t && !r && !b && !l {
            return None;
        }

        // 境界線の外側（-border）から内側（+border）までのあそびの範囲を厳密に判定
        let on_t = t
            && (pos.y >= rect.y - border && pos.y <= rect.y + border)
            && (pos.x >= rect.x - border && pos.x <= rect.x + rect.width + border);

        let on_b = b
            && (pos.y >= rect.y + rect.height - border && pos.y <= rect.y + rect.height + border)
            && (pos.x >= rect.x - border && pos.x <= rect.x + rect.width + border);

        let on_l = l
            && (pos.x >= rect.x - border && pos.x <= rect.x + border)
            && (pos.y >= rect.y - border && pos.y <= rect.y + rect.height + border);

        let on_r = r
            && (pos.x >= rect.x + rect.width - border && pos.x <= rect.x + rect.width + border)
            && (pos.y >= rect.y - border && pos.y <= rect.y + rect.height + border);

        match (on_t, on_r, on_b, on_l) {
            (true, true, _, _) => Some(ResizeDirection::TopRight),
            (true, _, _, true) => Some(ResizeDirection::TopLeft),
            (_, true, true, _) => Some(ResizeDirection::BottomRight),
            (_, _, true, true) => Some(ResizeDirection::BottomLeft),
            (true, _, _, _) => Some(ResizeDirection::Top),
            (_, true, _, _) => Some(ResizeDirection::Right),
            (_, _, true, _) => Some(ResizeDirection::Bottom),
            (_, _, _, true) => Some(ResizeDirection::Left),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn drag_overhang_distance(
        pointer_pos: LayoutPoint,
        clip: &LayoutRect,
    ) -> LayoutPoint {
        let mut dx = 0.0f32;
        let mut dy = 0.0f32;

        // はみ出し距離
        if pointer_pos.x < clip.x {
            dx = pointer_pos.x - clip.x; // 左はみ出し：負値
        } else if pointer_pos.x > clip.x + clip.width {
            dx = pointer_pos.x - (clip.x + clip.width); // 右はみ出し：正値
        }

        if pointer_pos.y < clip.y {
            dy = pointer_pos.y - clip.y;
        } else if pointer_pos.y > clip.y + clip.height {
            dy = pointer_pos.y - (clip.y + clip.height);
        }

        LayoutPoint { x: dx, y: dy }
    }

    pub(crate) fn state_pressed_resize_drag(
        id: EntityId,
        dir: ResizeDirection,
        evt_resizing_state: &mut Option<ResizingState>,
        evt_interaction_states: &mut InteractionStates,
        evt_current_pointer_position: Option<LayoutPoint>,
        topo_parents: &ParentsSecondary,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        out_rects: &RectsSecondary,
    ) {
        let rect = out_rects.get(id).copied().unwrap_or_default();
        let (position, mut start_inset) = lay_basic
            .get(id)
            .map_or((Position::Relative, BasicLayout::default().inset), |l| {
                (l.position, l.inset)
            });

        let resolve_length = |length: Length, ref_size: f32| match length {
            Length::Px(v) => v,
            Length::Percent(p) => ref_size * (p / 100.0),
        };

        // 親要素の矩形と、その左・上ボーダーの厚みを取得
        let parent_id = topo_parents.get(id).copied().flatten();
        let (parent_rect, parent_border_left, parent_border_top) =
            parent_id.map_or((LayoutRect::ZERO, 0.0, 0.0), |p_id| {
                let p_rect = out_rects.get(p_id).copied().unwrap_or_default();
                // ボーダー幅の抽出
                let (border_l, border_t) = lay_basic.get(p_id).map_or((0.0, 0.0), |l| {
                    let left = resolve_length(l.border.left, p_rect.width);
                    let top = resolve_length(l.border.top, p_rect.height);
                    (left, top)
                });
                (p_rect, border_l, border_t)
            });

        // 親コンテナのボーダー内側を基準点として物理相対位置を逆算
        let local_x = rect.x - (parent_rect.x + parent_border_left);
        let local_y = rect.y - (parent_rect.y + parent_border_top);

        // 絶対配置の場合、開始時に Top-Left 基準に完全に正規化
        if position == Position::Absolute {
            start_inset = Rect {
                top: Val::Px(local_y),
                right: Val::Auto,
                bottom: Val::Auto,
                left: Val::Px(local_x),
            };

            // SoA 側も、この Top-Left 座標で即時上書きアップデート
            let basic = lay_basic.get_mut(id);
            let base_basic = lay_base_basic.get_mut(id);
            for layout in [basic, base_basic].into_iter().flatten() {
                layout.inset = start_inset;
            }
        }

        let start_pos = evt_current_pointer_position.unwrap_or_default();

        *evt_resizing_state = Some(ResizingState {
            entity_id: id,
            direction: dir,
            start_mouse_pos: start_pos,
            start_rect: rect,
            start_inset,
        });

        // リサイズ中の要素は pressed とマーク
        evt_interaction_states.pressed = Some(id);
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
    pub(crate) fn get_user_select(
        id: EntityId,
        rnd_visual: &VisualPropertiesSecondary,
    ) -> UserSelect {
        rnd_visual
            .get(id)
            .and_then(|v| v.user_select)
            .unwrap_or(UserSelect::None)
    }

    pub(crate) fn autoscroll_occurred(
        id: EntityId,
        win_last_size: Option<LayoutSize>,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        sys_text_engine: &TextEngine,
        evt_current_pointer_position: Option<LayoutPoint>,
        cont_input_contents: &InputContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        cont_text_contents: &TextContentsSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        out_scroll_offsets: &mut ScrollOffsetsSecondary,
        out_rects: &RectsSecondary,
        out_clip_rects: &ClipRectsSecondary,
        out_scroll_sizes: &ScrollSizesSecondary,
    ) -> (bool, Option<LayoutPoint>) {
        // ポインタ位置、またはクリップ領域がない場合
        let Some(pointer_pos) = evt_current_pointer_position else {
            return (false, None);
        };
        let Some(clip) = out_clip_rects.get(id).copied() else {
            return (false, None);
        };

        // テキスト選択状態
        let user_select = EventStore::get_user_select(id, rnd_visual);
        if user_select != UserSelect::Text {
            return (false, None);
        }

        // はみ出し距離
        let distance = EventStore::drag_overhang_distance(pointer_pos, &clip);
        if distance.x.abs() <= 1.0 && distance.y.abs() <= 1.0 {
            return (false, None);
        }

        // オートスクロール実行
        let speed_factor = 0.15f32;
        let dx = distance.x * speed_factor;
        let dy = distance.y * speed_factor;

        let scroll = OutputStore::scroll_by(
            id,
            dx,
            dy,
            win_last_size,
            topo_active_masks,
            topo_parents,
            lay_taffy,
            lay_dirty_entities,
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

        if scroll {
            (true, Some(pointer_pos))
        } else {
            (false, None)
        }
    }

    pub(crate) fn pressed_local_point(
        id: EntityId,
        logical_pos: LayoutPoint,
        cont_input_contents: &InputContentsSparseSecondary,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparseSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        out_scroll_offsets: &ScrollOffsetsSecondary,
        out_rects: &RectsSecondary,
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
            EventStore::update_state(cx, old_id, STATE_HOVERED, false);
            handle_on_mouse_leave(cx, old_id);
        }

        // 新ホバー要素にマウスが入った
        if let Some(new_id) = target_id {
            EventStore::update_state(cx, new_id, STATE_HOVERED, true);
            handle_on_mouse_enter(cx, new_id);
            handle_on_hover(cx, new_id);
        }
    }

    /// 各インタラクション状態（ステート）を更新し、レイアウト変更を伴うか自動的に判別して Dirty フラグを制御する共通ヘルパー
    pub(crate) fn update_state(cx: &mut Context, id: EntityId, state_flag: u128, active: bool) {
        let mut was_active = false;
        let mut state_changed = false;

        let Some(mask) = cx.topology.topo_active_masks.get_mut(id) else {
            return;
        };

        was_active = mask.has(state_flag);
        if was_active == active {
            return;
        }

        state_changed = true;

        if active {
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
                &mut cx.layouts.lay_taffy,
                &mut cx.layouts.lay_basic,
                &mut cx.layouts.lay_dirty_entities,
                &cx.layouts.lay_taffy_nodes,
                &cx.layouts.lay_base_basic,
                &mut cx.renders.rnd_visual,
                &mut cx.renders.rnd_dirty_entities,
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
                &cx.renders.rnd_interaction,
                &cx.renders.rnd_visual,
                &cx.renders.rnd_active_transitions,
            );
        };

        let mark_dirty = |cx: &mut Context, id: EntityId| {
            if RenderStore::does_state_require_layout(id, state_flag, &cx.renders.rnd_interaction) {
                LayoutStore::mark_layout_dirty(
                    id,
                    &mut cx.topology.topo_active_masks,
                    &cx.topology.topo_parents,
                    &mut cx.layouts.lay_taffy,
                    &mut cx.layouts.lay_dirty_entities,
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
                    .is_some_and(|m| m.has(STYLE_INTERACTION_PARENT));

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
                    .is_some_and(|m| m.has(STYLE_INTERACTION_WITHIN));

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
        if active {
            match state_flag {
                STATE_DISABLED => handle_on_disable(cx, id),
                STATE_ACTIVED => handle_on_active(cx, id),
                STATE_SELECTED => handle_on_select(cx, id),

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

    fn spawn_dnd_placeholder(
        root: EntityId,
        pressed_id: EntityId,
        drag_prop: &DndDragProperty,
        topo_entities: &mut EntitiesSlot,
        topo_parents: &mut ParentsSecondary,
        topo_children: &mut ChildrenSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_active_entities: &mut ActiveEntitiesVec,
        topo_session_spawned: &mut SessionSpawnedVec,
        topo_is_structure_dirty: &mut bool,
        topo_is_sort_dirty: &mut bool,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &mut TaffyNodesSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_basic: &BasicLayoutsSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        out_rects: &RectsSecondary,
    ) -> EntityId {
        let placeholder =
            EventStore::resolve_dnd_placeholder_parent(root, drag_prop, lay_basic, out_rects);

        let placeholder_id = TopologyStore::spawn(
            placeholder.parent_id,
            topo_entities,
            topo_parents,
            topo_children,
            topo_active_masks,
            topo_active_entities,
            topo_session_spawned,
            topo_is_structure_dirty,
            topo_is_sort_dirty,
            lay_taffy,
            lay_taffy_nodes,
            rnd_dirty_entities,
        );

        if let Some(p_id) = placeholder.parent_id {
            TopologyStore::add_child(
                p_id,
                placeholder_id,
                topo_parents,
                topo_children,
                topo_is_structure_dirty,
                topo_is_sort_dirty,
                topo_active_masks,
                lay_taffy_nodes,
                lay_taffy,
                lay_dirty_entities,
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
        if let Some(basic) = cx.layouts.lay_base_basic.get(pressed_id).copied() {
            cx.layouts.lay_base_basic.insert(placeholder_id, basic);
            cx.layouts.lay_basic.insert(placeholder_id, basic);
        }
        if let Some(visual) = cx.renders.rnd_base_visual.get(pressed_id).cloned() {
            cx.renders
                .rnd_base_visual
                .insert(placeholder_id, visual.clone());
            cx.renders.rnd_visual.insert(placeholder_id, visual);
        }
        if let Some(interaction) = cx.renders.rnd_interaction.get(pressed_id).cloned() {
            cx.renders
                .rnd_interaction
                .insert(placeholder_id, interaction);
        }

        // ドラッグ元とプレースホルダーの状態を同期
        EventStore::update_state(cx, pressed_id, STATE_DND_DRAGGING, true);
        EventStore::update_state(cx, placeholder_id, STATE_DND_DRAG_OVER, true);

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
        if let Some(mask) = cx.topology.topo_active_masks.get_mut(placeholder_id) {
            mask.set(STYLE_POINTER_EVENTS);
        }
    }

    fn transfer_children_to_placeholder(
        pressed_id: EntityId,
        placeholder_id: EntityId,
        topo_parents: &mut ParentsSecondary,
        topo_children: &mut ChildrenSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
    ) {
        let Some(src_children) = topo_children.get(pressed_id).cloned() else {
            return;
        };

        for child_id in src_children {
            // 子要素の親ポインタをプレースホルダーに付け替え
            topo_parents.insert(child_id, Some(placeholder_id));

            // プレースホルダー側の子要素リストへ追加
            if let Some(ph_children) = topo_children.get_mut(placeholder_id) {
                ph_children.push(child_id);
            }

            // Taffy 側の親子構造も、一時的にプレースホルダーに繋ぎ替え
            if let Some(&src_node) = lay_taffy_nodes.get(pressed_id)
                && let Some(&ph_node) = lay_taffy_nodes.get(placeholder_id)
                && let Some(&child_node) = lay_taffy_nodes.get(child_id)
            {
                let _ = lay_taffy.remove_child(src_node, child_node);
                let _ = lay_taffy.add_child(ph_node, child_node);
            }
        }

        // 元の要素の子要素リストは一時的にクリア（プレースホルダーに避難しているため）
        if let Some(src_children_mut) = topo_children.get_mut(pressed_id) {
            src_children_mut.clear();
        }

        // 元要素とプレースホルダー要素の両方をダーティマーク
        for id in [pressed_id, placeholder_id] {
            LayoutStore::mark_layout_dirty(
                id,
                topo_active_masks,
                topo_parents,
                lay_taffy,
                lay_dirty_entities,
                lay_taffy_nodes,
            );
        }
    }

    fn start_dnd_drag_session(cx: &mut Context, pressed_id: EntityId, logical_pos: LayoutPoint) {
        let drag_prop = cx
            .events
            .evt_dnd_drag_properties
            .get(pressed_id)
            .copied()
            .unwrap();
        let start_rect = cx
            .outputs
            .out_rects
            .get(pressed_id)
            .copied()
            .unwrap_or_default();

        // 開始時のクリック位置と要素左上の相対的なズレを計算
        let click_offset =
            LayoutPoint::new(logical_pos.x - start_rect.x, logical_pos.y - start_rect.y);

        // ウィンドウのルート要素を自己解決
        let root = TopologyStore::find_root_entity(
            &cx.topology.topo_entities,
            &cx.topology.topo_parents,
            &cx.topology.topo_flat_dfs_sequence,
        )
        .expect("Root EntityId not found in Context");

        // プレースホルダーをアタッチ先親の直下へ spawn して生成
        let placeholder_id = EventStore::spawn_dnd_placeholder(
            root,
            pressed_id,
            &drag_prop,
            &mut cx.topology.topo_entities,
            &mut cx.topology.topo_parents,
            &mut cx.topology.topo_children,
            &mut cx.topology.topo_active_masks,
            &mut cx.topology.topo_active_entities,
            &mut cx.topology.topo_session_spawned,
            &mut cx.topology.topo_is_structure_dirty,
            &mut cx.topology.topo_is_sort_dirty,
            &mut cx.layouts.lay_taffy,
            &mut cx.layouts.lay_taffy_nodes,
            &mut cx.layouts.lay_dirty_entities,
            &cx.layouts.lay_basic,
            &mut cx.renders.rnd_dirty_entities,
            &cx.outputs.out_rects,
        );

        // プレースホルダーの初期スタイル・透過・状態情報をセットアップ
        EventStore::setup_placeholder_properties(cx, pressed_id, placeholder_id, start_rect);

        // 元の要素から子要素トポロジーをプレースホルダーへ移行
        EventStore::transfer_children_to_placeholder(
            pressed_id,
            placeholder_id,
            &mut cx.topology.topo_parents,
            &mut cx.topology.topo_children,
            &mut cx.topology.topo_active_masks,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_taffy,
            &cx.layouts.lay_taffy_nodes,
        );

        // プレースホルダーアタッチ前の、本当の元の親要素のIDを記録
        let original_parent = cx.topology.topo_parents.get(pressed_id).copied().flatten();

        // セッション開始
        cx.events.evt_active_dnd_drag_state = Some(ActiveDndDragState {
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

        EventStore::update_state(cx, pressed_id, STATE_DRAGGED, true);
        cx.events.evt_interaction_states.dragged = Some(pressed_id);

        // D&D 設定（STYLE_DRAGGABLE）を持っている場合のセッションのキック
        if cx
            .topology
            .topo_active_masks
            .get(pressed_id)
            .is_some_and(|m| m.has(STYLE_DND_DRAGGABLE))
            && cx.events.evt_active_dnd_drag_state.is_none()
        {
            EventStore::start_dnd_drag_session(cx, pressed_id, logical_pos);
        }

        handle_on_drag(cx, pressed_id, delta);
    }

    pub(crate) fn calculate_dnd_relative_local(
        root: EntityId,
        drag_prop: &DndDragProperty,
        lay_basic: &BasicLayoutsSecondary,
        out_rects: &RectsSecondary,
    ) -> (LayoutRect, f32, f32) {
        let p = EventStore::resolve_dnd_placeholder_parent(root, drag_prop, lay_basic, out_rects);
        (p.rect, p.border_left, p.border_top)
    }

    pub(crate) fn update_inset_based_relative_local(
        root: EntityId,
        placeholder: EntityId,
        logical_pos: LayoutPoint,
        drag_prop: &DndDragProperty,
        drag_state: &ActiveDndDragState,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_nodes: &TaffyNodesSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        out_rects: &RectsSecondary,
    ) {
        // アタッチ先親コンテナ基準での相対ローカル座標を逆算して追従（Inset更新）
        let (parent_rect, b_l, b_t) =
            EventStore::calculate_dnd_relative_local(root, drag_prop, lay_basic, out_rects);

        // マウスのドラッグ開始時クリックオフセットを用いて、ローカル Top-Left 座標を算出
        let local_x = logical_pos.x - (parent_rect.x + b_l) - drag_state.click_offset.x;
        let local_y = logical_pos.y - (parent_rect.y + b_t) - drag_state.click_offset.y;

        if let Some(layout) = lay_basic.get_mut(placeholder) {
            layout.inset.left = Val::Px(local_x);
            layout.inset.top = Val::Px(local_y);
            layout.inset.right = Val::Auto;
            layout.inset.bottom = Val::Auto;
        }
        if let Some(layout) = lay_base_basic.get_mut(placeholder) {
            layout.inset.left = Val::Px(local_x);
            layout.inset.top = Val::Px(local_y);
            layout.inset.right = Val::Auto;
            layout.inset.bottom = Val::Auto;
        }

        LayoutStore::mark_layout_dirty(
            placeholder,
            topo_active_masks,
            topo_parents,
            lay_taffy,
            lay_dirty_entities,
            lay_taffy_nodes,
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
            topo_parents.get(src_id).copied().flatten()
        } else {
            Some(hit_id)
        };

        while let Some(id) = current_id {
            let is_dnd = topo_active_masks
                .get(id)
                .is_some_and(|f| f.has(STYLE_DND_DROPPABLE));

            if id != placeholder && is_dnd {
                return Some(id); // ドロップ先を見つけたら即座に返す
            }
            current_id = topo_parents.get(id).copied().flatten();
        }

        None
    }

    pub(crate) fn sync_state_drag_in(cx: &mut Context, found_drop_target: Option<EntityId>) {
        let Some(mut drag_state) = cx.events.evt_active_dnd_drag_state.take() else {
            return;
        };

        if found_drop_target == drag_state.current_drop_target {
            cx.events.evt_active_dnd_drag_state = Some(drag_state);
            return;
        }

        if let Some(old_target) = drag_state.current_drop_target {
            EventStore::update_state(cx, old_target, STATE_DND_DRAG_IN, false);
        }
        if let Some(new_target) = found_drop_target {
            EventStore::update_state(cx, new_target, STATE_DND_DRAG_IN, true);
        }

        drag_state.current_drop_target = found_drop_target;
        cx.events.evt_active_dnd_drag_state = Some(drag_state);
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
        win_last_size: Option<LayoutSize>,
        win_scale_factor: f32,
        sys_text_engine: &TextEngine,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_contents: &mut TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparseSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        out_scroll_offsets: &mut ScrollOffsetsSecondary,
        out_selected_rects: &mut SelectedRectsSparseSecondary,
        out_text_selections: &mut TextSelectionsSparseSecondary,
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
                win_last_size,
                win_scale_factor,
                sys_text_engine,
                sys_dwrite_layouts,
                cont_input_contents,
                cont_text_contents,
                cont_text_spans,
                topo_active_masks,
                topo_parents,
                lay_taffy,
                lay_dirty_entities,
                lay_scrollbar_styles,
                lay_taffy_nodes,
                lay_resolved_basic,
                lay_resolved_flex,
                lay_resolved_grid,
                rnd_visual,
                rnd_dirty_entities,
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

    #[inline]
    pub(crate) fn remove_dragged_elemet(
        src_id: EntityId,
        drag_state: &ActiveDndDragState,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_children: &mut ChildrenSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_nodes: &TaffyNodesSecondary,
    ) {
        let Some(src_parent_id) = drag_state.original_parent else {
            return;
        };

        if let Some(src_children) = topo_children.get_mut(src_parent_id) {
            src_children.retain(|x| *x != src_id);
        }
        // 旧親側の Taffy 順序も再同期
        LayoutStore::resync_taffy_children_order(
            src_parent_id,
            topo_children,
            lay_taffy,
            lay_taffy_nodes,
        );
        LayoutStore::mark_layout_dirty(
            src_parent_id,
            topo_active_masks,
            topo_parents,
            lay_taffy,
            lay_dirty_entities,
            lay_taffy_nodes,
        );
    }

    pub(crate) fn inject_pointer_move_internal(cx: &mut Context, logical_pos: LayoutPoint) {
        let _context_guard = bind_context(cx);
        let prev_pos = cx.events.evt_current_pointer_position;
        cx.events.evt_current_pointer_position = Some(logical_pos);

        // リサイズ中のドラッグ同期処理
        if let Some(ref state) = cx.events.evt_resizing_state {
            LayoutStore::sync_resizing_drag(
                logical_pos,
                state,
                cx.window.win_last_size.as_ref(),
                &mut cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_taffy,
                &mut cx.layouts.lay_basic,
                &mut cx.layouts.lay_base_basic,
                &mut cx.layouts.lay_dirty_entities,
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
            &mut cx.layouts.lay_taffy,
            &mut cx.layouts.lay_dirty_entities,
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
        let hit_id = TopologyStore::hit_test(
            logical_pos,
            cx.window.win_last_size,
            &cx.events.evt_interaction_states,
            &mut cx.topology.topo_sorted_entities,
            &mut cx.topology.topo_effective_z_indices,
            &mut cx.topology.topo_dfs_indices,
            &mut cx.topology.topo_sort_cache,
            &mut cx.topology.topo_is_sort_dirty,
            &mut cx.topology.topo_active_masks,
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
        let prev_resize_hover = cx.events.evt_active_resize_hover;
        // リサイズホバー情報を一旦リセット
        cx.events.evt_active_resize_hover = None;

        // ヒットした要素、およびその親先祖に向かってツリーを遡上
        let (current_id, found_resize_hover) = EventStore::found_resize_hover(
            target_id,
            logical_pos,
            &cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &cx.layouts.lay_basic,
            &cx.outputs.out_rects,
        );

        if let Some((id, dir)) = found_resize_hover {
            cx.events.evt_active_resize_hover = Some((id, dir));
            EventStore::apply_resizable_cursor_style(id, dir, &mut cx.renders.rnd_visual);
            RenderStore::mark_render_dirty(
                id,
                &mut cx.topology.topo_active_masks,
                &mut cx.renders.rnd_dirty_entities,
            );
        }

        // 枠線から外れた、または異なる要素に変わった場合
        if let Some((prev_id, _)) = prev_resize_hover {
            let now_id = cx.events.evt_active_resize_hover.map(|(id, _)| id);

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
                    &mut cx.layouts.lay_taffy,
                    &mut cx.layouts.lay_basic,
                    &mut cx.layouts.lay_dirty_entities,
                    &cx.layouts.lay_taffy_nodes,
                    &cx.layouts.lay_base_basic,
                    &mut cx.renders.rnd_visual,
                    &mut cx.renders.rnd_dirty_entities,
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
            let user_select = EventStore::get_user_select(pressed_id, &cx.renders.rnd_visual);

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

                let local = EventStore::pressed_local_point(
                    pressed_id,
                    logical_pos,
                    &cx.contents.cont_input_contents,
                    &cx.topology.topo_active_masks,
                    &cx.topology.topo_parents,
                    &cx.layouts.lay_resolved_basic,
                    &cx.layouts.lay_resolved_flex,
                    &cx.layouts.lay_resolved_grid,
                    &cx.renders.rnd_active_transitions,
                    &cx.renders.rnd_interaction,
                    &cx.renders.rnd_visual,
                    &cx.outputs.out_scroll_offsets,
                    &cx.outputs.out_rects,
                );
                EventStore::handle_text_selection_click(
                    pressed_id,
                    start_pos,
                    local,
                    cx.window.win_last_size,
                    cx.window.win_scale_factor,
                    &cx.system.sys_text_engine,
                    &cx.system.sys_dwrite_layouts,
                    &mut cx.contents.cont_input_contents,
                    &mut cx.contents.cont_text_contents,
                    &cx.contents.cont_text_spans,
                    &mut cx.topology.topo_active_masks,
                    &cx.topology.topo_parents,
                    &cx.topology.topo_children,
                    &mut cx.layouts.lay_taffy,
                    &mut cx.layouts.lay_scrollbar_styles,
                    &mut cx.layouts.lay_dirty_entities,
                    &cx.layouts.lay_resolved_basic,
                    &cx.layouts.lay_resolved_flex,
                    &cx.layouts.lay_resolved_grid,
                    &cx.layouts.lay_taffy_nodes,
                    &mut cx.renders.rnd_visual,
                    &mut cx.renders.rnd_dirty_entities,
                    &cx.renders.rnd_base_visual,
                    &cx.renders.rnd_interaction,
                    &cx.renders.rnd_active_transitions,
                    &mut cx.outputs.out_scroll_offsets,
                    &mut cx.outputs.out_selected_rects,
                    &mut cx.outputs.out_text_selections,
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
        EventStore::propagate_dnd_drag_events(cx, prev_pos, logical_pos);

        // D&D プレースホルダーの移動とドロップ先ホバー検知
        let Some(ref drag_state) = cx.events.evt_active_dnd_drag_state else {
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
            .evt_dnd_drag_properties
            .get(src_id)
            .copied()
            .unwrap();

        // アタッチ先親コンテナ基準での相対ローカル座標を逆算して追従
        EventStore::update_inset_based_relative_local(
            root,
            placeholder_id,
            logical_pos,
            &drag_prop,
            drag_state,
            &mut cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &mut cx.layouts.lay_basic,
            &mut cx.layouts.lay_base_basic,
            &mut cx.layouts.lay_taffy,
            &mut cx.layouts.lay_dirty_entities,
            &cx.layouts.lay_taffy_nodes,
            &mut cx.renders.rnd_dirty_entities,
            &cx.outputs.out_rects,
        );

        // 現在ホバー侵入中のドロップターゲット要素を検知
        let found_drop_target = EventStore::detect_drop_target_during_intrusion(
            src_id,
            hit_id,
            placeholder_id,
            &cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
        );

        // ドロップ先のホバー切り替えイベントを解決（STATE_DRAG_IN の同期）
        EventStore::sync_state_drag_in(cx, found_drop_target);

        EventStore::callback_drag_prop(cx, src_id, found_drop_target, &drag_prop);
    }

    /// 指定要素またはその親階層において、フォーカスの略奪を防止すべきか判定
    #[inline]
    fn should_prevent_focus_steal(cx: &Context, target_id: EntityId) -> bool {
        let mut curr = Some(target_id);
        while let Some(curr_id) = curr {
            let Some(mask) = cx.topology.topo_active_masks.get(curr_id) else {
                break;
            };

            if mask.has(STYLE_PREVENT_FOCUS_STEAL)
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

            if mask.has(STYLE_PREVENT_FOCUS_STEAL_WITHIN)
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

    fn handle_pointer_pressed(cx: &mut Context, button: MouseButton, modifiers: Modifiers) {
        let current_hovered = cx.events.evt_interaction_states.hovered;

        // リサイズドラッグの開始判定（左クリック時のみ）
        if button == MouseButton::Left
            && let Some((id, dir)) = cx.events.evt_active_resize_hover
        {
            EventStore::state_pressed_resize_drag(
                id,
                dir,
                &mut cx.events.evt_resizing_state,
                &mut cx.events.evt_interaction_states,
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
                &cx.contents.cont_input_contents,
                &cx.contents.cont_text_contents,
                &cx.contents.cont_text_spans,
                &mut cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &cx.topology.topo_children,
                &mut cx.layouts.lay_taffy,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_scrollbar_styles,
                &cx.layouts.lay_resolved_basic,
                &cx.layouts.lay_taffy_nodes,
                &mut cx.renders.rnd_dirty_entities,
                &cx.renders.rnd_visual,
                &cx.renders.rnd_active_transitions,
                &cx.renders.rnd_interaction,
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
        EventStore::update_state(cx, target_id, STATE_PRESSED, true);

        // テキスト選択処理
        let user_select = EventStore::get_user_select(target_id, &cx.renders.rnd_visual);
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
                &cx.contents.cont_input_contents,
                &cx.contents.cont_text_contents,
                &cx.contents.cont_text_spans,
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
                &mut cx.outputs.out_selected_rects,
                &mut cx.outputs.out_selection_start_index,
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

    /// ドラッグ＆ドロップの終了・ドロップ確定処理をカプセル化
    fn handle_dnd_drop(cx: &mut Context, drag_state: &ActiveDndDragState) {
        let src_id = drag_state.source_entity;
        let holder = drag_state.placeholder_entity;

        let Some(drag_prop) = cx.events.evt_dnd_drag_properties.get(src_id).copied() else {
            return;
        };

        // 疑似クラスの解除
        EventStore::update_state(cx, src_id, STATE_DND_DRAGGING, false);
        if let Some(target_id) = drag_state.current_drop_target {
            EventStore::update_state(cx, target_id, STATE_DND_DRAG_IN, false);
        }

        cx.events.evt_interaction_states.pressed = None;
        cx.events.evt_interaction_states.dragged = None;

        let drop_success = drag_state.current_drop_target;

        // トポロジー書き換え（要素移動時のみ）
        if let Some(target_id) = drop_success
            && drag_prop.drag_mode == DndDragPayload::Element
            && let Some(_prop) = cx.events.evt_dnd_drop_properties.get(target_id).copied()
        {
            EventStore::remove_dragged_elemet(
                src_id,
                drag_state,
                &mut cx.topology.topo_active_masks,
                &mut cx.topology.topo_children,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_taffy,
                &mut cx.layouts.lay_dirty_entities,
                &cx.layouts.lay_taffy_nodes,
            );
            EventStore::rewrite_tree_topology(
                src_id,
                target_id,
                holder,
                &drag_prop,
                drag_state,
                cx.events.evt_current_pointer_position,
                &mut cx.topology.topo_active_masks,
                &mut cx.topology.topo_parents,
                &mut cx.topology.topo_children,
                &mut cx.topology.topo_is_structure_dirty,
                &mut cx.topology.topo_is_sort_dirty,
                &mut cx.layouts.lay_basic,
                &mut cx.layouts.lay_base_basic,
                &mut cx.layouts.lay_taffy_nodes,
                &mut cx.layouts.lay_taffy,
                &mut cx.layouts.lay_dirty_entities,
                &cx.layouts.lay_flex,
                &cx.outputs.out_rects,
            );
            cx.topology.topo_is_structure_dirty = true;
            cx.topology.topo_is_sort_dirty = true;
        }

        // 子要素のツリー構造復元
        if let Some(ph_children) = cx.topology.topo_children.get(holder).cloned() {
            TopologyStore::restore_child(
                src_id,
                holder,
                ph_children,
                &mut cx.topology.topo_parents,
                &mut cx.topology.topo_children,
                &mut cx.layouts.lay_taffy,
                &mut cx.layouts.lay_taffy_nodes,
            );
            if let Some(ph_children_mut) = cx.topology.topo_children.get_mut(holder) {
                ph_children_mut.clear();
            }

            for id in [src_id, holder] {
                LayoutStore::mark_layout_dirty(
                    id,
                    &mut cx.topology.topo_active_masks,
                    &cx.topology.topo_parents,
                    &mut cx.layouts.lay_taffy,
                    &mut cx.layouts.lay_dirty_entities,
                    &cx.layouts.lay_taffy_nodes,
                );
            }
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
            &mut cx.layouts,
            &mut cx.renders,
            &mut cx.outputs,
        );

        if let Some(pos) = cx.events.evt_current_pointer_position {
            EventStore::inject_pointer_move_internal(cx, pos);
        }

        RenderStore::mark_render_dirty(
            src_id,
            &mut cx.topology.topo_active_masks,
            &mut cx.renders.rnd_dirty_entities,
        );
    }

    #[inline]
    fn handle_pointer_released(cx: &mut Context, button: MouseButton, modifiers: Modifiers) {
        // リサイズドラッグの終了処理
        if let Some(state) = cx.events.evt_resizing_state.take() {
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
        if let Some(drag_state) = cx.events.evt_active_dnd_drag_state.take() {
            EventStore::handle_dnd_drop(cx, &drag_state);
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
            EventStore::update_state(cx, pressed_id, STATE_PRESSED, false);
            EventStore::update_state(cx, pressed_id, STATE_DRAGGED, false);
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
        let _context_guard = bind_context(cx);

        let current_hovered = cx.events.evt_interaction_states.hovered;

        match state {
            ElementState::Pressed => EventStore::handle_pointer_pressed(cx, button, modifiers),
            ElementState::Released => EventStore::handle_pointer_released(cx, button, modifiers),
        }
    }

    pub fn inject_pointer_double_click_internal(cx: &mut Context, modifiers: Modifiers) {
        let _context_guard = bind_context(cx);
        let current_hovered = cx.events.evt_interaction_states.hovered;

        let Some(target_id) = current_hovered else {
            return;
        };

        let user_select = EventStore::get_user_select(target_id, &cx.renders.rnd_visual);
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
                cx.window.win_last_size,
                cx.window.win_scale_factor,
                &cx.system.sys_text_engine,
                &cx.system.sys_dwrite_layouts,
                &mut cx.contents.cont_input_contents,
                &mut cx.contents.cont_text_contents,
                &cx.contents.cont_text_spans,
                &mut cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_taffy,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_scrollbar_styles,
                &cx.layouts.lay_taffy_nodes,
                &cx.layouts.lay_resolved_basic,
                &cx.layouts.lay_resolved_flex,
                &cx.layouts.lay_resolved_grid,
                &mut cx.renders.rnd_visual,
                &mut cx.renders.rnd_dirty_entities,
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
        let _context_guard = bind_context(cx);

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
                .is_some_and(|m| m.has(STYLE_OVERFLOW));
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
                        &mut cx.layouts.lay_taffy,
                        &mut cx.layouts.lay_dirty_entities,
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
        win_last_size: Option<LayoutSize>,
        win_scale_factor: f32,
        sys_text_engine: &TextEngine,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_contents: &mut TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparseSecondary,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
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
                win_last_size,
                win_scale_factor,
                sys_text_engine,
                sys_dwrite_layouts,
                cont_input_contents,
                cont_text_contents,
                cont_text_spans,
                topo_active_masks,
                topo_parents,
                lay_taffy,
                lay_dirty_entities,
                lay_scrollbar_styles,
                lay_taffy_nodes,
                lay_resolved_basic,
                lay_resolved_flex,
                lay_resolved_grid,
                rnd_visual,
                rnd_dirty_entities,
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
        let _context_guard = bind_context(cx);

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
            let user_select = EventStore::get_user_select(focused_id, &cx.renders.rnd_visual);
            if user_select == UserSelect::Text {
                EventStore::handle_select_all(
                    focused_id,
                    cx.window.win_last_size,
                    cx.window.win_scale_factor,
                    &cx.system.sys_text_engine,
                    &cx.system.sys_dwrite_layouts,
                    &mut cx.contents.cont_input_contents,
                    &mut cx.contents.cont_text_contents,
                    &cx.contents.cont_text_spans,
                    &mut cx.topology.topo_active_masks,
                    &cx.topology.topo_parents,
                    &cx.topology.topo_children,
                    &mut cx.layouts.lay_taffy,
                    &mut cx.layouts.lay_dirty_entities,
                    &mut cx.layouts.lay_scrollbar_styles,
                    &cx.layouts.lay_taffy_nodes,
                    &cx.layouts.lay_resolved_basic,
                    &cx.layouts.lay_resolved_flex,
                    &cx.layouts.lay_resolved_grid,
                    &mut cx.renders.rnd_visual,
                    &mut cx.renders.rnd_dirty_entities,
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
                &cx.topology.topo_active_masks,
                &cx.topology.topo_entities,
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
        let _context_guard = bind_context(cx);

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
            cx.window.win_last_size,
            cx.window.win_scale_factor,
            &cx.system.sys_text_engine,
            &cx.system.sys_dwrite_layouts,
            &mut cx.contents.cont_input_contents,
            &mut cx.contents.cont_text_contents,
            &cx.contents.cont_text_spans,
            &mut cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &mut cx.layouts.lay_taffy,
            &mut cx.layouts.lay_dirty_entities,
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
            cx.window.win_last_size,
            cx.window.win_scale_factor,
            &cx.system.sys_text_engine,
            &cx.system.sys_dwrite_layouts,
            &mut cx.contents.cont_input_contents,
            &mut cx.contents.cont_text_contents,
            &cx.contents.cont_text_spans,
            &mut cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &mut cx.layouts.lay_taffy,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_scrollbar_styles,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
            &cx.layouts.lay_resolved_grid,
            &mut cx.renders.rnd_visual,
            &mut cx.renders.rnd_dirty_entities,
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
        let _context_guard = bind_context(cx);

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
            cx.window.win_last_size,
            cx.window.win_scale_factor,
            &cx.system.sys_text_engine,
            &cx.system.sys_dwrite_layouts,
            &mut cx.contents.cont_input_contents,
            &mut cx.contents.cont_text_contents,
            &cx.contents.cont_text_spans,
            &mut cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &mut cx.layouts.lay_taffy,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_scrollbar_styles,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
            &cx.layouts.lay_resolved_grid,
            &mut cx.renders.rnd_visual,
            &mut cx.renders.rnd_dirty_entities,
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
        let _context_guard = bind_context(cx);
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
            cx.window.win_last_size,
            cx.window.win_scale_factor,
            &cx.system.sys_text_engine,
            &cx.system.sys_dwrite_layouts,
            &mut cx.contents.cont_input_contents,
            &mut cx.contents.cont_text_contents,
            &cx.contents.cont_text_spans,
            &mut cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &mut cx.layouts.lay_taffy,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_scrollbar_styles,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
            &cx.layouts.lay_resolved_grid,
            &mut cx.renders.rnd_visual,
            &mut cx.renders.rnd_dirty_entities,
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

    pub(crate) fn inject_cut_internal(cx: &mut Context) -> Option<String> {
        let _context_guard = bind_context(cx);
        let focused_id = cx.events.evt_interaction_states.focused?;
        let user_select = EventStore::get_user_select(focused_id, &cx.renders.rnd_visual);

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
                cx.window.win_last_size,
                cx.window.win_scale_factor,
                &cx.system.sys_text_engine,
                &cx.system.sys_dwrite_layouts,
                &mut cx.contents.cont_input_contents,
                &mut cx.contents.cont_text_contents,
                &cx.contents.cont_text_spans,
                &mut cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_taffy,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_scrollbar_styles,
                &cx.layouts.lay_taffy_nodes,
                &cx.layouts.lay_resolved_basic,
                &cx.layouts.lay_resolved_flex,
                &cx.layouts.lay_resolved_grid,
                &mut cx.renders.rnd_visual,
                &mut cx.renders.rnd_dirty_entities,
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
        Some(cut_text)
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
        cont_input_contents: &InputContentsSparseSecondary,
        cont_text_contents: &TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
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
                    WindowStore::calculate_visible_size(win_last_size, container_rect);

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
                    lay_taffy,
                    lay_dirty_entities,
                    lay_scrollbar_styles,
                    lay_taffy_nodes,
                    lay_resolved_basic,
                    rnd_visual,
                    rnd_interaction,
                    rnd_active_transitions,
                    out_rects,
                    out_scroll_offsets,
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
        cont_input_contents: &InputContentsSparseSecondary,
        cont_text_contents: &TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
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
        out_selected_rects: &mut SelectedRectsSparseSecondary,
        out_selection_start_index: &mut SelectionStartIndexSparseSecondary,
        out_rects: &RectsSecondary,
        out_scroll_offsets: &ScrollOffsetsSecondary,
    ) {
        let Some(layout) = SystemStore::get_or_create_layout(
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
            cont_input_contents,
            topo_active_masks,
            topo_parents,
            lay_resolved_basic,
            lay_resolved_flex,
            lay_resolved_grid,
            rnd_active_transitions,
            rnd_interaction,
            rnd_visual,
            out_scroll_offsets,
            out_rects,
        );
        let (clicked_index, is_trailing) =
            sys_text_engine.hit_test_point(&layout, local.x, local.y);
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
                &layout,
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
    pub fn set_focused_by_trigger(
        cx: &mut Context,
        id: EntityId,
        focused: bool,
        trigger: ActiveFocusTrigger,
    ) {
        EventStore::update_state(cx, id, STATE_FOCUSED, focused);
        let show_visible = focused && (trigger == ActiveFocusTrigger::Keyboard);
        EventStore::update_state(cx, id, STATE_FOCUSED_VISIBLE, show_visible);
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

    pub(crate) fn rewrite_tree_topology(
        src_id: EntityId,
        target_id: EntityId,
        holder: EntityId,
        drag_prop: &DndDragProperty,
        drag_state: &ActiveDndDragState,
        evt_current_pointer_position: Option<LayoutPoint>,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &mut ParentsSecondary,
        topo_children: &mut ChildrenSecondary,
        topo_is_structure_dirty: &mut bool,
        topo_is_sort_dirty: &mut bool,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        lay_taffy_nodes: &mut TaffyNodesSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_flex: &FlexLayoutsSecondary,
        out_rects: &RectsSecondary,
    ) {
        // ドラッグ元要素の配置（Position）の取得
        let position = lay_basic
            .get(src_id)
            .map(|l| l.position)
            .unwrap_or_default();

        if position == Position::Absolute {
            // 絶対配置: 位置移動（補正）を伴うアタッチ
            if drag_prop.update_position {
                // プレースホルダーの最終的な絶対画面座標を取得
                let ph_abs_rect = out_rects.get(holder).copied().unwrap_or_default();
                // 新しい親（target_id）の絶対画面座標とボーダー厚みを取得
                let target_rect = out_rects.get(target_id).copied().unwrap_or_default();
                let (border_l, border_t) = if let Some(basic) = lay_basic.get(target_id) {
                    let border = LayoutStore::get_physical_border(target_rect, basic.border);
                    (border.left, border.top)
                } else {
                    (0.0, 0.0)
                };

                // 新しい親を基準にした新しいローカル相対位置を逆算して割り出す
                let new_inset_left = ph_abs_rect.x - (target_rect.x + border_l);
                let new_inset_top = ph_abs_rect.y - (target_rect.y + border_t);

                let new_inset = Rect {
                    top: Val::Px(new_inset_top),
                    right: Val::Auto,
                    bottom: Val::Auto,
                    left: Val::Px(new_inset_left),
                };

                if let Some(basic) = lay_basic.get_mut(src_id) {
                    basic.inset = new_inset;
                }
                if let Some(base_basic) = lay_base_basic.get_mut(src_id) {
                    base_basic.inset = new_inset;
                }
            }

            // ドロップ先コンテナ（target_id）の末尾の子要素としてマウント
            TopologyStore::add_child(
                target_id,
                src_id,
                topo_parents,
                topo_children,
                topo_is_structure_dirty,
                topo_is_sort_dirty,
                topo_active_masks,
                lay_taffy_nodes,
                lay_taffy,
                lay_dirty_entities,
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

            if let Some(parent_children) = topo_children.get_mut(target_id) {
                // 算出されたインデックス位置へ挿入
                parent_children.insert(insert_idx, src_id);
            }
            topo_parents.insert(src_id, Some(target_id));

            // Taffy 側のノード順序を物理並び替え結果に沿って一括して再同期
            LayoutStore::resync_taffy_children_order(
                target_id,
                topo_children,
                lay_taffy,
                lay_taffy_nodes,
            );
        } else {
            // 自動更新オフの場合は末尾に通常アタッチ
            TopologyStore::add_child(
                target_id,
                src_id,
                topo_parents,
                topo_children,
                topo_is_structure_dirty,
                topo_is_sort_dirty,
                topo_active_masks,
                lay_taffy_nodes,
                lay_taffy,
                lay_dirty_entities,
            );
        }
        LayoutStore::mark_layout_dirty(
            target_id,
            topo_active_masks,
            topo_parents,
            lay_taffy,
            lay_dirty_entities,
            lay_taffy_nodes,
        );
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
        win_last_size: Option<LayoutSize>,
        win_scale_factor: f32,
        sys_text_engine: &TextEngine,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_contents: &mut TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparseSecondary,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
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
                    win_last_size,
                    win_scale_factor,
                    sys_text_engine,
                    sys_dwrite_layouts,
                    cont_input_contents,
                    cont_text_contents,
                    cont_text_spans,
                    topo_active_masks,
                    topo_parents,
                    lay_taffy,
                    lay_dirty_entities,
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
                    win_last_size,
                    win_scale_factor,
                    sys_text_engine,
                    sys_dwrite_layouts,
                    cont_input_contents,
                    cont_text_contents,
                    cont_text_spans,
                    topo_active_masks,
                    topo_parents,
                    lay_taffy,
                    lay_dirty_entities,
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
                    lay_taffy,
                    lay_dirty_entities,
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
                    lay_taffy,
                    lay_dirty_entities,
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
            self.window.win_last_size,
            self.window.win_scale_factor,
            &self.system.sys_text_engine,
            &self.system.sys_dwrite_layouts,
            &mut self.contents.cont_input_contents,
            &mut self.contents.cont_text_contents,
            &self.contents.cont_text_spans,
            &mut self.topology.topo_active_masks,
            &self.topology.topo_parents,
            &mut self.layouts.lay_taffy,
            &mut self.layouts.lay_dirty_entities,
            &mut self.layouts.lay_scrollbar_styles,
            &self.layouts.lay_taffy_nodes,
            &self.layouts.lay_resolved_basic,
            &self.layouts.lay_resolved_flex,
            &self.layouts.lay_resolved_grid,
            &mut self.renders.rnd_visual,
            &mut self.renders.rnd_dirty_entities,
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
