#![allow(dead_code)]
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
    pub(crate) transform: [[f32; 4]; 4],    // 64B. offset: 16
    pub(crate) color: Color,                // 16B. offset: 80
    pub(crate) corner_radius: CornerRadius, // 16B. offset: 96
    pub(crate) border_width: EdgeInsets,    // 16B. offset: 112
    pub(crate) border_color: Color,         // 16B. offset: 128

    // opacity, mode を 1つの 16B 配列 [opacity, mode, sizing, 0.0] としてパック
    pub(crate) opacity_mode_sizing: [f32; 4], // 16B. offset: 144 (16の倍数。完璧にOK)

    pub(crate) uv_min: [f32; 2], // 8B.  offset: 160 (16の倍数。完璧にOK)
    pub(crate) uv_max: [f32; 2], // 8B.  offset: 168 (あわせて location 11 [160..176] とする)

    pub(crate) gradient_end_color: Color, // 16B. offset: 176 (16の倍数)

    pub(crate) gradient_angle: f32, // 4B.  offset: 192 (16の倍数)
    pub(crate) transform_origin: [f32; 2], // 8B.  offset: 196
    pub(crate) _padding: f32,       // 4B.  offset: 204

    pub(crate) shadow_color: Color,     // 16B. offset: 208
    pub(crate) shadow_params: [f32; 4], // 16B. offset: 224 (offset_x, offset_y, blur, spread)
}

impl QuadInstance {
    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<QuadInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // (0..9 番目までは一切の変更なし。オフセット 0 ～ 128)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 48,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 64,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 80,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 96,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 112,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 128,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // 10. opacity_and_mode (location 10)
                wgpu::VertexAttribute {
                    offset: 144,
                    shader_location: 10,
                    format: wgpu::VertexFormat::Float32x4, // Float32x4 に変更
                },
                // ─── 属性位置(location)を綺麗に前に詰め、すべて16の倍数のオフセットに配置 ───
                // 11. uv_range (location 11)
                wgpu::VertexAttribute {
                    offset: 160,
                    shader_location: 11,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // 12. gradient_end_color (location 12)
                wgpu::VertexAttribute {
                    offset: 176,
                    shader_location: 12,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // 13. gradient_angle_and_origin (location 13)
                wgpu::VertexAttribute {
                    offset: 192,
                    shader_location: 13,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // 14. shadow_color (location 14)
                wgpu::VertexAttribute {
                    offset: 208,
                    shader_location: 14,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // 15. shadow_params [offset_x, offset_y, blur, spread] (location 15)
                wgpu::VertexAttribute {
                    offset: 224,
                    shader_location: 15,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
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
