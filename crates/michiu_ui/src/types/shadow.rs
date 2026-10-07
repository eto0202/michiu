use crate::{Color, Convert, IntoLayoutPoint, LayoutPoint, rgba};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct BoxShadow {
    pub offset: LayoutPoint,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
}

impl Default for BoxShadow {
    #[inline]
    fn default() -> Self {
        Self {
            offset: LayoutPoint::ZERO,
            blur: 0.0,
            spread: 0.0,
            color: Color::BLACK, // デフォルトは黒
        }
    }
}

impl BoxShadow {
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            offset: LayoutPoint::ZERO,
            blur: 0.0,
            spread: 0.0,
            color: Color::BLACK,
        }
    }

    /// Set the shadow offset (x, y).
    #[inline]
    #[must_use]
    pub fn offset(mut self, value: impl IntoLayoutPoint) -> Self {
        self.offset = value.into_layout_point();
        self
    }

    /// Set the shadow blur width.
    #[inline]
    #[must_use]
    pub fn blur(mut self, value: impl Convert<f32>) -> Self {
        self.blur = value.convert();
        self
    }

    /// Set the shadow spread width.
    #[inline]
    #[must_use]
    pub fn spread(mut self, value: impl Convert<f32>) -> Self {
        self.spread = value.convert();
        self
    }

    /// Set the shadow color width.
    ///
    /// Default is [`Color::BLACK`].
    #[inline]
    #[must_use]
    pub fn color(mut self, value: Color) -> Self {
        self.color = value;
        self
    }

    /// A subtle, ultra-fine soft shadow
    #[must_use]
    pub fn sm() -> Self {
        BoxShadow::new()
            .blur(2)
            .color(rgba(0, 0, 0, 0.05))
            .offset((0, 1))
    }

    /// Standard Medium Soft Shadow
    #[must_use]
    pub fn md() -> Self {
        BoxShadow::new()
            .blur(6)
            .spread(-1)
            .color(rgba(0, 0, 0, 0.1))
            .offset((0, 4))
    }

    /// A large, soft shadow that appears to be slightly raised
    #[must_use]
    pub fn lg() -> Self {
        BoxShadow::new()
            .blur(15)
            .spread(-3)
            .color(rgba(0, 0, 0, 0.1))
            .offset((0, 10))
    }

    #[must_use]
    pub fn none() -> Self {
        Self {
            offset: LayoutPoint::ZERO,
            blur: 0.0,
            spread: 0.0,
            color: Color::TRANSPARENT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Display {
    #[default]
    Flex,
    Grid,
    Block,
    None,
}
