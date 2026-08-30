#![allow(unused)]

mod composed_renderer;
mod init_webview2;
mod interop;
mod text;
mod wgpu_renderer;

pub use composed_renderer::*;
pub use init_webview2::*;
pub use interop::*;
use rustc_hash::FxHashMap;
pub use text::*;
pub use wgpu_renderer::*;

use crate::{Color, CornerRadius, EdgeInsets, EntityId, LayoutRect};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct QuadInstance {
    pub(crate) rect: LayoutRect,            // 16B
    pub(crate) transform: [[f32; 4]; 3],    // 48B
    pub(crate) color: Color,                // 16B.
    pub(crate) corner_radius: CornerRadius, // 16B
    pub(crate) border_width: EdgeInsets,    // 16B
    pub(crate) border_color: Color,         // 16B

    pub(crate) opacity_mode_sizing: [f32; 4], // 16B ([opacity, mode, sizing, 0.0])

    pub(crate) uv_min: [f32; 2], // 8B
    pub(crate) uv_max: [f32; 2], // 8B

    pub(crate) gradient_end_color: Color, // 16B

    pub(crate) gradient_angle: f32,        // 4B
    pub(crate) transform_origin: [f32; 2], // 8B
    pub(crate) _padding: f32,              // 4B.

    pub(crate) shadow_color: Color,     // 16B.
    pub(crate) shadow_params: [f32; 4], // 16B.

    pub(crate) border_lengths: EdgeInsets, // 16B

    pub(crate) outline_width: EdgeInsets,          // 16B.
    pub(crate) outline_color: Color,               // 16B
    pub(crate) outline_lengths: EdgeInsets,        // 16B
    pub(crate) outline_offset_and_flags: [f32; 4], // 16B (flags: [offset, flags, 0.0, 0.0])
    pub(crate) alpha_mode_y_flip_srgb: [f32; 4],   // 16B ([alpha_mode, y_flip, srbg, 0.0])
}

impl Default for QuadInstance {
    fn default() -> Self {
        Self {
            rect: LayoutRect::ZERO,
            transform: [[0.0; 4]; 3],
            transform_origin: [0.5, 0.5],
            color: Color::TRANSPARENT,
            corner_radius: CornerRadius::ZERO,
            border_width: EdgeInsets::ZERO,
            border_color: Color::TRANSPARENT,
            opacity_mode_sizing: [1.0, 0.0, 0.0, 0.0],
            uv_min: [0.0; 2],
            uv_max: [0.0; 2],
            gradient_end_color: Color::TRANSPARENT,
            gradient_angle: 0.0,
            _padding: 0.0,
            shadow_color: Color::TRANSPARENT,
            shadow_params: [0.0; 4],
            border_lengths: EdgeInsets::ZERO,
            outline_width: EdgeInsets::ZERO,
            outline_color: Color::TRANSPARENT,
            outline_lengths: EdgeInsets::ZERO,
            outline_offset_and_flags: [0.0; 4],
            alpha_mode_y_flip_srgb: [0.0; 4],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchType {
    Normal,
    Punchout,
}

/// 同じクリップ範囲で描画できるインスタンスの塊
pub struct DrawBatch {
    pub scissor_rect: LayoutRect,
    // フラットバッファ上のインデックス範囲
    pub instance_offset: usize,
    pub instance_count: usize,
    pub(crate) batch_type: BatchType,
}

pub(crate) struct RendererView<'a> {
    pub(crate) render_data: &'a mut RenderData,
    pub(crate) atlas: &'a mut TextureAtlas,
    pub(crate) text_rasterizer: &'a TextRasterizer,
    pub(crate) text_cache: &'a mut FxHashMap<TextCacheKey, TextCacheValue>,
    pub(crate) queue: &'a wgpu::Queue,
}

#[derive(Default)]
pub struct RenderData {
    pub batches: Vec<DrawBatch>,
    // 1フレーム分の全インスタンス
    pub instances: Vec<QuadInstance>,
    pub entity_ids: Vec<EntityId>,
}

impl RenderData {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            batches: Vec::new(),
            instances: Vec::new(),
            entity_ids: Vec::new(),
        }
    }
    #[inline]
    pub fn clear(&mut self) {
        self.batches.clear();
        self.instances.clear();
        self.entity_ids.clear();
    }
    #[inline]
    pub fn push(&mut self, id: EntityId, instance: QuadInstance) {
        self.instances.push(instance);
        self.entity_ids.push(id);
    }
}

pub(crate) const IDENTITY_MATRIX: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub(crate) struct Vertex {
    position: [f32; 2],
}

// 1x1 の矩形 (Unit Quad)
pub(crate) const VERTICES: &[Vertex] = &[
    Vertex {
        position: [0.0, 0.0],
    }, // 左上
    Vertex {
        position: [1.0, 0.0],
    }, // 右上
    Vertex {
        position: [1.0, 1.0],
    }, // 右下
    Vertex {
        position: [0.0, 1.0],
    }, // 左下
];

pub(crate) const INDICES: &[u16] = &[0, 1, 2, 2, 3, 0];
