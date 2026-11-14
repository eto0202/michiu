use crate::IconSize::Custom;

use super::core::{MichiuTheme, MichiuWidget, Oklch};
use michiu::{accessibility::*, prelude::*};
use resvg::{tiny_skia, usvg};
use std::{
    borrow::Cow,
    hash::{DefaultHasher, Hash, Hasher},
    path::Path,
    sync::Arc,
};

/// Visual appearance variants for [`SimpleIcon`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum IconIntent {
    #[default]
    Primary,
    Secondary,
    Destructive,
    Success,
    Warning,
}

/// Sizing options for [`SimpleIcon`].
///
/// All sizes strictly satisfy the WCAG 2.5.8 minimum target size requirement (>= 24x24px).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum IconSize {
    /// Extra small size: 16x16px.
    #[default]
    Xs,
    /// Small size: 24x24px.
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
    #[inline]
    pub fn image(label: impl Into<Cow<'static, str>>) -> Self {
        Self::Image(label.into())
    }
}

/// Accessible SVG icon widget compliant with WAI-ARIA and WCAG 2.2 AA.
///
/// Designed to render vector graphics efficiently via `ExternalMask` while providing
/// accessible defaults (decorative by default to prevent screen reader noise, or
/// labeled as `Role::Image` when conveying standalone information).
///
/// # Scope
/// - **Stand-alone**: Can be used by copying only this file (or struct and its `impl` blocks),
///   provided `core.rs` (`MichiuWidget`, `MichiuTheme`, `Oklch`) is present in your project.
///
/// # Required Assets
/// - **Crate Dependency**: Requires `resvg = "0.49.0"` (along with `tiny_skia` and `usvg`) in your `Cargo.toml`.
///   - `cargo add resvg@0.49.0`
/// - **Icon Assets**: Valid SVG files (with a valid `viewBox` or positive `width`/`height` attributes).
///
/// # Event Loop Requirements
/// - None (Self-contained rasterization and caching within the widget tree).
///
/// # Feature Details
/// - **Keyboard**:
///   - Non-interactive by default (`focusable_none()`, `pointer_events_none()`).
///   - Tab focus is intentionally disallowed since static icons do not receive keyboard focus.
///     For interactive icon buttons, wrap this within a [`SimpleButton`].
/// - **Screen Reader**:
///   - **Decoration (`IconCategory::Decoration`, Default)**: Treated as `Role::GenericContainer`
///     and excluded from redundant reading, ensuring decorative glyphs beside text do not pollute the announcement.
///   - **Image (`IconCategory::Image(label)`)**: Assigned `Role::Image` with the given accessible label,
///     allowing screen readers to announce standalone informative icons.
/// - **Visual / WCAG**:
///   - **Scalability**: High-quality vector rasterization rendered on demand via `tiny_skia`.
///   - **Target Sizing**: When standalone, adheres to discrete sizing presets (16px to 48px).
///     When placed inside interactive widgets (e.g., [`SimpleButton`]), the parent container
///     satisfies the WCAG 2.5.8 minimum target size (>= 24x24px).
///   - **Theming & States**: Dynamic OKLCH color binding via `MichiuTheme`. Automatically dims chroma
///     and adjusts border contrast when `disabled` is set.
/// - **Performance**:
///   - Rasterized masks are uniquely cached using the 64-bit hash of the raw SVG byte buffer,
///     preventing redundant SVG parsing and tessellation on rebuilds.
/// - **Customization**:
///   - Additional styling can be chained **directly after calling `.build(&theme)`**.
///
/// # Example
///
/// ```rust,ignore
/// // 1. Decorative icon (default, ignored by screen readers, ideal for buttons)
/// let save_icon = SimpleIcon::from_data(include_bytes!(/* file path */))
///     .size(IconSize::Sm)
///     .variant(IconColor::White)
///     .build(&theme);
///
/// let save_icon = include_simple_icon!(/* file path */).build(&theme);
///
/// // 2. Informative icon (standalone warning with accessible text)
/// let warning_icon = SimpleIcon::new(/* file path */)
///     .category(IconCategory::image("Warning: Low storage"))
///     .variant(IconColor::Accent)
///     .size(IconSize::Md)
///     .build(&theme);
/// ```
#[derive(Debug, Clone)]
pub struct SimpleIcon {
    pub icon: SvgIcon,
    /// Visual style intent.
    pub intent: IconIntent,
    /// Icon dimensions.
    pub size: IconSize,
    /// Whether the icon is disabled.
    pub disabled: bool,
    /// Icon category (Button, Image, Decoration)
    pub category: IconCategory,
    /// Whether to change the text color based on the parent's state (such as hover).
    pub follow_parent: bool,
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
            intent: IconIntent::default(),
            size: IconSize::default(),
            disabled: false,
            category: IconCategory::default(),
            follow_parent: false,
        }
    }

    /// Sets the visual style intent.
    #[inline]
    pub fn intent(mut self, intent: IconIntent) -> Self {
        self.intent = intent;
        self
    }
    /// Set intent [`IconIntent::Primary`]
    #[inline]
    pub fn primary(self) -> Self {
        self.intent(IconIntent::Primary)
    }
    /// Set intent [`IconIntent::Secondary`]
    #[inline]
    pub fn secondary(self) -> Self {
        self.intent(IconIntent::Secondary)
    }
    /// Set intent [`IconIntent::Destructive`]
    #[inline]
    pub fn destructive(self) -> Self {
        self.intent(IconIntent::Destructive)
    }
    /// Set intent [`IconIntent::Success`]
    #[inline]
    pub fn success(self) -> Self {
        self.intent(IconIntent::Success)
    }
    /// Set intent [`IconIntent::Warning`]
    #[inline]
    pub fn warning(self) -> Self {
        self.intent(IconIntent::Warning)
    }

    /// Sets the dimensions of the icon.
    #[inline]
    pub fn size(mut self, size: IconSize) -> Self {
        self.size = size;
        self
    }
    /// Set size custom
    #[inline]
    pub fn size_custom(self, s: f32) -> Self {
        self.size(Custom(s))
    }
    /// Set size [`IconSize::Xs`]
    #[inline]
    pub fn xs(self) -> Self {
        self.size(IconSize::Xs)
    }
    /// Set size [`IconSize::Sm`]
    #[inline]
    pub fn sm(self) -> Self {
        self.size(IconSize::Sm)
    }
    /// Set size [`IconSize::Md`]
    #[inline]
    pub fn md(self) -> Self {
        self.size(IconSize::Md)
    }
    /// Set size [`IconSize::Lg`]
    #[inline]
    pub fn lg(self) -> Self {
        self.size(IconSize::Lg)
    }

    /// Sets whether the icon is disabled.
    /// All Chroma values will be set to 0.0, and the Lightness of the border will be adjusted.
    #[inline]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    /// Set disabled `true`
    #[inline]
    pub fn disabled(self) -> Self {
        self.with_disabled(true)
    }
    /// Set disabled `false`
    #[inline]
    pub fn enabled(self) -> Self {
        self.with_disabled(false)
    }

    /// Set the icon category.
    #[inline]
    pub fn category(mut self, cat: IconCategory) -> Self {
        self.category = cat;
        self
    }
    /// Set category [`IconCategory::Image`]
    #[inline]
    pub fn image(self, label: impl Into<Cow<'static, str>>) -> Self {
        self.category(IconCategory::Image(label.into()))
    }
    /// Set category [`IconCategory::Decoration`]
    #[inline]
    pub fn decoration(self) -> Self {
        self.category(IconCategory::Decoration)
    }

    /// Set whether to change the text color based on the parent's state (such as hover).
    #[inline]
    pub fn follow_parent(mut self, enabled: bool) -> Self {
        self.follow_parent = enabled;
        self
    }
    /// Set follow parent `true`
    #[inline]
    pub fn follow_allow(self) -> Self {
        self.follow_parent(true)
    }
    /// Set follow parent `false`
    #[inline]
    pub fn follow_deny(self) -> Self {
        self.follow_parent(false)
    }
}

impl MichiuWidget for SimpleIcon {
    #[inline]
    fn name(&self) -> &'static str {
        "Simple Icon"
    }

    #[inline]
    fn a11y_spec(&self) -> &'static str {
        "WAI-ARIA Graphics / WCAG 2.2 AA (Decorative by default, Role::Image when labeled)"
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

        let (text_color, hover, active) = match self.intent {
            IconIntent::Primary => (theme.accent_9(), theme.accent_10(), theme.accent_11()),
            IconIntent::Secondary => (theme.gray_9(), theme.gray_10(), theme.gray_11()),
            IconIntent::Destructive => (
                theme.destructive(9),
                theme.destructive(10),
                theme.destructive(11),
            ),
            IconIntent::Success => (theme.success(9), theme.success(10), theme.success(11)),
            IconIntent::Warning => (theme.warning(9), theme.warning(10), theme.warning(11)),
        };

        let disabled_text_color = if self.disabled {
            theme.gray_8()
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

        if self.follow_parent {
            modifier = modifier
                .hovered_parent(ii().text_color(hover))
                .pressed_parent(ii().text_color(active));
        }

        let mut node = Node::new(Role::GenericContainer);
        match &self.category {
            IconCategory::Decoration => {}
            IconCategory::Image(label) => {
                node.set_role(Role::Image);
                node.set_label(label.to_string());
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
    #[inline]
    pub fn new(path: impl AsRef<Path>) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let path = path.as_ref();
        let svg_data = std::fs::read(path)?;
        Self::from_data(&svg_data)
    }

    #[inline]
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
        SimpleIcon::from_data(include_bytes!($path))
    };
}
