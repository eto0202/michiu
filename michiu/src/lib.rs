use michiu_guard::Validated;
#[cfg(feature = "ui")]
#[doc(inline)]
pub use michiu_ui as ui;
use michiu_ui::{
    CapacityConfig, CharIndex, ComposedRenderer, Context, CursorIcon, Element, ElementState,
    EntityId, ImeState, InteractionState, LayoutPoint, MichiuInspector, Modifiers, MouseButton,
    TickType, UserAction, VirtualKey, build_ui, dispatch_raw_input_to_external_visual,
    get_win32_clipboard, raw_wheel_delta_to_logical_pixels, set_win32_clipboard,
};

#[cfg(feature = "guard")]
#[doc(inline)]
pub use michiu_guard as guard;

#[cfg(feature = "window")]
#[doc(inline)]
pub use michiu_window as window;
use michiu_window::{Event, EventPump, MichiuEvent, RawEvent, Window, WindowHandle};
use windows::Win32::UI::WindowsAndMessaging::{WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_NULL};

pub struct MichiuApp {
    pub context: Context,
    pub renderer: ComposedRenderer,
    pub window: Window,
    pub handle: Validated<WindowHandle>,
    root_id: Option<EntityId>,
    pub config: BuilderConfig,
}

#[derive(Debug, Clone, Copy)]
pub struct MichiuEventResponse {
    pub consumed: bool,
    pub needs_redraw: bool,
    pub needs_update_window: bool,
}

impl MichiuApp {
    // UIツリーを構築し、自動的にルート要素として登録する
    #[inline]
    pub fn build_ui<F>(&mut self, f: F) -> EntityId
    where
        F: FnOnce() -> Element,
    {
        let root = build_ui(&mut self.context, f);
        let id = root.id();
        self.set_root(id);
        id
    }

    #[inline]
    pub fn set_root(&mut self, root_id: EntityId) {
        self.root_id = Some(root_id);
    }

    #[must_use]
    #[inline]
    pub fn root_id(&self) -> EntityId {
        self.root_id.expect(
            "The root ID was not found.\
             Possible cause:\
                - The root ID was not registered using `set_root`.",
        )
    }

    #[must_use]
    #[inline]
    pub fn try_root_id(&self) -> Option<EntityId> {
        self.root_id
    }

    #[inline]
    pub fn redraw_requested(&self) {
        self.handle.redraw_requested();
    }

    #[inline]
    pub fn update_window(&self) {
        self.handle.update_window();
    }

    #[inline]
    pub fn destroy(&self) {
        self.handle.destroy();
    }

    #[inline]
    pub fn quit(&self) {
        self.handle.quit();
    }

    pub fn standard_handle_window_event(
        &mut self,
        event: &MichiuEvent,
        raw: &RawEvent,
    ) -> MichiuEventResponse {
        let mut consumed = false;
        let mut ext_consumed = false;
        let mut needs_redraw = false;
        let mut needs_update_window = false;

        let has_hovered = self
            .context
            .interaction_id(InteractionState::Hovered)
            .is_some();
        let has_focused = self
            .context
            .interaction_id(InteractionState::Focused)
            .is_some();

        match raw.msg {
            WM_NULL => {
                // バックグラウンドから届いた CSS 更新タスクなどを安全に消化
                self.context.process_main_thread_tasks();

                // 消化によってレイアウトや描画に変更があった場合のみ再描画を実行
                if self.context.has_dirty() {
                    needs_redraw = true;
                }
                consumed = true;
            }
            WM_ENTERSIZEMOVE => {
                self.context.set_window_resized(true);
                needs_redraw = true;
                needs_update_window = true;
            }
            // ウィンドウドラッグリサイズの完了をキャッチ
            WM_EXITSIZEMOVE => {
                self.context.set_window_resized(false);
                // リサイズ完了後の再描画を即座にキックして、新サイズでの静止画キャプチャを誘発
                needs_redraw = true;
                needs_update_window = true;
            }

            _ => {}
        }

        if let MichiuEvent::Window { event, .. } = event {
            match event {
                Event::Resized(phy_size) => {
                    let size = phy_size.assume_valid().into_inner();
                    let width = size.width as u32;
                    let height = size.height as u32;
                    // レンダラーのリサイズとレイアウト物理サイズの更新
                    self.renderer
                        .resize((width, height), self.renderer.scale_factor());

                    // 再描画要求
                    needs_redraw = true;
                    needs_update_window = true;
                }
                Event::CursorMoved { position } => {
                    let pos = position.assume_valid().into_inner();
                    let x = pos.x as f32;
                    let y = pos.y as f32;
                    let phys_pos = LayoutPoint::new(x, y);
                    let logical_pos = LayoutPoint::new(
                        x / self.renderer.scale_factor(),
                        y / self.renderer.scale_factor(),
                    );

                    self.context
                        .inject_user_action(UserAction::PointerMove(logical_pos));

                    if self.config.external_visual_support {
                        ext_consumed = dispatch_raw_input_to_external_visual(
                            &mut self.context,
                            raw.msg,
                            raw.wparam,
                            raw.lparam,
                            phys_pos,
                            self.renderer.scale_factor(),
                        );
                    }

                    consumed = has_hovered || ext_consumed;
                    needs_redraw = true;
                }
                Event::CursorLeft => {
                    // ウィンドウ外に去ったため、論理空間外へポインタを移動させてホバーを確実に解除
                    self.context
                        .inject_user_action(UserAction::PointerMove(LayoutPoint::new(
                            -9999.0, -9999.0,
                        )));

                    needs_redraw = true;
                }
                Event::MouseInput {
                    button,
                    modifiers,
                    state,
                    click_count,
                } => {
                    let button = match button {
                        michiu_window::MouseButton::Left => MouseButton::Left,
                        michiu_window::MouseButton::Right => MouseButton::Right,
                        michiu_window::MouseButton::Middle => MouseButton::Middle,
                        michiu_window::MouseButton::Other(_) => MouseButton::X1,
                    };
                    let modifiers = Modifiers {
                        shift: modifiers.contains(michiu_window::Modifiers::SHIFT),
                        ctrl: modifiers.contains(michiu_window::Modifiers::CONTROL),
                        alt: modifiers.contains(michiu_window::Modifiers::ALT),
                        logo: modifiers.contains(michiu_window::Modifiers::LOGO),
                    };
                    let state = match state {
                        michiu_window::ElementState::Pressed => ElementState::Pressed,
                        michiu_window::ElementState::Released => ElementState::Released,
                    };

                    self.context.inject_user_action(UserAction::PointerButton {
                        button,
                        state,
                        modifiers,
                    });

                    if click_count == &2 {
                        self.context
                            .inject_user_action(UserAction::PointerDoubleClick { modifiers });
                    }

                    if self.config.external_visual_support {
                        let x = (raw.lparam.0 & 0xffff) as i16 as f32;
                        let y = ((raw.lparam.0 >> 16) & 0xffff) as i16 as f32;
                        let phys_pos = LayoutPoint::new(x, y);
                        ext_consumed = dispatch_raw_input_to_external_visual(
                            &mut self.context,
                            raw.msg,
                            raw.wparam,
                            raw.lparam,
                            phys_pos,
                            self.renderer.scale_factor(),
                        );
                    }

                    consumed = has_hovered || ext_consumed;
                    needs_redraw = true;
                }
                Event::CharacterInput(c) => {
                    self.context.inject_user_action(UserAction::Character(*c));
                    consumed = has_focused;
                    needs_redraw = true;
                }
                Event::KeyboardInput {
                    key_code,
                    modifiers,
                    state,
                } => {
                    let mut shortcut_handled = false;

                    let key_code = key_code.assume_valid().into_inner();
                    let key = VirtualKey::from_windows(key_code);
                    let modifiers = Modifiers {
                        shift: modifiers.contains(michiu_window::Modifiers::SHIFT),
                        ctrl: modifiers.contains(michiu_window::Modifiers::CONTROL),
                        alt: modifiers.contains(michiu_window::Modifiers::ALT),
                        logo: modifiers.contains(michiu_window::Modifiers::LOGO),
                    };
                    let state = match state {
                        michiu_window::ElementState::Pressed => ElementState::Pressed,
                        michiu_window::ElementState::Released => ElementState::Released,
                    };

                    if modifiers.ctrl
                        && self.config.default_shortcuts
                        && state == ElementState::Pressed
                    {
                        match raw.wparam.0 as i32 {
                            // Ctrl + C
                            0x43 if let Some(selected_text) = self.context.get_selected_text() => {
                                let _ = set_win32_clipboard(&selected_text);
                                shortcut_handled = true;
                            }
                            // Ctrl + V
                            0x56 if has_focused
                                && let Some(pasted_text) = get_win32_clipboard() =>
                            {
                                self.context
                                    .inject_user_action(UserAction::Paste(pasted_text.into()));
                                shortcut_handled = true;
                            }
                            // Ctrl + X (切り取り)
                            0x58 if let Some(selected_text) = self.context.get_selected_text() => {
                                let _ = set_win32_clipboard(&selected_text);
                                self.context.inject_user_action(UserAction::Cut);
                                shortcut_handled = true;
                            }
                            // Ctrl + Z (Undo)
                            0x5A if has_focused => {
                                self.context.inject_user_action(UserAction::Undo);
                                shortcut_handled = true;
                            }
                            // Ctrl + Y (Redo)
                            0x59 if has_focused => {
                                self.context.inject_user_action(UserAction::Redo);
                                shortcut_handled = true;
                            }
                            _ => {}
                        }
                    }

                    self.context.inject_user_action(UserAction::KeyboardKey {
                        key,
                        state,
                        modifiers,
                    });

                    consumed = has_focused || shortcut_handled;
                    needs_redraw = true;
                }
                Event::MouseWheel {
                    raw_delta_x,
                    raw_delta_y,
                } => {
                    let mut pt = windows::Win32::Foundation::POINT {
                        x: (raw.lparam.0 & 0xffff) as i16 as i32,
                        y: ((raw.lparam.0 >> 16) & 0xffff) as i16 as i32,
                    };
                    let _ = unsafe {
                        windows::Win32::Graphics::Gdi::ScreenToClient(
                            self.handle.hwnd(),
                            &raw mut pt,
                        )
                    };

                    // 横スクロール時は、右チルト（プラス値）された際に
                    // 右方向へスクロール（オフセット加算）させるため、符号の方向性を補正
                    let scroll_x = {
                        let x = raw_delta_x.assume_valid().into_inner();
                        -raw_wheel_delta_to_logical_pixels(x.raw() as f32)
                    };

                    let scroll_y = {
                        let y = raw_delta_y.assume_valid().into_inner();
                        raw_wheel_delta_to_logical_pixels(y.raw() as f32)
                    };

                    self.context
                        .inject_user_action(UserAction::MouseWheel { scroll_x, scroll_y });

                    if self.config.external_visual_support {
                        let phys_pos = LayoutPoint::new(pt.x as f32, pt.y as f32);
                        ext_consumed = dispatch_raw_input_to_external_visual(
                            &mut self.context,
                            raw.msg,
                            raw.wparam,
                            raw.lparam,
                            phys_pos,
                            self.renderer.scale_factor(),
                        );
                    }

                    consumed = has_hovered || ext_consumed;
                    needs_redraw = true;
                }
                Event::Ime(ime) => {
                    let ime = ime.clone().assume_valid().into_inner();
                    let ime_state = ImeState {
                        is_open: ime.is_open,
                        conversion_mode: ime.conversion_mode,
                        sentence_mode: ime.sentence_mode,
                        keyboard_layout_id: ime.keyboard_layout_id,
                        composition_text: ime.composition_text.into(),
                        result_text: ime.result_text.into(),
                        caret_position: ime.caret_position.map(|p| LayoutPoint {
                            x: p.x as f32,
                            y: p.y as f32,
                        }),
                        composition_cursor: CharIndex(ime.composition_cursor),
                        composition_attrs: ime.composition_attrs,
                    };

                    self.context.inject_user_action(UserAction::Ime(ime_state));
                    consumed = has_focused;
                    needs_redraw = true;
                }
                _ => {}
            }
        }

        MichiuEventResponse {
            consumed,
            needs_redraw,
            needs_update_window,
        }
    }

    pub fn standard_redraw(&mut self) {
        let root_id = self.root_id();

        self.context.begin_frame();

        self.standard_update_cursor_icon();

        self.context.tick_system_frame(&TickType::All);
        self.context
            .sync_layout(root_id, self.renderer.layout_size());
        self.renderer.update_composition_tree(&mut self.context);

        if self.config.accessibility_support {
            self.context.update_accessibility();
        }

        if let Some(_ctx) = self.handle.begin_paint() {
            self.renderer.draw(&mut self.context);
        }

        if self.context.has_active_frame() {
            if self.config.sync_mode == AutoSyncMode::DwmFlush {
                self.handle.dwm_flush();
            }
            self.handle.redraw_requested();
        }
    }

    pub fn standard_update_cursor_icon(&mut self) {
        if !self.config.auto_resolve_cursor {
            return;
        }

        let target_cursor =
            if let Some(hovered) = self.context.interaction_id(InteractionState::Hovered) {
                self.context.resolve_cursor(hovered)
            } else {
                CursorIcon::Default(None)
            };

        if let Ok(hcursor) = target_cursor.to_hcursor() {
            self.handle
                .set_cursor_icon(michiu_window::CursorIcon::Other(hcursor));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutoSyncMode {
    #[default]
    DwmFlush,
    Immediate,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy)]
pub struct BuilderConfig {
    pub external_visual_support: bool,
    pub default_shortcuts: bool,
    pub sync_mode: AutoSyncMode,
    pub accessibility_support: bool,
    pub auto_resolve_cursor: bool,
}

impl BuilderConfig {
    fn new() -> Self {
        Self {
            external_visual_support: false,
            default_shortcuts: true,
            sync_mode: AutoSyncMode::default(),
            accessibility_support: false,
            auto_resolve_cursor: true,
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
pub struct MichiuAppBuilder {
    pub config: BuilderConfig,
    pub window: Window,
    pub inspector: Option<MichiuInspector>,
    pub capacity: Option<CapacityConfig>,
}

impl MichiuAppBuilder {
    #[must_use]
    #[inline]
    pub fn new(window: Window) -> Self {
        Self {
            config: BuilderConfig::new(),
            window,
            inspector: None,
            capacity: None,
        }
    }

    #[must_use]
    #[inline]
    pub fn build(
        self,
        renderer: ComposedRenderer,
    ) -> michiu_window::Result<(MichiuApp, EventPump)> {
        let handle: Validated<WindowHandle> = self.window.handle().try_into()?;
        let hwnd = handle.hwnd();

        let mut context = match (&self.capacity, &self.inspector) {
            (Some(c), Some(i)) => Context::with_capacity_and_inspector(c, i),
            (Some(c), None) => Context::with_capacity(c),
            (None, Some(i)) => Context::with_inspector(i),
            (None, None) => Context::new(),
        };

        if self.config.accessibility_support {
            self.window.set_visible(false);
            context = context.with_accessibility(hwnd);
        }

        let h_waker = handle.clone();
        context.set_waker(move || h_waker.wake_up());

        let pump = EventPump::new();
        Ok((
            MichiuApp {
                window: self.window,
                context,
                renderer,
                handle,
                root_id: None,
                config: self.config,
            },
            pump,
        ))
    }

    // 初期化と同時に UI ルートの構築・登録まで完了する
    #[inline]
    pub fn build_with_ui<F>(
        self,
        renderer: ComposedRenderer,
        ui_fn: F,
    ) -> michiu_window::Result<(MichiuApp, EventPump)>
    where
        F: FnOnce() -> Element,
    {
        let (mut app, pump) = self.build(renderer)?;
        app.build_ui::<F>(ui_fn);
        Ok((app, pump))
    }

    #[must_use]
    #[inline]
    pub fn with_auto_resolve_cursor(mut self, enabled: bool) -> Self {
        self.config.auto_resolve_cursor = enabled;
        self
    }

    #[must_use]
    #[inline]
    pub fn with_external_visual_support(mut self, enabled: bool) -> Self {
        self.config.external_visual_support = enabled;
        self
    }

    #[must_use]
    #[inline]
    pub fn with_default_shortcuts(mut self, enabled: bool) -> Self {
        self.config.default_shortcuts = enabled;
        self
    }

    #[must_use]
    #[inline]
    pub fn with_sync_mode(mut self, mode: AutoSyncMode) -> Self {
        self.config.sync_mode = mode;
        self
    }

    #[must_use]
    #[inline]
    pub fn with_accessibility_support(mut self, enabled: bool) -> Self {
        self.config.accessibility_support = enabled;
        self
    }

    #[must_use]
    #[inline]
    pub fn with_inspector(mut self, inspector: MichiuInspector) -> Self {
        self.inspector = Some(inspector);
        self
    }

    #[must_use]
    #[inline]
    pub fn with_capacity_config(mut self, config: CapacityConfig) -> Self {
        self.capacity = Some(config);
        self
    }
}
