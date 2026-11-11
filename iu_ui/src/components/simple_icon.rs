use super::core::{MichiuTheme, MichiuWidget, Oklch};
use michiu::{
    accessibility::*,
    prelude::*,
    ui::{ExternalMask, ExternalMaskMetadata, MaskImage},
};
use resvg::{tiny_skia, usvg};
use std::{
    borrow::Cow,
    hash::{DefaultHasher, Hash, Hasher},
    path::Path,
    sync::Arc,
};

/// Visual appearance variants for [`SimpleIcon`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum IconVariant {
    #[default]
    White,
    Gray,
    Accent,
}

/// Sizing options for [`SimpleIcon`].
///
/// All sizes strictly satisfy the WCAG 2.5.8 minimum target size requirement (>= 24x24px).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum IconSize {
    /// Extra small size: 16x16px.
    Xs,
    /// Small size: 24x24px.
    #[default]
    Sm,
    /// Medium size: 36x36px.
    Md,
    /// Large size: 48x48px.
    Lg,
    /// Cutom size
    Custom(f32),
}

/// Icon category for [`SimpleIcon`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum IconCategory {
    // 画像として扱う。代替テキスト有り。インタラクション不可。
    Image(Cow<'static, str>),
    // 装飾のためツリーから外す。
    #[default]
    Decoration,
}
impl IconCategory {
    pub fn image(label: impl Into<Cow<'static, str>>) -> Self {
        Self::Image(label.into())
    }
}

#[derive(Debug, Clone)]
pub struct SimpleIcon {
    pub icon: SvgIcon,
    /// Visual style variant (White, Gray, Accent).
    pub variant: IconVariant,
    /// Icon dimensions.
    pub size: IconSize,
    /// Whether the icon is disabled.
    pub disabled: bool,
    /// Icon category (Button, Image, Decoration)
    pub category: IconCategory,
}

impl SimpleIcon {
    /// Creates a new accessibility icon from a file path with default options.
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        let svg_data = match std::fs::read(path) {
            Ok(data) => data,
            Err(e) => {
                let reason = match e.kind() {
                    std::io::ErrorKind::NotFound => "File does not exist",
                    std::io::ErrorKind::PermissionDenied => "Permission denied to read the file",
                    std::io::ErrorKind::IsADirectory => {
                        "The specified path is a directory, not a file"
                    }
                    _ => "Failed to read the file",
                };
                panic!(
                    "\n[Simple Icon Error] Failed to load icon\n\
                         Reason: {reason}\n\
                         Path: \"{}\"\n\
                         Details: {e}\n",
                    path.display()
                );
            }
        };

        Self::from_data(&svg_data)
    }

    /// Creates a new accessibility icon from raw SVG bytes with default options.
    pub fn from_data(svg_data: &[u8]) -> Self {
        let icon = match SvgIcon::from_data(svg_data) {
            Ok(i) => i,
            Err(e) => {
                if let Some(usvg_err) = e.downcast_ref() {
                    let hint = match usvg_err {
                        usvg::Error::InvalidSize => {
                            "Please check if the SVG has a 'viewBox' or positive 'width'/'height' attributes."
                        }
                        usvg::Error::ParsingFailed(_) => {
                            "Malformed XML/SVG syntax. Please check for missing closing tags or unclosed quotes."
                        }
                        usvg::Error::NotAnUtf8Str => {
                            "Data is not valid UTF-8. Did you accidentally pass a binary image like PNG or JPEG?"
                        }
                        usvg::Error::MalformedGZip | usvg::Error::SvgzFeatureNotEnabled => {
                            "Failed to decompress gzip-compressed SVG (svgz). Please ensure the 'svgz' feature is enabled."
                        }
                        usvg::Error::ElementsLimitReached => {
                            "The number of SVG elements exceeds the limit (1,000,000 elements)."
                        }
                    };
                    panic!(
                        "[Simple Icon Error] Failed to parse SVG icon.\n[Cause]: {e}\n[Hint]: {hint}"
                    );
                } else {
                    panic!("[Simple Icon Error] Unexpected error: {e}");
                }
            }
        };
        Self {
            icon,
            variant: IconVariant::default(),
            size: IconSize::default(),
            disabled: false,
            category: IconCategory::default(),
        }
    }

    /// Sets the visual style variant.
    pub fn variant(mut self, variant: IconVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets the dimensions of the icon.
    pub fn size(mut self, size: IconSize) -> Self {
        self.size = size;
        self
    }

    /// Sets whether the icon is disabled.
    /// All Chroma values will be set to 0.0, and the Lightness of the border will be adjusted.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set the icon category.
    pub fn category(mut self, cat: IconCategory) -> Self {
        self.category = cat;
        self
    }
}

impl MichiuWidget for SimpleIcon {
    fn name(&self) -> &'static str {
        "Simple Icon"
    }

    fn a11y_spec(&self) -> &'static str {
        ""
    }

    /// Builds and returns the underlying accessible UI entity.
    ///
    /// Additional styling or event handlers (such as `.on_click(...)`) can be chained
    /// onto the returned `Entity` to overwrite defaults.
    fn build(&self, theme: &dyn MichiuTheme) -> Entity {
        let size = match self.size {
            IconSize::Xs => (16.0, 16.0),
            IconSize::Sm => (24.0, 24.0),
            IconSize::Md => (36.0, 36.0),
            IconSize::Lg => (48.0, 48.0),
            IconSize::Custom(s) => (s, s),
        };

        let text_color = match self.variant {
            IconVariant::White => theme.text_on_solid(),
            IconVariant::Gray => theme.gray_text_low(),
            IconVariant::Accent => theme.accent_solid(),
        };
        let disabled_text_color = if self.disabled {
            theme.gray_7()
        } else {
            text_color
        };

        let mut modifier = ii()
            .justify_center()
            .items_center()
            .pointer_events_none()
            .min_size(12.0)
            .size(size)
            .text_color(text_color)
            .disabled(
                ii().text_color(disabled_text_color)
                    .cursor_default()
                    .focusable_none(),
            );
        let mut node = Node::new(Role::GenericContainer);

        let modifier = match &self.category {
            IconCategory::Decoration => modifier,
            IconCategory::Image(label) => {
                node.set_role(Role::Image);
                node.set_label(label.to_string());
                modifier
            }
        };

        external_mask(self.icon.clone())
            .modify(modifier)
            .a11y(node)
            .disable(self.disabled)
    }
}

#[derive(Clone, Debug)]
pub struct SvgIcon {
    tree: Arc<usvg::Tree>,
    metadata: ExternalMaskMetadata,
}

impl SvgIcon {
    pub fn new(path: impl AsRef<Path>) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let path = path.as_ref();
        let svg_data = std::fs::read(path)?;
        Self::from_data(&svg_data)
    }

    pub fn from_data(svg_data: &[u8]) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let opt = usvg::Options::default();
        let tree = usvg::Tree::from_data(svg_data, &opt)?;

        let original_size = tree.size();
        let size = LayoutSize::new(original_size.width(), original_size.height());

        let mut hasher = DefaultHasher::new();
        svg_data.hash(&mut hasher);
        let cache_key = hasher.finish();

        let metadata = ExternalMaskMetadata::new(size).with_cache_key(cache_key);

        Ok(Self {
            tree: Arc::new(tree),
            metadata,
        })
    }
}

impl ExternalMask for SvgIcon {
    fn metadata(&self) -> ExternalMaskMetadata {
        self.metadata
    }

    fn resolve_mask(&self, width: u32, height: u32) -> MaskImage {
        let w = width.max(1);
        let h = height.max(1);

        let mut pixmap =
            tiny_skia::Pixmap::new(w, h).expect("[SvgIcon Error] Failed to allocate pixmap");

        let orig_size = self.tree.size();
        let sx = w as f32 / orig_size.width();
        let sy = h as f32 / orig_size.height();
        let transform = tiny_skia::Transform::from_scale(sx, sy);

        resvg::render(&self.tree, transform, &mut pixmap.as_mut());

        let bytes: Vec<u8> = pixmap.pixels().iter().map(|p| p.alpha()).collect();

        MaskImage {
            width: w,
            height: h,
            bytes,
        }
    }
}

#[macro_export]
macro_rules! include_simple_icon {
    ($path:expr) => {
        $crate::SimpleIcon::from_data(include_bytes!($path))
    };
}
