use crate::{
    Context, Element, ElementState, EntityId, ImeState, LayoutPoint, Modifiers, MouseButton,
    VirtualKey,
};
use std::path::PathBuf;

macro_rules! define_event_dispatchers {
    (
        $(
            $fn_name:ident, $field_name:ident $(, $arg_name:ident : $arg_type:ty)*;
        )*
    ) => {
        $(
            #[inline]
            #[allow(dead_code)]
            pub(crate) fn $fn_name(
                cx: &mut Context,
                id: EntityId,
                $($arg_name : $arg_type),*
            ) {
                // events.evt_listeners から該当のハンドラを一時的に take する
                if let Some(mut handler) = cx
                    .events
                    .evt_listeners
                    .get_mut(id)
                    .and_then(|l| l.$field_name.take())
                {
                    let _guard = crate::ActiveElementGuard::new(id);

                    // コールバックを安全に実行
                    handler(cx, $($arg_name),*);

                    // 実行後コールバックを書き戻す
                    if let Some(l) = cx.events.evt_listeners.get_mut(id) {
                        l.$field_name = Some(handler);
                    }
                }
            }
        )*
    };
}

pub type ClickCallback = Box<dyn FnMut(&mut Context) + 'static>;
pub type MouseCallback =
    Box<dyn FnMut(&mut Context, MouseButton, Modifiers, ElementState) + 'static>;
pub type CursorMovedCallback = Box<dyn FnMut(&mut Context, LayoutPoint) + 'static>;
pub type MouseWheelCallback = Box<dyn FnMut(&mut Context, f32, f32) + 'static>;
pub type DragCallback = Box<dyn FnMut(&mut Context, LayoutPoint) + 'static>;
pub type KeyCallback = Box<dyn FnMut(&mut Context, VirtualKey, Modifiers, ElementState) + 'static>;
pub type CharCallback = Box<dyn FnMut(&mut Context, char) + 'static>;
pub type ImeCallback = Box<dyn FnMut(&mut Context, ImeState) + 'static>;
pub type FileDropCallback = Box<dyn FnMut(&mut Context, Vec<PathBuf>) + 'static>;
pub type FileDragCallback = Box<dyn FnMut(&mut Context) + 'static>;
pub type SimpleCallback = Box<dyn FnMut(&mut Context) + 'static>;

// 各コールバックの引数: (context, ドラッグ元のElement, 現在ホバーまたはドロップされた対象のElement)
// 失敗時は対象が None
pub type EntityDragCallback = Box<dyn FnMut(&mut Context, Element, Option<Element>) + 'static>;
pub type IdDragCallback = Box<dyn FnMut(&mut Context, EntityId, Option<EntityId>) + 'static>;
pub type EntityDropCallback = Box<dyn FnMut(&mut Context, Element, Option<Element>) + 'static>;
pub type IdDropCallback = Box<dyn FnMut(&mut Context, EntityId, Option<EntityId>) + 'static>;
// ドラッグ開始時のコールバック型。生成されたプレースホルダーの Element を受け取れます。
// 引数: (context, ドラッグ元のオリジナル要素, 生成されたプレースホルダー要素)
pub type DragStartCallback = Box<dyn FnMut(&mut Context, Element, Element) + 'static>;

/// Events bound to individual elements.
#[derive(Default)]
#[allow(clippy::struct_field_names)]
pub struct EventListeners {
    pub on_click: Option<ClickCallback>,
    pub on_right_click: Option<SimpleCallback>,
    pub on_mouse_input: Option<MouseCallback>,
    pub on_mouse_enter: Option<SimpleCallback>,
    pub on_mouse_leave: Option<SimpleCallback>,
    pub on_cursor_moved: Option<CursorMovedCallback>,
    pub on_mouse_wheel: Option<MouseWheelCallback>,
    pub on_drag: Option<DragCallback>,
    pub on_hover: Option<SimpleCallback>,
    pub on_focus: Option<SimpleCallback>,
    pub on_blur: Option<SimpleCallback>,
    pub on_disable: Option<SimpleCallback>,
    pub on_active: Option<SimpleCallback>,
    pub on_select: Option<SimpleCallback>,
    pub on_keyboard_input: Option<KeyCallback>,
    pub on_char_input: Option<CharCallback>,
    pub on_ime: Option<ImeCallback>,
    pub on_file_dropped: Option<FileDropCallback>,
    pub on_file_drag_enter: Option<FileDragCallback>,
    pub on_file_drag_leave: Option<FileDragCallback>,
    pub on_dnd_entity_drag: Option<EntityDragCallback>,
    pub on_dnd_id_drag: Option<IdDragCallback>,
    pub on_dnd_entity_drop: Option<EntityDropCallback>,
    pub on_dnd_id_drop: Option<IdDropCallback>,
    pub on_dnd_drag_start: Option<DragStartCallback>,
}

define_event_dispatchers! {
    // マウス・クリック
    handle_on_click, on_click;
    handle_on_right_click, on_right_click;
    handle_on_mouse_input, on_mouse_input, button: MouseButton, modifiers: Modifiers, state: ElementState;
    handle_on_mouse_enter, on_mouse_enter;
    handle_on_mouse_leave, on_mouse_leave;
    handle_on_cursor_moved, on_cursor_moved, pos: LayoutPoint;
    handle_on_mouse_wheel, on_mouse_wheel, delta_x: f32, delta_y: f32;
    handle_on_drag, on_drag, delta: LayoutPoint;

    // ステート変化
    handle_on_hover, on_hover;
    handle_on_focus, on_focus;
    handle_on_blur, on_blur;
    handle_on_disable, on_disable;
    handle_on_active, on_active;
    handle_on_select, on_select;

    // キーボード・入力
    handle_on_keyboard_input, on_keyboard_input, key: VirtualKey, modifiers: Modifiers, state: ElementState;
    handle_on_char_input, on_char_input, ch: char;
    handle_on_ime, on_ime, info: ImeState;

    // ファイルドロップ
    handle_on_file_dropped, on_file_dropped, paths: Vec<PathBuf>;
    handle_on_file_drag_enter, on_file_drag_enter;
    handle_on_file_drag_leave, on_file_drag_leave;

    // ドラッグ＆ドロップ
    handle_on_dnd_entity_drag, on_dnd_entity_drag, origin: Element, target: Option<Element>;
    handle_on_dnd_id_drag, on_dnd_id_drag, origin_id: EntityId, target_id: Option<EntityId>;
    handle_on_dnd_entity_drop, on_dnd_entity_drop, origin: Element, target: Option<Element>;
    handle_on_dnd_id_drop, on_dnd_id_drop, origin_id: EntityId, target_id: Option<EntityId>;
    handle_on_dnd_drag_start, on_dnd_drag_start, origin: Element, placeholder: Element;
}

impl std::fmt::Debug for EventListeners {
    #[allow(clippy::too_many_lines)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventListeners")
            .field("on_click", &self.on_click.as_ref().map(|_| "FnMut"))
            .field(
                "on_right_click",
                &self.on_right_click.as_ref().map(|_| "FnMut"),
            )
            .field(
                "on_mouse_input",
                &self.on_mouse_input.as_ref().map(|_| "MouseCallback"),
            )
            .field(
                "on_mouse_enter",
                &self.on_mouse_enter.as_ref().map(|_| "FnMut"),
            )
            .field(
                "on_mouse_leave",
                &self.on_mouse_leave.as_ref().map(|_| "FnMut"),
            )
            .field(
                "on_cursor_moved",
                &self.on_cursor_moved.as_ref().map(|_| "FnMut(Point)"),
            )
            .field(
                "on_mouse_wheel",
                &self.on_mouse_wheel.as_ref().map(|_| "FnMut(f32)"),
            )
            .field("on_drag", &self.on_drag.as_ref().map(|_| "FnMut(Point)"))
            .field(
                "on_keyboard_input",
                &self.on_keyboard_input.as_ref().map(|_| "KeyCallback"),
            )
            .field(
                "on_char_input",
                &self.on_char_input.as_ref().map(|_| "FnMut(char)"),
            )
            .field("on_ime", &self.on_ime.as_ref().map(|_| "FnMut(ImeState)"))
            .field(
                "on_file_dropped",
                &self.on_file_dropped.as_ref().map(|_| "FnMut(Vec<PathBuf>)"),
            )
            .field(
                "on_file_drag_enter",
                &self.on_file_drag_enter.as_ref().map(|_| "FnMut"),
            )
            .field(
                "on_file_drag_leave",
                &self.on_file_drag_leave.as_ref().map(|_| "FnMut"),
            )
            .field("on_hover", &self.on_hover.as_ref().map(|_| "FnMut"))
            .field("on_focus", &self.on_focus.as_ref().map(|_| "FnMut"))
            .field("on_blur", &self.on_blur.as_ref().map(|_| "FnMut"))
            .field("on_disable", &self.on_disable.as_ref().map(|_| "FnMut"))
            .field("on_active", &self.on_active.as_ref().map(|_| "FnMut"))
            .field("on_select", &self.on_select.as_ref().map(|_| "FnMut"))
            .field(
                "on_dnd_entity_drag",
                &self
                    .on_dnd_entity_drag
                    .as_ref()
                    .map(|_| "FnMut(EntityId, Option<EntityId>)"),
            )
            .field(
                "on_dnd_id_drag",
                &self
                    .on_dnd_id_drag
                    .as_ref()
                    .map(|_| "FnMut(EntityId, Option<EntityId>)"),
            )
            .field(
                "on_dnd_entity_drop",
                &self
                    .on_dnd_entity_drop
                    .as_ref()
                    .map(|_| "FnMut(EntityId, Option<EntityId>)"),
            )
            .field(
                "on_dnd_id_drop",
                &self
                    .on_dnd_id_drop
                    .as_ref()
                    .map(|_| "FnMut(EntityId, Option<EntityId>)"),
            )
            .field(
                "on_dnd_drag_start",
                &self
                    .on_dnd_drag_start
                    .as_ref()
                    .map(|_| "FnMut(EntityId, Element)"),
            )
            .finish()
    }
}
