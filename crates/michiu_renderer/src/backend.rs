use crate::{CpuRenderer, MichiuRendererResult, WgpuRenderer};
use michiu_ui::Context;

/// Abstraction over a rendering backend (such as [`WgpuRenderer`] or [`CpuRenderer`]).
///
/// Implementations manage their own backing swap chain or surface, texture atlases,
/// and frame presentation for a DirectComposition visual.
pub trait Renderer {
    /// Resizes the renderer's backing surface or swap chain and updates DPI scaling state.
    ///
    /// `p_width` and `p_height` specify the new surface dimensions in physical pixels,
    /// and `scale_factor` specifies the current display DPI scale factor.
    ///
    /// # Errors
    ///
    /// Returns an error if resizing the underlying surface, swap chain buffers,
    /// or CPU backing buffers fails.
    fn resize(
        &mut self,
        p_width: u32,
        p_height: u32,
        scale_factor: f32,
    ) -> MichiuRendererResult<()>;

    /// Collects render batches from [`Context`] and renders and presents a single frame.
    ///
    /// Note that callers using DirectComposition are still responsible for committing
    /// the composition device if visual tree changes need to be flushed.
    ///
    /// # Errors
    ///
    /// Returns an error if acquiring a frame from the swap chain, uploading resources,
    /// or presenting the rendered frame fails.
    fn render(&mut self, cx: &mut Context, scale_factor: f32) -> MichiuRendererResult<()>;
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

    /// Resizes the active rendering backend by delegating to [`Renderer::resize`].
    ///
    /// `p_width` and `p_height` specify the new surface dimensions in physical pixels,
    /// and `scale_factor` specifies the current display DPI scale factor.
    ///
    /// # Errors
    ///
    /// Returns an error if the active backend fails to resize its underlying surface,
    /// swap chain buffers, or CPU backing buffers.
    #[inline]
    pub fn resize(
        &mut self,
        p_width: u32,
        p_height: u32,
        scale_factor: f32,
    ) -> MichiuRendererResult<()> {
        match self {
            Self::Gpu(r) => r.resize(p_width, p_height, scale_factor)?,
            Self::Cpu(r) => r.resize(p_width, p_height, scale_factor)?,
        }
        Ok(())
    }

    /// Renders and presents a frame using the active rendering backend by delegating
    /// to [`Renderer::render`].
    ///
    /// # Errors
    ///
    /// Returns an error if the active backend fails to acquire a swap chain frame,
    /// upload resources from [`Context`], or present the rendered frame.
    #[inline]
    pub fn render(&mut self, cx: &mut Context, scale_factor: f32) -> MichiuRendererResult<()> {
        match self {
            Self::Gpu(r) => r.render(cx, scale_factor)?,
            Self::Cpu(r) => r.render(cx, scale_factor)?,
        }
        Ok(())
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
