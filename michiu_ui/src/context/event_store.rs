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
}
