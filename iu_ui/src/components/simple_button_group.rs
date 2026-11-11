use super::core::{MichiuTheme, MichiuWidget};
use michiu::{accessibility::*, prelude::*};

/// Visual appearance variants for [`SimpleButtonGroup`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonGroupVariant {
    /// Filled background using `theme.bg_ui()` / `theme.solid()`.
    #[default]
    Solid,
    /// Transparent background with an outline border using `theme.border()`.
    Outline,
    /// Fully transparent background that subtly highlights on hover.
    Ghost,
}

/// Inner button sizing options for [`SimpleButtonGroup`].
///
/// All sizes strictly satisfy the WCAG 2.5.8 minimum target size requirement (>= 24x24px).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonGroupSize {
    /// Extra small size.
    Xs,
    /// Small size.
    Sm,
    /// Standard size.
    #[default]
    Md,
    /// Large size.
    Lg,
}

/// An item within a [`SimpleButtonGroup`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ButtonGroupItem {
    /// An interactive button (custom `Entity`, with events, icons, etc.).
    Button(Entity),
    /// A simple text label button (automatically styled by the group).
    Label(String),
    /// A visual separator line hidden from screen readers (`NO_A11Y_NODE`).
    Separator,
}

/// Accessible grouped buttons / segmented controls widget compliant with WAI-ARIA and WCAG 2.2 AA.
///
/// This component provides a unified container with standardized sizing, hover effects,
/// and keyboard focus rings, while allowing child items to be raw labels or custom interactive [`Entity`] instances.
///
/// # Scope
/// - **Stand-alone**: Can be used by copying only this file, provided `core.rs`
///   (`MichiuWidget`, `MichiuTheme`) is present in your project.
///
/// # Required Assets
/// - None
///
/// # Event Loop Requirements
/// - None (Self-contained within the widget tree).
///
/// # Feature Details
/// - **Direction**:
///   - Supports horizontal (`Row`) and vertical (`Column`) layouts via `.direction(...)`.
///   - Separator orientation, thickness, and margins adapt dynamically based on direction.
/// - **Keyboard**:
///   - `Tab` / `Shift + Tab`: Moves focus sequentially between interactive buttons within the group.
///   - `Space` / `Enter`: Activates the currently focused button.
/// - **Screen Reader**:
///   - Role: `Role::Button` for each interactive child item.
///   - Separators are excluded from the accessibility tree via `NO_A11Y_NODE` to prevent announcement noise.
/// - **Visual / WCAG**:
///   - **Target Size**: Minimum 28x28px per button slot (exceeds WCAG 2.5.8 minimum of 24x24px).
///   - **Focus Indicator**: 2px outline displayed only on keyboard focus (`focused_visible`),
///     maintaining >= 3.0:1 contrast ratio against the container background (WCAG 1.4.11, 2.4.11).
///   - **Slot Encapsulation**: Injected child [`Entity`] items are wrapped in a standardized
///     slot modifier to guarantee uniform size, corner radius, and focus outlines regardless of child contents.
/// - **Customization**:
///   - Chain modifier methods directly after `.build(&theme)` to override outer container styles,
///     padding, gap, or border properties.
///
/// # Example
///
/// ```rust
/// // 1. Simple text & separator layout
/// let group = SimpleButtonGroup::new(["1".into(), "|".into(), "2".into()])
///     .variant(ButtonGroupVariant::Outline)
///     .build(&theme);
///
/// let group = simple_button_group!["1", "|", "2", "3", "4", "|", "5",].build(&theme);
///
/// // 2. Mixing custom clickable entities with labels
/// let custom_group = SimpleButtonGroup::new([
///     // Custom entity with click handler
///     text("☆").on_click(|| println!("Star!")).into(),
///     // Separator
///     "|".into(),
///     // Simple labels
///     "☆".into(),
/// ])
/// .variant(ButtonGroupVariant::Solid)
/// .build(&theme);
///
/// // 3. Vertical toolbar
/// let vertical_toolbar = simple_button_group!["1", "2", "|", "3"]
///     .direction(FlexDirection::Column)
///     .build(&theme);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SimpleButtonGroup {
    /// Layout direction of the group (Row, Column, etc.).
    pub direction: FlexDirection,
    /// List of buttons, labels, and separators.
    pub items: Vec<ButtonGroupItem>,
    /// Visual style variant (Solid, Outline, Ghost).
    pub variant: ButtonGroupVariant,
    /// Button dimensions.
    pub size: ButtonGroupSize,
}

impl SimpleButtonGroup {
    /// Creates a new button group.
    ///
    /// Accepts any combination of:
    /// - Custom `Entity` (e.g. `button.on_click(...)`)
    /// - `&str` labels (e.g. `"1"`, `"2"`)
    /// - `"|"` for separators
    pub fn new<I>(items: I) -> Self
    where
        I: IntoIterator<Item = ButtonGroupItem>,
    {
        Self {
            direction: FlexDirection::Row,
            items: items.into_iter().collect(),
            variant: ButtonGroupVariant::default(),
            size: ButtonGroupSize::default(),
        }
    }

    /// Sets the layout direction of the group.
    pub fn direction(mut self, direction: FlexDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Sets the visual style variant (Solid, Outline, Ghost).
    pub fn variant(mut self, variant: ButtonGroupVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets the dimensions of the button.
    pub fn size(mut self, size: ButtonGroupSize) -> Self {
        self.size = size;
        self
    }
}
impl Default for SimpleButtonGroup {
    /// Creates a default pagination-style button group (`["1", "|", "2", "3", "4", "|", "5"]`).
    fn default() -> Self {
        Self::new([
            "1".into(),
            "|".into(),
            "2".into(),
            "3".into(),
            "4".into(),
            "|".into(),
            "5".into(),
        ])
    }
}

impl MichiuWidget for SimpleButtonGroup {
    fn name(&self) -> &'static str {
        "Simple Button Group"
    }

    fn a11y_spec(&self) -> &'static str {
        "WAI-ARIA Button Group / WCAG 2.2 AA (Target Size >= 24px, Focus Ring >= 3:1)"
    }

    fn build(&self, theme: &dyn MichiuTheme) -> Entity {
        let (font_size, padding) = match self.size {
            ButtonGroupSize::Xs => (14.0, 4.0),
            ButtonGroupSize::Sm => (14.0, 6.0),
            ButtonGroupSize::Md => (15.0, 8.0),
            ButtonGroupSize::Lg => (15.0, 10.0),
        };

        let (bg, hover, active, border, separator, text_color, focus_ring) = match self.variant {
            ButtonGroupVariant::Solid => (
                theme.accent_solid(),
                theme.accent_solid_hover(),
                theme.accent_8(),
                Color::TRANSPARENT,
                theme.text_on_solid().with_lightness(0.8),
                theme.text_on_solid(),
                theme.text_on_solid(),
            ),
            ButtonGroupVariant::Outline => (
                Color::TRANSPARENT,
                theme.gray_bg_ui_hover(),
                theme.gray_bg_ui_active(),
                theme.gray_border(),
                theme.gray_border(),
                theme.gray_text_high(),
                theme.focus_ring(),
            ),
            ButtonGroupVariant::Ghost => (
                Color::TRANSPARENT,
                theme.gray_bg_ui_hover(),
                theme.gray_bg_ui_active(),
                Color::TRANSPARENT,
                theme.gray_border(),
                theme.gray_text_high(),
                theme.focus_ring(),
            ),
        };

        let btn_modifier = ii()
            .justify_center()
            .items_center()
            .min_size(24.0)
            .p(padding)
            .r(3.0)
            .cursor_pointer()
            .bg_color(Color::TRANSPARENT)
            .hovered(ii().bg_color(hover))
            .pressed(ii().bg_color(active))
            .focusable_self_both()
            .focused_visible(
                ii().border_color(Color::TRANSPARENT)
                    .outline_solid(2.0)
                    .outline_offset(0.0)
                    .outline_color(focus_ring),
            );

        let separator_size = match self.direction {
            FlexDirection::Row | FlexDirection::RowReverse => ii().size((1.0, pct(80.0))).m_x(1.0),
            _ => ii().size((pct(80.0), 1.0)).m_y(1.0),
        };

        let mut children = Vec::with_capacity(self.items.len());

        for item in &self.items {
            match item {
                // ユーザーが渡したカスタム Entity
                ButtonGroupItem::Button(user_entity) => {
                    children.push(
                        flex(&btn_modifier)
                            .tag::<a11y::AButton>()
                            .child(*user_entity),
                    );
                }
                // 単純なテキストラベル
                ButtonGroupItem::Label(text) => {
                    children.push(flex(&btn_modifier).tag::<a11y::AButton>().label(
                        text.clone(),
                        ii().font_size(font_size).text_color(text_color),
                    ));
                }
                // セパレーター
                ButtonGroupItem::Separator => {
                    children.push(
                        div(separator_size
                            .clone()
                            .shrink_0()
                            .self_center()
                            .bg_color(separator))
                        .a11y(NO_A11Y_NODE),
                    );
                }
            }
        }

        flex(
            ii().flex_direction(self.direction)
                .justify_center()
                .items_stretch()
                .size_fit()
                .p(2.0)
                .gap(2.0)
                .r(3.0)
                .bg_color(bg)
                .border_solid(1.0)
                .border_color(border),
        )
        .children(children)
    }
}

impl From<Entity> for ButtonGroupItem {
    /// Converts a custom [`Entity`] directly into a [`ButtonGroupItem::Button`].
    fn from(entity: Entity) -> Self {
        ButtonGroupItem::Button(entity)
    }
}

fn str_to_item(s: &str) -> ButtonGroupItem {
    if s == "|" {
        ButtonGroupItem::Separator
    } else {
        ButtonGroupItem::Label(s.to_string())
    }
}
impl From<&str> for ButtonGroupItem {
    /// Converts a string-like type into a [`ButtonGroupItem`].
    ///
    /// Passing `"|"` produces a [`ButtonGroupItem::Separator`]; any other string
    /// produces a [`ButtonGroupItem::Label`].
    fn from(s: &str) -> Self {
        str_to_item(s)
    }
}
impl From<String> for ButtonGroupItem {
    /// Converts a string-like type into a [`ButtonGroupItem`].
    ///
    /// Passing `"|"` produces a [`ButtonGroupItem::Separator`]; any other string
    /// produces a [`ButtonGroupItem::Label`].
    fn from(s: String) -> Self {
        if s == "|" {
            ButtonGroupItem::Separator
        } else {
            ButtonGroupItem::Label(s)
        }
    }
}

/// Creates a new button group.
///
/// # Example
///
/// ```rust
/// let group = simple_button_group!["1", "|", "2", "3", "4", "|", "5",].build(&theme);
/// ```
#[macro_export]
macro_rules! simple_button_group {
    ($($item:expr),* $(,)?) => {
        SimpleButtonGroup::new([
            $(
                $crate::components::simple_button_group::ButtonGroupItem::from($item),
            )*
        ])
    };
}
