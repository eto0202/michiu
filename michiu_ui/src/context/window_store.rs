use crate::*;

pub struct WindowStore {
    pub scale_factor: f32,
    pub is_window_resizing: bool,
    pub(crate) last_window_size: Option<LayoutSize>,
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
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.scale_factor = 1.0;
        self.is_window_resizing = false;
        self.last_window_size = None;
    }

    #[inline]
    pub fn despawn(&mut self, _id: EntityId) {}
}
