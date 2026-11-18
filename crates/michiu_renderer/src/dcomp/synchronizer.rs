use michiu_ui::LayoutSize;

pub(crate) struct WindowSynchronizer {
    pub(crate) window_size: LayoutSize,
    pub(crate) scale_factor: f32,
    // リサイズが完全に安定するまでキャプチャを保留するカウンター
    pub(crate) resize_cooldown_frames: u32,
}

impl WindowSynchronizer {
    #[inline]
    pub fn new(window_size: LayoutSize, scale_factor: f32) -> Self {
        Self {
            window_size,
            scale_factor,
            resize_cooldown_frames: 0,
        }
    }

    #[expect(clippy::cast_precision_loss)]
    #[inline]
    pub fn resize(&mut self, p_width: u32, p_height: u32, scale_factor: f32) {
        self.scale_factor = scale_factor;
        self.window_size = LayoutSize::new(
            p_width as f32 / scale_factor,
            p_height as f32 / scale_factor,
        );
        // リサイズが発生したため、キャプチャクールダウンを 15 フレームに設定
        // 拡大リサイズ中およびリサイズ直後の不安定なバッファへのキャプチャを遮断
        self.resize_cooldown_frames = 15;
    }
}
