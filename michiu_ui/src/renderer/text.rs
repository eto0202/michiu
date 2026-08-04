use std::borrow::Cow;

use crate::LayoutRect;
use crate::types::LayoutSize;
use smallvec::SmallVec;
use windows::Win32::Graphics::Direct2D::{
    D2D1_RENDER_TARGET_TYPE_SOFTWARE, D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, ID2D1RenderTarget,
};
use windows::core::PCWSTR;
use windows::{
    Win32::{
        Graphics::{
            Direct2D::{
                Common::{D2D_RECT_F, D2D1_COLOR_F},
                D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
                D2D1_RENDER_TARGET_PROPERTIES, D2D1CreateFactory, ID2D1Factory1,
            },
            DirectWrite::{DWRITE_TEXT_RANGE, IDWriteFactory, IDWriteTextFormat, IDWriteRenderingParams, DWriteCreateFactory, DWRITE_FACTORY_TYPE_SHARED, IDWriteFactory_Impl, IDWriteFactory6_Impl, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, IDWriteRenderingParams_Impl, IDWriteFactory1_Impl, IDWriteFactory2_Impl, IDWriteFactory3_Impl, IDWriteTextLayout, IDWriteTextLayout_Impl, IDWriteTextFormat_Impl, IDWriteTextFormat2_Impl, IDWriteTextLayout3_Impl, DWRITE_LINE_SPACING_METHOD_UNIFORM, DWRITE_FONT_WEIGHT, DWRITE_FONT_STYLE, DWRITE_TEXT_METRICS, IDWriteFont_Impl, IDWriteFont1_Impl, IDWriteFontFace_Impl, IDWriteFontFace1_Impl, IDWriteInlineObject_Impl, IDWriteTextLayout2_Impl, DWRITE_HIT_TEST_METRICS, IDWriteBitmapRenderTarget1_Impl},
            Imaging::{
                CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
                WICBitmapCacheOnDemand, WICBitmapLockRead,
            },
        },
        System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
    },
    core::Interface,
};
use windows_numerics::Vector2;
use windows_result::BOOL;

pub(crate) struct TextEngine {
    pub(crate) dwrite_factory: IDWriteFactory,
    pub(crate) default_format: IDWriteTextFormat,
    pub(crate) rendering_params: IDWriteRenderingParams,
}

impl TextEngine {
    pub(crate) fn new() -> Self {
        let dwrite_factory: IDWriteFactory =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).unwrap() };

        // デフォルトのフォント設定（ユーザーが後で変更できるように修正）
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

        let rendering_params = unsafe {
            let default_params = dwrite_factory.CreateRenderingParams().unwrap();
            let system_gamma = default_params.GetGamma();
            // TODO: ユーザー設定可能に
            let enhanced_contrast = 0.0;

            dwrite_factory
                .CreateCustomRenderingParams(
                    system_gamma,
                    enhanced_contrast,
                    0.0, // グレースケールなので不要
                    windows::Win32::Graphics::DirectWrite::DWRITE_PIXEL_GEOMETRY_FLAT,
                    windows::Win32::Graphics::DirectWrite::DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC,
                )
                .unwrap()
        };

        Self {
            dwrite_factory,
            default_format,
            rendering_params,
        }
    }

    /// 各パラメータを考慮して、完全な `IDWriteTextLayout` を生成する内部共通ロジック
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn create_layout(
        &self,
        text: &str,
        font_size: f32,
        font_family: Option<&str>,
        font_weight: Option<u32>, // DWRITE_FONT_WEIGHT (100..900)
        font_style: Option<u32>,  // DWRITE_FONT_STYLE (Normal=0, Italic=2)
        max_width: Option<f32>,
        spans: &[crate::TextSpan],
    ) -> IDWriteTextLayout {
        unsafe {
            let text_u16: SmallVec<[u16; 64]> = text.encode_utf16().collect();

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

            let line_spacing = font_size * 1.2;
            let baseline = font_size * 0.95;

            let _ =
                layout.SetLineSpacing(DWRITE_LINE_SPACING_METHOD_UNIFORM, line_spacing, baseline);

            // 2. フォントファミリーの上書き (指定があれば)
            if let Some(family) = font_family {
                // ヌル終端したUTF-16としてフォント名を作成
                let family_u16: Vec<u16> = family.encode_utf16().chain(Some(0)).collect();
                layout
                    .SetFontFamilyName(PCWSTR(family_u16.as_ptr()), range)
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

            for span in spans {
                let s_pos = span.range.start as u32;
                let s_len = (span.range.end - span.range.start) as u32;
                if s_len == 0 || s_pos + s_len > text_u16.len() as u32 {
                    continue;
                }

                let span_range = DWRITE_TEXT_RANGE {
                    startPosition: s_pos,
                    length: s_len,
                };

                if let Some(size) = span.font_size {
                    let _ = layout.SetFontSize(size, span_range);
                }
                if let Some(ref family) = span.font_family {
                    let family_u16: Vec<u16> = family.encode_utf16().chain(Some(0)).collect();
                    let _ = layout.SetFontFamilyName(PCWSTR(family_u16.as_ptr()), span_range);
                }
                if let Some(weight) = span.font_weight {
                    let _ = layout.SetFontWeight(DWRITE_FONT_WEIGHT(weight as i32), span_range);
                }
                if let Some(style) = span.font_style {
                    let _ = layout.SetFontStyle(DWRITE_FONT_STYLE(style as i32), span_range);
                }
            }

            layout
        }
    }

    /// Taffy から呼ばれる計測ロジックの実体
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn measure_text(
        &self,
        text: &str,
        font_size: f32,
        font_family: Option<&str>,
        font_weight: Option<u32>,
        font_style: Option<u32>,
        max_width: Option<f32>,
        spans: &[crate::TextSpan],
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
                spans,
            );

            // メトリクス（正確な物理幅・高さ）を取得
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&raw mut metrics).unwrap();

            LayoutSize::new(metrics.width, metrics.height)
        }
    }

    /// 既に作成済みの `IDWriteTextLayout` から正確なサイズを取得する
    pub(crate) fn get_layout_size(&self, layout: &IDWriteTextLayout) -> LayoutSize {
        unsafe {
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&raw mut metrics).unwrap();
            LayoutSize::new(metrics.width, metrics.height)
        }
    }

    /// 指定された文字位置（UTF-16 インデックス）の要素ローカルな物理座標 (X, Y) および高さを取得します
    pub(crate) fn get_caret_position(
        &self,
        layout: &IDWriteTextLayout,
        index: usize,
        text_len: usize,
    ) -> (f32, f32, f32) {
        unsafe {
            let mut point_x: f32 = 0.0;
            let mut point_y: f32 = 0.0;
            let mut metrics = DWRITE_HIT_TEST_METRICS::default();

            let (target_pos, is_trailing) = if index >= text_len && text_len > 0 {
                (text_len - 1, true)
            } else {
                (index, false)
            };

            let _ = layout.HitTestTextPosition(
                index as u32,
                is_trailing,
                &raw mut point_x,
                &raw mut point_y,
                &raw mut metrics,
            );

            // 実際の文字の上端位置を、metrics.top から算出して補正
            let caret_y = point_y;

            // ローカルX, ローカルY, 文字ブロックの高さ
            (point_x, caret_y, metrics.height)
        }
    }

    /// 物理的なローカル座標 (x, y) から、対応する文字インデックス（UTF-16 単位）を逆引きします
    pub(crate) fn hit_test_point(
        &self,
        layout: &IDWriteTextLayout,
        x: f32,
        y: f32,
    ) -> (usize, bool) {
        unsafe {
            let mut is_trailing: BOOL = false.into();
            let mut is_inside: BOOL = false.into();
            let mut metrics = DWRITE_HIT_TEST_METRICS::default();

            // DirectWrite の HitTestPoint API を呼び出し
            let _ = layout.HitTestPoint(x, y, &raw mut is_trailing, &raw mut is_inside, &raw mut metrics);

            // (ヒットした文字インデックス, 文字ブロックの後半部分（右半分）をクリックしたかどうかのフラグ)
            (metrics.textPosition as usize, is_trailing.into())
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

    pub(crate) fn rasterize(
        &self,
        layout: &IDWriteTextLayout,
        size: LayoutSize,
        spans: &[crate::TextSpan],
        rendering_params: &IDWriteRenderingParams,
    ) -> Vec<u8> {
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
            // RenderDocなどのグラフィックスツールによるフック時でもクラッシュ(E_NOINTERFACE)を防ぐため、
            // 明示的にソフトウェアレンダリングを使用するように指示
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                ..Default::default()
            };
            let target = self
                .d2d_factory
                .CreateWicBitmapRenderTarget(&wic_bitmap, &raw const props)
                .unwrap();

            target.SetTextRenderingParams(rendering_params);
            target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);

            target.BeginDraw();
            target.Clear(None);

            // 3. ブラシの作成 (白固定、wgpu側で着色するため)
            let color = D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            };
            let default_color = D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            };
            let default_brush = target.CreateSolidColorBrush(&raw const default_color, None).unwrap();

            for span in spans {
                if let Some(bg_color) = span.bg_color {
                    let start = span.range.start as u32;
                    let len = (span.range.end - span.range.start) as u32;
                    if len == 0 {
                        continue;
                    }

                    let mut hit_test_metrics = vec![DWRITE_HIT_TEST_METRICS::default(); 16];
                    let mut actual_count: u32 = 0;

                    let res = layout.HitTestTextRange(
                        start,
                        len,
                        0.0,
                        0.0,
                        Some(&mut hit_test_metrics),
                        &raw mut actual_count,
                    );
                    if res.is_ok() && actual_count as usize > hit_test_metrics.len() {
                        hit_test_metrics
                            .resize(actual_count as usize, DWRITE_HIT_TEST_METRICS::default());
                        let _ = layout.HitTestTextRange(
                            start,
                            len,
                            0.0,
                            0.0,
                            Some(&mut hit_test_metrics),
                            &raw mut actual_count,
                        );
                    }

                    let d2d_bg_color = D2D1_COLOR_F {
                        r: bg_color.r,
                        g: bg_color.g,
                        b: bg_color.b,
                        a: bg_color.a,
                    };
                    if let Ok(bg_brush) = target.CreateSolidColorBrush(&raw const d2d_bg_color, None) {
                        (0..actual_count as usize).for_each(|m_idx| {
                            let metric = &hit_test_metrics[m_idx];
                            let rect = D2D_RECT_F {
                                left: metric.left,
                                top: metric.top,
                                right: metric.left + metric.width,
                                bottom: metric.top + metric.height,
                            };
                            target.FillRectangle(&raw const rect, &bg_brush);
                        });
                    }
                }

                if let Some(color) = span.color {
                    let start = span.range.start as u32;
                    let len = (span.range.end - span.range.start) as u32;
                    if len > 0 {
                        let span_range = DWRITE_TEXT_RANGE {
                            startPosition: start,
                            length: len,
                        };
                        let d2d_color = D2D1_COLOR_F {
                            r: color.r,
                            g: color.g,
                            b: color.b,
                            a: color.a,
                        };
                        if let Ok(span_brush) = target.CreateSolidColorBrush(&raw const d2d_color, None) {
                            let _ = layout.SetDrawingEffect(&span_brush, span_range);
                        }
                    }
                }
            }

            let origin = windows_numerics::Vector2 { X: 0.0, Y: 0.0 };
            target.DrawTextLayout(origin, layout, &default_brush, D2D1_DRAW_TEXT_OPTIONS_NONE);

            // TextSpan に基づく下線・波線の上載せ描画
            for span in spans {
                let start = span.range.start as u32;
                let len = (span.range.end - span.range.start) as u32;
                if len == 0 {
                    continue;
                }

                let mut hit_test_metrics = vec![DWRITE_HIT_TEST_METRICS::default(); 16];
                let mut actual_count: u32 = 0;

                let res = layout.HitTestTextRange(
                    start,
                    len,
                    0.0,
                    0.0,
                    Some(&mut hit_test_metrics),
                    &raw mut actual_count,
                );
                if res.is_ok() && actual_count as usize > hit_test_metrics.len() {
                    hit_test_metrics
                        .resize(actual_count as usize, DWRITE_HIT_TEST_METRICS::default());
                    let _ = layout.HitTestTextRange(
                        start,
                        len,
                        0.0,
                        0.0,
                        Some(&mut hit_test_metrics),
                        &raw mut actual_count,
                    );
                }

                // A. 取り消し線（strikethrough）の描画
                if let Some(style) = span.strikethrough {
                    let st_brush = if let Some(color) = span.strikethrough_color {
                        let d2d_color = D2D1_COLOR_F {
                            r: color.r,
                            g: color.g,
                            b: color.b,
                            a: color.a,
                        };
                        target.CreateSolidColorBrush(&raw const d2d_color, None).ok()
                    } else if let Some(color) = span.color {
                        let d2d_color = D2D1_COLOR_F {
                            r: color.r,
                            g: color.g,
                            b: color.b,
                            a: color.a,
                        };
                        target.CreateSolidColorBrush(&raw const d2d_color, None).ok()
                    } else {
                        None
                    };
                    let active_st_brush = st_brush.as_ref().unwrap_or(&default_brush);
                    let stroke_width = match style {
                        crate::StrikethroughStyle::Solid => 1.0,
                        crate::StrikethroughStyle::Thick => 2.5,
                    };

                    (0..actual_count as usize).for_each(|m_idx| {
                        let metric = &hit_test_metrics[m_idx];
                        let strikethrough_y = metric.top + metric.height * 0.5;

                        target.DrawLine(
                            Vector2 {
                                X: metric.left,
                                Y: strikethrough_y,
                            },
                            Vector2 {
                                X: metric.left + metric.width,
                                Y: strikethrough_y,
                            },
                            active_st_brush,
                            stroke_width,
                            None,
                        );
                    });
                }

                if let Some(style) = span.underline {
                    let ul_brush = if let Some(color) = span.underline_color {
                        let d2d_color = D2D1_COLOR_F {
                            r: color.r,
                            g: color.g,
                            b: color.b,
                            a: color.a,
                        };
                        target.CreateSolidColorBrush(&raw const d2d_color, None).ok()
                    } else if let Some(color) = span.color {
                        let d2d_color = D2D1_COLOR_F {
                            r: color.r,
                            g: color.g,
                            b: color.b,
                            a: color.a,
                        };
                        target.CreateSolidColorBrush(&raw const d2d_color, None).ok()
                    } else {
                        None
                    };
                    let active_ul_brush = ul_brush.as_ref().unwrap_or(&default_brush);

                    (0..actual_count as usize).for_each(|m_idx| {
                        let metric = &hit_test_metrics[m_idx];
                        let left = metric.left;
                        let right = metric.left + metric.width;
                        let underline_y = metric.top + metric.height - 1.0;

                        match style {
                            crate::UnderlineStyle::Solid => {
                                target.DrawLine(
                                    Vector2 {
                                        X: left,
                                        Y: underline_y,
                                    },
                                    Vector2 {
                                        X: right,
                                        Y: underline_y,
                                    },
                                    active_ul_brush,
                                    1.0,
                                    None,
                                );
                            }
                            crate::UnderlineStyle::Thick => {
                                target.DrawLine(
                                    Vector2 {
                                        X: left,
                                        Y: underline_y,
                                    },
                                    Vector2 {
                                        X: right,
                                        Y: underline_y,
                                    },
                                    active_ul_brush,
                                    2.5,
                                    None,
                                );
                            }
                            crate::UnderlineStyle::Wave => {
                                let mut x = left;
                                let mut y_up = false;
                                let mut prev_x = x;
                                let mut prev_y = underline_y;
                                let wave_amplitude = 1.0;
                                let wave_step = 2.0;

                                while x < right {
                                    x += wave_step;
                                    let target_x = x.min(right);
                                    let target_y = if y_up {
                                        underline_y - wave_amplitude
                                    } else {
                                        underline_y + wave_amplitude
                                    };

                                    target.DrawLine(
                                        Vector2 {
                                            X: prev_x,
                                            Y: prev_y,
                                        },
                                        Vector2 {
                                            X: target_x,
                                            Y: target_y,
                                        },
                                        active_ul_brush,
                                        1.0,
                                        None,
                                    );
                                    prev_x = target_x;
                                    prev_y = target_y;
                                    y_up = !y_up;
                                }
                            }
                            _ => {}
                        }
                    });
                }
            }

            target.EndDraw(None, None).unwrap();

            // 5. ピクセルデータの抽出
            let mut bgra_pixels = vec![0u8; (width * height * 4) as usize];
            wic_bitmap
                .CopyPixels(std::ptr::null(), width * 4, &mut bgra_pixels)
                .unwrap();

            //  Alphaだけを集めて、wgpuに書き込む用の1チャネルバッファを作成
            let r8_pixels: Vec<u8> = bgra_pixels.chunks_exact(4).map(|p| p[3]).collect();

            r8_pixels
        }
    }
}

/// 安全に `TextSpan` 配列全体の等価ハッシュを計算するヘルパー
pub(crate) fn hash_text_spans(spans: &[crate::TextSpan]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    for span in spans {
        span.range.start.hash(&mut hasher);
        span.range.end.hash(&mut hasher);

        if let Some(c) = span.color {
            c.r.to_bits().hash(&mut hasher);
            c.g.to_bits().hash(&mut hasher);
            c.b.to_bits().hash(&mut hasher);
            c.a.to_bits().hash(&mut hasher);
        }
        if let Some(c) = span.bg_color {
            c.r.to_bits().hash(&mut hasher);
            c.g.to_bits().hash(&mut hasher);
            c.b.to_bits().hash(&mut hasher);
            c.a.to_bits().hash(&mut hasher);
        }
        if let Some(sz) = span.font_size {
            sz.to_bits().hash(&mut hasher);
        }

        span.font_family.hash(&mut hasher);
        span.font_weight.hash(&mut hasher);
        span.font_style.hash(&mut hasher);

        if let Some(u) = span.underline {
            (u as u8).hash(&mut hasher);
        }
        if let Some(c) = span.underline_color {
            c.r.to_bits().hash(&mut hasher);
            c.g.to_bits().hash(&mut hasher);
            c.b.to_bits().hash(&mut hasher);
            c.a.to_bits().hash(&mut hasher);
        }
        if let Some(s) = span.strikethrough {
            (s as u8).hash(&mut hasher);
        }
        if let Some(c) = span.strikethrough_color {
            c.r.to_bits().hash(&mut hasher);
            c.g.to_bits().hash(&mut hasher);
            c.b.to_bits().hash(&mut hasher);
            c.a.to_bits().hash(&mut hasher);
        }
    }
    hasher.finish()
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct TextCacheKey {
    pub(crate) text: Cow<'static, str>,
    pub(crate) font_size_bits: u32,
    pub(crate) font_family: Option<Cow<'static, str>>,
    pub(crate) font_weight: Option<u32>,
    pub(crate) font_style: Option<u32>,
    pub(crate) spans_hash: u64,
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
            // 色空間を sRGB フォーマットに明示変更
            // 再度 R8Unorm に修正
            format: wgpu::TextureFormat::R8Unorm,
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

