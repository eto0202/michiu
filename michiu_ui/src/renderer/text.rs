use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Range;

use crate::types::LayoutSize;
use crate::{
    ByteIndex, DebugStore, EdgeInsets, FontDate, InputContents, LayoutPoint, LayoutRect,
    MichiuError, MichiuString, OptionTraceExt, RendererView, TextAlign, TextSpan, VisualProperty,
};
use cosmic_text::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight, Wrap,
};
use rustc_hash::FxHashMap;
use smallvec::{SmallVec, smallvec};

#[derive(Debug, Clone, Copy)]
pub(crate) struct TextLayoutSize {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) is_multiline: Option<bool>,
}

impl TextLayoutSize {
    pub(crate) const DEFAULT: Self = Self {
        width: 0.0,
        height: 0.0,
        is_multiline: Some(false),
    };

    #[inline]
    pub(crate) fn new(width: f32, height: f32, is_multiline: Option<bool>) -> Self {
        Self {
            width,
            height,
            is_multiline,
        }
    }

    #[inline]
    pub(crate) fn set_multiline(mut self, is_multiline: Option<bool>) -> Self {
        self.is_multiline = is_multiline;
        self
    }
}

pub struct TextEngine {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TextCacheKey {
    pub cache_key: CacheKey,
}

#[derive(Clone, Debug, Copy)]
pub struct TextCacheValue {
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub offset_x: i32,
    pub offset_y: i32,
    pub width: f32,
    pub height: f32,
}

impl TextCacheValue {
    pub(crate) const ZERO: Self = Self {
        uv_min: [0.0, 0.0],
        uv_max: [0.0, 0.0],
        offset_x: 0,
        offset_y: 0,
        width: 0.0,
        height: 0.0,
    };
}

impl TextEngine {
    pub(crate) fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
        }
    }

    pub(crate) fn create_buffer(
        &mut self,
        text: &MichiuString,
        font: &FontDate,
        text_align: TextAlign,
        max_width: Option<f32>,
        auto_wrap: bool,
        spans: &[TextSpan],
    ) -> Buffer {
        let font_size = font.size.unwrap_or(FontDate::FONT_SIZE);
        let metrics = Metrics::new(font_size, font_size * 1.4);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);

        let mut default_attrs = Attrs::new();

        if let Some(family) = font.family.as_deref() {
            default_attrs = default_attrs.family(Family::Name(family));
        }
        if let Some(weight) = font.weight {
            default_attrs = default_attrs.weight(Weight(weight as u16));
        }
        if let Some(style) = font.style {
            default_attrs = default_attrs.style(match style {
                1 => Style::Italic,
                2 => Style::Oblique,
                _ => Style::Normal,
            });
        }

        let align = Self::map_to_align(text_align);

        buffer.set_size(max_width, None);
        if auto_wrap && max_width.is_some() {
            buffer.set_wrap(Wrap::WordOrGlyph);
        } else {
            buffer.set_wrap(Wrap::None);
        }

        if spans.is_empty() {
            buffer.set_text(text, &default_attrs, Shaping::Advanced, align);
        } else {
            let text_len = text.byte_len();
            let mut boundaries = vec![ByteIndex(0), text_len];
            for span in spans {
                if span.range.start < text_len && text.is_char_boundary(span.range.start.0) {
                    boundaries.push(span.range.start);
                }
                if span.range.end < text_len && text.is_char_boundary(span.range.end.0) {
                    boundaries.push(span.range.end);
                }
            }
            boundaries.sort_unstable();
            boundaries.dedup();

            let mut rich_spans = Vec::with_capacity(boundaries.len().saturating_sub(1));

            for window in boundaries.windows(2) {
                let start = window[0];
                let end = window[1];
                let slice_str = text.slice(start..end);

                let mut attrs = default_attrs.clone();

                for span in spans {
                    if span.range.start <= start && span.range.end >= end {
                        if let Some(size) = span.font_size {
                            attrs = attrs.metrics(Metrics::new(size, size * 1.4));
                        }
                        if let Some(color) = span.color {
                            attrs = attrs.color(cosmic_text::Color::rgba(
                                (color.r * 255.0).clamp(0.0, 255.0).round() as u8,
                                (color.g * 255.0).clamp(0.0, 255.0).round() as u8,
                                (color.b * 255.0).clamp(0.0, 255.0).round() as u8,
                                (color.a * 255.0).clamp(0.0, 255.0).round() as u8,
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
                }
                rich_spans.push((slice_str, attrs));
            }

            buffer.set_rich_text(rich_spans, &default_attrs, Shaping::Advanced, align);
        }

        buffer.shape_until_scroll(&mut self.font_system, false);
        buffer
    }

    fn map_to_align(align: TextAlign) -> Option<cosmic_text::Align> {
        match align {
            TextAlign::Center => Some(cosmic_text::Align::Center),
            TextAlign::Right => Some(cosmic_text::Align::Right),
            _ => None, // TextAlign::Left や Auto は None（Left）
        }
    }

    pub(crate) fn measure_text(
        &mut self,
        text: &MichiuString,
        font: &FontDate,
        text_align: TextAlign,
        max_width: Option<f32>,
        auto_wrap: bool,
        spans: &[TextSpan],
    ) -> TextLayoutSize {
        if text.is_empty() {
            return TextLayoutSize::DEFAULT;
        }

        let buffer = self.create_buffer(text, font, text_align, max_width, auto_wrap, spans);

        Self::get_layout_size(&buffer)
    }

    pub(crate) fn get_layout_size(buffer: &Buffer) -> TextLayoutSize {
        let mut width = 0.0f32;
        let mut height = 0.0f32;

        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            height = height.max(run.line_top + run.line_height);
        }

        TextLayoutSize {
            width,
            height,
            is_multiline: None,
        }
    }

    pub(crate) fn get_caret_position(buffer: &Buffer, index: ByteIndex) -> (f32, f32, f32) {
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        let mut height = 0.0f32;
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
    pub(crate) fn flat_idx_to_cursor(buffer: &Buffer, flat_idx: ByteIndex) -> cosmic_text::Cursor {
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
            if flat_idx.0 < line_end_with_nl || is_last_line {
                let index_in_line = (flat_idx.0.saturating_sub(accum)).min(line_len);
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
    pub(crate) fn cursor_to_flat_idx(buffer: &Buffer, cursor: &cosmic_text::Cursor) -> ByteIndex {
        let mut flat_idx = 0;
        for (line_idx, line) in buffer.lines.iter().enumerate() {
            if line_idx == cursor.line {
                break;
            }
            flat_idx += line.text().len() + 1; // 各行の末尾にある '\n'
        }
        ByteIndex(flat_idx + cursor.index)
    }

    pub(crate) fn hit_test_point(buffer: &Buffer, point: LayoutPoint) -> (ByteIndex, bool) {
        if let Some(cursor) = buffer.hit(point.x, point.y) {
            // 2D位置をフラットなバイトインデックスに変換
            let flat_index = Self::cursor_to_flat_idx(buffer, &cursor);
            (flat_index, false)
        } else {
            (ByteIndex(0), false)
        }
    }

    pub(crate) fn get_or_create_glyph_uv(
        &mut self,
        cache_key: CacheKey,
        view: &mut RendererView,
        scale_factor: f32,
        debug: &mut DebugStore,
    ) -> (TextCacheValue, bool) {
        let key = TextCacheKey { cache_key };
        if let Some(cached) = view.text_cache.get(&key) {
            return (*cached, false);
        }

        let image_opt = self.swash_cache.get_image(&mut self.font_system, cache_key);

        let Some(image) = image_opt else {
            return (TextCacheValue::ZERO, false);
        };

        let width = image.placement.width;
        let height = image.placement.height;

        // スペース文字など、描画ピクセルを持たないグリフの早期リターン
        if width == 0 || height == 0 {
            view.text_cache.insert(key, TextCacheValue::ZERO);
            return (TextCacheValue::ZERO, false);
        }

        let mut alloc_res = view.atlas.allocate(width, height);
        let mut cleared = false;

        if alloc_res.is_none() {
            view.atlas.clear();
            view.text_cache.clear();
            alloc_res = view.atlas.allocate(width, height);
            cleared = true;
        }

        let (x, y) =
            alloc_res.unwrap_or_trace(None, debug, || MichiuError::GlyphAllocationFailed {
                width,
                height,
                atlas_w: view.atlas.size,
                atlas_h: view.atlas.size,
            });

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

        let value = TextCacheValue {
            uv_min,
            uv_max,
            offset_x,
            offset_y,
            width: log_width,
            height: log_height,
        };

        view.text_cache.insert(key.clone(), value);

        (value, cleared)
    }

    /// cosmic-text の Buffer から、指定されたインデックス範囲が占める各行の矩形を計算
    pub(crate) fn calc_span_rects(buffer: &Buffer, range: Range<ByteIndex>) -> Vec<LayoutRect> {
        let mut rects = Vec::with_capacity((range.start.0..range.end.0).len());

        // 各段落の開始バイト位置を事前計算
        let mut line_starts = Vec::with_capacity(buffer.lines.len());
        let mut accum = 0;
        for line in &buffer.lines {
            line_starts.push(accum);
            accum += line.text().len() + 1; // '\n' を考慮
        }

        for run in buffer.layout_runs() {
            let line_flat_start = line_starts.get(run.line_i).copied().unwrap_or(0);

            let mut start_x: Option<f32> = None;
            let mut end_x: Option<f32> = None;

            for glyph in run.glyphs {
                // グリフのフラットな開始・終了バイト位置を復元して交差判定
                let glyph_flat_start = line_flat_start + glyph.start;
                let glyph_flat_end = line_flat_start + glyph.end;

                if glyph_flat_start < range.end.0 && glyph_flat_end > range.start.0 {
                    if start_x.is_none() {
                        start_x = Some(glyph.x);
                    }
                    end_x = Some(glyph.x + glyph.w);
                }
            }

            // この行で交差するグリフが見つかった場合、その範囲で矩形を作成
            if let (Some(sx), Some(ex)) = (start_x, end_x) {
                rects.push(LayoutRect::new(sx, run.line_top, ex - sx, run.line_height));
            }
        }
        rects
    }
}

#[derive(Debug, Clone)]
pub struct TextureAtlas {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub size: u32,

    // パッキング状態
    pub current_x: u32,
    pub current_y: u32,
    pub row_max_height: u32,
    pub padding: u32,
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
