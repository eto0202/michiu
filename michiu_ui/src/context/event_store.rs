use std::path::PathBuf;

use crate::{
    ActiveAnimationsSparseSecondary, ActiveEntitiesVec, ActiveFocusTrigger, ActiveMasksSecondary,
    ActiveTransitionsSparseSecondary, BaseBasicLayoutsSecondary, BaseVisualPropertiesSecondary,
    BasicLayout, BasicLayoutsSecondary, ChildrenSecondary, ClipRectsSecondary, ComponentMask,
    ContentStore, Context, CursorIcon, DirtyLayoutEntitiesVec, DirtyRenderEntitiesVec,
    DndDragPayload, DndDragPlaceholderParent, DndDragProperty, DndDropProperty,
    DwriteLayoutsSparseSecondary, EffectiveZindicesSecondary, Element, ElementEffectsSecondary,
    ElementState, EntitiesSlot, EntityId, EventListeners, FlatDfsSequenceVec, FlexLayoutsSecondary,
    FocusTrigger, Focusable, GridLayoutsSecondary, InputContentsSparseSecondary,
    InteractionPropertiesSecondary, InteractionStates, LayoutPoint, LayoutRect, LayoutSize,
    LayoutStore, Length, Modifiers, MouseButton, OutputStore, ParentsSecondary, PointerEvents,
    Position, ReactiveStore, Rect, RectsSecondary, RenderStore, STATE_ACTIVED, STATE_DISABLED,
    STATE_DND_DRAG_IN, STATE_DND_DRAG_OVER, STATE_DND_DRAGGING, STATE_DRAGGED, STATE_FOCUSED,
    STATE_HOVERED, STATE_PRESSED, STATE_SELECTED, STYLE_DND_DRAGGABLE, STYLE_DND_DROPPABLE,
    STYLE_INTERACTION_PARENT, STYLE_INTERACTION_WITHIN, STYLE_POINTER_EVENTS,
    STYLE_PREVENT_FOCUS_STEAL, STYLE_PREVENT_FOCUS_STEAL_WITHIN, STYLE_RESIZABLE,
    ScrollOffsetsSecondary, ScrollbarStylesSecondary, SelectedRectsSparseSecondary,
    SelectionStartIndexSparseSecondary, SessionSpawnedVec, SortedEntitiesVec, SystemStore,
    TaffyNodesSecondary, TaffyTreeEntityId, TextAlign, TextContentsSparseSecondary, TextEngine,
    TextSelectionsSparseSecondary, TextSpansSparseSecondary, TopologyStore, UserSelect, Val,
    VirtualKey, VisualPropertiesSecondary, WindowStore, bind_context, handle_on_active,
    handle_on_blur, handle_on_click, handle_on_cursor_moved, handle_on_disable,
    handle_on_dnd_drag_start, handle_on_dnd_entity_drag, handle_on_dnd_entity_drop,
    handle_on_dnd_id_drag, handle_on_dnd_id_drop, handle_on_drag, handle_on_focus, handle_on_hover,
    handle_on_mouse_enter, handle_on_mouse_input, handle_on_mouse_leave, handle_on_right_click,
    handle_on_select,
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
    pub(crate) event_listeners: EventListenersSparseSecondary,
    pub interaction_states: InteractionStates,
    pub(crate) current_pointer_position: Option<LayoutPoint>,
    pub(crate) resizing_state: Option<ResizingState>,
    pub(crate) active_resize_hover: ActiveResizeHoverOption,
    pub(crate) dnd_drag_properties: DndDragPropertiesSparseSecondary,
    pub(crate) dnd_drop_properties: DndDropPropertiesSparseSecondary,
    pub(crate) active_dnd_drag_state: Option<ActiveDndDragState>,
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
            event_listeners: SparseSecondaryMap::new(),
            interaction_states: InteractionStates::new(),
            current_pointer_position: None,
            resizing_state: None,
            active_resize_hover: None,
            dnd_drag_properties: SparseSecondaryMap::new(),
            dnd_drop_properties: SparseSecondaryMap::new(),
            active_dnd_drag_state: None,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.event_listeners.clear();
        self.interaction_states = InteractionStates::new();
        self.current_pointer_position = None;
        self.resizing_state = None;
        self.active_resize_hover = None;
        self.dnd_drag_properties.clear();
        self.dnd_drop_properties.clear();
        self.active_dnd_drag_state = None;
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.event_listeners.remove(id);
        self.interaction_states.clear_entity(id);
        self.dnd_drag_properties.remove(id);
        self.dnd_drop_properties.remove(id);

        if let Some(ref state) = self.active_dnd_drag_state
            && (state.source_entity == id || state.placeholder_entity == id)
        {
            self.active_dnd_drag_state = None;
        }
    }
}

impl EventStore {
    pub(crate) fn resolve_dnd_placeholder_parent(
        root: EntityId,
        drag_prop: &DndDragProperty,
        rects: &RectsSecondary,
        basic_layouts: &BasicLayoutsSecondary,
    ) -> PlaceholderAttachment {
        match drag_prop.placeholder_parent {
            DndDragPlaceholderParent::Root => PlaceholderAttachment {
                parent_id: Some(root),
                rect: OutputStore::rect(root, rects).unwrap_or_default(),
                border_left: 0.0,
                border_top: 0.0,
            },
            DndDragPlaceholderParent::Custom(p_id) => {
                let p_rect = OutputStore::rect(p_id, rects).unwrap_or_default();
                let (b_l, b_t) = basic_layouts
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
        visual_properties: &mut VisualPropertiesSecondary,
    ) {
        let Some(vis) = visual_properties.get_mut(id) else {
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
        active_masks: &ActiveMasksSecondary,
        parents: &ParentsSecondary,
        rects: &RectsSecondary,
        basic_layouts: &BasicLayoutsSecondary,
    ) -> (Option<EntityId>, Option<(EntityId, ResizeDirection)>) {
        let mut current_id = target_id;
        let mut found_resize_hover = None;
        while let Some(id) = current_id {
            if active_masks[id].has(STYLE_RESIZABLE) {
                let rect = rects.get(id).copied().unwrap_or_default();
                let resizable_flags = basic_layouts.get(id).map_or([false; 4], |l| l.resizable);

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
            current_id = parents.get(id).copied().flatten();
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

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn state_pressed_resize_drag(
        id: EntityId,
        dir: ResizeDirection,
        rects: &RectsSecondary,
        basic_layouts: &mut BasicLayoutsSecondary,
        base_basic_layouts: &mut BaseBasicLayoutsSecondary,
        parents: &ParentsSecondary,
        current_pointer_position: Option<LayoutPoint>,
        resizing_state: &mut Option<ResizingState>,
        interaction_states: &mut InteractionStates,
    ) {
        let rect = rects.get(id).copied().unwrap_or_default();
        let position = basic_layouts
            .get(id)
            .map_or(Position::Relative, |l| l.position);

        let resolve_length = |length: Length, ref_size: f32| match length {
            Length::Px(v) => v,
            Length::Percent(p) => ref_size * (p / 100.0),
        };

        // 親要素の矩形と、その左・上ボーダーの厚みを取得
        let parent_id = parents.get(id).copied().flatten();
        let (parent_rect, parent_border_left, parent_border_top) =
            parent_id.map_or((LayoutRect::ZERO, 0.0, 0.0), |p_id| {
                let p_rect = rects.get(p_id).copied().unwrap_or_default();
                // ボーダー幅の抽出
                let (border_l, border_t) = basic_layouts.get(p_id).map_or((0.0, 0.0), |l| {
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
        let mut start_inset = Rect {
            top: Val::Px(0.0),
            right: Val::Px(0.0),
            bottom: Val::Px(0.0),
            left: Val::Px(0.0),
        };
        if position == Position::Absolute {
            start_inset.top = Val::Px(local_y);
            start_inset.left = Val::Px(local_x);
            start_inset.right = Val::Auto;
            start_inset.bottom = Val::Auto;

            // SoA 側も、この Top-Left 座標で即時上書きアップデート
            let basic = basic_layouts.get_mut(id);
            let base_basic = base_basic_layouts.get_mut(id);
            for layout in [basic, base_basic].into_iter().flatten() {
                layout.inset = start_inset;
            }
        } else {
            // 相対配置時は、通常通りそのままのインセットを使用
            start_inset = basic_layouts
                .get(id)
                .map_or(BasicLayout::default().inset, |l| l.inset);
        }

        let start_pos = current_pointer_position.unwrap_or(LayoutPoint::ZERO);

        *resizing_state = Some(ResizingState {
            entity_id: id,
            direction: dir,
            start_mouse_pos: start_pos,
            start_rect: rect,
            start_inset,
        });

        // リサイズ中の要素は pressed とマーク
        interaction_states.pressed = Some(id);
    }

    pub(crate) fn restrict_focusable_element(
        id: EntityId,
        active_masks: &ActiveMasksSecondary,
        visual_properties: &VisualPropertiesSecondary,
    ) -> bool {
        let focusable = visual_properties
            .get(id)
            .and_then(|v| v.focusable)
            .or_else(|| {
                let mask = active_masks.get(id).copied().unwrap_or_default();
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
        scrollbar_styles: &mut ScrollbarStylesSecondary,
    ) -> SmallVec<[EntityId; 4]> {
        let mut dirty_ids = SmallVec::<[EntityId; 4]>::new();

        for (id, state) in scrollbar_styles {
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
        visual_properties: &VisualPropertiesSecondary,
    ) -> UserSelect {
        visual_properties
            .get(id)
            .and_then(|v| v.user_select)
            .unwrap_or(UserSelect::None)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn autoscroll_occurred(
        id: EntityId,
        current_pointer_position: Option<LayoutPoint>,
        clip_rects: &ClipRectsSecondary,
        active_masks: &mut ActiveMasksSecondary,
        input_contents: &InputContentsSparseSecondary,
        text_engine: &TextEngine,
        text_contents: &TextContentsSparseSecondary,
        visual_properties: &VisualPropertiesSecondary,
        text_spans: &TextSpansSparseSecondary,
        dwrite_layouts: &DwriteLayoutsSparseSecondary,
        basic_layouts: &BasicLayoutsSecondary,
        flex_layouts: &FlexLayoutsSecondary,
        grid_layouts: &GridLayoutsSecondary,
        active_transitions: &ActiveTransitionsSparseSecondary,
        parents: &ParentsSecondary,
        children: &ChildrenSecondary,
        interaction_properties: &InteractionPropertiesSecondary,
        rects: &RectsSecondary,
        scrollbar_styles: &mut ScrollbarStylesSecondary,
        scroll_offsets: &mut ScrollOffsetsSecondary,
        last_window_size: Option<LayoutSize>,
        taffy_nodes: &TaffyNodesSecondary,
        taffy: &mut TaffyTreeEntityId,
        dirty_layout_entities: &mut DirtyLayoutEntitiesVec,
    ) -> (bool, Option<LayoutPoint>) {
        // ポインタ位置、またはクリップ領域がない場合
        let Some(pointer_pos) = current_pointer_position else {
            return (false, None);
        };
        let Some(clip) = clip_rects.get(id).copied() else {
            return (false, None);
        };

        // テキスト選択状態
        let user_select = EventStore::get_user_select(id, visual_properties);
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
            active_masks,
            input_contents,
            text_engine,
            text_contents,
            visual_properties,
            text_spans,
            dwrite_layouts,
            basic_layouts,
            flex_layouts,
            grid_layouts,
            active_transitions,
            parents,
            children,
            interaction_properties,
            rects,
            scrollbar_styles,
            scroll_offsets,
            last_window_size,
            taffy_nodes,
            taffy,
            dirty_layout_entities,
        );

        if scroll {
            (true, Some(pointer_pos))
        } else {
            (false, None)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn pressed_local_point(
        pressed_id: EntityId,
        logical_pos: LayoutPoint,
        scroll_offsets: &mut ScrollOffsetsSecondary,
        input_contents: &InputContentsSparseSecondary,
        rects: &RectsSecondary,
        basic_layouts: &BasicLayoutsSecondary,
        flex_layouts: &FlexLayoutsSecondary,
        grid_layouts: &GridLayoutsSecondary,
        active_masks: &ActiveMasksSecondary,
        active_transitions: &ActiveTransitionsSparseSecondary,
        parents: &ParentsSecondary,
        interaction_properties: &InteractionPropertiesSecondary,
        visual_properties: &VisualPropertiesSecondary,
    ) -> LayoutPoint {
        let rect = OutputStore::rect(pressed_id, rects).unwrap_or_default();
        let (basic, flex, _) = LayoutStore::resolve_active_layouts(
            pressed_id,
            basic_layouts,
            flex_layouts,
            grid_layouts,
            active_masks,
            active_transitions,
            parents,
            interaction_properties,
            visual_properties,
        );
        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

        let scroll = scroll_offsets.get(pressed_id).copied().unwrap_or_default();

        let text_size = if let Some(contents) = input_contents.get(pressed_id)
            && let Some(layout_rect) = contents.last_layout
        {
            LayoutSize::new(layout_rect.width, layout_rect.height)
        } else {
            LayoutSize::ZERO
        };

        let align_offset =
            OutputStore::calc_align_offset(rect, border, padding, text_size, flex.text_align);

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
        let old_id = cx.events.interaction_states.hovered;

        if old_id == target_id {
            return;
        }

        cx.events.interaction_states.hovered = target_id;

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
    #[allow(clippy::too_many_lines)]
    pub(crate) fn update_state(cx: &mut Context, id: EntityId, state_flag: u128, active: bool) {
        let mut was_active = false;
        let mut state_changed = false;

        let TopologyStore {
            active_masks,
            entities,
            parents,
            children,
            ..
        } = &mut cx.topology;
        let LayoutStore {
            taffy,
            taffy_nodes,
            basic_layouts,
            base_basic_layouts,
            dirty_layout_entities,
            ..
        } = &mut cx.layouts;
        let RenderStore {
            visual_properties,
            base_visual_properties,
            interaction_properties,
            active_transitions,
            active_animations,
            dirty_render_entities,
            ..
        } = &mut cx.renders;
        let OutputStore { rects, .. } = &mut cx.outputs;
        let ContentStore { input_contents, .. } = &mut cx.contents;
        let ReactiveStore {
            element_effects, ..
        } = &mut cx.reactive;
        let EventStore {
            event_listeners, ..
        } = &mut cx.events;
        let WindowStore {
            last_window_size, ..
        } = &mut cx.window;

        let Some(mask) = active_masks.get_mut(id) else {
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

        // 状態変化の発生時に即座に動的なスタイルを解決する
        RenderStore::resolve_element_style_state(
            id,
            true,
            active_masks,
            base_visual_properties,
            interaction_properties,
            visual_properties,
            parents,
            entities,
            children,
            input_contents,
            element_effects,
            active_transitions,
            active_animations,
            dirty_render_entities,
            basic_layouts,
            base_basic_layouts,
            rects,
            last_window_size.as_ref(),
            taffy_nodes,
            taffy,
            dirty_layout_entities,
        );

        // 親から子方向へのスタイル解決の伝播
        if let Some(child) = children.get(id).cloned() {
            for child_id in child {
                if active_masks[child_id].has(STYLE_INTERACTION_PARENT) {
                    RenderStore::resolve_element_style_state(
                        child_id,
                        true,
                        active_masks,
                        base_visual_properties,
                        interaction_properties,
                        visual_properties,
                        parents,
                        entities,
                        children,
                        input_contents,
                        element_effects,
                        active_transitions,
                        active_animations,
                        dirty_render_entities,
                        basic_layouts,
                        base_basic_layouts,
                        rects,
                        last_window_size.as_ref(),
                        taffy_nodes,
                        taffy,
                        dirty_layout_entities,
                    );

                    if RenderStore::does_state_require_layout(
                        child_id,
                        interaction_properties,
                        state_flag,
                    ) {
                        LayoutStore::mark_layout_dirty(
                            child_id,
                            taffy_nodes,
                            taffy,
                            active_masks,
                            dirty_layout_entities,
                            parents,
                        );
                        RenderStore::mark_render_dirty(id, active_masks, dirty_render_entities);
                    } else {
                        RenderStore::mark_render_dirty(id, active_masks, dirty_render_entities);
                    }
                }
            }
        }

        // STYLE_INTERACTION_WITHIN マスク判定による親先祖の早期バイパス
        let mut curr = id;
        while let Some(Some(parent_id)) = parents.get(curr).copied() {
            if entities.contains_key(parent_id) {
                let parent_mask = active_masks[parent_id];

                // 先祖要素が within スタイルを持っている場合のみそのスタイル評価を実行
                if parent_mask.has(STYLE_INTERACTION_WITHIN) {
                    RenderStore::resolve_element_style_state(
                        parent_id,
                        true,
                        active_masks,
                        base_visual_properties,
                        interaction_properties,
                        visual_properties,
                        parents,
                        entities,
                        children,
                        input_contents,
                        element_effects,
                        active_transitions,
                        active_animations,
                        dirty_render_entities,
                        basic_layouts,
                        base_basic_layouts,
                        rects,
                        last_window_size.as_ref(),
                        taffy_nodes,
                        taffy,
                        dirty_layout_entities,
                    );

                    if RenderStore::does_state_require_layout(
                        parent_id,
                        interaction_properties,
                        state_flag,
                    ) {
                        LayoutStore::mark_layout_dirty(
                            parent_id,
                            taffy_nodes,
                            taffy,
                            active_masks,
                            dirty_layout_entities,
                            parents,
                        );
                        RenderStore::mark_render_dirty(id, active_masks, dirty_render_entities);
                    } else {
                        RenderStore::mark_render_dirty(
                            parent_id,
                            active_masks,
                            dirty_render_entities,
                        );
                    }
                }
            }
            curr = parent_id;
        }

        // 状態変化による本要素のレイアウト汚染チェック
        if RenderStore::does_state_require_layout(id, interaction_properties, state_flag) {
            LayoutStore::mark_layout_dirty(
                id,
                taffy_nodes,
                taffy,
                active_masks,
                dirty_layout_entities,
                parents,
            );
            RenderStore::mark_render_dirty(id, active_masks, dirty_render_entities);
        } else {
            RenderStore::mark_render_dirty(id, active_masks, dirty_render_entities);
        }

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
            .event_listeners
            .get(id)
            .is_some_and(|l| l.on_cursor_moved.is_some());

        if has_listener {
            let rect = OutputStore::rect(id, &cx.outputs.rects).unwrap_or_default();
            let relative_pos = LayoutPoint::new(logical_pos.x - rect.x, logical_pos.y - rect.y);
            handle_on_cursor_moved(cx, id, relative_pos);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_dnd_placeholder(
        root: EntityId,
        pressed_id: EntityId,
        drag_prop: &DndDragProperty,
        rects: &RectsSecondary,
        basic_layouts: &BasicLayoutsSecondary,
        entities: &mut EntitiesSlot,
        parents: &mut ParentsSecondary,
        children: &mut ChildrenSecondary,
        active_masks: &mut ActiveMasksSecondary,
        active_entities: &mut ActiveEntitiesVec,
        session_spawned: &mut SessionSpawnedVec,
        is_structure_dirty: &mut bool,
        taffy: &mut TaffyTreeEntityId,
        taffy_nodes: &mut TaffyNodesSecondary,
        dirty_layout_entities: &mut DirtyLayoutEntitiesVec,
        dirty_render_entities: &mut DirtyRenderEntitiesVec,
    ) -> EntityId {
        let placeholder =
            EventStore::resolve_dnd_placeholder_parent(root, drag_prop, rects, basic_layouts);

        let placeholder_id = TopologyStore::spawn(
            placeholder.parent_id,
            entities,
            parents,
            children,
            active_masks,
            active_entities,
            session_spawned,
            is_structure_dirty,
            taffy,
            taffy_nodes,
            dirty_render_entities,
        );

        if let Some(p_id) = placeholder.parent_id {
            TopologyStore::add_child(
                p_id,
                placeholder_id,
                parents,
                children,
                is_structure_dirty,
                active_masks,
                taffy_nodes,
                taffy,
                dirty_layout_entities,
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
        if let Some(basic) = cx.layouts.base_basic_layouts.get(pressed_id).copied() {
            cx.layouts.base_basic_layouts.insert(placeholder_id, basic);
            cx.layouts.basic_layouts.insert(placeholder_id, basic);
        }
        if let Some(visual) = cx.renders.base_visual_properties.get(pressed_id).cloned() {
            cx.renders
                .base_visual_properties
                .insert(placeholder_id, visual.clone());
            cx.renders.visual_properties.insert(placeholder_id, visual);
        }
        if let Some(interaction) = cx.renders.interaction_properties.get(pressed_id).cloned() {
            cx.renders
                .interaction_properties
                .insert(placeholder_id, interaction);
        }

        // ドラッグ元とプレースホルダーの状態を同期
        EventStore::update_state(cx, pressed_id, STATE_DND_DRAGGING, true);
        EventStore::update_state(cx, placeholder_id, STATE_DND_DRAG_OVER, true);

        // プレースホルダー側を Absolute 配置化
        let basic = cx.layouts.basic_layouts.get_mut(placeholder_id);
        let base_basic = cx.layouts.base_basic_layouts.get_mut(placeholder_id);
        for layout in [basic, base_basic].into_iter().flatten() {
            layout.position = Position::Absolute;
            layout.size.width = Val::Px(start_rect.width);
            layout.size.height = Val::Px(start_rect.height);
        }

        // ヒットテストを透過
        let visual = cx.renders.visual_properties.get_mut(placeholder_id);
        let base_visual = cx.renders.base_visual_properties.get_mut(placeholder_id);
        for vis in [visual, base_visual].into_iter().flatten() {
            vis.pointer_events = Some(PointerEvents::None);
        }
        if let Some(mask) = cx.topology.active_masks.get_mut(placeholder_id) {
            mask.set(STYLE_POINTER_EVENTS);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn transfer_children_to_placeholder(
        pressed_id: EntityId,
        placeholder_id: EntityId,
        parents: &mut ParentsSecondary,
        children: &mut ChildrenSecondary,
        taffy_nodes: &TaffyNodesSecondary,
        taffy: &mut TaffyTreeEntityId,
        active_masks: &mut ActiveMasksSecondary,
        dirty_layout_entities: &mut DirtyLayoutEntitiesVec,
    ) {
        let Some(src_children) = children.get(pressed_id).cloned() else {
            return;
        };

        for child_id in src_children {
            // 子要素の親ポインタをプレースホルダーに付け替え
            parents.insert(child_id, Some(placeholder_id));

            // プレースホルダー側の子要素リストへ追加
            if let Some(ph_children) = children.get_mut(placeholder_id) {
                ph_children.push(child_id);
            }

            // Taffy 側の親子構造も、一時的にプレースホルダーに繋ぎ替え
            if let Some(&src_node) = taffy_nodes.get(pressed_id)
                && let Some(&ph_node) = taffy_nodes.get(placeholder_id)
                && let Some(&child_node) = taffy_nodes.get(child_id)
            {
                let _ = taffy.remove_child(src_node, child_node);
                let _ = taffy.add_child(ph_node, child_node);
            }
        }

        // 元の要素の子要素リストは一時的にクリア（プレースホルダーに避難しているため）
        if let Some(src_children_mut) = children.get_mut(pressed_id) {
            src_children_mut.clear();
        }

        // 元要素とプレースホルダー要素の両方をダーティマーク
        for id in [pressed_id, placeholder_id] {
            LayoutStore::mark_layout_dirty(
                id,
                taffy_nodes,
                taffy,
                active_masks,
                dirty_layout_entities,
                parents,
            );
        }
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn start_dnd_drag_session(cx: &mut Context, pressed_id: EntityId, logical_pos: LayoutPoint) {
        let drag_prop = cx
            .events
            .dnd_drag_properties
            .get(pressed_id)
            .copied()
            .unwrap();
        let start_rect = OutputStore::rect(pressed_id, &cx.outputs.rects).unwrap_or_default();

        // 開始時のクリック位置と要素左上の相対的なズレを計算
        let click_offset =
            LayoutPoint::new(logical_pos.x - start_rect.x, logical_pos.y - start_rect.y);

        // ウィンドウのルート要素を自己解決
        let root = TopologyStore::find_root_entity(
            &cx.topology.entities,
            &cx.topology.parents,
            &cx.topology.flat_dfs_sequence,
        )
        .expect("Root EntityId not found in Context");

        // プレースホルダーをアタッチ先親の直下へ spawn して生成
        let placeholder_id = EventStore::spawn_dnd_placeholder(
            root,
            pressed_id,
            &drag_prop,
            &cx.outputs.rects,
            &cx.layouts.basic_layouts,
            &mut cx.topology.entities,
            &mut cx.topology.parents,
            &mut cx.topology.children,
            &mut cx.topology.active_masks,
            &mut cx.topology.active_entities,
            &mut cx.topology.session_spawned,
            &mut cx.topology.is_structure_dirty,
            &mut cx.layouts.taffy,
            &mut cx.layouts.taffy_nodes,
            &mut cx.layouts.dirty_layout_entities,
            &mut cx.renders.dirty_render_entities,
        );

        // プレースホルダーの初期スタイル・透過・状態情報をセットアップ
        EventStore::setup_placeholder_properties(cx, pressed_id, placeholder_id, start_rect);

        // 元の要素から子要素トポロジーをプレースホルダーへ移行
        EventStore::transfer_children_to_placeholder(
            pressed_id,
            placeholder_id,
            &mut cx.topology.parents,
            &mut cx.topology.children,
            &cx.layouts.taffy_nodes,
            &mut cx.layouts.taffy,
            &mut cx.topology.active_masks,
            &mut cx.layouts.dirty_layout_entities,
        );

        // プレースホルダーアタッチ前の、本当の元の親要素のIDを記録
        let original_parent = cx.topology.parents.get(pressed_id).copied().flatten();

        // セッション開始
        cx.events.active_dnd_drag_state = Some(ActiveDndDragState {
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

    #[allow(clippy::too_many_lines)]
    pub(crate) fn propagate_dnd_drag_events(
        cx: &mut Context,
        prev_pos: Option<LayoutPoint>,
        logical_pos: LayoutPoint,
    ) {
        let Some(pressed_id) = cx.events.interaction_states.pressed else {
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
        cx.events.interaction_states.dragged = Some(pressed_id);

        // D&D 設定（STYLE_DRAGGABLE）を持っている場合のセッションのキック
        if cx.topology.active_masks[pressed_id].has(STYLE_DND_DRAGGABLE)
            && cx.events.active_dnd_drag_state.is_none()
        {
            EventStore::start_dnd_drag_session(cx, pressed_id, logical_pos);
        }

        handle_on_drag(cx, pressed_id, delta);
    }

    pub(crate) fn calculate_dnd_relative_local(
        root: EntityId,
        drag_prop: &DndDragProperty,
        rects: &RectsSecondary,
        basic_layouts: &BasicLayoutsSecondary,
    ) -> (LayoutRect, f32, f32) {
        let p = EventStore::resolve_dnd_placeholder_parent(root, drag_prop, rects, basic_layouts);
        (p.rect, p.border_left, p.border_top)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn update_inset_based_relative_local(
        root: EntityId,
        placeholder: EntityId,
        logical_pos: LayoutPoint,
        drag_prop: &DndDragProperty,
        drag_state: &ActiveDndDragState,
        rects: &RectsSecondary,
        basic_layouts: &mut BasicLayoutsSecondary,
        base_basic_layouts: &mut BaseBasicLayoutsSecondary,
        taffy_nodes: &TaffyNodesSecondary,
        taffy: &mut TaffyTreeEntityId,
        active_masks: &mut ActiveMasksSecondary,
        dirty_layout_entities: &mut DirtyLayoutEntitiesVec,
        parents: &ParentsSecondary,
        dirty_render_entities: &mut DirtyRenderEntitiesVec,
    ) {
        // アタッチ先親コンテナ基準での相対ローカル座標を逆算して追従（Inset更新）
        let (parent_rect, b_l, b_t) =
            EventStore::calculate_dnd_relative_local(root, drag_prop, rects, basic_layouts);

        // マウスのドラッグ開始時クリックオフセットを用いて、ローカル Top-Left 座標を算出
        let local_x = logical_pos.x - (parent_rect.x + b_l) - drag_state.click_offset.x;
        let local_y = logical_pos.y - (parent_rect.y + b_t) - drag_state.click_offset.y;

        if let Some(layout) = basic_layouts.get_mut(placeholder) {
            layout.inset.left = Val::Px(local_x);
            layout.inset.top = Val::Px(local_y);
            layout.inset.right = Val::Auto;
            layout.inset.bottom = Val::Auto;
        }
        if let Some(layout) = base_basic_layouts.get_mut(placeholder) {
            layout.inset.left = Val::Px(local_x);
            layout.inset.top = Val::Px(local_y);
            layout.inset.right = Val::Auto;
            layout.inset.bottom = Val::Auto;
        }

        LayoutStore::mark_layout_dirty(
            placeholder,
            taffy_nodes,
            taffy,
            active_masks,
            dirty_layout_entities,
            parents,
        );
        RenderStore::mark_render_dirty(placeholder, active_masks, dirty_render_entities);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn detect_drop_target_during_intrusion(
        src_id: EntityId,
        placeholder: EntityId,
        logical_pos: LayoutPoint,
        active_entities: &ActiveEntitiesVec,
        active_masks: &ActiveMasksSecondary,
        flat_dfs_sequence: &FlatDfsSequenceVec,
        parents: &ParentsSecondary,
        effective_z_indices: &mut EffectiveZindicesSecondary,
        sorted_entities: &mut SortedEntitiesVec,
        visual_properties: &VisualPropertiesSecondary,
        base_visual_properties: &BaseVisualPropertiesSecondary,
        interaction_states: &InteractionStates,
        rects: &RectsSecondary,
        clip_rects: &ClipRectsSecondary,
    ) -> Option<EntityId> {
        let hit_id = TopologyStore::hit_test(
            logical_pos,
            active_entities,
            active_masks,
            flat_dfs_sequence,
            parents,
            effective_z_indices,
            sorted_entities,
            visual_properties,
            base_visual_properties,
            interaction_states,
            rects,
            clip_rects,
        )?;

        // ヒットした要素がドラッグ元自身、またはその子孫である場合は、
        // 自身のサブツリーをすべてスキップするためにドラッグ元の親から探索を開始
        let is_descendant = TopologyStore::is_descendant_of(hit_id, src_id, parents);
        let mut current_id = if hit_id == src_id || is_descendant {
            parents.get(src_id).copied().flatten()
        } else {
            Some(hit_id)
        };

        while let Some(id) = current_id {
            let is_dnd = active_masks
                .get(id)
                .is_some_and(|f| f.has(STYLE_DND_DROPPABLE));

            if id != placeholder && is_dnd {
                return Some(id); // ドロップ先を見つけたら即座に返す
            }
            current_id = parents.get(id).copied().flatten();
        }

        None
    }

    pub(crate) fn sync_state_drag_in(cx: &mut Context, found_drop_target: Option<EntityId>) {
        let Some(mut drag_state) = cx.events.active_dnd_drag_state.take() else {
            return;
        };

        if found_drop_target == drag_state.current_drop_target {
            cx.events.active_dnd_drag_state = Some(drag_state);
            return;
        }

        if let Some(old_target) = drag_state.current_drop_target {
            EventStore::update_state(cx, old_target, STATE_DND_DRAG_IN, false);
        }
        if let Some(new_target) = found_drop_target {
            EventStore::update_state(cx, new_target, STATE_DND_DRAG_IN, true);
        }

        drag_state.current_drop_target = found_drop_target;
        cx.events.active_dnd_drag_state = Some(drag_state);
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
        text_engine: &TextEngine,
        dw_layout: &IDWriteTextLayout,
    ) -> (std::ops::Range<usize>, bool) {
        let (current_index, is_trailing) = text_engine.hit_test_point(dw_layout, local.x, local.y);
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

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_text_selection_click(
        id: EntityId,
        start_pos: usize,
        local: LayoutPoint,
        rects: &RectsSecondary,
        selected_rects: &mut SelectedRectsSparseSecondary,
        dwrite_layouts: &DwriteLayoutsSparseSecondary,
        input_contents: &mut InputContentsSparseSecondary,
        text_contents: &mut TextContentsSparseSecondary,
        text_selections: &mut TextSelectionsSparseSecondary,
        text_spans: &TextSpansSparseSecondary,
        text_engine: &TextEngine,
        basic_layouts: &BasicLayoutsSecondary,
        flex_layouts: &FlexLayoutsSecondary,
        grid_layouts: &GridLayoutsSecondary,
        active_masks: &mut ActiveMasksSecondary,
        children: &ChildrenSecondary,
        interaction_properties: &InteractionPropertiesSecondary,
        scrollbar_styles: &mut ScrollbarStylesSecondary,
        scroll_offsets: &mut ScrollOffsetsSecondary,
        last_window_size: Option<LayoutSize>,
        taffy_nodes: &TaffyNodesSecondary,
        taffy: &mut TaffyTreeEntityId,
        dirty_layout_entities: &mut DirtyLayoutEntitiesVec,
        active_transitions: &ActiveTransitionsSparseSecondary,
        parents: &ParentsSecondary,
        visual_properties: &mut VisualPropertiesSecondary,
        base_visual_properties: &BaseVisualPropertiesSecondary,
        dirty_render_entities: &mut DirtyRenderEntitiesVec,
        scale_factor: f32,
    ) {
        let Some(dw_layout) = SystemStore::get_or_create_layout(
            id,
            text_contents,
            visual_properties,
            dwrite_layouts,
            text_spans,
            text_engine,
        ) else {
            return;
        };

        let (range, is_reversed) =
            EventStore::calculate_text_selection(start_pos, local, text_engine, &dw_layout);

        text_selections.insert(id, range.clone());

        OutputStore::update_selection_rects(id, &dw_layout, text_selections, selected_rects);

        if let Some(contents) = input_contents.get_mut(id) {
            contents.selection_reversed = is_reversed;
            contents.selected_range = range;

            OutputStore::update_input_caret_position(
                id,
                rects,
                dwrite_layouts,
                input_contents,
                text_contents,
                text_selections,
                text_spans,
                text_engine,
                basic_layouts,
                flex_layouts,
                grid_layouts,
                active_masks,
                children,
                interaction_properties,
                scrollbar_styles,
                scroll_offsets,
                last_window_size,
                taffy_nodes,
                taffy,
                dirty_layout_entities,
                active_transitions,
                parents,
                visual_properties,
                base_visual_properties,
                scale_factor,
            );
        }
        RenderStore::mark_render_dirty(id, active_masks, dirty_render_entities);
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn remove_dragged_elemet(
        src_id: EntityId,
        drag_state: &ActiveDndDragState,
        taffy_nodes: &TaffyNodesSecondary,
        taffy: &mut TaffyTreeEntityId,
        active_masks: &mut ActiveMasksSecondary,
        dirty_layout_entities: &mut DirtyLayoutEntitiesVec,
        parents: &ParentsSecondary,
        children: &mut ChildrenSecondary,
    ) {
        let Some(src_parent_id) = drag_state.original_parent else {
            return;
        };

        if let Some(src_children) = children.get_mut(src_parent_id) {
            src_children.retain(|x| *x != src_id);
        }
        // 旧親側の Taffy 順序も再同期
        LayoutStore::resync_taffy_children_order(src_parent_id, taffy_nodes, taffy, children);
        LayoutStore::mark_layout_dirty(
            src_parent_id,
            taffy_nodes,
            taffy,
            active_masks,
            dirty_layout_entities,
            parents,
        );
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn pointer_move_inner(cx: &mut Context, logical_pos: LayoutPoint) {
        let _context_guard = bind_context(cx);
        let prev_pos = cx.events.current_pointer_position;
        cx.events.current_pointer_position = Some(logical_pos);

        // リサイズ中のドラッグ同期処理
        if let Some(ref state) = cx.events.resizing_state {
            LayoutStore::sync_resizing_drag(
                logical_pos,
                state,
                &mut cx.layouts.basic_layouts,
                &mut cx.layouts.base_basic_layouts,
                &cx.outputs.rects,
                &cx.topology.parents,
                cx.window.last_window_size.as_ref(),
                &cx.layouts.taffy_nodes,
                &mut cx.layouts.taffy,
                &mut cx.topology.active_masks,
                &mut cx.layouts.dirty_layout_entities,
                &mut cx.renders.dirty_render_entities,
            );
            return; // リサイズドラッグ中は、通常のホバーやドラッグ判定を完全にスキップして早期リターン
        }

        OutputStore::sync_scrollbar_drag(
            logical_pos,
            &mut cx.topology.active_masks,
            &cx.contents.input_contents,
            &cx.system.text_engine,
            &cx.contents.text_contents,
            &cx.renders.visual_properties,
            &mut cx.renders.dirty_render_entities,
            &cx.contents.text_spans,
            &cx.system.dwrite_layouts,
            &cx.layouts.basic_layouts,
            &cx.layouts.flex_layouts,
            &cx.layouts.grid_layouts,
            &cx.renders.active_transitions,
            &cx.topology.parents,
            &cx.topology.children,
            &cx.layouts.taffy_nodes,
            &mut cx.layouts.taffy,
            &mut cx.layouts.dirty_layout_entities,
            &cx.renders.interaction_properties,
            &cx.outputs.rects,
            &mut cx.layouts.scrollbar_styles,
            &mut cx.outputs.scroll_offsets,
            cx.window.last_window_size,
        );

        // ヒットテストのキャッシュ
        let hit_id = TopologyStore::hit_test(
            logical_pos,
            &cx.topology.active_entities,
            &cx.topology.active_masks,
            &cx.topology.flat_dfs_sequence,
            &cx.topology.parents,
            &mut cx.topology.effective_z_indices,
            &mut cx.topology.sorted_entities,
            &cx.renders.visual_properties,
            &cx.renders.base_visual_properties,
            &cx.events.interaction_states,
            &cx.outputs.rects,
            &cx.outputs.clip_rects,
        );

        // マウスボタン押し下げ中は、他の要素へのインタラクション漏洩を防ぐためヒット先を押し下げ要素に強制ロック
        let target_id = cx.events.interaction_states.pressed.or(hit_id);

        // 直前のリサイズホバー対象を退避
        let prev_resize_hover = cx.events.active_resize_hover;
        // リサイズホバー情報を一旦リセット
        cx.events.active_resize_hover = None;

        // ヒットした要素、およびその親先祖に向かってツリーを遡上
        let (current_id, found_resize_hover) = EventStore::found_resize_hover(
            target_id,
            logical_pos,
            &cx.topology.active_masks,
            &cx.topology.parents,
            &cx.outputs.rects,
            &cx.layouts.basic_layouts,
        );

        if let Some((id, dir)) = found_resize_hover {
            cx.events.active_resize_hover = Some((id, dir));
            let vis = cx.renders.visual_properties.get(id).unwrap();
            EventStore::apply_resizable_cursor_style(id, dir, &mut cx.renders.visual_properties);
            RenderStore::mark_render_dirty(
                id,
                &mut cx.topology.active_masks,
                &mut cx.renders.dirty_render_entities,
            );
        }

        // 枠線から外れた、または異なる要素に変わった場合
        if let Some((prev_id, _)) = prev_resize_hover {
            let now_id = cx.events.active_resize_hover.map(|(id, _)| id);

            // 異なるホバー状態になった場合、旧要素のカーソル上書きを破棄し本来のスタイルに即時強制リセット
            if Some(prev_id) != now_id {
                // スタイルの再解決を叩き、上書きされていた vis.cursor を本来のカーソル（通常ホバー/ベース等）へ復旧
                RenderStore::resolve_element_style_state(
                    prev_id,
                    false,
                    &mut cx.topology.active_masks,
                    &cx.renders.base_visual_properties,
                    &cx.renders.interaction_properties,
                    &mut cx.renders.visual_properties,
                    &cx.topology.parents,
                    &cx.topology.entities,
                    &cx.topology.children,
                    &cx.contents.input_contents,
                    &cx.reactive.element_effects,
                    &mut cx.renders.active_transitions,
                    &mut cx.renders.active_animations,
                    &mut cx.renders.dirty_render_entities,
                    &mut cx.layouts.basic_layouts,
                    &cx.layouts.base_basic_layouts,
                    &cx.outputs.rects,
                    cx.window.last_window_size.as_ref(),
                    &cx.layouts.taffy_nodes,
                    &mut cx.layouts.taffy,
                    &mut cx.layouts.dirty_layout_entities,
                );
                RenderStore::mark_render_dirty(
                    prev_id,
                    &mut cx.topology.active_masks,
                    &mut cx.renders.dirty_render_entities,
                );
            }
        }

        if let Some(pressed_id) = cx.events.interaction_states.pressed {
            let user_select =
                EventStore::get_user_select(pressed_id, &cx.renders.visual_properties);

            if user_select == UserSelect::Text
                && let Some(start_pos) = cx.outputs.selection_start_index.get(pressed_id).copied()
            {
                // プレースホルダー選択のドラッグ遮断
                if let Some(contents) = cx.contents.input_contents.get(pressed_id) {
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
                    &mut cx.outputs.scroll_offsets,
                    &cx.contents.input_contents,
                    &cx.outputs.rects,
                    &cx.layouts.basic_layouts,
                    &cx.layouts.flex_layouts,
                    &cx.layouts.grid_layouts,
                    &cx.topology.active_masks,
                    &cx.renders.active_transitions,
                    &cx.topology.parents,
                    &cx.renders.interaction_properties,
                    &cx.renders.visual_properties,
                );
                EventStore::handle_text_selection_click(
                    pressed_id,
                    start_pos,
                    local,
                    &cx.outputs.rects,
                    &mut cx.outputs.selected_rects,
                    &cx.system.dwrite_layouts,
                    &mut cx.contents.input_contents,
                    &mut cx.contents.text_contents,
                    &mut cx.outputs.text_selections,
                    &cx.contents.text_spans,
                    &cx.system.text_engine,
                    &cx.layouts.basic_layouts,
                    &cx.layouts.flex_layouts,
                    &cx.layouts.grid_layouts,
                    &mut cx.topology.active_masks,
                    &cx.topology.children,
                    &cx.renders.interaction_properties,
                    &mut cx.layouts.scrollbar_styles,
                    &mut cx.outputs.scroll_offsets,
                    cx.window.last_window_size,
                    &cx.layouts.taffy_nodes,
                    &mut cx.layouts.taffy,
                    &mut cx.layouts.dirty_layout_entities,
                    &cx.renders.active_transitions,
                    &cx.topology.parents,
                    &mut cx.renders.visual_properties,
                    &cx.renders.base_visual_properties,
                    &mut cx.renders.dirty_render_entities,
                    cx.window.scale_factor,
                );
            }
        }

        // ホバー（Enter/Leave）状態の解決
        if hit_id != cx.events.interaction_states.hovered {
            EventStore::resolve_hover_state(cx, hit_id);
        }

        // カーソル移動イベントの伝播
        EventStore::propagate_cursor_move_events(cx, hit_id, logical_pos);

        // ドラッグイベントの伝播
        EventStore::propagate_dnd_drag_events(cx, prev_pos, logical_pos);

        // D&D プレースホルダーの移動とドロップ先ホバー検知
        let Some(ref drag_state) = cx.events.active_dnd_drag_state else {
            return;
        };

        // ウィンドウのルート要素を解決
        let Some(root) = TopologyStore::find_root_entity(
            &cx.topology.entities,
            &cx.topology.parents,
            &cx.topology.flat_dfs_sequence,
        ) else {
            return; // TODO: エラー処理
        };

        let src_id = drag_state.source_entity;
        let placeholder_id = drag_state.placeholder_entity;

        let drag_prop = cx.events.dnd_drag_properties.get(src_id).copied().unwrap();

        // アタッチ先親コンテナ基準での相対ローカル座標を逆算して追従
        EventStore::update_inset_based_relative_local(
            root,
            placeholder_id,
            logical_pos,
            &drag_prop,
            &drag_state,
            &cx.outputs.rects,
            &mut cx.layouts.basic_layouts,
            &mut cx.layouts.base_basic_layouts,
            &cx.layouts.taffy_nodes,
            &mut cx.layouts.taffy,
            &mut cx.topology.active_masks,
            &mut cx.layouts.dirty_layout_entities,
            &cx.topology.parents,
            &mut cx.renders.dirty_render_entities,
        );

        // 現在ホバー侵入中のドロップターゲット要素を検知
        let found_drop_target = EventStore::detect_drop_target_during_intrusion(
            src_id,
            placeholder_id,
            logical_pos,
            &cx.topology.active_entities,
            &cx.topology.active_masks,
            &cx.topology.flat_dfs_sequence,
            &cx.topology.parents,
            &mut cx.topology.effective_z_indices,
            &mut cx.topology.sorted_entities,
            &cx.renders.visual_properties,
            &cx.renders.base_visual_properties,
            &cx.events.interaction_states,
            &cx.outputs.rects,
            &cx.outputs.clip_rects,
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
            let Some(mask) = cx.topology.active_masks.get(curr_id) else {
                break;
            };

            if mask.has(STYLE_PREVENT_FOCUS_STEAL)
                && curr_id == target_id
                && cx
                    .renders
                    .visual_properties
                    .get(curr_id)
                    .and_then(|v| v.prevent_focus_steal)
                    .unwrap_or(false)
            {
                return true;
            }

            if mask.has(STYLE_PREVENT_FOCUS_STEAL_WITHIN)
                && cx
                    .renders
                    .visual_properties
                    .get(curr_id)
                    .and_then(|v| v.prevent_focus_steal_within)
                    .unwrap_or(false)
            {
                return true;
            }

            curr = cx.topology.parents.get(curr_id).copied().flatten();
        }
        false
    }

    #[allow(clippy::too_many_lines)]
    fn handle_pointer_pressed(cx: &mut Context, button: MouseButton, modifiers: Modifiers) {
        let current_hovered = cx.events.interaction_states.hovered;

        // リサイズドラッグの開始判定（左クリック時のみ）
        if button == MouseButton::Left
            && let Some((id, dir)) = cx.events.active_resize_hover
        {
            EventStore::state_pressed_resize_drag(
                id,
                dir,
                &cx.outputs.rects,
                &mut cx.layouts.basic_layouts,
                &mut cx.layouts.base_basic_layouts,
                &cx.topology.parents,
                cx.events.current_pointer_position,
                &mut cx.events.resizing_state,
                &mut cx.events.interaction_states,
            );
            RenderStore::mark_render_dirty(
                id,
                &mut cx.topology.active_masks,
                &mut cx.renders.dirty_render_entities,
            );
            return; // リサイズ開始時は以降の処理を完全にスキップ
        }

        // スクロールバーのクリック判定
        if let Some(pointer_pos) = cx.events.current_pointer_position
            && let Some(target_id) = current_hovered
        {
            let clicked_scrollbar = EventStore::hit_decision_element_scrollbar(
                target_id,
                pointer_pos,
                &cx.contents.input_contents,
                &cx.system.text_engine,
                &cx.contents.text_contents,
                &cx.renders.visual_properties,
                &mut cx.events.interaction_states,
                &cx.contents.text_spans,
                &cx.system.dwrite_layouts,
                &cx.layouts.basic_layouts,
                &cx.layouts.flex_layouts,
                &cx.layouts.grid_layouts,
                &cx.renders.active_transitions,
                &cx.topology.parents,
                &cx.topology.children,
                &mut cx.topology.active_masks,
                &mut cx.renders.dirty_render_entities,
                &cx.renders.interaction_properties,
                &cx.outputs.rects,
                &mut cx.layouts.scrollbar_styles,
                &mut cx.outputs.scroll_offsets,
                cx.window.last_window_size,
                &cx.layouts.taffy_nodes,
                &mut cx.layouts.taffy,
                &mut cx.layouts.dirty_layout_entities,
            );

            if clicked_scrollbar {
                return; // スクロールバー上の場合は背後への透過を防ぐ
            }
        }

        // 一般要素のプレス
        let Some(target_id) = current_hovered else {
            return;
        };

        cx.events.interaction_states.pressed = Some(target_id);
        EventStore::update_state(cx, target_id, STATE_PRESSED, true);

        // テキスト選択処理
        let user_select = EventStore::get_user_select(target_id, &cx.renders.visual_properties);
        let is_input = cx
            .topology
            .active_masks
            .get(target_id)
            .is_some_and(ComponentMask::has_input_content);

        if user_select == UserSelect::Text
            && !is_input
            && let Some(pointer_pos) = cx.events.current_pointer_position
        {
            EventStore::handle_user_select_text(
                target_id,
                pointer_pos,
                modifiers.shift,
                &cx.contents.text_contents,
                &cx.renders.visual_properties,
                &cx.system.dwrite_layouts,
                &cx.contents.text_spans,
                &cx.system.text_engine,
                &mut cx.outputs.scroll_offsets,
                &cx.contents.input_contents,
                &cx.outputs.rects,
                &cx.layouts.basic_layouts,
                &cx.layouts.flex_layouts,
                &cx.layouts.grid_layouts,
                &mut cx.topology.active_masks,
                &cx.renders.active_transitions,
                &cx.topology.parents,
                &cx.renders.interaction_properties,
                &mut cx.renders.dirty_render_entities,
                &mut cx.outputs.text_selections,
                &mut cx.outputs.selected_rects,
                &mut cx.outputs.selection_start_index,
            );
        }

        // フォーカスの解決
        if !EventStore::should_prevent_focus_steal(cx, target_id) {
            let is_focusable = EventStore::restrict_focusable_element(
                target_id,
                &cx.topology.active_masks,
                &cx.renders.visual_properties,
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
    #[allow(clippy::too_many_lines)]
    fn handle_dnd_drop(cx: &mut Context, drag_state: &ActiveDndDragState) {
        let src_id = drag_state.source_entity;
        let holder = drag_state.placeholder_entity;

        let Some(drag_prop) = cx.events.dnd_drag_properties.get(src_id).copied() else {
            return;
        };

        // 疑似クラスの解除
        EventStore::update_state(cx, src_id, STATE_DND_DRAGGING, false);
        if let Some(target_id) = drag_state.current_drop_target {
            EventStore::update_state(cx, target_id, STATE_DND_DRAG_IN, false);
        }

        cx.events.interaction_states.pressed = None;
        cx.events.interaction_states.dragged = None;

        let drop_success = drag_state.current_drop_target;

        // トポロジー書き換え（要素移動時のみ）
        if let Some(target_id) = drop_success
            && drag_prop.drag_mode == DndDragPayload::Element
            && let Some(_prop) = cx.events.dnd_drop_properties.get(target_id).copied()
        {
            EventStore::remove_dragged_elemet(
                src_id,
                drag_state,
                &cx.layouts.taffy_nodes,
                &mut cx.layouts.taffy,
                &mut cx.topology.active_masks,
                &mut cx.layouts.dirty_layout_entities,
                &cx.topology.parents,
                &mut cx.topology.children,
            );
            EventStore::rewrite_tree_topology(
                src_id,
                target_id,
                holder,
                &drag_prop,
                drag_state,
                &cx.layouts.flex_layouts,
                &mut cx.layouts.basic_layouts,
                &mut cx.layouts.base_basic_layouts,
                &cx.outputs.rects,
                cx.events.current_pointer_position,
                &mut cx.topology.parents,
                &mut cx.topology.children,
                &mut cx.topology.is_structure_dirty,
                &mut cx.topology.active_masks,
                &mut cx.layouts.taffy_nodes,
                &mut cx.layouts.taffy,
                &mut cx.layouts.dirty_layout_entities,
            );
            cx.topology.is_structure_dirty = true;
        }

        // 子要素のツリー構造復元
        if let Some(ph_children) = cx.topology.children.get(holder).cloned() {
            TopologyStore::restore_child(
                src_id,
                holder,
                ph_children,
                &mut cx.topology.parents,
                &mut cx.topology.children,
                &mut cx.layouts.taffy,
                &mut cx.layouts.taffy_nodes,
            );
            if let Some(ph_children_mut) = cx.topology.children.get_mut(holder) {
                ph_children_mut.clear();
            }

            for id in [src_id, holder] {
                LayoutStore::mark_layout_dirty(
                    id,
                    &cx.layouts.taffy_nodes,
                    &mut cx.layouts.taffy,
                    &mut cx.topology.active_masks,
                    &mut cx.layouts.dirty_layout_entities,
                    &cx.topology.parents,
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
            &mut cx.topology,
            &mut cx.layouts,
            &mut cx.renders,
            &mut cx.outputs,
            &mut cx.contents,
            &mut cx.events,
            &mut cx.reactive,
            &mut cx.window,
            &mut cx.system,
        );

        if let Some(pos) = cx.events.current_pointer_position {
            EventStore::pointer_move_inner(cx, pos);
        }

        RenderStore::mark_render_dirty(
            src_id,
            &mut cx.topology.active_masks,
            &mut cx.renders.dirty_render_entities,
        );
    }

    #[inline]
    fn handle_pointer_released(cx: &mut Context, button: MouseButton, modifiers: Modifiers) {
        // リサイズドラッグの終了処理
        if let Some(state) = cx.events.resizing_state.take() {
            let id = state.entity_id;
            cx.events.interaction_states.pressed = None;

            if let Some(pos) = cx.events.current_pointer_position {
                EventStore::pointer_move_inner(cx, pos);
            }
            RenderStore::mark_render_dirty(
                id,
                &mut cx.topology.active_masks,
                &mut cx.renders.dirty_render_entities,
            );
            return;
        }

        // D&D ドラッグ終了・ドロップ確定処理
        if let Some(drag_state) = cx.events.active_dnd_drag_state.take() {
            EventStore::handle_dnd_drop(cx, &drag_state);
            return;
        }

        // スクロールバーの表示更新
        let dirty_ids = EventStore::get_scrollbar_dirty_ids(&mut cx.layouts.scrollbar_styles);
        for id in dirty_ids {
            RenderStore::mark_render_dirty(
                id,
                &mut cx.topology.active_masks,
                &mut cx.renders.dirty_render_entities,
            );
        }

        // 通常要素のリリース
        if let Some(pressed_id) = cx.events.interaction_states.pressed {
            EventStore::update_state(cx, pressed_id, STATE_PRESSED, false);
            EventStore::update_state(cx, pressed_id, STATE_DRAGGED, false);
            cx.events.interaction_states.dragged = None;

            if let Some(contents) = cx.contents.input_contents.get_mut(pressed_id) {
                contents.is_selecting = false;
            }

            // マウスリリースイベントの発火
            handle_on_mouse_input(cx, pressed_id, button, modifiers, ElementState::Released);

            // 同一要素上で離された場合のクリックイベント解決
            if cx.events.interaction_states.hovered == Some(pressed_id) {
                match button {
                    MouseButton::Left => handle_on_click(cx, pressed_id),
                    MouseButton::Right => handle_on_right_click(cx, pressed_id),
                    _ => {}
                }
            }

            cx.events.interaction_states.pressed = None;
        }
    }

    #[inline]
    pub(crate) fn pointer_button_inner(
        cx: &mut Context,
        button: MouseButton,
        state: ElementState,
        modifiers: Modifiers,
    ) {
        let _context_guard = bind_context(cx);

        let current_hovered = cx.events.interaction_states.hovered;

        match state {
            ElementState::Pressed => EventStore::handle_pointer_pressed(cx, button, modifiers),
            ElementState::Released => EventStore::handle_pointer_released(cx, button, modifiers),
        }
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
    #[allow(clippy::too_many_lines, clippy::too_many_arguments)]
    pub(crate) fn hit_decision_element_scrollbar(
        target_id: EntityId,
        pointer_pos: LayoutPoint,
        input_contents: &InputContentsSparseSecondary,
        text_engine: &TextEngine,
        text_contents: &TextContentsSparseSecondary,
        visual_properties: &VisualPropertiesSecondary,
        interaction_states: &mut InteractionStates,
        text_spans: &TextSpansSparseSecondary,
        dwrite_layouts: &DwriteLayoutsSparseSecondary,
        basic_layouts: &BasicLayoutsSecondary,
        flex_layouts: &FlexLayoutsSecondary,
        grid_layouts: &GridLayoutsSecondary,
        active_transitions: &ActiveTransitionsSparseSecondary,
        parents: &ParentsSecondary,
        children: &ChildrenSecondary,
        active_masks: &mut ActiveMasksSecondary,
        dirty_render_entities: &mut DirtyRenderEntitiesVec,
        interaction_properties: &InteractionPropertiesSecondary,
        rects: &RectsSecondary,
        scrollbar_styles: &mut ScrollbarStylesSecondary,
        scroll_offsets: &mut ScrollOffsetsSecondary,
        last_window_size: Option<LayoutSize>,
        taffy_nodes: &TaffyNodesSecondary,
        taffy: &mut TaffyTreeEntityId,
        dirty_layout_entities: &mut DirtyLayoutEntitiesVec,
    ) -> bool {
        let Some((c_id, component)) = scrollbar_styles.iter().find_map(|(c_id, sb_state)| {
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
        let sb_state = scrollbar_styles.get(c_id).cloned().unwrap();
        let container_rect = OutputStore::rect(c_id, rects).unwrap_or_default();
        let scroll_size = OutputStore::get_scroll_size(
            c_id,
            active_masks,
            input_contents,
            text_engine,
            text_contents,
            visual_properties,
            text_spans,
            dwrite_layouts,
            basic_layouts,
            flex_layouts,
            grid_layouts,
            active_transitions,
            parents,
            children,
            interaction_properties,
            rects,
            scrollbar_styles,
            scroll_offsets,
        );
        let offset = scroll_offsets.get(c_id).copied().unwrap_or_default();

        match component {
            ScrollbarComponent::VThumb | ScrollbarComponent::HThumb => {
                // サムをクリックした場合：ドラッグを開始
                if let Some(st) = scrollbar_styles.get_mut(c_id) {
                    if component == ScrollbarComponent::VThumb {
                        st.v_thumb_dragged = true;
                    } else {
                        st.h_thumb_dragged = true;
                    }
                    st.drag_start_mouse = pointer_pos;
                    st.drag_start_offset = offset;
                }
                interaction_states.pressed = Some(target_id);
                RenderStore::mark_render_dirty(target_id, active_masks, dirty_render_entities);
            }
            ScrollbarComponent::VTrack | ScrollbarComponent::HTrack => {
                // レールをクリックした場合：ダイレクトジャンプスクロールを実行
                let is_vertical = component == ScrollbarComponent::VTrack;
                let thumb_id = if is_vertical {
                    sb_state.v_thumb_id
                } else {
                    sb_state.h_thumb_id
                };

                let track_rect = OutputStore::rect(target_id, rects).unwrap_or_default();
                let thumb_rect = thumb_id
                    .and_then(|i| OutputStore::rect(i, rects))
                    .unwrap_or_default();
                let visible_size =
                    WindowStore::calculate_visible_size(last_window_size, container_rect);

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
                    active_masks,
                    input_contents,
                    text_engine,
                    text_contents,
                    visual_properties,
                    text_spans,
                    dwrite_layouts,
                    basic_layouts,
                    flex_layouts,
                    grid_layouts,
                    active_transitions,
                    parents,
                    children,
                    interaction_properties,
                    rects,
                    scrollbar_styles,
                    scroll_offsets,
                    last_window_size,
                    taffy_nodes,
                    taffy,
                    dirty_layout_entities,
                );

                let new_offset = scroll_offsets.get(c_id).copied().unwrap_or_default();
                if let Some(st) = scrollbar_styles.get_mut(c_id) {
                    if is_vertical {
                        st.v_thumb_dragged = true;
                    } else {
                        st.h_thumb_dragged = true;
                    }
                    st.drag_start_mouse = pointer_pos;
                    st.drag_start_offset = new_offset;
                }

                interaction_states.pressed = thumb_id;
                if let Some(tid) = thumb_id {
                    RenderStore::mark_render_dirty(tid, active_masks, dirty_render_entities);
                }
            }
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_user_select_text(
        id: EntityId,
        pointer_pos: LayoutPoint,
        pressed_shift: bool,
        text_contents: &TextContentsSparseSecondary,
        visual_properties: &VisualPropertiesSecondary,
        dwrite_layouts: &DwriteLayoutsSparseSecondary,
        text_spans: &TextSpansSparseSecondary,
        text_engine: &TextEngine,
        scroll_offsets: &mut ScrollOffsetsSecondary,
        input_contents: &InputContentsSparseSecondary,
        rects: &RectsSecondary,
        basic_layouts: &BasicLayoutsSecondary,
        flex_layouts: &FlexLayoutsSecondary,
        grid_layouts: &GridLayoutsSecondary,
        active_masks: &mut ActiveMasksSecondary,
        active_transitions: &ActiveTransitionsSparseSecondary,
        parents: &ParentsSecondary,
        interaction_properties: &InteractionPropertiesSecondary,
        dirty_render_entities: &mut DirtyRenderEntitiesVec,
        text_selections: &mut TextSelectionsSparseSecondary,
        selected_rects: &mut SelectedRectsSparseSecondary,
        selection_start_index: &mut SelectionStartIndexSparseSecondary,
    ) {
        let Some(layout) = SystemStore::get_or_create_layout(
            id,
            text_contents,
            visual_properties,
            dwrite_layouts,
            text_spans,
            text_engine,
        ) else {
            return;
        };

        let local = EventStore::pressed_local_point(
            id,
            pointer_pos,
            scroll_offsets,
            input_contents,
            rects,
            basic_layouts,
            flex_layouts,
            grid_layouts,
            active_masks,
            active_transitions,
            parents,
            interaction_properties,
            visual_properties,
        );
        let (clicked_index, is_trailing) = text_engine.hit_test_point(&layout, local.x, local.y);
        let final_index = if is_trailing {
            clicked_index + 1
        } else {
            clicked_index
        };

        if pressed_shift {
            // 共通の Shift選択拡張
            let anchor = selection_start_index
                .get(id)
                .copied()
                .unwrap_or(final_index);
            if !selection_start_index.contains_key(id) {
                selection_start_index.insert(id, final_index);
            }
            let range = if anchor <= final_index {
                anchor..final_index
            } else {
                final_index..anchor
            };
            text_selections.insert(id, range);
            OutputStore::update_selection_rects(id, &layout, text_selections, selected_rects);
        } else {
            // 共通の通常クリックリセット
            selection_start_index.insert(id, final_index);
            text_selections.insert(id, final_index..final_index);
            selected_rects.remove(id);
        }

        RenderStore::mark_render_dirty(id, active_masks, dirty_render_entities);
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
        EventStore::update_state(cx, id, STATE_FOCUSED, focused);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn auto_focus_switch_by_trigger(
        cx: &mut Context,
        id: EntityId,
        trigger: ActiveFocusTrigger,
    ) {
        // 同一要素をクリックした場合はフォーカス可視化の同期のみ
        if cx.events.interaction_states.focused == Some(id) {
            EventStore::set_focused_by_trigger(cx, id, true, trigger);
            return;
        }

        if let Some(old_focus_id) = cx.events.interaction_states.focused {
            EventStore::set_focused_by_trigger(cx, old_focus_id, false, trigger);

            // 古いフォーカス要素の選択範囲とハイライト矩形をクリア
            OutputStore::clear_selection_highlight_rect(
                old_focus_id,
                &mut cx.topology.active_masks,
                &mut cx.outputs.text_selections,
                &mut cx.outputs.selected_rects,
                &mut cx.contents.input_contents,
                &mut cx.contents.text_spans,
            );
            // 進行中の IME コンポジションを強制的に確定させ候補窓を閉じる
            SystemStore::force_complete_ime_composition();

            handle_on_blur(cx, old_focus_id);
        }

        // 新しいフォーカス可能要素にフォーカスを設定
        EventStore::set_focused_by_trigger(cx, id, true, trigger);

        // 新しいフォーカス先が is_ime(false) の場合は IME 関連付けを解除
        let is_input = cx.topology.active_masks[id].has_input_content();
        if is_input && let Some(contents) = cx.contents.input_contents.get(id) {
            SystemStore::unassociate_ime(contents, &mut cx.window.default_himc);
        } else {
            // インプット以外の場合は IME をデフォルト状態に戻す
            SystemStore::reset_ime_default_state(cx.window.default_himc.as_ref());
        }

        cx.events.interaction_states.focused = Some(id);

        handle_on_focus(cx, id);
    }

    #[inline]
    pub(crate) fn handle_remove_focus(cx: &mut Context) {
        let Some(old_focus_id) = cx.events.interaction_states.focused else {
            return;
        };

        // 先にフォーカス状態を解除しておく
        // コールバック内で再フォーカスされても上書きしないため
        cx.events.interaction_states.focused = None;

        EventStore::set_focused_by_trigger(cx, old_focus_id, false, ActiveFocusTrigger::Mouse);

        // 古いフォーカス要素の選択範囲とハイライト矩形をクリア
        OutputStore::clear_selection_highlight_rect(
            old_focus_id,
            &mut cx.topology.active_masks,
            &mut cx.outputs.text_selections,
            &mut cx.outputs.selected_rects,
            &mut cx.contents.input_contents,
            &mut cx.contents.text_spans,
        );
        // 進行中の IME コンポジションを強制的に確定させ候補窓を閉じる
        SystemStore::force_complete_ime_composition();

        // IME をデフォルトの有効化状態に戻す
        SystemStore::reset_ime_default_state(cx.window.default_himc.as_ref());

        handle_on_blur(cx, old_focus_id);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn rewrite_tree_topology(
        src_id: EntityId,
        target_id: EntityId,
        holder: EntityId,
        drag_prop: &DndDragProperty,
        drag_state: &ActiveDndDragState,
        flex_layouts: &FlexLayoutsSecondary,
        basic_layouts: &mut BasicLayoutsSecondary,
        base_basic_layouts: &mut BaseBasicLayoutsSecondary,
        rects: &RectsSecondary,
        current_pointer_position: Option<LayoutPoint>,
        parents: &mut ParentsSecondary,
        children: &mut ChildrenSecondary,
        is_structure_dirty: &mut bool,
        active_masks: &mut ActiveMasksSecondary,
        taffy_nodes: &mut TaffyNodesSecondary,
        taffy: &mut TaffyTreeEntityId,
        dirty_layout_entities: &mut DirtyLayoutEntitiesVec,
    ) {
        // ドラッグ元要素の配置（Position）の取得
        let position = basic_layouts
            .get(src_id)
            .map(|l| l.position)
            .unwrap_or_default();

        if position == Position::Absolute {
            // 絶対配置: 位置移動（補正）を伴うアタッチ
            if drag_prop.update_position {
                // プレースホルダーの最終的な絶対画面座標を取得
                let ph_abs_rect = OutputStore::rect(holder, rects).unwrap_or_default();
                // 新しい親（target_id）の絶対画面座標とボーダー厚みを取得
                let target_rect = OutputStore::rect(target_id, rects).unwrap_or_default();
                let (border_l, border_t) = if let Some(basic) = basic_layouts.get(target_id) {
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

                if let Some(basic) = basic_layouts.get_mut(src_id) {
                    basic.inset = new_inset;
                }
                if let Some(base_basic) = base_basic_layouts.get_mut(src_id) {
                    base_basic.inset = new_inset;
                }
            }

            // ドロップ先コンテナ（target_id）の末尾の子要素としてマウント
            TopologyStore::add_child(
                target_id,
                src_id,
                parents,
                children,
                is_structure_dirty,
                active_masks,
                taffy_nodes,
                taffy,
                dirty_layout_entities,
            );

            return;
        }
        // 相対配置: マウス座標に基づいた子要素の動的並び替えアタッチ
        if drag_prop.update_position {
            let mouse_pos = current_pointer_position.unwrap_or_default();
            let insert_idx = TopologyStore::calculate_insert_index(
                target_id,
                mouse_pos,
                children,
                flex_layouts,
                rects,
            );

            if let Some(parent_children) = children.get_mut(target_id) {
                // 算出されたインデックス位置へ挿入
                parent_children.insert(insert_idx, src_id);
            }
            parents.insert(src_id, Some(target_id));

            // Taffy 側のノード順序を物理並び替え結果に沿って一括して再同期
            LayoutStore::resync_taffy_children_order(target_id, taffy_nodes, taffy, children);
        } else {
            // 自動更新オフの場合は末尾に通常アタッチ
            TopologyStore::add_child(
                target_id,
                src_id,
                parents,
                children,
                is_structure_dirty,
                active_masks,
                taffy_nodes,
                taffy,
                dirty_layout_entities,
            );
        }
        LayoutStore::mark_layout_dirty(
            target_id,
            taffy_nodes,
            taffy,
            active_masks,
            dirty_layout_entities,
            parents,
        );
    }
}

impl Context {
    
    #[inline]
    pub(crate) fn auto_focus_switch(&mut self, id: EntityId) {
        self.auto_focus_switch_by_trigger(id, ActiveFocusTrigger::Mouse);
    }

    #[inline]
    pub(crate) fn auto_focus_switch_by_trigger(
        &mut self,
        id: EntityId,
        trigger: ActiveFocusTrigger,
    ) {
        EventStore::auto_focus_switch_by_trigger(self, id, trigger);
    }

    #[inline]
    pub(crate) fn get_user_select(&self, id: EntityId) -> UserSelect {
        let RenderStore {
            visual_properties, ..
        } = &self.renders;

        EventStore::get_user_select(id, visual_properties)
    }
}
