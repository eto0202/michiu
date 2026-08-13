use crate::{Context, EntityId, LayoutRect, LayoutSize};
use windows::Win32::UI::Input::Ime::HIMC;

pub struct WindowStore {
    pub win_scale_factor: f32,
    pub win_is_resizing: bool,
    pub(crate) win_last_size: Option<LayoutSize>,
    pub(crate) win_default_himc: Option<HIMC>,
}

impl Default for WindowStore {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowStore {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            win_scale_factor: 1.0,
            win_is_resizing: false,
            win_last_size: None,
            win_default_himc: None,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.win_scale_factor = 1.0;
        self.win_is_resizing = false;
        self.win_last_size = None;
        self.win_default_himc = None;
    }

    #[inline]
    pub fn despawn(&mut self, _id: EntityId) {}
}

impl WindowStore {
    /// ウィンドウサイズの変更検知
    #[inline]
    pub(crate) fn window_resize_detection(
        window_size: LayoutSize,
        win_last_size: &mut Option<LayoutSize>,
    ) -> bool {
        win_last_size.replace(window_size) != Some(window_size)
    }

    /// 与えられたコンテナ矩形の、現在のウィンドウ領域において実際に画面上に見えている物理的な可視サイズを算出
    #[inline]
    pub(crate) fn calculate_visible_size(
        win_last_size: Option<LayoutSize>,
        container_rect: LayoutRect,
    ) -> LayoutSize {
        let window_size = win_last_size.unwrap_or_default();

        // 幅または高さにおける可視領域の長さを計算
        let calc_visible_len = |pos: f32, size: f32, max_dim: f32| -> f32 {
            if max_dim > 0.0 {
                let start = pos.max(0.0);
                let end = (pos + size).min(max_dim);
                (end - start).max(0.0)
            } else {
                size
            }
        };

        let width = calc_visible_len(container_rect.x, container_rect.width, window_size.width);
        let height = calc_visible_len(container_rect.y, container_rect.height, window_size.height);

        LayoutSize::new(width, height)
    }
}

