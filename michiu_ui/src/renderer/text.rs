use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Range;

use crate::types::LayoutSize;
use crate::{
    EdgeInsets, LayoutRect, NewTextCacheKey, NewTextCacheValue, RendererView, TextAlign, TextSpan,
    VisualProperty,
};
use cosmic_text::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight, Wrap,
};
use rustc_hash::FxHashMap;
use smallvec::{SmallVec, smallvec};
use windows::Win32::Graphics::Direct2D::{
    D2D1_RENDER_TARGET_TYPE_SOFTWARE, D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, ID2D1RenderTarget,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_WORD_WRAPPING_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP, DWRITE_WORD_WRAPPING_WRAP,
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
            DirectWrite::{
                DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE,
                DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_HIT_TEST_METRICS, DWRITE_LINE_SPACING_METHOD_UNIFORM, DWRITE_TEXT_METRICS,
                DWRITE_TEXT_RANGE, DWriteCreateFactory, IDWriteBitmapRenderTarget1_Impl,
                IDWriteFactory, IDWriteFactory_Impl, IDWriteFactory1_Impl, IDWriteFactory2_Impl,
                IDWriteFactory3_Impl, IDWriteFactory6_Impl, IDWriteFont_Impl, IDWriteFont1_Impl,
                IDWriteFontFace_Impl, IDWriteFontFace1_Impl, IDWriteInlineObject_Impl,
                IDWriteRenderingParams, IDWriteRenderingParams_Impl, IDWriteTextFormat,
                IDWriteTextFormat_Impl, IDWriteTextFormat2_Impl, IDWriteTextLayout,
                IDWriteTextLayout_Impl, IDWriteTextLayout2_Impl, IDWriteTextLayout3_Impl,
            },
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

    pub(crate) font_system: FontSystem,
    pub(crate) swash_cache: SwashCache,
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

            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
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
        auto_wrap: Option<bool>,
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

            // 自動折り返し
            if auto_wrap.unwrap_or(false) && max_width.is_some() {
                // DWrite の DWRITE_WORD_WRAPPING_WRAP は単語単位で改行を決定
                // スペースのない英数字の連続の直後に日本語が密着して続いている場合、
                // DWrite は英数字＋日本語の一部を一つの巨大な単語と誤判定しコンテナ幅に収める
                // 結果として、単語の途中で切れるのを避けるために英数字部分を不自然に手前で改行させる
                layout
                    .SetWordWrapping(DWRITE_WORD_WRAPPING_CHARACTER)
                    .unwrap();
            } else {
                layout
                    .SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)
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
        auto_wrap: Option<bool>,
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
                auto_wrap,
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
            let _ = layout.HitTestPoint(
                x,
                y,
                &raw mut is_trailing,
                &raw mut is_inside,
                &raw mut metrics,
            );

            // (ヒットした文字インデックス, 文字ブロックの後半部分（右半分）をクリックしたかどうかのフラグ)
            (metrics.textPosition as usize, is_trailing.into())
        }
    }

    /// 与えられたレイアウトに配置されているすべての文字クラスターの個別位置情報を、
    /// サロゲートペアを考慮しながら1文字ずつ確実に分離・分解して解決
    pub(crate) fn get_all_char_metrics(
        &self,
        layout: &IDWriteTextLayout,
        text_u16_len: usize,
    ) -> Vec<DWRITE_HIT_TEST_METRICS> {
        if text_u16_len == 0 {
            return Vec::new();
        }
        let mut results = Vec::with_capacity(text_u16_len);
        let mut idx = 0;

        while idx < text_u16_len {
            // 1文字分のバッファを確保
            let mut hit_test_metrics = vec![DWRITE_HIT_TEST_METRICS::default(); 4];
            let mut actual_count: u32 = 0;

            // 1文字ずつ正確にレンジを切り出して個別に位置を逆算
            let res = unsafe {
                layout.HitTestTextRange(
                    idx as u32,
                    1, // 1文字制限
                    0.0,
                    0.0,
                    Some(&mut hit_test_metrics),
                    &raw mut actual_count,
                )
            };

            if res.is_ok() && actual_count > 0 {
                // その文字をピッタリ囲む最初の矩形メトリクスを採用
                let metric = hit_test_metrics[0];
                results.push(metric);

                // サロゲートペアや複雑な文字結合を考慮してDWrite が消費した実コードユニット数で安全に進める
                // 無限ループを防止するためのガード
                let step = if metric.length > 0 {
                    metric.length as usize
                } else {
                    1
                };
                idx += step;
            } else {
                idx += 1;
            }
        }

        results
    }

    /// グリフのUV座標を解決
    /// キャッシュに存在しない場合は指定されたアトラスとラスタライザを用いてテクスチャへ描き込み
    pub(crate) fn get_or_create_glyph_uv(
        &self,
        key: &TextCacheKey,
        view: &mut RendererView,
    ) -> ([f32; 2], [f32; 2], bool) {
        if let Some(cached) = view.text_cache.get(key) {
            return (cached.uv_min, cached.uv_max, false);
        }

        let text_str = key.character.to_string();
        let font_size_phys = f32::from_bits(key.font_size_bits);

        // 1文字用の最小レイアウトを構築
        let physical_layout = self.create_layout(
            &text_str,
            font_size_phys,
            key.font_family.as_deref(),
            key.font_weight,
            key.font_style,
            None,
            Some(false),
            &[],
        );

        let size = self.get_layout_size(&physical_layout);
        let r8_pixels =
            view.text_rasterizer
                .rasterize_glyph(&physical_layout, size, &self.rendering_params);

        let width = size.width.ceil() as u32;
        let height = size.height.ceil() as u32;

        let mut alloc_res = view.atlas.allocate(width, height);
        let mut cleared = false;

        if alloc_res.is_none() {
            view.atlas.clear();
            view.text_cache.clear();
            alloc_res = view.atlas.allocate(width, height);
            cleared = true;
        }

        let (x, y) = alloc_res.expect("Glyph exceeds maximum atlas size!");

        view.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &view.atlas.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &r8_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let (uv_min, uv_max) = view.atlas.texel_to_uv(x, y, width, height);
        view.text_cache
            .insert(key.clone(), TextCacheValue { uv_min, uv_max });

        (uv_min, uv_max, cleared)
    }

    pub(crate) fn create_buffer_cosmic(
        &mut self,
        text: &str,
        font_size: f32,
        font_family: Option<&str>,
        font_weight: Option<u32>,
        font_style: Option<u32>,
        text_align: TextAlign,
        max_width: Option<f32>,
        auto_wrap: Option<bool>,
        spans: &[TextSpan],
    ) -> Buffer {
        let metrics = Metrics::new(font_size, font_size * 1.2);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);

        let mut default_attrs = Attrs::new();
        if let Some(family) = font_family {
            default_attrs = default_attrs.family(Family::Name(family));
        }
        if let Some(weight) = font_weight {
            default_attrs = default_attrs.weight(Weight(weight as u16));
        }
        if let Some(style) = font_style {
            default_attrs = default_attrs.style(match style {
                2 => Style::Italic,
                _ => Style::Normal,
            });
        }

        let align = Self::map_to_cosmic_align(text_align);

        buffer.set_size(max_width, Some(f32::MAX));
        if auto_wrap.unwrap_or(false) && max_width.is_some() {
            buffer.set_wrap(Wrap::Glyph);
        } else {
            buffer.set_wrap(Wrap::None);
        }

        if spans.is_empty() {
            buffer.set_text(text, &default_attrs, Shaping::Advanced, align);
        } else {
            let text_len = text.len();
            let mut boundaries = vec![0, text_len];
            for span in spans {
                if span.range.start < text_len && text.is_char_boundary(span.range.start) {
                    boundaries.push(span.range.start);
                }
                if span.range.end < text_len && text.is_char_boundary(span.range.end) {
                    boundaries.push(span.range.end);
                }
            }
            boundaries.sort_unstable();
            boundaries.dedup();

            let mut slice_strings = Vec::with_capacity(boundaries.len());
            let mut rich_spans = Vec::with_capacity(boundaries.len());

            for window in boundaries.windows(2) {
                let start = window[0];
                let end = window[1];
                if start >= end {
                    continue;
                }

                let slice_str = &text[start..end];
                slice_strings.push(slice_str);
            }

            for (i, window) in boundaries.windows(2).enumerate() {
                let start = window[0];
                let end = window[1];
                if start >= end {
                    continue;
                }

                let mut attrs = default_attrs.clone();
                if let Some(span) = spans
                    .iter()
                    .find(|s| s.range.start <= start && s.range.end >= end)
                {
                    if let Some(size) = span.font_size {
                        attrs = attrs.metrics(Metrics::new(size, size * 1.2));
                    }
                    if let Some(color) = span.color {
                        attrs = attrs.color(cosmic_text::Color::rgba(
                            (color.r * 255.0) as u8,
                            (color.g * 255.0) as u8,
                            (color.b * 255.0) as u8,
                            (color.a * 255.0) as u8,
                        ));
                    }
                    if let Some(ref family) = span.font_family {
                        attrs = attrs.family(Family::Name(family));
                    }
                    if let Some(weight) = span.font_weight {
                        attrs = attrs.weight(Weight(weight as u16));
                    }
                    if let Some(style) = span.font_style {
                        attrs = attrs.style(match style {
                            2 => Style::Italic,
                            _ => Style::Normal,
                        });
                    }
                }
                rich_spans.push((slice_strings[i], attrs));
            }

            buffer.set_rich_text(rich_spans, &default_attrs, Shaping::Advanced, align);
        }

        buffer.shape_until_scroll(&mut self.font_system, false);
        buffer
    }

    fn map_to_cosmic_align(align: TextAlign) -> Option<cosmic_text::Align> {
        match align {
            TextAlign::Center => Some(cosmic_text::Align::Center),
            TextAlign::Right => Some(cosmic_text::Align::Right),
            _ => None, // TextAlign::Left や Auto は None（Left）
        }
    }

    pub(crate) fn measure_text_cosmic(
        &mut self,
        text: &str,
        font_size: f32,
        font_family: Option<&str>,
        font_weight: Option<u32>,
        font_style: Option<u32>,
        text_align: TextAlign,
        max_width: Option<f32>,
        auto_wrap: Option<bool>,
        spans: &[TextSpan],
    ) -> LayoutSize {
        if text.is_empty() {
            return LayoutSize::ZERO;
        }

        let buffer = self.create_buffer_cosmic(
            text,
            font_size,
            font_family,
            font_weight,
            font_style,
            text_align,
            max_width,
            auto_wrap,
            spans,
        );

        self.get_layout_size_cosmic(&buffer)
    }

    pub(crate) fn get_layout_size_cosmic(&self, buffer: &Buffer) -> LayoutSize {
        let mut width = 0.0f32;
        let mut height = 0.0f32;

        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            height += run.line_height;
        }

        LayoutSize::new(width, height)
    }

    pub(crate) fn get_caret_position_cosmic(
        &self,
        buffer: &Buffer,
        index: usize,
        text_len: usize,
    ) -> (f32, f32, f32) {
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        let mut height = 16.0f32;
        let mut found = false;

        // フラットなインデックスから 2D Cursor へ
        let cursor = Self::flat_idx_to_cursor(buffer, index);

        // この段落に属するすべてのビジュアル行を抽出
        let mut candidate_runs: SmallVec<[_; 16]> = SmallVec::new();
        for run in buffer.layout_runs() {
            if run.line_i == cursor.line {
                candidate_runs.push(run);
            }
        }

        // 複数あるビジュアル行の中から、キャレットが実際に属している1行を特定
        let mut matched_run = None;
        if !candidate_runs.is_empty() {
            for (i, run) in candidate_runs.iter().enumerate() {
                let run_start = run.glyphs.first().map_or(0, |g| g.start);
                let run_end = run.glyphs.last().map_or(0, |g| g.end);

                let is_last_run = i == candidate_runs.len() - 1;

                // 折り返された最後の行であれば、開始位置以降はすべてこの行に収める
                if is_last_run && cursor.index >= run_start {
                    matched_run = Some(run);
                    break;
                }
                // 途中の折り返し行であれば、開始位置から終了位置の手前までに収まるかチェック
                if cursor.index >= run_start && cursor.index < run_end {
                    matched_run = Some(run);
                    break;
                }
            }
        }

        // 特定した正しいビジュアル行からX/Y/Heightを算出
        if let Some(run) = matched_run {
            y = run.line_top;
            height = run.line_height;
            found = true;

            let mut glyph_found = false;
            for glyph in run.glyphs {
                if cursor.index >= glyph.start && cursor.index < glyph.end {
                    x = glyph.x;
                    glyph_found = true;
                    break;
                }
            }

            // 行末、または空行などでグリフがヒットしなかった場合の補正
            if !glyph_found {
                if let Some(last_glyph) = run.glyphs.last() {
                    x = last_glyph.x + last_glyph.w;
                } else {
                    x = 0.0;
                }
            }
        }

        // 万が一見つからなかった場合のフォールバック
        if !found && let Some(last_run) = buffer.layout_runs().last() {
            height = last_run.line_height;
            y = last_run.line_top;
            if let Some(last_glyph) = last_run.glyphs.last() {
                x = last_glyph.x + last_glyph.w;
            }
        }

        (x, y, height)
    }

    // フラットなバイト位置から 2D Cursor を算出
    pub(crate) fn flat_idx_to_cursor(buffer: &Buffer, flat_idx: usize) -> cosmic_text::Cursor {
        let mut accum = 0;
        let lines_len = buffer.lines.len();

        if lines_len == 0 {
            return cosmic_text::Cursor::default();
        }

        for (line_idx, line) in buffer.lines.iter().enumerate() {
            let line_len = line.text().len();
            // 最終行以外は '\n' の 1 バイトを考慮
            let is_last_line = line_idx == lines_len - 1;
            let line_end_with_nl = accum + line_len + usize::from(!is_last_line);

            // 現在の行の範囲内（末尾の改行を含む）かチェック
            if flat_idx < line_end_with_nl || is_last_line {
                let index_in_line = (flat_idx.saturating_sub(accum)).min(line_len);
                return cosmic_text::Cursor {
                    line: line_idx,
                    index: index_in_line,
                    affinity: cosmic_text::Affinity::Before,
                };
            }

            accum = line_end_with_nl;
        }

        let last_line = buffer.lines.len().saturating_sub(1);
        let last_line_len = buffer.lines.get(last_line).map_or(0, |l| l.text().len());
        cosmic_text::Cursor {
            line: last_line,
            index: last_line_len,
            affinity: cosmic_text::Affinity::Before,
        }
    }

    // 2D Cursor からフラットなバイト位置を逆算
    pub(crate) fn cursor_to_flat_idx(buffer: &Buffer, cursor: &cosmic_text::Cursor) -> usize {
        let mut flat_idx = 0;
        for (line_idx, line) in buffer.lines.iter().enumerate() {
            if line_idx == cursor.line {
                break;
            }
            flat_idx += line.text().len() + 1; // 各行の末尾にある '\n'
        }
        flat_idx + cursor.index
    }

    pub(crate) fn hit_test_point_cosmic(&self, buffer: &Buffer, x: f32, y: f32) -> (usize, bool) {
        if let Some(cursor) = buffer.hit(x, y) {
            // 2D位置をフラットなバイトインデックスに変換
            let flat_index = Self::cursor_to_flat_idx(buffer, &cursor);
            (flat_index, false)
        } else {
            (0, false)
        }
    }

    pub(crate) fn get_or_create_glyph_uv_cosmic(
        &mut self,
        cache_key: CacheKey,
        view: &mut RendererView,
        scale_factor: f32,
    ) -> ([f32; 2], [f32; 2], i32, i32, f32, f32, bool) {
        let key = NewTextCacheKey { cache_key };
        if let Some(cached) = view.new_text_cache.get(&key) {
            return (
                cached.uv_min,
                cached.uv_max,
                cached.offset_x,
                cached.offset_y,
                cached.width,
                cached.height,
                false,
            );
        }

        let image_opt = self.swash_cache.get_image(&mut self.font_system, cache_key);

        let Some(image) = image_opt else {
            return ([0.0, 0.0], [0.0, 0.0], 0, 0, 0.0, 0.0, false);
        };

        let width = image.placement.width;
        let height = image.placement.height;

        let mut alloc_res = view.atlas.allocate(width, height);
        let mut cleared = false;

        if alloc_res.is_none() {
            view.atlas.clear();
            view.text_cache.clear();
            alloc_res = view.atlas.allocate(width, height);
            cleared = true;
        }

        let (x, y) = alloc_res.expect("Glyph exceeds maximum atlas size");

        view.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &view.atlas.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &image.data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let (uv_min, uv_max) = view.atlas.texel_to_uv(x, y, width, height);
        let offset_x = image.placement.left;
        let offset_y = image.placement.top;
        let log_width = width as f32 / scale_factor;
        let log_height = height as f32 / scale_factor;

        view.new_text_cache.insert(
            key.clone(),
            NewTextCacheValue {
                uv_min,
                uv_max,
                offset_x,
                offset_y,
                width: log_width,
                height: log_height,
            },
        );

        (
            uv_min, uv_max, offset_x, offset_y, log_width, log_height, cleared,
        )
    }

    /// cosmic-text の Buffer から、指定されたインデックス範囲が占める各行の矩形を計算
    pub(crate) fn calc_span_rects_cosmic(buffer: &Buffer, range: Range<usize>) -> Vec<LayoutRect> {
        let mut rects = Vec::new();

        for run in buffer.layout_runs() {
            let mut start_x: Option<f32> = None;
            let mut end_x: Option<f32> = None;

            for glyph in run.glyphs {
                // スパンの範囲に文字のインデックスが一部でも交差しているか判定
                if glyph.start < range.end && glyph.end > range.start {
                    if start_x.is_none() {
                        start_x = Some(glyph.x);
                    }
                    // グリフの右端座標
                    end_x = Some(glyph.x + glyph.w);
                }
            }

            // この行で交差するグリフが見つかった場合、その範囲で矩形を作成
            if let (Some(sx), Some(ex)) = (start_x, end_x) {
                rects.push(LayoutRect::new(sx, run.line_y, ex - sx, run.line_height));
            }
        }
        rects
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
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).unwrap()
        };
        Self {
            d2d_factory,
            wic_factory,
        }
    }

    pub(crate) fn rasterize_glyph(
        &self,
        layout: &IDWriteTextLayout,
        size: LayoutSize,
        rendering_params: &IDWriteRenderingParams,
    ) -> Vec<u8> {
        // 文字の物理ピクセルバッファ境界（最低 1x1 ）
        let width = (size.width.ceil() as u32).max(1);
        let height = (size.height.ceil() as u32).max(1);

        let wic_bitmap = unsafe {
            self.wic_factory
                .CreateBitmap(
                    width,
                    height,
                    &GUID_WICPixelFormat32bppPBGRA,
                    WICBitmapCacheOnDemand,
                )
                .unwrap()
        };

        let props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
            ..Default::default()
        };

        let target = unsafe {
            self.d2d_factory
                .CreateWicBitmapRenderTarget(&wic_bitmap, &raw const props)
                .unwrap()
        };

        unsafe {
            target.SetTextRenderingParams(rendering_params);
            target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);

            target.BeginDraw();
            target.Clear(None);
        }

        let color = D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let default_brush = unsafe {
            target
                .CreateSolidColorBrush(&raw const color, None)
                .unwrap()
        };

        // 装飾は wgpu Quad 側で行うため単にレイアウトを描画
        let origin = Vector2 { X: 0.0, Y: 0.0 };
        unsafe {
            target.DrawTextLayout(origin, layout, &default_brush, D2D1_DRAW_TEXT_OPTIONS_NONE);
            target.EndDraw(None, None).unwrap();
        }

        let mut bgra_pixels = vec![0u8; (width * height * 4) as usize];
        unsafe {
            wic_bitmap
                .CopyPixels(std::ptr::null(), width * 4, &mut bgra_pixels)
                .unwrap()
        };

        let r8_pixels: Vec<u8> = bgra_pixels.chunks_exact(4).map(|p| p[3]).collect();

        r8_pixels
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
    pub(crate) character: char,
    pub(crate) font_size_bits: u32,
    pub(crate) font_family: Option<Cow<'static, str>>,
    pub(crate) font_weight: Option<u32>,
    pub(crate) font_style: Option<u32>,
}

impl TextCacheKey {
    #[inline]
    pub(crate) fn new(
        span: Option<&TextSpan>,
        visual: &VisualProperty,
        character: char,
        scale_factor: f32,
    ) -> Self {
        let font_size = span
            .and_then(|s| s.font_size)
            .unwrap_or(visual.font_size.unwrap_or(16.0));
        let font_family = span
            .and_then(|s| s.font_family.clone())
            .or_else(|| visual.font_family.clone());
        let font_weight = span.and_then(|s| s.font_weight).or(visual.font_weight);
        let font_style = span.and_then(|s| s.font_style).or(visual.font_style);

        TextCacheKey {
            character,
            font_size_bits: (font_size * scale_factor).to_bits(),
            font_style,
            font_family,
            font_weight,
        }
    }
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
