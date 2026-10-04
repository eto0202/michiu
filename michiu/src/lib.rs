use michiu_guard::Validated;
#[cfg(feature = "ui")]
#[doc(inline)]
pub use michiu_ui as ui;
use michiu_ui::{ComposedRenderer, Context, EntityId, TickType};

#[cfg(feature = "guard")]
#[doc(inline)]
pub use michiu_guard as guard;

#[cfg(feature = "window")]
#[doc(inline)]
pub use michiu_window as window;
use michiu_window::{MichiuEvent, RawEvent, WindowBuilder, WindowHandle};

pub struct MichiuApp {
    pub context: Context,
    pub renderer: ComposedRenderer,
    pub root_id: EntityId,
}

pub struct MichiuEventResponse {
    pub consumed: bool,
    pub needs_redraw: bool,
}

impl MichiuApp {
    pub fn handle_event(
        &mut self,
        event: &MichiuEvent,
        raw: &RawEvent,
        handle: &Validated<WindowHandle>,
    ) -> MichiuEventResponse {
        let mut needs_redraw = false;

        MichiuEventResponse {
            consumed: true,
            needs_redraw,
        }
    }

    pub fn standard_render_frame(&mut self, handle: &Validated<WindowHandle>) {
        self.context.begin_frame();
        self.context.tick_system_frame(&TickType::All);
        self.context
            .sync_layout(self.root_id, self.renderer.layout_size());
        self.renderer.update_composition_tree(&mut self.context);
        self.context.update_accessibility();

        if let Some(_ctx) = handle.begin_paint() {
            self.renderer.draw(&mut self.context);
        }

        if self.context.has_active_frame() {
            handle.dwm_flush();
            handle.redraw_requested();
        }
    }
}

pub struct MichiuAppBuilder<'a> {
    builder: WindowBuilder<'a>,
}