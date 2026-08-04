use std::path::PathBuf;

use crate::{
    ActiveFocusTrigger, ActiveMasksSecondary, ActiveTransitionsSparseSecondary,
    BaseBasicLayoutsSecondary, BasicLayout, BasicLayoutsSecondary, ChildrenSecondary,
    ClipRectsSecondary, Context, CursorIcon, DirtyLayoutEntitiesVec, DragPayload,
    DragPlaceholderParent, DragProperty, DropProperty, DwriteLayoutsSparseSecondary, Element,
    ElementState, EntityId, EventListeners, FlexLayoutsSecondary, FocusTrigger, Focusable,
    GridLayoutsSecondary, InputContentsSparseSecondary, InteractionPropertiesSecondary,
    InteractionStates, LayoutPoint, LayoutRect, LayoutSize, LayoutStore, Length, Modifiers,
    MouseButton, OutputStore, ParentsSecondary, PointerEvents, Position, Rect, RectsSecondary,
    RenderStore, STATE_ACTIVED, STATE_DISABLED, STATE_DRAG_IN, STATE_DRAG_OVER, STATE_DRAGGING,
    STATE_SELECTED, STYLE_DRAGGABLE, STYLE_DROPPABLE, STYLE_INTERACTION_PARENT,
    STYLE_INTERACTION_WITHIN, STYLE_POINTER_EVENTS, STYLE_RESIZABLE, ScrollOffsetsSecondary,
    ScrollbarStylesSecondary, SystemStore, TaffyNodesSecondary, TaffyTreeEntityId, TextAlign,
    TextContentsSparseSecondary, TextEngine, TextSpansSparseSecondary, TopologyStore, UserSelect,
    Val, VirtualKey, VisualPropertiesSecondary,
};
use slotmap::{SecondaryMap, SparseSecondaryMap};
use smallvec::SmallVec;

#[derive(Debug, Clone)]
pub(crate) struct ActiveDragState {
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
pub(crate) type DragPropertiesSparseSecondary = SparseSecondaryMap<EntityId, DragProperty>;
pub(crate) type DropPropertiesSparseSecondary = SparseSecondaryMap<EntityId, DropProperty>;

pub struct EventStore {
    pub(crate) event_listeners: EventListenersSparseSecondary,
    pub interaction_states: InteractionStates,
    pub(crate) current_pointer_position: Option<LayoutPoint>,
    pub(crate) resizing_state: Option<ResizingState>,
    pub(crate) active_resize_hover: ActiveResizeHoverOption,
    pub(crate) drag_properties: DragPropertiesSparseSecondary,
    pub(crate) drop_properties: DropPropertiesSparseSecondary,
    pub(crate) active_drag_state: Option<ActiveDragState>,
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
            drag_properties: SparseSecondaryMap::new(),
            drop_properties: SparseSecondaryMap::new(),
            active_drag_state: None,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.event_listeners.clear();
        self.interaction_states = InteractionStates::new();
        self.current_pointer_position = None;
        self.resizing_state = None;
        self.active_resize_hover = None;
        self.drag_properties.clear();
        self.drop_properties.clear();
        self.active_drag_state = None;
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.event_listeners.remove(id);
        self.interaction_states.clear_entity(id);
        self.drag_properties.remove(id);
        self.drop_properties.remove(id);

        if let Some(ref state) = self.active_drag_state
            && (state.source_entity == id || state.placeholder_entity == id)
        {
            self.active_drag_state = None;
        }
    }
}

impl EventStore {
    pub(crate) fn resolve_placeholder_parent(
        root: EntityId,
        drag_prop: DragProperty,
        rects: &RectsSecondary,
        basic_layouts: &BasicLayoutsSecondary,
    ) -> PlaceholderAttachment {
        match drag_prop.placeholder_parent {
            DragPlaceholderParent::Root => PlaceholderAttachment {
                parent_id: Some(root),
                rect: OutputStore::rect(root, rects).unwrap_or_default(),
                border_left: 0.0,
                border_top: 0.0,
            },
            DragPlaceholderParent::Custom(p_id) => {
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
                let direction = Context::detect_resize_direction(
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
        pointer_pos: &LayoutPoint,
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
        let distance = EventStore::drag_overhang_distance(&pointer_pos, &clip);
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
}

impl Context {
    /// リサイズ方向から対応するカーソル種別へ変換するヘルパー
    #[inline]
    pub(crate) fn resize_direction_to_cursor(dir: ResizeDirection) -> CursorIcon {
        EventStore::resize_direction_to_cursor(dir)
    }

    /// マウス位置と要素の境界・リサイズ許可フラグから、該当するリサイズ方向を算出するヘルパー
    #[inline]
    pub(crate) fn detect_resize_direction(
        rect: LayoutRect,
        resizable: [bool; 4], // [top, right, bottom, left]
        pos: LayoutPoint,
        border: f32,
    ) -> Option<ResizeDirection> {
        EventStore::detect_resize_direction(rect, resizable, pos, border)
    }

    #[inline]
    pub(crate) fn resolve_placeholder_parent(
        &self,
        root: EntityId,
        drag_prop: DragProperty,
    ) -> PlaceholderAttachment {
        let OutputStore { rects, .. } = &self.outputs;
        let LayoutStore { basic_layouts, .. } = &self.layouts;

        EventStore::resolve_placeholder_parent(root, drag_prop, rects, basic_layouts)
    }

    #[inline]
    pub(crate) fn apply_resizable_cursor_style(&mut self, id: EntityId, dir: ResizeDirection) {
        let RenderStore {
            visual_properties, ..
        } = &mut self.renders;

        EventStore::apply_resizable_cursor_style(id, dir, visual_properties);
    }

    #[inline]
    pub(crate) fn found_resize_hover(
        &mut self,
        target_id: Option<EntityId>,
        logical_pos: LayoutPoint,
    ) -> (Option<EntityId>, Option<(EntityId, ResizeDirection)>) {
        let TopologyStore {
            active_masks,
            parents,
            ..
        } = &self.topology;
        let OutputStore { rects, .. } = &self.outputs;
        let LayoutStore { basic_layouts, .. } = &self.layouts;

        EventStore::found_resize_hover(
            target_id,
            logical_pos,
            active_masks,
            parents,
            rects,
            basic_layouts,
        )
    }

    #[inline]
    pub(crate) fn pressed_local_point(
        &self,
        pressed_id: EntityId,
        logical_pos: LayoutPoint,
    ) -> LayoutPoint {
        let rect = self.rect(pressed_id).unwrap_or_default();
        let (basic, flex, _) = self.resolve_active_layouts(pressed_id);
        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

        let scroll = self
            .outputs
            .scroll_offsets
            .get(pressed_id)
            .copied()
            .unwrap_or(LayoutPoint::ZERO);

        let text_size = if let Some(contents) = self.contents.input_contents.get(pressed_id)
            && let Some(layout_rect) = contents.last_layout
        {
            LayoutSize::new(layout_rect.width, layout_rect.height)
        } else {
            LayoutSize::ZERO
        };

        let content_w =
            (rect.width - border.left - border.right - padding.left - padding.right).max(0.0);
        let align_offset_x = match flex.text_align {
            TextAlign::Center => ((content_w - text_size.width) * 0.5).max(0.0),
            TextAlign::Right => (content_w - text_size.width).max(0.0),
            _ => 0.0,
        };

        let content_h =
            (rect.height - border.top - border.bottom - padding.top - padding.bottom).max(0.0);
        let align_offset_y = ((content_h - text_size.height) * 0.5).max(0.0);

        let local_x =
            logical_pos.x - (rect.x + border.left + padding.left + align_offset_x) + scroll.x;
        let local_y =
            logical_pos.y - (rect.y + border.top + padding.top + align_offset_y) + scroll.y;
        LayoutPoint {
            x: local_x,
            y: local_y,
        }
    }

    pub(crate) fn resolve_hover_state(&mut self, target_id: Option<EntityId>) {
        // 旧ホバー要素からマウスが去った
        if let Some(old_id) = self.events.interaction_states.hovered {
            self.set_hovered(old_id, false);

            if let Some(mut listeners) = self.events.event_listeners.get_mut(old_id)
                && let Some(mut handler) = listeners.on_mouse_leave.take()
            {
                let _guard = crate::ActiveElementGuard::new(old_id);
                handler(self);
                if let Some(l) = self.events.event_listeners.get_mut(old_id) {
                    l.on_mouse_leave = Some(handler);
                }
            }
        }

        // 新ホバー要素にマウスが入った
        if let Some(new_id) = target_id {
            self.set_hovered(new_id, true);

            if let Some(mut listeners) = self.events.event_listeners.get_mut(new_id)
                && let Some(mut handler) = listeners.on_mouse_enter.take()
            {
                let _guard = crate::ActiveElementGuard::new(new_id);
                handler(self);
                if let Some(l) = self.events.event_listeners.get_mut(new_id) {
                    l.on_mouse_enter = Some(handler);
                }
            }

            if let Some(mut listeners) = self.events.event_listeners.get_mut(new_id)
                && let Some(mut handler) = listeners.on_hover.take()
            {
                let _guard = crate::ActiveElementGuard::new(new_id);
                handler(self);
                if let Some(l) = self.events.event_listeners.get_mut(new_id) {
                    l.on_hover = Some(handler);
                }
            }
        }

        self.events.interaction_states.hovered = target_id;
    }

    #[inline]
    pub(crate) fn propagate_cursor_move_events(
        &mut self,
        target_id: Option<EntityId>,
        logical_pos: LayoutPoint,
    ) {
        if let Some(target_id) = target_id
            && let Some(mut listeners) = self.events.event_listeners.get_mut(target_id)
            && let Some(mut handler) = listeners.on_cursor_moved.take()
        {
            let rect = self
                .outputs
                .rects
                .get(target_id)
                .copied()
                .unwrap_or_default();
            let relative_pos = LayoutPoint::new(logical_pos.x - rect.x, logical_pos.y - rect.y);
            let _guard = crate::ActiveElementGuard::new(target_id);
            handler(self, relative_pos);
            if let Some(l) = self.events.event_listeners.get_mut(target_id) {
                l.on_cursor_moved = Some(handler);
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    #[inline]
    pub(crate) fn propagate_drag_events(
        &mut self,
        prev_pos: Option<LayoutPoint>,
        logical_pos: LayoutPoint,
    ) {
        if let Some(pressed_id) = self.events.interaction_states.pressed
            && let Some(prev) = prev_pos
        {
            let delta = LayoutPoint::new(logical_pos.x - prev.x, logical_pos.y - prev.y);
            if delta.x != 0.0 || delta.y != 0.0 {
                self.set_dragged(pressed_id, true);
                self.events.interaction_states.dragged = Some(pressed_id);

                // D&D 設定（STYLE_DRAGGABLE）を持っている場合のセッションのキック
                if self.topology.active_masks[pressed_id].has(STYLE_DRAGGABLE)
                    && self.events.active_drag_state.is_none()
                {
                    let drag_prop = self
                        .events
                        .drag_properties
                        .get(pressed_id)
                        .copied()
                        .unwrap();
                    let start_rect = self.outputs.rects[pressed_id];

                    // 開始時のクリック位置と要素左上の相対的なズレを計算
                    let click_offset = LayoutPoint::new(
                        logical_pos.x - start_rect.x,
                        logical_pos.y - start_rect.y,
                    );

                    // ウィンドウの真のルート要素をライブラリ側で自己解決
                    let root = self
                        .find_root_entity()
                        .expect("Root EntityId not found in Context");

                    // プレースホルダーアタッチ先親要素の決定
                    let placeholder = self.resolve_placeholder_parent(root, drag_prop);

                    // プレースホルダー（クローン）をアタッチ先親の直下へ spawn して生成
                    let placeholder_id = self.spawn(placeholder.parent_id);
                    if let Some(p_id) = placeholder.parent_id {
                        self.add_child(p_id, placeholder_id);
                    }

                    // 元要素のレイアウトおよびビジュアル情報をコピーして初期マウント
                    if let Some(basic) = self.layouts.base_basic_layouts.get(pressed_id).copied() {
                        self.layouts
                            .base_basic_layouts
                            .insert(placeholder_id, basic);
                        self.layouts.basic_layouts.insert(placeholder_id, basic);
                    }
                    if let Some(visual) =
                        self.renders.base_visual_properties.get(pressed_id).cloned()
                    {
                        self.renders
                            .base_visual_properties
                            .insert(placeholder_id, visual.clone());
                        self.renders
                            .visual_properties
                            .insert(placeholder_id, visual);
                    }
                    if let Some(interaction) =
                        self.renders.interaction_properties.get(pressed_id).cloned()
                    {
                        self.renders
                            .interaction_properties
                            .insert(placeholder_id, interaction);
                    }

                    // ドラッグ元の元の要素は非可視（または半透明）にするため STATE_DRAGGING 状態をセット
                    self.set_drag_state(pressed_id, STATE_DRAGGING, true);

                    // プレースホルダー側は absolute 配置化し、STATE_DRAG_OVER 状態をセット
                    self.set_drag_state(placeholder_id, STATE_DRAG_OVER, true);
                    if let Some(layout) = self.layouts.basic_layouts.get_mut(placeholder_id) {
                        layout.position = Position::Absolute;
                        layout.size.width = Val::Px(start_rect.width);
                        layout.size.height = Val::Px(start_rect.height);
                    }
                    if let Some(layout) = self.layouts.base_basic_layouts.get_mut(placeholder_id) {
                        layout.position = Position::Absolute;
                        layout.size.width = Val::Px(start_rect.width);
                        layout.size.height = Val::Px(start_rect.height);
                    }

                    // 元の要素が持つ本物の子要素トポロジーを、一時的にプレースホルダー配下へ自動アタッチ
                    if let Some(src_children) = self.topology.children.get(pressed_id).cloned() {
                        for child_id in src_children {
                            // 子要素の親ポインタをプレースホルダーに付け替え
                            self.topology.parents.insert(child_id, Some(placeholder_id));

                            // プレースホルダー側の子要素リストへ追加
                            if let Some(ph_children) =
                                self.topology.children.get_mut(placeholder_id)
                            {
                                ph_children.push(child_id);
                            }

                            // Taffy 側の親子構造も、一時的にプレースホルダーに繋ぎ替え
                            if let Some(&src_node) = self.layouts.taffy_nodes.get(pressed_id)
                                && let Some(&ph_node) = self.layouts.taffy_nodes.get(placeholder_id)
                                && let Some(&child_node) = self.layouts.taffy_nodes.get(child_id)
                            {
                                let _ = self.layouts.taffy.remove_child(src_node, child_node);
                                let _ = self.layouts.taffy.add_child(ph_node, child_node);
                            }
                        }

                        // 元の要素の子要素リストは一時的にクリア（プレースホルダーに避難しているため）
                        if let Some(src_children_mut) = self.topology.children.get_mut(pressed_id) {
                            src_children_mut.clear();
                        }
                        self.mark_layout_dirty(pressed_id);
                        self.mark_layout_dirty(placeholder_id);
                    }

                    // プレースホルダー自体はヒットテストを完全に透過させる
                    if let Some(vis) = self.renders.visual_properties.get_mut(placeholder_id) {
                        vis.pointer_events = Some(PointerEvents::None);
                    }
                    if let Some(vis) = self.renders.base_visual_properties.get_mut(placeholder_id) {
                        vis.pointer_events = Some(PointerEvents::None);
                    }
                    if let Some(mask) = self.topology.active_masks.get_mut(placeholder_id) {
                        mask.set(STYLE_POINTER_EVENTS);
                    }

                    // プレースホルダーアタッチ前の、本当の元の親要素のIDを安全に記録
                    let original_parent = self.topology.parents.get(pressed_id).copied().flatten();

                    // セッション開始
                    self.events.active_drag_state = Some(ActiveDragState {
                        source_entity: pressed_id,
                        placeholder_entity: placeholder_id,
                        current_drop_target: None,
                        start_mouse_pos: logical_pos,
                        start_rect,
                        click_offset,
                        original_parent,
                    });

                    // ドラッグ開始コールバックに、Original(pressed_id) と Placeholder(placeholder_id) の両ハンドルを渡して実行

                    if let Some(mut l) = self.events.event_listeners.get_mut(pressed_id)
                        && let Some(mut listener) = l.on_drag_start.take()
                    {
                        {
                            let _guard = crate::ActiveElementGuard::new(pressed_id);
                            listener(
                                self,
                                Element::from(pressed_id),
                                Element::from(placeholder_id),
                            );
                        }
                        if let Some(l) = self.events.event_listeners.get_mut(pressed_id) {
                            l.on_drag_start = Some(listener);
                        }
                    }
                }

                if let Some(mut l) = self.events.event_listeners.get_mut(pressed_id)
                    && let Some(mut handler) = l.on_drag.take()
                {
                    let _guard = crate::ActiveElementGuard::new(pressed_id);
                    handler(self, delta);
                    if let Some(l) = self.events.event_listeners.get_mut(pressed_id) {
                        l.on_drag = Some(handler);
                    }
                }
            }
        }
    }

    pub(crate) fn calculate_relative_local(
        &self,
        root: EntityId,
        drag_prop: DragProperty,
    ) -> (LayoutRect, f32, f32) {
        let OutputStore { rects, .. } = &self.outputs;
        let LayoutStore { basic_layouts, .. } = &self.layouts;

        let p = EventStore::resolve_placeholder_parent(root, drag_prop, rects, basic_layouts);
        (p.rect, p.border_left, p.border_top)
    }

    #[inline]
    pub(crate) fn update_inset_based_relative_local(
        &mut self,
        root: EntityId,
        placeholder: EntityId,
        logical_pos: LayoutPoint,
        drag_prop: DragProperty,
        drag_state: &ActiveDragState,
    ) {
        // アタッチ先親コンテナ基準での相対ローカル座標を逆算して追従（Inset更新）
        let (parent_rect, b_l, b_t) = self.calculate_relative_local(root, drag_prop);

        // マウスのドラッグ開始時クリックオフセットを用いて、ローカル Top-Left 座標を算出
        let local_x = logical_pos.x - (parent_rect.x + b_l) - drag_state.click_offset.x;
        let local_y = logical_pos.y - (parent_rect.y + b_t) - drag_state.click_offset.y;

        if let Some(layout) = self.layouts.basic_layouts.get_mut(placeholder) {
            layout.inset.left = Val::Px(local_x);
            layout.inset.top = Val::Px(local_y);
            layout.inset.right = Val::Auto;
            layout.inset.bottom = Val::Auto;
        }
        if let Some(layout) = self.layouts.base_basic_layouts.get_mut(placeholder) {
            layout.inset.left = Val::Px(local_x);
            layout.inset.top = Val::Px(local_y);
            layout.inset.right = Val::Auto;
            layout.inset.bottom = Val::Auto;
        }

        self.mark_layout_dirty(placeholder);
        self.mark_render_dirty(placeholder);
    }

    pub(crate) fn detect_drop_target_during_intrusion(
        &self,
        src_id: EntityId,
        placeholder: EntityId,
        logical_pos: LayoutPoint,
    ) -> Option<EntityId> {
        let hit_id_opt = self.hit_test(logical_pos);
        let mut found_drop_target = None;

        if let Some(hit_id) = hit_id_opt {
            let mut current_id = Some(hit_id);
            while let Some(id) = current_id {
                // ヒットした要素がドラッグ元（src_id）自身、またはその子孫である場合は
                // ドロップ先として誤認されるのを完全に防ぐため、スルーしてさらに上の親を辿る
                if id == src_id
                    || TopologyStore::is_descendant_of(id, src_id, &self.topology.parents)
                {
                    current_id = self.topology.parents.get(id).copied().flatten();
                    continue;
                }

                if id != placeholder && self.topology.active_masks[id].has(STYLE_DROPPABLE) {
                    found_drop_target = Some(id);
                    break;
                }
                current_id = self.topology.parents.get(id).copied().flatten();
            }
        }
        found_drop_target
    }

    pub(crate) fn sync_state_drag_in(
        &mut self,
        drag_state: &mut ActiveDragState,
        found_drop_target: Option<EntityId>,
    ) {
        if found_drop_target != drag_state.current_drop_target {
            if let Some(old_target) = drag_state.current_drop_target {
                self.set_drag_state(old_target, STATE_DRAG_IN, false);
            }
            if let Some(new_target) = found_drop_target {
                self.set_drag_state(new_target, STATE_DRAG_IN, true);
            }
            drag_state.current_drop_target = found_drop_target;
            self.events.active_drag_state = Some(drag_state.clone());
        }
    }

    pub(crate) fn callback_drag_prop(
        &mut self,
        src_id: EntityId,
        found_drop_target: Option<EntityId>,
        drag_prop: DragProperty,
    ) {
        match drag_prop.drag_mode {
            DragPayload::Element => {
                if let Some(l) = self.events.event_listeners.get_mut(src_id)
                    && let Some(mut listener) = l.on_entity_drag.take()
                {
                    {
                        let _guard = crate::ActiveElementGuard::new(src_id);
                        listener(
                            self,
                            Element::from(src_id),
                            found_drop_target.map(Element::from),
                        );
                    }
                    // 再度元の場所へ戻す
                    if let Some(l) = self.events.event_listeners.get_mut(src_id) {
                        l.on_entity_drag = Some(listener);
                    }
                }
            }
            DragPayload::EntityId => {
                if let Some(l) = self.events.event_listeners.get_mut(src_id)
                    && let Some(mut listener) = l.on_id_drag.take()
                {
                    {
                        let _guard = crate::ActiveElementGuard::new(src_id);
                        listener(self, src_id, found_drop_target);
                    }
                    if let Some(l) = self.events.event_listeners.get_mut(src_id) {
                        l.on_id_drag = Some(listener);
                    }
                }
            }
        }
    }

    pub(crate) fn handle_text_selection_click(
        &mut self,
        id: EntityId,
        start_pos: usize,
        local: LayoutPoint,
    ) {
        let Some(layout) = self.get_or_create_layout(id) else {
            return;
        };
        let (current_index, is_trailing) = self
            .system
            .text_engine
            .hit_test_point(&layout, local.x, local.y);
        let final_index = if is_trailing {
            current_index + 1
        } else {
            current_index
        };

        let range = if start_pos <= final_index {
            // 順選択（右方向ドラッグ）
            if let Some(contents) = self.contents.input_contents.get_mut(id) {
                contents.selection_reversed = false;
            }
            start_pos..final_index
        } else {
            // 逆選択（左方向ドラッグ）
            if let Some(contents) = self.contents.input_contents.get_mut(id) {
                contents.selection_reversed = true;
            }
            final_index..start_pos
        };

        self.outputs.text_selections.insert(id, range.clone());

        self.update_selection_rects(id);

        if let Some(contents) = self.contents.input_contents.get_mut(id) {
            contents.selected_range = range;
            self.update_input_caret_position(id);
        }
        self.mark_render_dirty(id);
    }

    #[inline]
    pub(crate) fn state_pressed_resize_drag(&mut self, id: EntityId, dir: ResizeDirection) {
        let OutputStore { rects, .. } = &self.outputs;
        let LayoutStore {
            basic_layouts,
            base_basic_layouts,
            ..
        } = &mut self.layouts;
        let TopologyStore { parents, .. } = &self.topology;
        let EventStore {
            current_pointer_position,
            resizing_state,
            interaction_states,
            ..
        } = &mut self.events;

        EventStore::state_pressed_resize_drag(
            id,
            dir,
            rects,
            basic_layouts,
            base_basic_layouts,
            parents,
            *current_pointer_position,
            resizing_state,
            interaction_states,
        );
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn hit_decision_element_scrollbar(
        &mut self,
        target_id: EntityId,
        pointer_pos: LayoutPoint,
    ) -> bool {
        let mut clicked_scrollbar = false;
        let mut parent_container = None;
        let mut is_v_thumb = false;
        let mut is_h_thumb = false;
        let mut is_v_track = false;
        let mut is_h_track = false;

        for (c_id, sb_state) in &self.layouts.scrollbar_styles {
            if sb_state.v_thumb_id == Some(target_id) {
                parent_container = Some(c_id);
                is_v_thumb = true;
                break;
            } else if sb_state.h_thumb_id == Some(target_id) {
                parent_container = Some(c_id);
                is_h_thumb = true;
                break;
            } else if sb_state.v_track_id == Some(target_id) {
                parent_container = Some(c_id);
                is_v_track = true;
                break;
            } else if sb_state.h_track_id == Some(target_id) {
                parent_container = Some(c_id);
                is_h_track = true;
                break;
            }
        }

        if let Some(c_id) = parent_container {
            clicked_scrollbar = true;

            let (sb_state, container_rect, scroll_size) = {
                let sb_state = self.layouts.scrollbar_styles.get(c_id).cloned().unwrap();
                let container_rect = self
                    .outputs
                    .rects
                    .get(c_id)
                    .copied()
                    .unwrap_or(LayoutRect::ZERO);
                let scroll_size = self.get_scroll_size(c_id);
                (sb_state, container_rect, scroll_size)
            };

            let offset = self
                .outputs
                .scroll_offsets
                .get(c_id)
                .copied()
                .unwrap_or(LayoutPoint::ZERO);

            if is_v_thumb || is_h_thumb {
                // サムをクリックした場合：ドラッグを開始
                if let Some(st) = self.layouts.scrollbar_styles.get_mut(c_id) {
                    if is_v_thumb {
                        st.v_thumb_dragged = true;
                    } else {
                        st.h_thumb_dragged = true;
                    }
                    st.drag_start_mouse = pointer_pos;
                    st.drag_start_offset = offset;
                }
                self.events.interaction_states.pressed = Some(target_id); // サム要素自体を pressed に設定
                self.mark_render_dirty(target_id);
            } else if is_v_track || is_h_track {
                let visible_size = self.calculate_visible_size(container_rect);

                // レールをクリックした場合：ダイレクトジャンプスクロールを実行
                if is_v_track {
                    let track_rect = self.outputs.rects[target_id];
                    let thumb_rect = self.outputs.rects[sb_state.v_thumb_id.unwrap()];
                    let relative_y = pointer_pos.y - track_rect.y;

                    let track_range = track_rect.height - thumb_rect.height;
                    let scroll_ratio = if track_range > 0.0 {
                        ((relative_y - thumb_rect.height * 0.5) / track_range).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };

                    let target_y = scroll_ratio * (scroll_size.height - visible_size.height);
                    self.scroll_to(c_id, offset.x, target_y);

                    let new_offset = self
                        .outputs
                        .scroll_offsets
                        .get(c_id)
                        .copied()
                        .unwrap_or(LayoutPoint::ZERO);
                    if let Some(st) = self.layouts.scrollbar_styles.get_mut(c_id) {
                        st.v_thumb_dragged = true;
                        st.drag_start_mouse = pointer_pos;
                        st.drag_start_offset = new_offset;
                    }
                    self.events.interaction_states.pressed = Some(sb_state.v_thumb_id.unwrap());
                    self.mark_render_dirty(sb_state.v_thumb_id.unwrap());
                } else {
                    let track_rect = self.outputs.rects[target_id];
                    let thumb_rect = self.outputs.rects[sb_state.h_thumb_id.unwrap()];
                    let relative_x = pointer_pos.x - track_rect.x;

                    let track_range = track_rect.width - thumb_rect.width;
                    let scroll_ratio = if track_range > 0.0 {
                        ((relative_x - thumb_rect.width * 0.5) / track_range).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };

                    let target_x = scroll_ratio * (scroll_size.width - visible_size.width);
                    self.scroll_to(c_id, target_x, offset.y);

                    let new_offset = self
                        .outputs
                        .scroll_offsets
                        .get(c_id)
                        .copied()
                        .unwrap_or(LayoutPoint::ZERO);
                    if let Some(st) = self.layouts.scrollbar_styles.get_mut(c_id) {
                        st.h_thumb_dragged = true;
                        st.drag_start_mouse = pointer_pos;
                        st.drag_start_offset = new_offset;
                    }
                    self.events.interaction_states.pressed = Some(sb_state.h_thumb_id.unwrap());
                    self.mark_render_dirty(sb_state.h_thumb_id.unwrap());
                }
            }
        }
        clicked_scrollbar
    }

    #[inline]
    pub(crate) fn handle_user_select_text(
        &mut self,
        id: EntityId,
        pointer_pos: LayoutPoint,
        pressed_shift: bool,
    ) {
        if let Some(layout) = self.get_or_create_layout(id) {
            let local = self.pressed_local_point(id, pointer_pos);
            let (clicked_index, is_trailing) = self
                .system
                .text_engine
                .hit_test_point(&layout, local.x, local.y);
            let final_index = if is_trailing {
                clicked_index + 1
            } else {
                clicked_index
            };

            if pressed_shift {
                // 共通の Shift選択拡張
                let anchor = self
                    .outputs
                    .selection_start_index
                    .get(id)
                    .copied()
                    .unwrap_or(final_index);
                if !self.outputs.selection_start_index.contains_key(id) {
                    self.outputs.selection_start_index.insert(id, final_index);
                }
                let range = if anchor <= final_index {
                    anchor..final_index
                } else {
                    final_index..anchor
                };
                self.outputs.text_selections.insert(id, range);
                self.update_selection_rects(id);
            } else {
                // 共通の通常クリックリセット
                self.outputs.selection_start_index.insert(id, final_index);
                self.outputs
                    .text_selections
                    .insert(id, final_index..final_index);
                self.outputs.selected_rects.remove(id);
            }

            self.mark_render_dirty(id);
        }
    }

    #[inline]
    pub(crate) fn restrict_focusable_element(&self, id: EntityId) -> bool {
        let TopologyStore { active_masks, .. } = &self.topology;
        let RenderStore {
            visual_properties, ..
        } = &self.renders;

        EventStore::restrict_focusable_element(id, active_masks, visual_properties)
    }

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
        if self.events.interaction_states.focused == Some(id) {
            // 同一要素をクリックした際にもマウス操作によるフォーカス可視化の消去を同期反映
            self.set_focused_by_trigger(id, true, trigger);
        } else {
            if let Some(old_focus_id) = self.events.interaction_states.focused {
                self.set_focused_by_trigger(old_focus_id, false, trigger);

                // 古いフォーカス要素の選択範囲とハイライト矩形をクリア
                self.clear_selection_highlight_rect(old_focus_id);
                // 進行中の IME コンポジションを強制的に確定させ候補窓を閉じる
                SystemStore::force_complete_ime_composition();

                if let Some(l) = self.events.event_listeners.get_mut(old_focus_id)
                    && let Some(mut handler) = l.on_blur.take()
                {
                    let _guard = crate::ActiveElementGuard::new(old_focus_id);
                    handler(self);
                    if let Some(l) = self.events.event_listeners.get_mut(old_focus_id) {
                        l.on_blur = Some(handler);
                    }
                }
            }

            // 新しいフォーカス可能要素にフォーカスを設定
            self.set_focused_by_trigger(id, true, trigger);

            // 新しいフォーカス先が is_ime(false) の場合は IME 関連付けを解除
            let is_input = self.topology.active_masks[id].has_input_content();
            if is_input && let Some(contents) = self.contents.input_contents.get(id) {
                SystemStore::unassociate_ime(contents, &mut self.window.default_himc);
            } else {
                // インプット以外の場合は IME をデフォルト状態に戻す
                self.reset_ime_default_state();
            }

            if let Some(l) = self.events.event_listeners.get_mut(id)
                && let Some(mut handler) = l.on_focus.take()
            {
                let _guard = crate::ActiveElementGuard::new(id);
                handler(self);
                if let Some(l) = self.events.event_listeners.get_mut(id) {
                    l.on_focus = Some(handler);
                }
            }

            self.events.interaction_states.focused = Some(id);
        }
    }

    #[inline]
    pub(crate) fn handle_remove_focus(&mut self) {
        if let Some(old_focus_id) = self.events.interaction_states.focused {
            self.set_focused_by_trigger(old_focus_id, false, ActiveFocusTrigger::Mouse);

            // 古いフォーカス要素の選択範囲とハイライト矩形をクリア
            self.clear_selection_highlight_rect(old_focus_id);
            // 進行中の IME コンポジションを強制的に確定させ候補窓を閉じる
            SystemStore::force_complete_ime_composition();

            // IME をデフォルトの有効化状態に戻す
            self.reset_ime_default_state();

            if let Some(l) = self.events.event_listeners.get_mut(old_focus_id)
                && let Some(mut handler) = l.on_blur.take()
            {
                let _guard = crate::ActiveElementGuard::new(old_focus_id);
                handler(self);
                if let Some(l) = self.events.event_listeners.get_mut(old_focus_id) {
                    l.on_blur = Some(handler);
                }
            }

            self.events.interaction_states.focused = None;
        }
    }

    pub(crate) fn handle_on_mouse_input(
        &mut self,
        id: EntityId,
        button: MouseButton,
        modifiers: Modifiers,
        state: ElementState,
    ) {
        if let Some(l) = self.events.event_listeners.get_mut(id)
            && let Some(mut handler) = l.on_mouse_input.take()
        {
            let _guard = crate::ActiveElementGuard::new(id);
            handler(self, button, modifiers, state);
            if let Some(l) = self.events.event_listeners.get_mut(id) {
                l.on_mouse_input = Some(handler);
            }
        }
    }

    #[inline]
    pub(crate) fn rewrite_tree_topology(
        &mut self,
        src_id: EntityId,
        target_id: EntityId,
        holder: EntityId,
        drag_prop: DragProperty,
        drag_state: &ActiveDragState,
    ) {
        // ドラッグ元要素の配置（Position）の取得
        let position = self
            .layouts
            .basic_layouts
            .get(src_id)
            .map(|l| l.position)
            .unwrap_or_default();

        if position == Position::Absolute {
            // 絶対配置: 位置移動（補正）を伴うアタッチ
            if drag_prop.update_position {
                // プレースホルダーの最終的な絶対画面座標を取得
                let ph_abs_rect = self.outputs.rects.get(holder).copied().unwrap_or_default();

                // 新しい親（target_id）の絶対画面座標とボーダー厚みを取得
                let target_rect = self
                    .outputs
                    .rects
                    .get(target_id)
                    .copied()
                    .unwrap_or_default();
                let (border_l, border_t) =
                    if let Some(basic) = self.layouts.basic_layouts.get(target_id) {
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

                if let Some(basic) = self.layouts.basic_layouts.get_mut(src_id) {
                    basic.inset = new_inset;
                }
                if let Some(base_basic) = self.layouts.base_basic_layouts.get_mut(src_id) {
                    base_basic.inset = new_inset;
                }
            }

            // ドロップ先コンテナ（target_id）の末尾の子要素としてマウント
            self.add_child(target_id, src_id);
        } else {
            // 相対配置: マウス座標に基づいた子要素の動的並び替えアタッチ
            if drag_prop.update_position {
                let mouse_pos = self
                    .events
                    .current_pointer_position
                    .unwrap_or(LayoutPoint::ZERO);
                let insert_idx = self.mouse_drop_insert_element_index(target_id, mouse_pos);

                if let Some(parent_children) = self.topology.children.get_mut(target_id) {
                    // 算出されたインデックス位置へ挿入
                    parent_children.insert(insert_idx, src_id);
                }
                self.topology.parents.insert(src_id, Some(target_id));

                // Taffy 側のノード順序を物理並び替え結果に沿って一括して再同期
                self.resync_taffy_children_order(target_id);
            } else {
                // 自動更新オフの場合は末尾に通常アタッチ
                self.add_child(target_id, src_id);
            }
            self.mark_layout_dirty(target_id);
        }
    }

    #[inline]
    pub(crate) fn remove_dragged_elemet(&mut self, src_id: EntityId, drag_state: &ActiveDragState) {
        if let Some(src_parent_id) = drag_state.original_parent {
            if let Some(src_children) = self.topology.children.get_mut(src_parent_id) {
                src_children.retain(|x| *x != src_id);
            }
            // 旧親側の Taffy 順序も再同期
            self.resync_taffy_children_order(src_parent_id);
            self.mark_layout_dirty(src_parent_id);
        }
    }

    #[inline]
    pub(crate) fn restore_child(
        &mut self,
        src_id: EntityId,
        holder: EntityId,
        ph_children: SmallVec<[EntityId; 4]>,
    ) {
        for child_id in ph_children {
            // 子要素の親ポインタを元の要素に書き戻し
            self.topology.parents.insert(child_id, Some(src_id));

            // 元の要素の子要素リストへ復旧
            if let Some(src_children) = self.topology.children.get_mut(src_id) {
                src_children.push(child_id);
            }

            // Taffy 側の親子構造も、元の要素に繋ぎ戻し
            if let Some(&src_node) = self.layouts.taffy_nodes.get(src_id)
                && let Some(&ph_node) = self.layouts.taffy_nodes.get(holder)
                && let Some(&child_node) = self.layouts.taffy_nodes.get(child_id)
            {
                let _ = self.layouts.taffy.remove_child(ph_node, child_node);
                let _ = self.layouts.taffy.add_child(src_node, child_node);
            }
        }
    }

    #[inline]
    pub(crate) fn callback_on_entity_drop(
        &mut self,
        src_id: EntityId,
        drop_success: Option<EntityId>,
    ) {
        if let Some(l) = self.events.event_listeners.get_mut(src_id)
            && let Some(mut listener) = l.on_entity_drop.take()
        {
            {
                let _guard = crate::ActiveElementGuard::new(src_id);
                listener(self, Element::from(src_id), drop_success.map(Element::from));
            }
            if let Some(l) = self.events.event_listeners.get_mut(src_id) {
                l.on_entity_drop = Some(listener);
            }
        }
    }

    #[inline]
    pub(crate) fn callback_on_id_drop(&mut self, src_id: EntityId, drop_success: Option<EntityId>) {
        if let Some(l) = self.events.event_listeners.get_mut(src_id)
            && let Some(mut listener) = l.on_id_drop.take()
        {
            {
                let _guard = crate::ActiveElementGuard::new(src_id);
                listener(self, src_id, drop_success);
            }
            if let Some(l) = self.events.event_listeners.get_mut(src_id) {
                l.on_id_drop = Some(listener);
            }
        }
    }

    #[inline]
    pub(crate) fn get_scrollbar_dirty_ids(&mut self) -> SmallVec<[EntityId; 4]> {
        let LayoutStore {
            scrollbar_styles, ..
        } = &mut self.layouts;

        EventStore::get_scrollbar_dirty_ids(scrollbar_styles)
    }

    #[inline]
    pub(crate) fn callback_on_mouse_input(
        &mut self,
        pressed_id: EntityId,
        button: MouseButton,
        modifiers: Modifiers,
        state: ElementState,
    ) {
        if let Some(l) = self.events.event_listeners.get_mut(pressed_id)
            && let Some(mut handler) = l.on_mouse_input.take()
        {
            let _guard = crate::ActiveElementGuard::new(pressed_id);
            handler(self, button, modifiers, state);
            if let Some(l) = self.events.event_listeners.get_mut(pressed_id) {
                l.on_mouse_input = Some(handler);
            }
        }
    }

    #[inline]
    pub(crate) fn callback_on_right_click(&mut self, pressed_id: EntityId) {
        if let Some(l) = self.events.event_listeners.get_mut(pressed_id)
            && let Some(mut handler) = l.on_right_click.take()
        {
            let _guard = crate::ActiveElementGuard::new(pressed_id);
            handler(self);
            if let Some(l) = self.events.event_listeners.get_mut(pressed_id) {
                l.on_right_click = Some(handler);
            }
        }
    }

    #[inline]
    pub(crate) fn callback_on_click(&mut self, pressed_id: EntityId) {
        if let Some(l) = self.events.event_listeners.get_mut(pressed_id)
            && let Some(mut handler) = l.on_click.take()
        {
            let _guard = crate::ActiveElementGuard::new(pressed_id);
            handler(self);
            if let Some(l) = self.events.event_listeners.get_mut(pressed_id) {
                l.on_click = Some(handler);
            }
        }
    }

    #[inline]
    pub(crate) fn get_user_select(&self, id: EntityId) -> UserSelect {
        let RenderStore {
            visual_properties, ..
        } = &self.renders;

        EventStore::get_user_select(id, visual_properties)
    }

    #[inline]
    pub(crate) fn callback_on_keyboard_input(
        &mut self,
        focused_id: EntityId,
        key: VirtualKey,
        modifiers: Modifiers,
        state: ElementState,
    ) {
        if let Some(l) = self.events.event_listeners.get_mut(focused_id)
            && let Some(mut handler) = l.on_keyboard_input.take()
        {
            let _guard = crate::ActiveElementGuard::new(focused_id);
            handler(self, key, modifiers, state);
            if let Some(l) = self.events.event_listeners.get_mut(focused_id) {
                l.on_keyboard_input = Some(handler);
            }
        }
    }

    /// 各インタラクション状態（ステート）を更新し、レイアウト変更を伴うか自動的に判別して Dirty フラグを制御する共通ヘルパー
    #[inline]
    pub(crate) fn update_state(&mut self, id: EntityId, state_flag: u128, active: bool) {
        let Some(mask) = self.topology.active_masks.get_mut(id) else {
            return;
        };

        let was_active = mask.has(state_flag);
        if (was_active == active) {
            return;
        }

        if active {
            mask.set(state_flag);
        } else {
            mask.unset(state_flag);
        }

        // 状態変化の発生時に即座に動的なスタイルを解決する
        self.resolve_element_style_state(id, true);

        // 親から子方向へのスタイル解決の伝播
        if let Some(children) = self.topology.children.get(id).cloned() {
            for child_id in children {
                if self.topology.active_masks[child_id].has(STYLE_INTERACTION_PARENT) {
                    self.resolve_element_style_state(child_id, true);

                    if self.does_state_require_layout(child_id, state_flag) {
                        self.mark_layout_dirty(child_id);
                        self.mark_render_dirty(id);
                    } else {
                        self.mark_render_dirty(child_id);
                    }
                }
            }
        }

        // STYLE_INTERACTION_WITHIN マスク判定による親先祖の早期バイパス
        let mut curr = id;
        while let Some(Some(parent_id)) = self.topology.parents.get(curr).copied() {
            if self.topology.entities.contains_key(parent_id) {
                let parent_mask = self.topology.active_masks[parent_id];

                // 先祖要素が within スタイルを持っている場合のみそのスタイル評価を実行
                if parent_mask.has(STYLE_INTERACTION_WITHIN) {
                    self.resolve_element_style_state(parent_id, true);

                    if self.does_state_require_layout(parent_id, state_flag) {
                        self.mark_layout_dirty(parent_id);
                        self.mark_render_dirty(id);
                    } else {
                        self.mark_render_dirty(parent_id);
                    }
                }
            }
            curr = parent_id;
        }

        // 残りの状態遷移イベントの解決
        if active {
            match state_flag {
                // Disabledになった瞬間
                STATE_DISABLED => {
                    if let Some(mut listeners) = self.events.event_listeners.get_mut(id)
                        && let Some(mut handler) = listeners.on_disable.take()
                    {
                        let _guard = crate::ActiveElementGuard::new(id);
                        handler(self);
                        if let Some(l) = self.events.event_listeners.get_mut(id) {
                            l.on_disable = Some(handler);
                        }
                    }
                }
                // アクティブになった瞬間
                STATE_ACTIVED => {
                    if let Some(mut listeners) = self.events.event_listeners.get_mut(id)
                        && let Some(mut handler) = listeners.on_active.take()
                    {
                        let _guard = crate::ActiveElementGuard::new(id);
                        handler(self);
                        if let Some(l) = self.events.event_listeners.get_mut(id) {
                            l.on_active = Some(handler);
                        }
                    }
                }
                // セレクトになった瞬間
                STATE_SELECTED => {
                    if let Some(mut listeners) = self.events.event_listeners.get_mut(id)
                        && let Some(mut handler) = listeners.on_select.take()
                    {
                        let _guard = crate::ActiveElementGuard::new(id);
                        handler(self);
                        if let Some(l) = self.events.event_listeners.get_mut(id) {
                            l.on_select = Some(handler);
                        }
                    }
                }
                _ => {}
            }
        }

        if self.does_state_require_layout(id, state_flag) {
            self.mark_layout_dirty(id);
            self.mark_render_dirty(id);
        } else {
            self.mark_render_dirty(id);
        }
    }
}
