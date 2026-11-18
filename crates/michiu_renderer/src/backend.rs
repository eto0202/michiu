use crate::{CpuRenderer, WgpuRenderer};
use michiu_ui::Context;

pub trait Renderer {
    fn resize(&mut self, p_width: u32, p_height: u32, scale_factor: f32);
    fn render(&mut self, cx: &mut Context, scale_factor: f32);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentBackend {
    Gpu,
    Cpu,
}

pub enum ActiveBackend {
    Gpu(Box<WgpuRenderer>),
    Cpu(Box<CpuRenderer>),
}

impl ActiveBackend {
    #[inline]
    #[must_use]
    pub fn kind(&self) -> CurrentBackend {
        match self {
            Self::Gpu(_) => CurrentBackend::Gpu,
            Self::Cpu(_) => CurrentBackend::Cpu,
        }
    }

    #[inline]
    pub fn resize(&mut self, p_width: u32, p_height: u32, scale_factor: f32) {
        match self {
            Self::Gpu(r) => r.resize(p_width, p_height, scale_factor),
            Self::Cpu(r) => r.resize(p_width, p_height, scale_factor),
        }
    }

    #[inline]
    pub fn render(&mut self, cx: &mut Context, scale_factor: f32) {
        match self {
            Self::Gpu(r) => r.render(cx, scale_factor),
            Self::Cpu(r) => r.render(cx, scale_factor),
        }
    }

    #[inline]
    #[must_use]
    pub fn gpu(&self) -> Option<&WgpuRenderer> {
        match self {
            Self::Gpu(r) => Some(r),
            Self::Cpu(_) => None,
        }
    }

    #[inline]
    pub fn gpu_mut(&mut self) -> Option<&mut WgpuRenderer> {
        match self {
            Self::Gpu(r) => Some(r),
            Self::Cpu(_) => None,
        }
    }

    #[inline]
    #[must_use]
    pub fn cpu(&self) -> Option<&CpuRenderer> {
        match self {
            Self::Cpu(r) => Some(r),
            Self::Gpu(_) => None,
        }
    }

    #[inline]
    pub fn cpu_mut(&mut self) -> Option<&mut CpuRenderer> {
        match self {
            Self::Cpu(r) => Some(r),
            Self::Gpu(_) => None,
        }
    }
}
