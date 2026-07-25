use crate::*;
use windows::Win32::UI::Input::Ime::HIMC;

pub struct WindowStore {
    pub scale_factor: f32,
    pub is_window_resizing: bool,
    pub(crate) last_window_size: Option<LayoutSize>,
    pub(crate) default_himc: Option<HIMC>,
}

impl Default for WindowStore {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowStore {
    #[inline]
    pub fn new() -> Self {
        Self {
            scale_factor: 1.0,
            is_window_resizing: false,
            last_window_size: None,
            default_himc: None,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.scale_factor = 1.0;
        self.is_window_resizing = false;
        self.last_window_size = None;
        self.default_himc = None;
    }

    #[inline]
    pub fn despawn(&mut self, _id: EntityId) {}
}

impl WindowStore {
    /// ウィンドウサイズの変更検知
    #[inline]
    pub(crate) fn window_resize_detection(
        window: &mut WindowStore,
        window_size: LayoutSize,
    ) -> bool {
        if window.last_window_size != Some(window_size) {
            window.last_window_size = Some(window_size);
            true
        } else {
            false
        }
    }

    /// 与えられたコンテナ矩形の、現在のウィンドウ領域において実際に画面上に見えている物理的な可視サイズを算出します。
    pub(crate) fn calculate_visible_size(
        window: &WindowStore,
        container_rect: LayoutRect,
    ) -> LayoutSize {
        let window_size = window.last_window_size.unwrap_or(LayoutSize::ZERO);

        let visible_w = if window_size.width > 0.0 {
            let left = container_rect.x.max(0.0);
            let right = (container_rect.x + container_rect.width).min(window_size.width);
            (right - left).max(0.0)
        } else {
            container_rect.width
        };

        let visible_h = if window_size.height > 0.0 {
            let top = container_rect.y.max(0.0);
            let bottom = (container_rect.y + container_rect.height).min(window_size.height);
            (bottom - top).max(0.0)
        } else {
            container_rect.height
        };

        LayoutSize::new(visible_w, visible_h)
    }
}

impl Context {
    /// ウィンドウサイズの変更検知
    #[inline]
    pub(crate) fn window_resize_detection(&mut self, window_size: LayoutSize) -> bool {
        WindowStore::window_resize_detection(&mut self.window, window_size)
    }

    /// 与えられたコンテナ矩形の、現在のウィンドウ領域において実際に画面上に見えている物理的な可視サイズを算出します。
    #[inline]
    pub(crate) fn calculate_visible_size(&self, container_rect: LayoutRect) -> LayoutSize {
        WindowStore::calculate_visible_size(&self.window, container_rect)
    }
}
