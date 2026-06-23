use crate::LayoutRect;
use crate::types::LayoutSize;
use windows::{
    Win32::{
        Graphics::{
            Direct2D::{
                Common::{D2D_RECT_F, D2D1_COLOR_F},
                D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
                D2D1_RENDER_TARGET_PROPERTIES, D2D1CreateFactory, ID2D1Factory1,
            },
            DirectWrite::{DWRITE_TEXT_RANGE, *},
            Imaging::{
                CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
                WICBitmapCacheOnDemand, WICBitmapLockRead,
            },
        },
        System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
    },
    core::Interface,
};

pub(crate) struct TextEngine {
    pub(crate) dwrite_factory: IDWriteFactory,
    pub(crate) default_format: IDWriteTextFormat,
}

impl TextEngine {
    pub(crate) fn new() -> Self {
        let dwrite_factory: IDWriteFactory =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).unwrap() };

        // デフォルトのフォント設定（ユーザーが後で変更できるように拡張可能）
        let default_format = unsafe {
            dwrite_factory
                .CreateTextFormat(
                    windows::core::w!("Segoe UI"), // Windows標準フォント
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    16.0, // デフォルトサイズ
                    windows::core::w!("ja-JP"),
                )
                .unwrap()
        };

        Self {
            dwrite_factory,
            default_format,
        }
    }

    /// 各パラメータを考慮して、完全な IDWriteTextLayout を生成する内部共通ロジック
    pub(crate) fn create_layout(
        &self,
        text: &str,
        font_size: f32,
        font_family: Option<&str>,
        font_weight: Option<u32>, // DWRITE_FONT_WEIGHT (100..900)
        font_style: Option<u32>,  // DWRITE_FONT_STYLE (Normal=0, Italic=2)
        max_width: Option<f32>,
    ) -> IDWriteTextLayout {
        unsafe {
            let text_u16: Vec<u16> = text.encode_utf16().collect();

            // 基本レイアウトオブジェクトを生成
            let layout = self
                .dwrite_factory
                .CreateTextLayout(
                    &text_u16,
                    &self.default_format,
                    max_width.unwrap_or(f32::MAX),
                    f32::MAX,
                )
                .unwrap();

            // 上書きを適用する文字列全体の範囲 (Range)
            let range = DWRITE_TEXT_RANGE {
                startPosition: 0,
                length: text_u16.len() as u32,
            };

            // 1. フォントサイズの上書き
            if font_size != 16.0 {
                layout.SetFontSize(font_size, range).unwrap();
            }

            // 2. フォントファミリーの上書き (指定があれば)
            if let Some(family) = font_family {
                // ヌル終端したUTF-16としてフォント名を作成
                let family_u16: Vec<u16> = family.encode_utf16().chain(Some(0)).collect();
                layout
                    .SetFontFamilyName(windows::core::PCWSTR(family_u16.as_ptr()), range)
                    .unwrap();
            }

            // 3. フォントウェイト（太さ）の上書き (指定があれば)
            if let Some(weight) = font_weight {
                layout
                    .SetFontWeight(DWRITE_FONT_WEIGHT(weight as i32), range)
                    .unwrap();
            }

            // 4. フォントスタイル（斜体など）の上書き (指定があれば)
            if let Some(style) = font_style {
                layout
                    .SetFontStyle(DWRITE_FONT_STYLE(style as i32), range)
                    .unwrap();
            }

            layout
        }
    }

    /// Taffy から呼ばれる計測ロジックの実体
    pub(crate) fn measure_text(
        &self,
        text: &str,
        font_size: f32,
        font_family: Option<&str>,
        font_weight: Option<u32>,
        font_style: Option<u32>,
        max_width: Option<f32>,
    ) -> LayoutSize {
        if text.is_empty() {
            return LayoutSize::ZERO;
        }

        unsafe {
            // 共通ロジックで詳細に設定された TextLayout を生成
            let layout = self.create_layout(
                text,
                font_size,
                font_family,
                font_weight,
                font_style,
                max_width,
            );

            // メトリクス（正確な物理幅・高さ）を取得
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics).unwrap();

            LayoutSize::new(metrics.width, metrics.height)
        }
    }

    /// 既に作成済みの IDWriteTextLayout から正確なサイズを取得する
    pub(crate) fn get_layout_size(&self, layout: &IDWriteTextLayout) -> LayoutSize {
        unsafe {
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics).unwrap();
            LayoutSize::new(metrics.width, metrics.height)
        }
    }
}

pub(crate) struct TextRasterizer {
    pub(crate) d2d_factory: ID2D1Factory1,
    pub(crate) wic_factory: IWICImagingFactory,
}

impl Default for TextRasterizer {
    fn default() -> Self {
        TextRasterizer::new()
    }
}

impl TextRasterizer {
    pub(crate) fn new() -> Self {
        let d2d_factory =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).unwrap() };
        let wic_factory: IWICImagingFactory = unsafe {
            windows::Win32::System::Com::CoCreateInstance(
                &CLSID_WICImagingFactory,
                None,
                windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
            )
            .unwrap()
        };
        Self {
            d2d_factory,
            wic_factory,
        }
    }

    pub(crate) fn rasterize(&self, layout: &IDWriteTextLayout, size: LayoutSize) -> Vec<u8> {
        unsafe {
            let width = (size.width.ceil() as u32).max(1);
            let height = (size.height.ceil() as u32).max(1);

            // 1. WIC ビットマップの作成 (RGBA8)
            let wic_bitmap = self
                .wic_factory
                .CreateBitmap(
                    width,
                    height,
                    &GUID_WICPixelFormat32bppPBGRA,
                    WICBitmapCacheOnDemand,
                )
                .unwrap();

            // 2. D2D レンダーターゲットの作成
            let props = D2D1_RENDER_TARGET_PROPERTIES::default();
            let target = self
                .d2d_factory
                .CreateWicBitmapRenderTarget(&wic_bitmap, &props)
                .unwrap();

            target.BeginDraw();
            target.Clear(None);

            // 3. ブラシの作成 (白固定、wgpu側で着色するため)
            let color = D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            };
            let brush = target.CreateSolidColorBrush(&color, None).unwrap();

            // 4. 描画
            let origin = windows_numerics::Vector2 { X: 0.0, Y: 0.0 };
            target.DrawTextLayout(origin, layout, &brush, D2D1_DRAW_TEXT_OPTIONS_NONE);
            target.EndDraw(None, None).unwrap();

            // 5. ピクセルデータの抽出
            let mut pixels = vec![0u8; (width * height * 4) as usize];
            wic_bitmap
                .CopyPixels(std::ptr::null(), width * 4, &mut pixels)
                .unwrap();

            pixels
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct TextCacheKey {
    pub(crate) text: String,
    pub(crate) font_size_bits: u32,
    pub(crate) font_family: Option<String>,
    pub(crate) font_weight: Option<u32>,
    pub(crate) font_style: Option<u32>,
}

#[derive(Clone, Debug, Copy)]
pub(crate) struct TextCacheValue {
    pub(crate) uv_min: [f32; 2],
    pub(crate) uv_max: [f32; 2],
}

pub(crate) struct TextureAtlas {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) size: u32,

    // パッキング状態
    pub(crate) current_x: u32,
    pub(crate) current_y: u32,
    pub(crate) row_max_height: u32,
    pub(crate) padding: u32,
}

impl TextureAtlas {
    pub(crate) fn new(device: &wgpu::Device, size: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Texture Atlas"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // WIC（32bppPBGRA）からの転送データと完全に一致させるため Bgra8Unorm へ変更
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
            size,
            current_x: 0,
            current_y: 0,
            row_max_height: 0,
            padding: 2, // 滲み防止用の余白
        }
    }

    /// 新しい要素のための領域をアトラス上に確保し、座標を返す
    pub(crate) fn allocate(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        let padded_w = width + self.padding;
        let padded_h = height + self.padding;

        // 横幅が足りない場合は次の段へ
        if self.current_x + padded_w > self.size {
            self.current_x = 0;
            self.current_y += self.row_max_height;
            self.row_max_height = 0;
        }

        // 縦幅が足りない場合は失敗（アトラス溢れ）
        if self.current_y + padded_h > self.size {
            return None;
        }

        let x = self.current_x;
        let y = self.current_y;

        // 状態更新
        self.current_x += padded_w;
        self.row_max_height = self.row_max_height.max(padded_h);

        Some((x, y))
    }

    pub(crate) fn clear(&mut self) {
        self.current_x = 0;
        self.current_y = 0;
        self.row_max_height = 0;
    }

    /// ピクセル座標を NDC (0.0 ~ 1.0) の UV 座標に変換する
    pub(crate) fn texel_to_uv(
        &self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> ([f32; 2], [f32; 2]) {
        let s = self.size as f32;
        (
            [x as f32 / s, y as f32 / s],
            [(x + width) as f32 / s, (y + height) as f32 / s],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};

    // テストスレッドの COM の初期化と自動アンロード
    struct ComGuard;

    impl ComGuard {
        fn new() -> Self {
            unsafe {
                // UIスレッド用の STA アパートメントとして COM を初期化
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            }
            Self
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe {
                // スコープを抜ける際に自動でアンロード
                CoUninitialize();
            }
        }
    }

    // TextEngine によるフォントサイズ計測のテスト
    #[test]
    fn test_text_engine_measure() {
        let _com = ComGuard::new();
        let engine = TextEngine::new();

        // 1. 基本計測
        let size_normal = engine.measure_text("Hello World", 16.0, None, None, None, None);
        assert!(size_normal.width > 0.0);
        assert!(size_normal.height > 0.0);

        // 2. 太字 (Bold: 700) にした際に幅が変わるか検証（詳細なフォント制御の確認）
        let size_bold = engine.measure_text("Hello World", 16.0, None, Some(700), None, None);
        assert!(
            size_bold.width > size_normal.width,
            "Bold text should be wider than normal text"
        );

        // 3. フォントを変更した際の計測検証
        let size_arial = engine.measure_text("Hello World", 16.0, Some("Arial"), None, None, None);
        assert!(size_arial.width > 0.0);

        // フォントサイズ変更による拡大の検証
        let size_large = engine.measure_text("Hello World", 32.0, None, None, None, None);
        assert!(size_large.width > size_normal.width);
        assert!(size_large.height > size_normal.height);

        // 空文字計測時の安全性検証 (ZEROを返す)
        let size_empty = engine.measure_text("", 16.0, None, None, None, None);
        assert_eq!(size_empty, LayoutSize::ZERO);

        // 折り返し制限（max_width）を掛けた際の挙動検証
        let size_wrapped = engine.measure_text(
            "Hello World, this is a very long text line",
            16.0,
            None,
            None,
            None,
            Some(50.0),
        );
        assert!(size_wrapped.width <= 50.0);
    }

    // TextRasterizer によるピクセル生成（レンダリング）のテスト
    #[test]
    fn test_text_rasterizer_pixel_output() {
        let _com = ComGuard::new();
        let engine = TextEngine::new();
        let rasterizer = TextRasterizer::new();

        let sample_text = "Michiu Test";
        let font_size = 20.0;

        let layout = engine.create_layout(sample_text, font_size, None, None, None, None);

        let size = engine.get_layout_size(&layout);
        let pixels = rasterizer.rasterize(&layout, size);

        let w = (size.width.ceil() as u32).max(1);
        let h = (size.height.ceil() as u32).max(1);

        // 生成されたピクセルバッファサイズが理論値 (W * H * 4バイト) と一致するか検証
        assert_eq!(pixels.len(), (w * h * 4) as usize);

        // テクスチャ上に透明（0）以外の描画（文字）ピクセルが存在するか検証
        let has_drawn_pixels = pixels.iter().any(|&p| p > 0);
        assert!(
            has_drawn_pixels,
            "Rasterizer should output non-empty pixels for text"
        );
    }

    // テキストキャッシュキーのハッシュ一貫性のテスト
    #[test]
    fn test_text_cache_key_hash_equality() {
        let key1 = TextCacheKey {
            text: "Michiu".to_string(),
            font_size_bits: 16.5f32.to_bits(),
            font_style: None,
            font_family: None,
            font_weight: None,
        };
        let key2 = TextCacheKey {
            text: "Michiu".to_string(),
            font_size_bits: 16.5f32.to_bits(),
            font_style: None,
            font_family: None,
            font_weight: None,
        };
        let key3 = TextCacheKey {
            text: "Michiu".to_string(),
            font_size_bits: 16.6f32.to_bits(), // サイズ違い
            font_style: None,
            font_family: None,
            font_weight: None,
        };

        // 同一の内容であれば正しく Eq が機能することを確認
        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }
}
