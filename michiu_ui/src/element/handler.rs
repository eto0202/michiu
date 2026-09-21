use crate::{
    Context, EffectCategory, Element, ElementState, EntityId, EventListeners, ImeState,
    LayoutPoint, MichiuSoA, Modifiers, MouseButton, Prop, StateFlag, VirtualKey, with_context,
};
use std::path::PathBuf;

impl Element {
    /// この要素に対応する `EventListeners` が `SoA` 上に存在しない場合は新規に作成し、
    /// 可変参照を取得して渡されたクロージャを実行。
    #[inline]
    pub(crate) fn get_or_create_listeners<R>(self, f: impl FnOnce(&mut EventListeners) -> R) -> R {
        with_context(|cx| {
            // SparseSecondaryMap にキーが存在しない場合は Default (すべて None) で差し込む
            if !cx.events.evt_listeners.contains_key(self.id) {
                cx.events
                    .evt_listeners
                    .insert(self.id, EventListeners::default());
            }
            let listeners = cx.events.evt_listeners.at_mut(self.id);
            f(listeners)
        })
    }

    /// Registers an event that fires when the left mouse button is released.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_click<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_click_with(move |_cx| f())
    }

    /// Registers an event that fires when the left mouse button is released.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_click_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_click.take() {
                let mut f = f;
                l.on_click = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_click = Some(Box::new(f));
            }
        });
        self
    }

    /// Registers an event that fires when the right mouse button is released.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_right_click<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_right_click_with(move |_cx| f())
    }

    /// Registers an event that fires when the right mouse button is released.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_right_click_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_right_click.take() {
                let mut f = f;
                l.on_right_click = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_right_click = Some(Box::new(f));
            }
        });
        self
    }

    /// Register an event that fires when raw mouse button input occurs.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_mouse_input<F>(self, mut f: F) -> Self
    where
        F: FnMut(MouseButton, Modifiers, ElementState) + 'static,
    {
        self.on_mouse_input_with(move |_cx, btn, mods, state| f(btn, mods, state))
    }

    /// Register an event that fires when raw mouse button input occurs.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_mouse_input_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, MouseButton, Modifiers, ElementState) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_mouse_input.take() {
                let mut f = f;
                l.on_mouse_input = Some(Box::new(move |cx, btn, mods, state| {
                    existing(cx, btn, mods, state);
                    f(cx, btn, mods, state);
                }));
            } else {
                l.on_mouse_input = Some(Box::new(f));
            }
        });
        self
    }

    /// Register an event that fires when the mouse pointer enters the visible bounds of an element.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_mouse_enter<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_mouse_enter_with(move |_cx| f())
    }

    /// Register an event that fires when the mouse pointer enters the visible bounds of an element.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_mouse_enter_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_mouse_enter.take() {
                let mut f = f;
                l.on_mouse_enter = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_mouse_enter = Some(Box::new(f));
            }
        });
        self
    }

    /// Register an event that fires when the mouse pointer moves outside the visible boundaries of an element.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_mouse_leave<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_mouse_leave_with(move |_cx| f())
    }

    /// Register an event that fires when the mouse pointer moves outside the visible boundaries of an element.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_mouse_leave_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_mouse_leave.take() {
                let mut f = f;
                l.on_mouse_leave = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_mouse_leave = Some(Box::new(f));
            }
        });
        self
    }

    /// Registers an event that fires when the mouse pointer moves within an element.
    ///
    /// The callback receives a `LayoutPoint`,
    /// which is a logical coordinate system with the element's top-left corner as the origin (0, 0).
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_cursor_moved<F>(self, mut f: F) -> Self
    where
        F: FnMut(LayoutPoint) + 'static,
    {
        self.on_cursor_moved_with(move |_cx, point| f(point))
    }

    /// Registers an event that fires when the mouse pointer moves within an element.
    ///
    /// The callback receives a `LayoutPoint`,
    /// which is a logical coordinate system with the element's top-left corner as the origin (0, 0).
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_cursor_moved_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, LayoutPoint) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_cursor_moved.take() {
                let mut f = f;
                l.on_cursor_moved = Some(Box::new(move |cx, p| {
                    existing(cx, p);
                    f(cx, p);
                }));
            } else {
                l.on_cursor_moved = Some(Box::new(f));
            }
        });
        self
    }

    /// Registers an event that fires when the mouse wheel is scrolled over this element.
    ///
    /// The callback is passed the values of `scroll_x` and `scroll_y`, converted to logical pixels.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_mouse_wheel<F>(self, mut f: F) -> Self
    where
        F: FnMut(f32, f32) + 'static,
    {
        self.on_mouse_wheel_with(move |_cx, sx, sy| f(sx, sy))
    }

    /// Registers an event that fires when the mouse wheel is scrolled over this element.
    ///
    /// The callback is passed the values of `scroll_x` and `scroll_y`, converted to logical pixels.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_mouse_wheel_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, f32, f32) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_mouse_wheel.take() {
                let mut f = f;
                l.on_mouse_wheel = Some(Box::new(move |cx, sx, sy| {
                    existing(cx, sx, sy);
                    f(cx, sx, sy);
                }));
            } else {
                l.on_mouse_wheel = Some(Box::new(f));
            }
        });
        self
    }

    /// Registers an event that fires when an element is dragged
    /// (moving the mouse while holding down the left mouse button).
    ///
    /// The callback receives a `LayoutPoint`, which represents the difference in position from the previous frame.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_drag<F>(self, mut f: F) -> Self
    where
        F: FnMut(LayoutPoint) + 'static,
    {
        self.on_drag_with(move |_cx, delta| f(delta))
    }

    /// Registers an event that fires when an element is dragged
    /// (moving the mouse while holding down the left mouse button).
    ///
    /// The callback receives a `LayoutPoint`, which represents the difference in position from the previous frame.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_drag_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, LayoutPoint) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_drag.take() {
                let mut f = f;
                l.on_drag = Some(Box::new(move |cx, p| {
                    existing(cx, p);
                    f(cx, p);
                }));
            } else {
                l.on_drag = Some(Box::new(f));
            }
        });
        self
    }

    /// Register an event that fires the moment the mouse hovers over an element.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_hover<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_hover_with(move |_cx| f())
    }

    /// Register an event that fires the moment the mouse hovers over an element.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_hover_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_hover.take() {
                let mut f = f;
                l.on_hover = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_hover = Some(Box::new(f));
            }
        });
        self
    }

    /// Register an event that fires when a physical keyboard key is pressed
    /// (valid only when the element has focus).
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_keyboard_input<F>(self, mut f: F) -> Self
    where
        F: FnMut(VirtualKey, Modifiers, ElementState) + 'static,
    {
        self.on_keyboard_input_with(move |_cx, key, mods, state| f(key, mods, state))
    }

    /// Register an event that fires when a physical keyboard key is pressed
    /// (valid only when the element has focus).
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_keyboard_input_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, VirtualKey, Modifiers, ElementState) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_keyboard_input.take() {
                let mut f = f;
                l.on_keyboard_input = Some(Box::new(move |cx, key, mods, state| {
                    existing(cx, key, mods, state);
                    f(cx, key, mods, state);
                }));
            } else {
                l.on_keyboard_input = Some(Box::new(f));
            }
        });
        self
    }

    /// Register an event that fires when a single character is entered and confirmed.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_char_input<F>(self, mut f: F) -> Self
    where
        F: FnMut(char) + 'static,
    {
        self.on_char_input_with(move |_cx, c| f(c))
    }

    /// Register an event that fires when a single character is entered and confirmed.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_char_input_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, char) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_char_input.take() {
                let mut f = f;
                l.on_char_input = Some(Box::new(move |cx, c| {
                    existing(cx, c);
                    f(cx, c);
                }));
            } else {
                l.on_char_input = Some(Box::new(f));
            }
        });
        self
    }

    /// Register an event to capture updates to text converted by the IME, the caret, and the confirmed text string.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_ime<F>(self, mut f: F) -> Self
    where
        F: FnMut(ImeState) + 'static,
    {
        self.on_ime_with(move |_cx, state| f(state))
    }

    /// Register an event to capture updates to text converted by the IME, the caret, and the confirmed text string.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_ime_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, ImeState) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_ime.take() {
                let mut f = f;
                l.on_ime = Some(Box::new(move |cx, state| {
                    existing(cx, state.clone());
                    f(cx, state);
                }));
            } else {
                l.on_ime = Some(Box::new(f));
            }
        });
        self
    }

    /// Register an event that fires when files or folders are dragged and dropped onto this element.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_file_dropped<F>(self, mut f: F) -> Self
    where
        F: FnMut(Vec<PathBuf>) + 'static,
    {
        self.on_file_dropped_with(move |_cx, paths| f(paths))
    }

    /// Register an event that fires when files are dropped onto this element.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_file_dropped_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, Vec<PathBuf>) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_file_dropped.take() {
                let mut f = f;
                l.on_file_dropped = Some(Box::new(move |cx, paths| {
                    existing(cx, paths.clone());
                    f(cx, paths);
                }));
            } else {
                l.on_file_dropped = Some(Box::new(f));
            }
        });
        self
    }

    /// Register an event that fires when a file is dragged onto an element.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_file_drag_enter<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_file_drag_enter_with(move |_cx| f())
    }

    /// Register an event that fires when a file is dragged onto an element.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_file_drag_enter_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_file_drag_enter.take() {
                let mut f = f;
                l.on_file_drag_enter = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_file_drag_enter = Some(Box::new(f));
            }
        });
        self
    }

    /// Register an event that fires when a file is dragged off an element.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_file_drag_leave<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_file_drag_leave_with(move |_cx| f())
    }

    /// Register an event that fires when a file is dragged off an element.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_file_drag_leave_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_file_drag_leave.take() {
                let mut f = f;
                l.on_file_drag_leave = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_file_drag_leave = Some(Box::new(f));
            }
        });
        self
    }

    /// Register an event that fires when an element gains input focus for the first time.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_focus<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_focus_with(move |_cx| f())
    }

    /// Register an event that fires when an element gains input focus for the first time.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_focus_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_focus.take() {
                let mut f = f;
                l.on_focus = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_focus = Some(Box::new(f));
            }
        });
        self
    }

    /// Register an event that fires when the element loses focus—for example, when another element is clicked.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_blur<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_blur_with(move |_cx| f())
    }

    /// Register an event that fires when the element loses focus—for example, when another element is clicked.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_blur_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_blur.take() {
                let mut f = f;
                l.on_blur = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_blur = Some(Box::new(f));
            }
        });
        self
    }

    /// Register an event that fires the moment an element is disabled.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_disable<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_disable_with(move |_cx| f())
    }

    /// Register an event that fires the moment an element is disabled.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_disable_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_disable.take() {
                let mut f = f;
                l.on_disable = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_disable = Some(Box::new(f));
            }
        });
        self
    }

    /// Register an event that fires the moment an element is actived.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_active<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_active_with(move |_cx| f())
    }

    /// Register an event that fires the moment an element is actived.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_active_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_active.take() {
                let mut f = f;
                l.on_active = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_active = Some(Box::new(f));
            }
        });
        self
    }

    /// Register an event that fires the moment an element is selected.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_select<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_select_with(move |_cx| f())
    }

    /// Register an event that fires the moment an element is selected.
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_select_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_select.take() {
                let mut f = f;
                l.on_select = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_select = Some(Box::new(f));
            }
        });
        self
    }

    /// Register an event that is called every frame during a dnd drag operation.
    ///
    /// Arguments: (Source `Element`, `Element` of the currently overlapping drop destination)
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_dnd_element_drag<F>(self, mut f: F) -> Self
    where
        F: FnMut(Element, Option<Element>) + 'static,
    {
        self.on_dnd_element_drag_with(move |_cx, src, dst| f(src, dst))
    }

    /// Register an event that is called every frame during a dnd drag operation.
    ///
    /// Arguments: (Source `Element`, `Element` of the currently overlapping drop destination)
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_dnd_element_drag_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, Element, Option<Element>) + 'static,
    {
        self.get_or_create_listeners(|l| {
            l.on_dnd_entity_drag = Some(Box::new(f));
        });
        self
    }

    /// Register an event that is triggered every frame during a drag-and-drop operation.
    ///
    /// Arguments: (Source `EntityId`, `EntityId` of the currently overlapping element)
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_dnd_id_drag<F>(self, mut f: F) -> Self
    where
        F: FnMut(EntityId, Option<EntityId>) + 'static,
    {
        self.on_dnd_id_drag_with(move |_cx, src, dst| f(src, dst))
    }

    /// Register an event that is triggered every frame during a drag-and-drop operation.
    ///
    /// Arguments: (Source `EntityId`, `EntityId` of the currently overlapping element)
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_dnd_id_drag_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, EntityId, Option<EntityId>) + 'static,
    {
        self.get_or_create_listeners(|l| {
            l.on_dnd_id_drag = Some(Box::new(f));
        });
        self
    }

    /// Register an event that is triggered when a drop is completed
    ///  (whether it succeeds or fails due to being outside the area).
    ///
    /// Arguments: (Source `Element`, Destination `Element` (None if the operation fails))
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_dnd_element_drop<F>(self, mut f: F) -> Self
    where
        F: FnMut(Element, Option<Element>) + 'static,
    {
        self.on_dnd_element_drop_with(move |_cx, src, dst| f(src, dst))
    }

    /// Register an event that is triggered when a drop is completed
    ///  (whether it succeeds or fails due to being outside the area).
    ///
    /// Arguments: (Source `Element`, Destination `Element` (None if the operation fails))
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_dnd_element_drop_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, Element, Option<Element>) + 'static,
    {
        self.get_or_create_listeners(|l| {
            l.on_dnd_entity_drop = Some(Box::new(f));
        });
        self
    }

    /// Register an event that is triggered when a drop is completed
    ///  (whether it succeeds or fails due to being outside the area).
    ///
    /// Arguments: (Source `EntityId`, Destination `EntityId` (None if the operation fails))
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_dnd_id_drop<F>(self, mut f: F) -> Self
    where
        F: FnMut(EntityId, Option<EntityId>) + 'static,
    {
        self.on_dnd_id_drop_with(move |_cx, src, dst| f(src, dst))
    }

    /// Register an event that is triggered when a drop is completed
    ///  (whether it succeeds or fails due to being outside the area).
    ///
    /// Arguments: (Source `EntityId`, Destination `EntityId` (None if the operation fails))
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_dnd_id_drop_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, EntityId, Option<EntityId>) + 'static,
    {
        self.get_or_create_listeners(|l| {
            l.on_dnd_id_drop = Some(Box::new(f));
        });
        self
    }

    /// Registers an event that is called when dragging begins (the moment the placeholder is created).
    ///
    /// Arguments: (original `Element`, created placeholder `Element`)
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_dnd_drag_start<F>(self, mut f: F) -> Self
    where
        F: FnMut(Element, Element) + 'static,
    {
        self.on_dnd_drag_start_with(move |_cx, src, placeholder| f(src, placeholder))
    }

    /// Registers an event that is called when dragging begins (the moment the placeholder is created).
    ///
    /// Arguments: (original `Element`, created placeholder `Element`)
    ///
    /// If called multiple times, events are added and executed in the order they were registered.
    #[must_use]
    #[inline]
    pub fn on_dnd_drag_start_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, Element, Element) + 'static,
    {
        self.get_or_create_listeners(|l| {
            l.on_dnd_drag_start = Some(Box::new(f));
        });
        self
    }

    /// Automatically maps an element's `StateFlag::Actived` based on signals and closures.
    #[inline]
    #[must_use]
    pub fn active(self, active: impl Into<Prop<bool>>) -> Self {
        self.bind_prop(active, EffectCategory::ActiveState, |cx, id, val| {
            cx.set_states(id, &StateFlag::Actived, val);
        })
    }

    /// Automatically maps an element's `StateFlag::Selected` based on signals and closures.
    #[inline]
    #[must_use]
    pub fn select(self, selected: impl Into<Prop<bool>>) -> Self {
        self.bind_prop(selected, EffectCategory::SelectState, |cx, id, val| {
            cx.set_states(id, &StateFlag::Selected, val);
        })
    }

    /// Automatically maps an element's `StateFlag::Disabled` based on signals and closures.
    #[inline]
    #[must_use]
    pub fn disable(self, disabled: impl Into<Prop<bool>>) -> Self {
        self.bind_prop(disabled, EffectCategory::DisableState, |cx, id, val| {
            cx.set_states(id, &StateFlag::Disabled, val);
        })
    }

    /// Automatically maps an element's `StateFlag::Focused` based on signals and closures.
    #[inline]
    #[must_use]
    pub fn focus(self, focused: impl Into<Prop<bool>>) -> Self {
        self.bind_prop(focused, EffectCategory::FocusState, |cx, id, val| {
            cx.set_states(id, &StateFlag::Focused, val);
        })
    }
}
