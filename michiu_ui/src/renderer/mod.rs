#![allow(unused)]

mod composed_renderer;
mod init_webview2;
mod interop;
mod text;
mod wgpu_renderer;

pub use composed_renderer::*;
pub use init_webview2::*;
pub use interop::*;
pub use text::*;
pub use wgpu_renderer::*;

use crate::{Color, CornerRadius, EdgeInsets, EntityId, LayoutRect};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct QuadInstance {
    pub(crate) rect: LayoutRect,            // 16B. offset: 0
    pub(crate) transform: [[f32; 4]; 3],    // 48B. offset: 16
    pub(crate) color: Color,                // 16B. offset: 64
    pub(crate) corner_radius: CornerRadius, // 16B. offset: 80
    pub(crate) border_width: EdgeInsets,    // 16B. offset: 96
    pub(crate) border_color: Color,         // 16B. offset: 112

    // opacity, mode をパック
    pub(crate) opacity_mode_sizing: [f32; 4], // 16B. offset: 128

    pub(crate) uv_min: [f32; 2], // 8B.  offset: 144
    pub(crate) uv_max: [f32; 2], // 8B.  offset: 152

    pub(crate) gradient_end_color: Color, // 16B. offset: 160

    pub(crate) gradient_angle: f32,        // 4B.  offset: 176
    pub(crate) transform_origin: [f32; 2], // 8B.  offset: 180
    pub(crate) _padding: f32,              // 4B.  offset: 188

    pub(crate) shadow_color: Color,     // 16B. offset: 192
    pub(crate) shadow_params: [f32; 4], // 16B. offset: 208

    pub(crate) border_lengths: EdgeInsets, // 16B. offset: 224

    pub(crate) outline_width: EdgeInsets,   // 16B. offset: 240
    pub(crate) outline_color: Color,        // 16B. offset: 256
    pub(crate) outline_lengths: EdgeInsets, // 16B. offset: 272
    pub(crate) outline_offset_and_flags: [f32; 4], // 16B. offset: 288 (flags: [offset, flags, 0.0, 0.0])
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
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchType {
    Normal,
    Punchout,
}

/// 同じクリップ（Scissor）範囲で描画できるインスタンスの塊
pub struct DrawBatch {
    pub scissor_rect: LayoutRect,
    pub instances: Vec<QuadInstance>,
    pub(crate) entity_ids: Vec<EntityId>,
    pub(crate) batch_type: BatchType,
}

pub struct RenderData {
    pub batches: Vec<DrawBatch>,
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
