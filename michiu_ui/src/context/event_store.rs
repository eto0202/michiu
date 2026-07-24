use std::path::PathBuf;

use crate::*;
use slotmap::{SecondaryMap, SparseSecondaryMap};

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

pub struct EventStore {
    pub(crate) event_listeners: SparseSecondaryMap<EntityId, EventListeners>,
    pub interaction_states: InteractionStates,
    pub(crate) current_pointer_position: Option<LayoutPoint>,
    pub(crate) resizing_state: Option<ResizingState>,
    pub(crate) active_resize_hover: Option<(EntityId, ResizeDirection)>,
    pub(crate) drag_properties: SparseSecondaryMap<EntityId, DragProperty>,
    pub(crate) drop_properties: SparseSecondaryMap<EntityId, DropProperty>,
    pub(crate) active_drag_state: Option<ActiveDragState>,
}

impl Default for EventStore {
    fn default() -> Self {
        Self::new()
    }
}

impl EventStore {
    #[inline]
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

        if self.interaction_states.hovered == Some(id) {
            self.interaction_states.hovered = None;
        }
        if self.interaction_states.focused == Some(id) {
            self.interaction_states.focused = None;
        }
        if self.interaction_states.pressed == Some(id) {
            self.interaction_states.pressed = None;
        }
        if self.interaction_states.dragged == Some(id) {
            self.interaction_states.dragged = None;
        }

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
    pub(crate) fn to_attach_placeholder(
        root: EntityId,
        drag_prop: DragProperty,
        outputs: &OutputStore,
        layouts: &LayoutStore,
    ) -> (Option<EntityId>, LayoutRect, f32, f32) {
        let (parent_id_opt, parent_rect, parent_border_left, parent_border_top) =
            match drag_prop.placeholder_parent {
                DragPlaceholderParent::Root => (
                    Some(root),
                    outputs.rects.get(root).copied().unwrap_or(LayoutRect::ZERO),
                    0.0,
                    0.0,
                ),
                DragPlaceholderParent::Custom(p_id) => {
                    let p_rect = outputs.rects.get(p_id).copied().unwrap_or(LayoutRect::ZERO);
                    let b_l = if let Some(l) = layouts.basic_layouts.get(p_id) {
                        match l.border.left {
                            Length::Px(v) => v,
                            _ => 0.0,
                        }
                    } else {
                        0.0
                    };
                    let b_t = if let Some(l) = layouts.basic_layouts.get(p_id) {
                        match l.border.top {
                            Length::Px(v) => v,
                            _ => 0.0,
                        }
                    } else {
                        0.0
                    };
                    (Some(p_id), p_rect, b_l, b_t)
                }
            };
        (
            parent_id_opt,
            parent_rect,
            parent_border_left,
            parent_border_top,
        )
    }

    pub(crate) fn get_resizable_cursor_icon(
        id: EntityId,
        dir: ResizeDirection,
        vis: &mut VisualProperty,
    ) -> Option<CursorIcon> {
        // 要素に resizable_cursor の個別指定があれば、方向に応じて該当カーソルを抽出
        let custom_cursor = if let Some(arr) = vis.resizable_cursor {
            let idx = match dir {
                ResizeDirection::Top | ResizeDirection::Bottom => 0, // Ns
                ResizeDirection::Left | ResizeDirection::Right => 1, // Ew
                ResizeDirection::TopRight | ResizeDirection::BottomLeft => 2, // Nesw
                ResizeDirection::TopLeft | ResizeDirection::BottomRight => 3, // Nwse
            };
            arr[idx]
        } else {
            None
        };

        // 独自指定があればそれを使い、無ければライブラリの自動マッピングを使用
        Some(custom_cursor.unwrap_or_else(|| Context::resize_direction_to_cursor(dir)))
    }

    pub(crate) fn found_resize_hover(
        target_id: Option<EntityId>,
        logical_pos: LayoutPoint,
        topology: &mut TopologyStore,
        layouts: &LayoutStore,
        outputs: &OutputStore,
    ) -> (Option<EntityId>, Option<(EntityId, ResizeDirection)>) {
        let mut current_id = target_id;
        let mut found_resize_hover = None;
        while let Some(id) = current_id {
            if topology.active_masks[id].has(STYLE_RESIZABLE) {
                let rect = outputs.rects[id];
                let resizable_flags = layouts
                    .basic_layouts
                    .get(id)
                    .map(|l| l.resizable)
                    .unwrap_or([false; 4]);

                // 境界外周に 6.0px のあそびを持たせてヒット判定
                let detect_border = 6.0f32;
                if let Some(dir) = Context::detect_resize_direction(
                    rect,
                    resizable_flags,
                    logical_pos,
                    detect_border,
                ) {
                    found_resize_hover = Some((id, dir));
                    break; // 最も前面寄りのリサイズ親要素を優先採用
                }
            }
            current_id = topology.parents.get(id).copied().flatten();
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
        clip: LayoutRect,
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

    pub(crate) fn autoscroll_occurred(&mut self) -> (bool, Option<LayoutPoint>) {
        let mut autoscroll_occurred = false;
        let mut active_pos = None;

        if let Some(id) = self.events.interaction_states.pressed
            && let Some(pointer_pos) = self.events.current_pointer_position
            && let Some(clip) = self.outputs.clip_rects.get(id).copied()
        {
            let user_select = self
                .renders
                .visual_properties
                .get(id)
                .and_then(|v| v.user_select)
                .unwrap_or_default();

            if user_select == UserSelect::Text {
                let distace = EventStore::drag_overhang_distance(pointer_pos, clip);

                // はみ出しがある場合、距離に比例したオートスクロールを実行
                if distace.x.abs() > 1.0 || distace.y.abs() > 1.0 {
                    // TODO: スクロール感度調整用メソッドを実装。
                    let speed_factor = 0.15f32;
                    let scroll_dx = distace.x * speed_factor;
                    let scroll_dy = distace.y * speed_factor;

                    if self.scroll_by(id, scroll_dx, scroll_dy) {
                        autoscroll_occurred = true;
                        active_pos = Some(pointer_pos);
                    }
                }
            }
        }

        (autoscroll_occurred, active_pos)
    }

    #[inline]
    pub(crate) fn to_attach_placeholder(
        &self,
        root: EntityId,
        drag_prop: DragProperty,
    ) -> (Option<EntityId>, LayoutRect, f32, f32) {
        EventStore::to_attach_placeholder(root, drag_prop, &self.outputs, &self.layouts)
    }

    #[inline]
    pub(crate) fn apply_resizable_cursor_style(&mut self, id: EntityId, dir: ResizeDirection) {
        if let Some(vis) = self.renders.visual_properties.get_mut(id) {
            vis.cursor = EventStore::get_resizable_cursor_icon(id, dir, vis);
        }
    }

    #[inline]
    pub(crate) fn found_resize_hover(
        &mut self,
        target_id: Option<EntityId>,
        logical_pos: LayoutPoint,
    ) -> (Option<EntityId>, Option<(EntityId, ResizeDirection)>) {
        EventStore::found_resize_hover(
            target_id,
            logical_pos,
            &mut self.topology,
            &self.layouts,
            &self.outputs,
        )
    }

    #[inline]
    pub(crate) fn pressed_local_point(
        &self,
        pressed_id: EntityId,
        logical_pos: LayoutPoint,
    ) -> LayoutPoint {
        let rect = self.outputs.rects[pressed_id];
        let (basic, _, _) = self.resolve_active_layouts(pressed_id);
        let border = self.get_physical_border(pressed_id, &basic);
        let padding = self.get_physical_padding(pressed_id, &basic);

        let scroll = self
            .outputs
            .scroll_offsets
            .get(pressed_id)
            .copied()
            .unwrap_or(LayoutPoint::ZERO);

        let local_x = logical_pos.x - (rect.x + border.left + padding.left) + scroll.x;
        let local_y = logical_pos.y - (rect.y + border.top + padding.top) + scroll.y;
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
            };
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
            };

            if let Some(mut listeners) = self.events.event_listeners.get_mut(new_id)
                && let Some(mut handler) = listeners.on_hover.take()
            {
                let _guard = crate::ActiveElementGuard::new(new_id);
                handler(self);
                if let Some(l) = self.events.event_listeners.get_mut(new_id) {
                    l.on_hover = Some(handler);
                }
            };
        }

        self.events.interaction_states.hovered = target_id;
    }

    #[inline]
    pub(crate) fn propagate_cursor_move_events(
        &mut self,
        target_id: EntityId,
        logical_pos: LayoutPoint,
    ) {
        if let Some(mut listeners) = self.events.event_listeners.get_mut(target_id)
            && let Some(mut handler) = listeners.on_cursor_moved.take()
        {
            let rect = self.outputs.rects[target_id];
            let relative_pos = LayoutPoint::new(logical_pos.x - rect.x, logical_pos.y - rect.y);
            let _guard = crate::ActiveElementGuard::new(target_id);
            handler(self, relative_pos);
            if let Some(l) = self.events.event_listeners.get_mut(target_id) {
                l.on_cursor_moved = Some(handler);
            }
        }
    }

    #[inline]
    pub(crate) fn propagate_drag_events(
        &mut self,
        pressed_id: EntityId,
        prev_pos: Option<LayoutPoint>,
        logical_pos: LayoutPoint,
    ) {
        if let Some(prev) = prev_pos {
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
                    let (parent_id_opt, parent_rect, parent_border_left, parent_border_top) =
                        self.to_attach_placeholder(root, drag_prop);

                    // プレースホルダー（クローン）をアタッチ先親の直下へ spawn して生成
                    let placeholder_id = self.spawn(parent_id_opt);
                    if let Some(p_id) = parent_id_opt {
                        self.add_child(p_id, placeholder_id);
                    }

                    // 元要素のレイアウトおよびビジュアル情報をコピーして初期マウント
                    if let Some(basic) = self.renders.base_basic_layouts.get(pressed_id).copied() {
                        self.renders
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
                    if let Some(layout) = self.renders.base_basic_layouts.get_mut(placeholder_id) {
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
                    let mut start_listener_opt = self
                        .events
                        .event_listeners
                        .get_mut(pressed_id)
                        .and_then(|l| l.on_drag_start.take());
                    if let Some(mut listener) = start_listener_opt {
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

                // on_drag
                let mut on_drag = self
                    .events
                    .event_listeners
                    .get_mut(pressed_id)
                    .and_then(|l| l.on_drag.take());
                if let Some(mut handler) = on_drag {
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
        match drag_prop.placeholder_parent {
            DragPlaceholderParent::Root => (
                self.outputs
                    .rects
                    .get(root)
                    .copied()
                    .unwrap_or(LayoutRect::ZERO),
                0.0,
                0.0,
            ),
            DragPlaceholderParent::Custom(p_id) => {
                let p_rect = self
                    .outputs
                    .rects
                    .get(p_id)
                    .copied()
                    .unwrap_or(LayoutRect::ZERO);
                let b_l = if let Some(l) = self.layouts.basic_layouts.get(p_id) {
                    match l.border.left {
                        Length::Px(v) => v,
                        _ => 0.0,
                    }
                } else {
                    0.0
                };
                let b_t = if let Some(l) = self.layouts.basic_layouts.get(p_id) {
                    match l.border.top {
                        Length::Px(v) => v,
                        _ => 0.0,
                    }
                } else {
                    0.0
                };
                (p_rect, b_l, b_t)
            }
        }
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
        if let Some(layout) = self.renders.base_basic_layouts.get_mut(placeholder) {
            layout.inset.left = Val::Px(local_x);
            layout.inset.top = Val::Px(local_y);
            layout.inset.right = Val::Auto;
            layout.inset.bottom = Val::Auto;
        }

        self.mark_layout_dirty(placeholder);
        self.mark_render_dirty(placeholder);
    }

    pub fn inject_pointer_move(&mut self, logical_pos: LayoutPoint) {
        let _context_guard = bind_context(self);

        let prev_pos = self.events.current_pointer_position;
        self.events.current_pointer_position = Some(logical_pos);

        // リサイズ中のドラッグ同期処理
        if let Some(state) = self.events.resizing_state.clone() {
            self.sync_resizing_drag(logical_pos, state);
            return; // リサイズドラッグ中は、通常のホバーやドラッグ判定を完全にスキップして早期リターン
        }

        self.sync_scrollbar_drag(logical_pos);

        // マウスボタン押し下げ中は、他の要素へのインタラクション漏洩を防ぐためヒット先を押し下げ要素に強制ロック
        let target_id = if let Some(pressed_id) = self.events.interaction_states.pressed {
            Some(pressed_id)
        } else {
            self.hit_test(logical_pos)
        };

        // 直前のリサイズホバー対象を退避
        let prev_resize_hover = self.events.active_resize_hover;
        // リサイズホバー情報を一旦リセット
        self.events.active_resize_hover = None;

        // ヒットした要素、およびその親先祖に向かってツリーを遡上
        let (current_id, found_resize_hover) = self.found_resize_hover(target_id, logical_pos);

        if let Some((id, dir)) = found_resize_hover {
            self.events.active_resize_hover = Some((id, dir));
            self.apply_resizable_cursor_style(id, dir);
            self.mark_render_dirty(id);
        }

        // 枠線から外れた、または異なる要素に変わった場合
        if let Some((prev_id, _)) = prev_resize_hover {
            let now_id = self.events.active_resize_hover.map(|(id, _)| id);

            // 異なるホバー状態になった場合、旧要素のカーソル上書きを破棄し本来のスタイルに即時強制リセット
            if Some(prev_id) != now_id {
                // スタイルの再解決を叩き、上書きされていた vis.cursor を本来のカーソル（通常ホバー/ベース等）へ復旧
                self.resolve_element_style_state(prev_id, false);
                self.mark_render_dirty(prev_id);
            }
        }

        if let Some(pressed_id) = self.events.interaction_states.pressed {
            let user_select = self
                .renders
                .visual_properties
                .get(pressed_id)
                .and_then(|v| v.user_select)
                .unwrap_or_default();

            if user_select == UserSelect::Text
                && let Some(start_pos) = self.outputs.selection_start_index.get(pressed_id).copied()
            {
                // プレースホルダー選択のドラッグ遮断
                if let Some(contents) = self.contents.input_contents.get(pressed_id) {
                    let is_placeholder = contents.text.0.get().is_empty();
                    let is_ime = contents
                        .ime_state
                        .as_ref()
                        .map(|s| s.composition_text.is_empty())
                        .unwrap_or(true);

                    if is_placeholder && is_ime && !contents.placeholder_select {
                        return;
                    }
                }

                let local = self.pressed_local_point(pressed_id, logical_pos);

                if let Some(layout) = self.get_or_create_layout(pressed_id) {
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
                        if let Some(contents) = self.contents.input_contents.get_mut(pressed_id) {
                            contents.selection_reversed = false;
                        }
                        start_pos..final_index
                    } else {
                        // 逆選択（左方向ドラッグ）
                        if let Some(contents) = self.contents.input_contents.get_mut(pressed_id) {
                            contents.selection_reversed = true;
                        }
                        final_index..start_pos
                    };

                    self.outputs
                        .text_selections
                        .insert(pressed_id, range.clone());

                    self.update_selection_rects(pressed_id);

                    if let Some(contents) = self.contents.input_contents.get_mut(pressed_id) {
                        contents.selected_range = range;
                        crate::update_input_caret_position(self, pressed_id);
                    }
                    self.mark_render_dirty(pressed_id);
                }
            }
        }

        // ヒットテスト
        let target_id = self.hit_test(logical_pos);

        // ホバー（Enter/Leave）状態の解決
        if target_id != self.events.interaction_states.hovered {
            self.resolve_hover_state(target_id);
        }

        // カーソル移動イベントの伝播
        if let Some(target_id) = target_id {
            self.propagate_cursor_move_events(target_id, logical_pos);
        }

        // ドラッグイベントの伝播
        if let Some(pressed_id) = self.events.interaction_states.pressed {
            self.propagate_drag_events(pressed_id, prev_pos, logical_pos);
        }

        // D&D プレースホルダーの移動とドロップ先ホバー検知
        let Some(mut drag_state) = self.events.active_drag_state.clone() else {
            return;
        };

        // ウィンドウの真のルート要素を解決
        let root = self
            .find_root_entity()
            .expect("Root EntityId not found in Context");

        let src_id = drag_state.source_entity;
        let placeholder_id = drag_state.placeholder_entity;
        let drag_prop = self.events.drag_properties.get(src_id).copied().unwrap();

        // アタッチ先親コンテナ基準での相対ローカル座標を逆算して追従（Inset更新）
        self.update_inset_based_relative_local(
            root,
            placeholder_id,
            logical_pos,
            drag_prop,
            &drag_state,
        );

        // 現在ホバー侵入中のドロップターゲット要素を検知
        let hit_id_opt = self.hit_test(logical_pos);
        let mut found_drop_target = None;

        if let Some(hit_id) = hit_id_opt {
            let mut current_id = Some(hit_id);
            while let Some(id) = current_id {
                // ヒットした要素がドラッグ元（src_id）自身、またはその子孫である場合は
                // ドロップ先として誤認されるのを完全に防ぐため、スルーしてさらに上の親を辿る
                if id == src_id || self.is_descendant_of(id, src_id) {
                    current_id = self.topology.parents.get(id).copied().flatten();
                    continue;
                }

                if id != placeholder_id && self.topology.active_masks[id].has(STYLE_DROPPABLE) {
                    found_drop_target = Some(id);
                    break;
                }
                current_id = self.topology.parents.get(id).copied().flatten();
            }
        }

        // ドロップ先のホバー切り替えイベントを解決（STATE_DRAG_IN の同期）
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

        // コールバックを一時的に take して借用を分離した後に実行
        match drag_prop.drag_mode {
            DragPayload::Element => {
                let mut listener_opt = self
                    .events
                    .event_listeners
                    .get_mut(src_id)
                    .and_then(|l| l.on_entity_drag.take());
                if let Some(mut listener) = listener_opt {
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
                let mut listener_opt = self
                    .events
                    .event_listeners
                    .get_mut(src_id)
                    .and_then(|l| l.on_id_drag.take());
                if let Some(mut listener) = listener_opt {
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

    pub fn inject_pointer_button(
        &mut self,
        button: MouseButton,
        state: ElementState,
        modifiers: Modifiers,
    ) {
        let _context_guard = bind_context(self);
        let current_hovered = self.events.interaction_states.hovered;

        match state {
            ElementState::Pressed => {
                if button == MouseButton::Left {
                    // リサイズドラッグの開始判定
                    if let Some((id, dir)) = self.events.active_resize_hover {
                        let rect = self.outputs.rects[id];
                        let position = self
                            .layouts
                            .basic_layouts
                            .get(id)
                            .map(|l| l.position)
                            .unwrap_or(Position::Relative);

                        // 親要素の矩形を取得
                        // 親要素の矩形と、その「左・上ボーダーの厚み」を正確に取得する
                        let (parent_rect, parent_border_left, parent_border_top) =
                            if let Some(Some(parent_id)) = self.topology.parents.get(id) {
                                let p_rect = self
                                    .outputs
                                    .rects
                                    .get(*parent_id)
                                    .copied()
                                    .unwrap_or(LayoutRect::ZERO);

                                let border_l = if let Some(layout) =
                                    self.layouts.basic_layouts.get(*parent_id)
                                {
                                    match layout.border.left {
                                        Length::Px(v) => v,
                                        Length::Percent(p) => p_rect.width * (p / 100.0),
                                    }
                                } else {
                                    0.0
                                };
                                let border_t = if let Some(layout) =
                                    self.layouts.basic_layouts.get(*parent_id)
                                {
                                    match layout.border.top {
                                        Length::Px(v) => v,
                                        Length::Percent(p) => p_rect.height * (p / 100.0),
                                    }
                                } else {
                                    0.0
                                };

                                (p_rect, border_l, border_t)
                            } else {
                                (LayoutRect::ZERO, 0.0, 0.0)
                            };

                        // 親コンテナのボーダー内側を基準点として物理相対位置を逆算
                        let local_x = rect.x - (parent_rect.x + parent_border_left);
                        let local_y = rect.y - (parent_rect.y + parent_border_top);

                        // 【解決】絶対配置の場合、開始時に Top-Left 基準に完全に正規化（コンバート）する
                        // これにより、もともと right / bottom 基準で配置されていた要素であっても、
                        // ドラッグ開始の瞬間に左上へ吹っ飛ぶ現象を完全に阻止します。
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
                            if let Some(layout) = self.layouts.basic_layouts.get_mut(id) {
                                layout.inset = start_inset;
                            }
                            if let Some(layout) = self.renders.base_basic_layouts.get_mut(id) {
                                layout.inset = start_inset;
                            }
                        } else {
                            // 相対配置時は、通常通りそのままのインセットを使用
                            start_inset = self
                                .layouts
                                .basic_layouts
                                .get(id)
                                .map(|l| l.inset)
                                .unwrap_or(BasicLayout::default().inset);
                        }

                        let start_pos = self
                            .events
                            .current_pointer_position
                            .unwrap_or(LayoutPoint::ZERO);

                        self.events.resizing_state = Some(ResizingState {
                            entity_id: id,
                            direction: dir,
                            start_mouse_pos: start_pos,
                            start_rect: rect,
                            start_inset,
                        });

                        // リサイズ中の要素は pressed とマーク（多重干渉防止）
                        self.events.interaction_states.pressed = Some(id);
                        self.mark_render_dirty(id);
                        return; // リサイズドラッグが開始されたため、通常のクリック・フォーカス処理を完全にバイパス
                    }
                }

                let mut clicked_scrollbar = false;

                if let Some(pointer_pos) = self.events.current_pointer_position
                    && let Some(target_id) = current_hovered
                {
                    // 1. ヒットした要素がサム、またはトラックであるかを判定
                    let mut parent_container = None;
                    let mut is_v_thumb = false;
                    let mut is_h_thumb = false;
                    let mut is_v_track = false;
                    let mut is_h_track = false;

                    for (c_id, sb_state) in self.layouts.scrollbar_styles.iter() {
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
                            let sb_state =
                                self.layouts.scrollbar_styles.get(c_id).cloned().unwrap();
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
                            // A. サムをクリックした場合：ドラッグを開始
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

                            // B. レールをクリックした場合：ダイレクトジャンプスクロールを実行
                            if is_v_track {
                                let track_rect = self.outputs.rects[target_id];
                                let thumb_rect = self.outputs.rects[sb_state.v_thumb_id.unwrap()];
                                let relative_y = pointer_pos.y - track_rect.y;

                                let track_range = track_rect.height - thumb_rect.height;
                                let scroll_ratio = if track_range > 0.0 {
                                    ((relative_y - thumb_rect.height * 0.5) / track_range)
                                        .clamp(0.0, 1.0)
                                } else {
                                    0.0
                                };

                                let target_y =
                                    scroll_ratio * (scroll_size.height - visible_size.height);
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
                                self.events.interaction_states.pressed =
                                    Some(sb_state.v_thumb_id.unwrap());
                                self.mark_render_dirty(sb_state.v_thumb_id.unwrap());
                            } else {
                                let track_rect = self.outputs.rects[target_id];
                                let thumb_rect = self.outputs.rects[sb_state.h_thumb_id.unwrap()];
                                let relative_x = pointer_pos.x - track_rect.x;

                                let track_range = track_rect.width - thumb_rect.width;
                                let scroll_ratio = if track_range > 0.0 {
                                    ((relative_x - thumb_rect.width * 0.5) / track_range)
                                        .clamp(0.0, 1.0)
                                } else {
                                    0.0
                                };

                                let target_x =
                                    scroll_ratio * (scroll_size.width - visible_size.width);
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
                                self.events.interaction_states.pressed =
                                    Some(sb_state.h_thumb_id.unwrap());
                                self.mark_render_dirty(sb_state.h_thumb_id.unwrap());
                            }
                        }
                    }
                }

                if clicked_scrollbar {
                    return; // 背後の一般子要素へのイベント透過を防止
                }

                if let Some(target_id) = current_hovered {
                    self.events.interaction_states.pressed = Some(target_id);
                    self.set_pressed(target_id, true);

                    let user_select = self
                        .renders
                        .visual_properties
                        .get(target_id)
                        .and_then(|v| v.user_select)
                        .unwrap_or(UserSelect::None);

                    let is_input = self.topology.active_masks[target_id].has(COMP_INPUT_CONTENT);

                    if user_select == UserSelect::Text
                        && !is_input
                        && let Some(pointer_pos) = self.events.current_pointer_position
                    {
                        let rect = self.outputs.rects[target_id];
                        let (basic, _, _) = self.resolve_active_layouts(target_id);
                        let border_left = match basic.border.left {
                            Length::Px(v) => v,
                            _ => 0.0,
                        };
                        let padding_left = match basic.padding.left {
                            Length::Px(v) => v,
                            _ => 0.0,
                        };
                        let border_top = match basic.border.top {
                            Length::Px(v) => v,
                            _ => 0.0,
                        };
                        let padding_top = match basic.padding.top {
                            Length::Px(v) => v,
                            _ => 0.0,
                        };

                        let scroll = self
                            .outputs
                            .scroll_offsets
                            .get(target_id)
                            .copied()
                            .unwrap_or(LayoutPoint::ZERO);

                        let local_x =
                            pointer_pos.x - (rect.x + border_left + padding_left) + scroll.x;
                        let local_y =
                            pointer_pos.y - (rect.y + border_top + padding_top) + scroll.y;

                        if let Some(layout) = self.get_or_create_layout(target_id) {
                            let (clicked_index, is_trailing) = self
                                .system
                                .text_engine
                                .hit_test_point(&layout, local_x, local_y);
                            let final_index = if is_trailing {
                                clicked_index + 1
                            } else {
                                clicked_index
                            };

                            if modifiers.shift {
                                // 共通の Shift選択拡張
                                let anchor = self
                                    .outputs
                                    .selection_start_index
                                    .get(target_id)
                                    .copied()
                                    .unwrap_or(final_index);
                                if !self.outputs.selection_start_index.contains_key(target_id) {
                                    self.outputs
                                        .selection_start_index
                                        .insert(target_id, final_index);
                                }
                                let range = if anchor <= final_index {
                                    anchor..final_index
                                } else {
                                    final_index..anchor
                                };
                                self.outputs.text_selections.insert(target_id, range);
                                self.update_selection_rects(target_id);
                            } else {
                                // 共通の通常クリックリセット
                                self.outputs
                                    .selection_start_index
                                    .insert(target_id, final_index);
                                self.outputs
                                    .text_selections
                                    .insert(target_id, final_index..final_index);
                                self.outputs.selected_rects.remove(target_id);
                            }

                            self.mark_render_dirty(target_id);
                        }
                    }

                    // フォーカス可能要素のみにフォーカスを制限
                    let is_focusable = self.topology.active_masks[target_id]
                        .has(COMP_INPUT_CONTENT)
                        || self.topology.active_masks[target_id].has(COMP_WEBVIEW_CONTENT)
                        || (self.topology.active_masks[target_id].has(STYLE_FOCUSABLE)
                            && self
                                .renders
                                .visual_properties
                                .get(target_id)
                                .and_then(|v| v.focusable)
                                .map(|f| match f {
                                    Focusable::SelfStyle(trigger) | Focusable::Inherit(trigger) => {
                                        trigger == FocusTrigger::Mouse
                                            || trigger == FocusTrigger::Both
                                    }
                                    Focusable::None => false,
                                })
                                .unwrap_or(false));

                    if is_focusable {
                        // フォーカスの自動切り替え
                        if self.events.interaction_states.focused != Some(target_id) {
                            if let Some(old_focus_id) = self.events.interaction_states.focused {
                                self.set_focused(old_focus_id, false);

                                // on_blur
                                let mut on_blur = self
                                    .events
                                    .event_listeners
                                    .get_mut(old_focus_id)
                                    .and_then(|l| l.on_blur.take());
                                if let Some(mut handler) = on_blur {
                                    let _guard = crate::ActiveElementGuard::new(old_focus_id);
                                    handler(self);
                                    if let Some(l) =
                                        self.events.event_listeners.get_mut(old_focus_id)
                                    {
                                        l.on_blur = Some(handler);
                                    }
                                }
                            }

                            // 新しいフォーカス可能要素にフォーカスを設定
                            self.set_focused(target_id, true);

                            // on_focus
                            let mut on_focus = self
                                .events
                                .event_listeners
                                .get_mut(target_id)
                                .and_then(|l| l.on_focus.take());
                            if let Some(mut handler) = on_focus {
                                let _guard = crate::ActiveElementGuard::new(target_id);
                                handler(self);
                                if let Some(l) = self.events.event_listeners.get_mut(target_id) {
                                    l.on_focus = Some(handler);
                                }
                            }
                            self.events.interaction_states.focused = Some(target_id);
                        }
                    } else {
                        // フォーカス不可能な要素をクリックした場合は、
                        // 現在フォーカスされているインプットからフォーカスを完全に外し状態をクリアする
                        if let Some(old_focus_id) = self.events.interaction_states.focused {
                            self.set_focused(old_focus_id, false);

                            let mut on_blur = self
                                .events
                                .event_listeners
                                .get_mut(old_focus_id)
                                .and_then(|l| l.on_blur.take());
                            if let Some(mut handler) = on_blur {
                                let _guard = crate::ActiveElementGuard::new(old_focus_id);
                                handler(self);
                                if let Some(l) = self.events.event_listeners.get_mut(old_focus_id) {
                                    l.on_blur = Some(handler);
                                }
                            }
                            self.events.interaction_states.focused = None;
                        }
                    }

                    // on_mouse_input
                    let mut on_input = self
                        .events
                        .event_listeners
                        .get_mut(target_id)
                        .and_then(|l| l.on_mouse_input.take());
                    if let Some(mut handler) = on_input {
                        let _guard = crate::ActiveElementGuard::new(target_id);
                        handler(self, button, modifiers, state);
                        if let Some(l) = self.events.event_listeners.get_mut(target_id) {
                            l.on_mouse_input = Some(handler);
                        }
                    }
                }
            }
            ElementState::Released => {
                // リサイズドラッグの終了処理
                if let Some(state) = self.events.resizing_state.take() {
                    let id = state.entity_id;
                    self.events.interaction_states.pressed = None;

                    // 元のリサイズホバーカーソル表示を維持するために再検出をマーク
                    // リサイズ状態が解除された「この瞬間」に現在の座標で move を再キックし、
                    // すり抜けていた通常のホバー・離脱判定（Leave）を正確に評価させる
                    if let Some(pos) = self.events.current_pointer_position {
                        self.inject_pointer_move(pos);
                    }
                    self.mark_render_dirty(id);
                    return;
                }

                // D&D ドラッグ終了・ドロップ確定処理
                if let Some(drag_state) = self.events.active_drag_state.take() {
                    let src_id = drag_state.source_entity;
                    let placeholder_id = drag_state.placeholder_entity;
                    let drag_prop = self.events.drag_properties.get(src_id).copied().unwrap();

                    // 疑似クラス（STATE_DRAGGING, STATE_DRAG_IN）を解除
                    self.set_drag_state(src_id, STATE_DRAGGING, false);
                    if let Some(target_id) = drag_state.current_drop_target {
                        self.set_drag_state(target_id, STATE_DRAG_IN, false);
                    }

                    // プレースホルダー要素を親および Taffy から安全にデスポーン
                    // このタイミングではまだ despawn_internal せず最後に移動させます。
                    self.events.interaction_states.pressed = None;
                    self.events.interaction_states.dragged = None;

                    let drop_success = drag_state.current_drop_target;

                    // A. 実体移動（DragMode::Entity）の場合のツリートポロジー書き換え
                    if let Some(target_id) = drop_success
                        && drag_prop.drag_mode == DragPayload::Element
                        && let Some(drop_prop) = self.events.drop_properties.get(target_id).copied()
                    {
                        // 1. まずドラッグ元要素を現在の親の children リストから安全に引き抜いて削除
                        if let Some(src_parent_id) = drag_state.original_parent {
                            if let Some(src_children) =
                                self.topology.children.get_mut(src_parent_id)
                            {
                                src_children.retain(|x| *x != src_id);
                            }
                            // 旧親側の Taffy 順序も再同期
                            self.resync_taffy_children_order(src_parent_id);
                            self.mark_layout_dirty(src_parent_id);
                        }

                        // ドラッグ元要素の配置（Position）の取得
                        let position = self
                            .layouts
                            .basic_layouts
                            .get(src_id)
                            .map(|l| l.position)
                            .unwrap_or(Position::Relative);

                        if position == Position::Absolute {
                            // 【絶対配置（Absolute）】: 位置移動（補正）を伴うアタッチ
                            if drag_prop.update_position {
                                // 1. プレースホルダーの最終的な絶対画面座標を取得
                                let ph_abs_rect = self
                                    .outputs
                                    .rects
                                    .get(placeholder_id)
                                    .copied()
                                    .unwrap_or(LayoutRect::ZERO);

                                // 2. 新しい親（target_id）の絶対画面座標とボーダー厚みを取得
                                let target_rect = self
                                    .outputs
                                    .rects
                                    .get(target_id)
                                    .copied()
                                    .unwrap_or(LayoutRect::ZERO);
                                let (border_l, border_t) = if let Some(layout) =
                                    self.layouts.basic_layouts.get(target_id)
                                {
                                    let border = self.get_physical_border(target_id, layout);
                                    (border.left, border.top)
                                } else {
                                    (0.0, 0.0)
                                };

                                // 3. 新しい親を基準にした新しいローカル相対位置を逆算して割り出す
                                let new_inset_left = ph_abs_rect.x - (target_rect.x + border_l);
                                let new_inset_top = ph_abs_rect.y - (target_rect.y + border_t);

                                let new_inset = Rect {
                                    top: Val::Px(new_inset_top),
                                    right: Val::Auto,
                                    bottom: Val::Auto,
                                    left: Val::Px(new_inset_left),
                                };

                                if let Some(layout) = self.layouts.basic_layouts.get_mut(src_id) {
                                    layout.inset = new_inset;
                                }
                                if let Some(layout) =
                                    self.renders.base_basic_layouts.get_mut(src_id)
                                {
                                    layout.inset = new_inset;
                                }
                            }

                            // ドロップ先コンテナ（target_id）の末尾の子要素としてマウント
                            self.add_child(target_id, src_id);
                        } else {
                            // 【相対配置（Relative）】: マウス座標に基づいた子要素の動的並び替えアタッチ
                            if drag_prop.update_position {
                                let mouse_pos = self
                                    .events
                                    .current_pointer_position
                                    .unwrap_or(LayoutPoint::ZERO);
                                let insert_idx = self.calculate_insert_index(target_id, mouse_pos);

                                if let Some(parent_children) =
                                    self.topology.children.get_mut(target_id)
                                {
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

                        self.layouts.is_structure_dirty = true;
                    }

                    // 避難していた本物の子要素トポロジーを、元の要素（src_id）の配下へ自動復元
                    if let Some(ph_children) = self.topology.children.get(placeholder_id).cloned() {
                        for child_id in ph_children {
                            // 子要素の親ポインタを元の要素に書き戻し
                            self.topology.parents.insert(child_id, Some(src_id));

                            // 元の要素の子要素リストへ復旧
                            if let Some(src_children) = self.topology.children.get_mut(src_id) {
                                src_children.push(child_id);
                            }

                            // Taffy 側の親子構造も、元の要素に繋ぎ戻し
                            if let Some(&src_node) = self.layouts.taffy_nodes.get(src_id)
                                && let Some(&ph_node) = self.layouts.taffy_nodes.get(placeholder_id)
                                && let Some(&child_node) = self.layouts.taffy_nodes.get(child_id)
                            {
                                let _ = self.layouts.taffy.remove_child(ph_node, child_node);
                                let _ = self.layouts.taffy.add_child(src_node, child_node);
                            }
                        }

                        // プレースホルダー側は空にして破棄に備える
                        if let Some(ph_children_mut) =
                            self.topology.children.get_mut(placeholder_id)
                        {
                            ph_children_mut.clear();
                        }
                        self.mark_layout_dirty(src_id);
                        self.mark_layout_dirty(placeholder_id);
                    }

                    // コールバックを一時的に take して借用を完全に切り離して実行する
                    match drag_prop.drag_mode {
                        DragPayload::Element => {
                            let mut listener_opt = self
                                .events
                                .event_listeners
                                .get_mut(src_id)
                                .and_then(|l| l.on_entity_drop.take());
                            if let Some(mut listener) = listener_opt {
                                {
                                    let _guard = crate::ActiveElementGuard::new(src_id);
                                    listener(
                                        self,
                                        Element::from(src_id),
                                        drop_success.map(Element::from),
                                    );
                                }
                                if let Some(l) = self.events.event_listeners.get_mut(src_id) {
                                    l.on_entity_drop = Some(listener);
                                }
                            }
                        }
                        DragPayload::EntityId => {
                            let mut listener_opt = self
                                .events
                                .event_listeners
                                .get_mut(src_id)
                                .and_then(|l| l.on_id_drop.take());
                            if let Some(mut listener) = listener_opt {
                                {
                                    let _guard = crate::ActiveElementGuard::new(src_id);
                                    listener(self, src_id, drop_success);
                                }
                                if let Some(l) = self.events.event_listeners.get_mut(src_id) {
                                    l.on_id_drop = Some(listener);
                                }
                            }
                        }
                    }

                    // 位置情報の安全な回収がすべて完了した、この最末尾で初めてプレースホルダーを破棄
                    self.despawn_internal(placeholder_id);

                    // 離脱直後に位置を再移動評価して、通常のホバーを正しく復元
                    if let Some(pos) = self.events.current_pointer_position {
                        self.inject_pointer_move(pos);
                    }

                    self.mark_render_dirty(src_id);
                    return; // 早期リターン
                }

                let mut dirty_ids = smallvec::SmallVec::<[EntityId; 4]>::new();
                for (id, state) in self.layouts.scrollbar_styles.iter_mut() {
                    if state.v_thumb_dragged || state.h_thumb_dragged {
                        state.v_thumb_dragged = false;
                        state.h_thumb_dragged = false;
                        dirty_ids.push(id);
                    }
                }

                for id in dirty_ids {
                    self.mark_render_dirty(id);
                }

                if let Some(pressed_id) = self.events.interaction_states.pressed {
                    self.set_pressed(pressed_id, false);
                    self.set_dragged(pressed_id, false);
                    self.events.interaction_states.dragged = None;

                    if let Some(contents) = self.contents.input_contents.get_mut(pressed_id) {
                        contents.is_selecting = false;
                    }

                    // 1. on_mouse_input の発火（ボタンの種類を問わず常に呼ぶ）
                    let mut on_input = self
                        .events
                        .event_listeners
                        .get_mut(pressed_id)
                        .and_then(|l| l.on_mouse_input.take());
                    if let Some(mut handler) = on_input {
                        let _guard = crate::ActiveElementGuard::new(pressed_id);
                        handler(self, button, modifiers, state);
                        if let Some(l) = self.events.event_listeners.get_mut(pressed_id) {
                            l.on_mouse_input = Some(handler);
                        }
                    }

                    // 2. 同一要素上で離された場合の各種クリック解決
                    if self.events.interaction_states.hovered == Some(pressed_id) {
                        match button {
                            // 左クリックの解決
                            MouseButton::Left => {
                                let mut on_click = self
                                    .events
                                    .event_listeners
                                    .get_mut(pressed_id)
                                    .and_then(|l| l.on_click.take());
                                if let Some(mut handler) = on_click {
                                    let _guard = crate::ActiveElementGuard::new(pressed_id);
                                    handler(self);
                                    if let Some(l) = self.events.event_listeners.get_mut(pressed_id)
                                    {
                                        l.on_click = Some(handler);
                                    }
                                }
                            }
                            // 右クリックの解決（追加）
                            MouseButton::Right => {
                                let mut on_right = self
                                    .events
                                    .event_listeners
                                    .get_mut(pressed_id)
                                    .and_then(|l| l.on_right_click.take());
                                if let Some(mut handler) = on_right {
                                    let _guard = crate::ActiveElementGuard::new(pressed_id);
                                    handler(self);
                                    if let Some(l) = self.events.event_listeners.get_mut(pressed_id)
                                    {
                                        l.on_right_click = Some(handler);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    self.events.interaction_states.pressed = None;
                }
            }
        }
    }

    // ダブルクリック
    pub fn inject_pointer_double_click(&mut self, modifiers: Modifiers) {
        let _context_guard = bind_context(self);
        let current_hovered = self.events.interaction_states.hovered;

        if let Some(target_id) = current_hovered {
            let user_select = self
                .renders
                .visual_properties
                .get(target_id)
                .and_then(|v| v.user_select)
                .unwrap_or(UserSelect::None);

            if user_select == UserSelect::Text
                && let Some(pointer_pos) = self.events.current_pointer_position
            {
                if let Some(contents) = self.contents.input_contents.get(target_id) {
                    let text_val = contents.text.0.get();
                    let is_placeholder = text_val.is_empty()
                        && contents
                            .ime_state
                            .as_ref()
                            .map(|s| s.composition_text.is_empty())
                            .unwrap_or(true);

                    if is_placeholder && !contents.placeholder_select {
                        return;
                    }
                }

                let rect = self.outputs.rects[target_id];
                let (basic, _, _) = self.resolve_active_layouts(target_id);
                let border = self.get_physical_border(target_id, &basic);
                let padding = self.get_physical_padding(target_id, &basic);

                let local_x = pointer_pos.x - (rect.x + border.left + padding.left);
                let local_y = pointer_pos.y - (rect.y + border.top + padding.top);

                if let Some(layout) = self.get_or_create_layout(target_id) {
                    let (clicked_index, is_trailing) = self
                        .system
                        .text_engine
                        .hit_test_point(&layout, local_x, local_y);
                    let final_index = if is_trailing {
                        clicked_index + 1
                    } else {
                        clicked_index
                    };

                    if let Some(text) = self.contents.text_contents.get(target_id) {
                        let text_u16: Vec<u16> = text.encode_utf16().collect();

                        // 高精度な文節境界を抽出
                        let range = crate::find_word_boundaries(&text_u16, final_index);

                        self.outputs
                            .text_selections
                            .insert(target_id, range.clone());
                        // アンカー開始を文節左端にセット
                        self.outputs
                            .selection_start_index
                            .insert(target_id, range.start);
                        self.update_selection_rects(target_id); // 選択矩形を更新

                        if let Some(contents) = self.contents.input_contents.get_mut(target_id) {
                            contents.selected_range = range;
                            contents.selection_reversed = false; // キャレットは右端に配置
                            crate::update_input_caret_position(self, target_id);
                        }

                        self.mark_render_dirty(target_id);
                    }
                }
            }
        }
    }

    /// 外部で計算された論理ピクセルスクロール移動量 (scroll_x, scroll_y) を注入し、
    /// バブリングによる自動スクロール処理、またはユーザーイベントハンドラへの配送を行います。
    pub fn inject_mouse_wheel(&mut self, scroll_x: f32, scroll_y: f32) {
        let _context_guard = bind_context(self);

        let mut curr = self.events.interaction_states.hovered;
        let mut handled = false;

        // イベントバブリング: ホバー要素から親へ辿る
        while let Some(curr_id) = curr {
            // 個別に定義された `on_mouse_wheel` ハンドラがあれば最優先実行
            let mut on_wheel = self
                .events
                .event_listeners
                .get_mut(curr_id)
                .and_then(|l| l.on_mouse_wheel.take());

            if let Some(mut handler) = on_wheel {
                let _guard = crate::ActiveElementGuard::new(curr_id);
                handler(self, scroll_x, scroll_y);
                if let Some(l) = self.events.event_listeners.get_mut(curr_id) {
                    l.on_mouse_wheel = Some(handler);
                }
                handled = true; // イベントが消費されたため、これ以降のコンテナスクロールは行わない
                break;
            }

            // ユーザーハンドラがない場合、要素がスクロールコンテナであるか判定
            let mask = self.topology.active_masks[curr_id];
            if mask.has(STYLE_OVERFLOW) {
                let (basic, _, _) = self.resolve_active_layouts(curr_id);

                let mut scrolled = false;

                // 縦方向スクロール
                if scroll_y != 0.0
                    && (basic.overflow.y == Overflow::Scroll
                        || basic.overflow.y == Overflow::Hidden)
                    && self.scroll_by(curr_id, 0.0, scroll_y)
                {
                    scrolled = true;
                }

                // 横方向スクロール
                if scroll_x != 0.0
                    && (basic.overflow.x == Overflow::Scroll
                        || basic.overflow.x == Overflow::Hidden)
                    && self.scroll_by(curr_id, scroll_x, 0.0)
                {
                    scrolled = true;
                }

                if scrolled {
                    handled = true;
                    break; // スクロールを実行したためバブリングを終了
                }
            }

            // 先祖へ伝播
            curr = self.topology.parents.get(curr_id).copied().flatten();
        }
    }

    pub fn inject_keyboard_key(
        &mut self,
        key: VirtualKey,
        state: ElementState,
        modifiers: Modifiers,
    ) {
        let _context_guard = bind_context(self);

        // Tabキー押下時は個別のフォーカス対象へのイベント配信前に巡回処理を実行
        if state == ElementState::Pressed && key == VirtualKey::TAB {
            self.cycle_keyboard_focus(modifiers.shift);
            return;
        }

        if let Some(focused_id) = self.events.interaction_states.focused {
            // フォーカス中に Enter または Space が押されたら自動的にクリックをエミュレートする
            if state == ElementState::Pressed
                && (key == VirtualKey::RETURN || key == VirtualKey::SPACE)
            {
                let mut on_click = self
                    .events
                    .event_listeners
                    .get_mut(focused_id)
                    .and_then(|l| l.on_click.take());

                if let Some(mut handler) = on_click {
                    let _guard = crate::ActiveElementGuard::new(focused_id);
                    handler(self);
                    if let Some(l) = self.events.event_listeners.get_mut(focused_id) {
                        l.on_click = Some(handler);
                    }
                }
                return;
            }
            // 内部で完結する全選択（Ctrl+A）のみを自動処理
            if state == ElementState::Pressed && modifiers.ctrl {
                let user_select = self
                    .renders
                    .visual_properties
                    .get(focused_id)
                    .and_then(|v| v.user_select)
                    .unwrap_or(UserSelect::None);

                if key == VirtualKey::A && user_select == UserSelect::Text {
                    if let Some(layout) = self.get_or_create_layout(focused_id)
                        && let Some(text) = self.contents.text_contents.get(focused_id)
                    {
                        let u16_len = text.encode_utf16().count();
                        let full_range = 0..u16_len;

                        self.outputs
                            .text_selections
                            .insert(focused_id, full_range.clone());

                        self.update_selection_rects(focused_id);

                        if let Some(contents) = self.contents.input_contents.get_mut(focused_id) {
                            contents.selected_range = full_range;
                            contents.selection_reversed = false;
                            crate::update_input_caret_position(self, focused_id);
                        }
                        self.mark_render_dirty(focused_id);
                    }
                    return;
                }
            }

            let mut on_key = self
                .events
                .event_listeners
                .get_mut(focused_id)
                .and_then(|l| l.on_keyboard_input.take());
            if let Some(mut handler) = on_key {
                let _guard = crate::ActiveElementGuard::new(focused_id);
                handler(self, key, modifiers, state);
                if let Some(l) = self.events.event_listeners.get_mut(focused_id) {
                    l.on_keyboard_input = Some(handler);
                }
            }
        }
    }

    /// キーボードフォーカスを次の適格な要素へ巡回させます
    pub fn cycle_keyboard_focus(&mut self, reverse: bool) {
        if self.layouts.flat_dfs_sequence.is_empty() {
            return;
        }

        let len = self.layouts.flat_dfs_sequence.len();

        // 現在フォーカスされている要素のインデックスを特定（無ければ探索方向の末端から開始）
        let current_focused = self.events.interaction_states.focused;
        let start_idx = current_focused
            .and_then(|id| self.layouts.flat_dfs_sequence.iter().position(|&x| x == id))
            .unwrap_or(if reverse { len - 1 } else { 0 });

        let mut idx = start_idx;
        loop {
            // インデックスの増減と循環
            if reverse {
                idx = if idx == 0 { len - 1 } else { idx - 1 };
            } else {
                idx = if idx == len - 1 { 0 } else { idx + 1 };
            }

            // 1周して元の位置に戻ってきた場合は、他にフォーカス可能な要素がないため終了
            if idx == start_idx {
                break;
            }

            let candidate_id = self.layouts.flat_dfs_sequence[idx];

            if self.is_keyboard_focusable(candidate_id) {
                // 古い要素のフォーカスを外し、新しい要素へフォーカスを設定
                if let Some(old_id) = self.events.interaction_states.focused {
                    self.set_focused(old_id, false);
                }
                self.set_focused(candidate_id, true);
                self.events.interaction_states.focused = Some(candidate_id);

                // WebView2 要素だった場合はシステム側にフォーカスをプログラム駆動で移譲
                if self.topology.active_masks[candidate_id].has(COMP_WEBVIEW_CONTENT) {
                    // 通常のレンダラーから focus_webview を呼び出すためここでは何もしない
                }

                self.mark_render_dirty(candidate_id);
                break;
            }
        }
    }

    pub fn inject_character(&mut self, c: char) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.events.interaction_states.focused {
            let mut on_char = self
                .events
                .event_listeners
                .get_mut(focused_id)
                .and_then(|l| l.on_char_input.take());
            if let Some(mut handler) = on_char {
                let _guard = crate::ActiveElementGuard::new(focused_id);
                handler(self, c);
                if let Some(l) = self.events.event_listeners.get_mut(focused_id) {
                    l.on_char_input = Some(handler);
                }
            }
        }
    }

    pub fn inject_ime(&mut self, ime_state: ImeState) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.events.interaction_states.focused {
            let mut on_ime = self
                .events
                .event_listeners
                .get_mut(focused_id)
                .and_then(|l| l.on_ime.take());
            if let Some(mut handler) = on_ime {
                let _guard = crate::ActiveElementGuard::new(focused_id);
                handler(self, ime_state);
                if let Some(l) = self.events.event_listeners.get_mut(focused_id) {
                    l.on_ime = Some(handler);
                }
            }
        }
    }

    pub fn inject_file_dropped(&mut self, paths: Vec<PathBuf>) {
        let _context_guard = bind_context(self);
        if let Some(target_id) = self.events.interaction_states.hovered {
            let mut on_drop = self
                .events
                .event_listeners
                .get_mut(target_id)
                .and_then(|l| l.on_file_dropped.take());
            if let Some(mut handler) = on_drop {
                let _guard = crate::ActiveElementGuard::new(target_id);
                handler(self, paths);
                if let Some(l) = self.events.event_listeners.get_mut(target_id) {
                    l.on_file_dropped = Some(handler);
                }
            }
        }
    }

    /// 外部から提供されたテキストを、現在フォーカスされている入力要素にペーストします。
    #[inline]
    pub fn inject_paste(&mut self, text: &str) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.events.interaction_states.focused
            && self.topology.active_masks[focused_id].has(COMP_INPUT_CONTENT)
            && let Some(contents) = self.contents.input_contents.get_mut(focused_id)
        {
            OutputStore::inject_paste_internal(focused_id, text, &mut self.outputs, contents);

            crate::update_input_caret_position(self, focused_id);
            self.mark_render_dirty(focused_id);
        }
    }

    /// Undo (元に戻す) のインジェクション
    #[inline]
    pub fn inject_undo(&mut self) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.events.interaction_states.focused
            && self.topology.active_masks[focused_id].has(COMP_INPUT_CONTENT)
            && let Some(contents) = self.contents.input_contents.get_mut(focused_id)
            && let Some((prev_text, prev_sel)) = contents.undo_stack.pop()
        {
            OutputStore::inject_undo_internal(
                focused_id,
                prev_sel,
                prev_text,
                &mut self.outputs,
                contents,
            );

            crate::update_input_caret_position(self, focused_id);
            self.mark_render_dirty(focused_id);
        }
    }

    /// Redo (やり直し) のインジェクション
    #[inline]
    pub fn inject_redo(&mut self) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.events.interaction_states.focused
            && self.topology.active_masks[focused_id].has(COMP_INPUT_CONTENT)
            && let Some(contents) = self.contents.input_contents.get_mut(focused_id)
            && let Some((next_text, next_sel)) = contents.redo_stack.pop()
        {
            OutputStore::inject_redo_internal(
                focused_id,
                next_sel,
                next_text,
                &mut self.outputs,
                contents,
            );

            crate::update_input_caret_position(self, focused_id);
            self.mark_render_dirty(focused_id);
        }
    }

    /// 切り取り (Ctrl+X) の実行と削除後のテキスト取得
    #[inline]
    pub fn inject_cut(&mut self) -> Option<String> {
        let _context_guard = bind_context(self);
        let focused_id = self.events.interaction_states.focused?;
        let user_select = self
            .renders
            .visual_properties
            .get(focused_id)
            .and_then(|v| v.user_select)
            .unwrap_or(UserSelect::None);

        if user_select == UserSelect::Text
            && let Some(range) = self.outputs.text_selections.get(focused_id).cloned()
            && range.start < range.end
            && let Some(text) = self.contents.text_contents.get(focused_id)
        {
            let u16_text: Vec<u16> = text.encode_utf16().collect();
            let slice = &u16_text[range.start.min(u16_text.len())..range.end.min(u16_text.len())];
            let cut_text = String::from_utf16(slice).ok()?;

            // 対象が Input コントロールである場合のみ、切り取り削除上書きを実行
            if self.topology.active_masks[focused_id].has(COMP_INPUT_CONTENT)
                && let Some(contents) = self.contents.input_contents.get_mut(focused_id)
            {
                OutputStore::inject_cut_internal(focused_id, range, &mut self.outputs, contents);

                crate::update_input_caret_position(self, focused_id);
                self.mark_render_dirty(focused_id);
            }

            return Some(cut_text);
        }

        None
    }
}
