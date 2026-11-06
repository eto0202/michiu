use michiu::{accessibility::*, prelude::*};

/// Simple accessible button (Light Theme).
///
/// # Scope
/// - **Stand-alone**: Can be used by copying only this function.
///
/// # Required Assets
/// - None
///
/// # Event Loop Requirements
/// - None
///
/// # Feature Details
///   - `Tab` / `Shift + Tab`: Move focus
///   - `Space` / `Enter`: Click the button (trigger `on_click`)
/// - **Screen Reader**:
///   - Role: `Role::Button` (inferred via `tag::<a11y::AButton>()`)
///   - Label: `"Button 1 L"`
/// - **Visual / WCAG**:
///   - `focused_visible`: Outline hidden when clicked with the mouse; border displayed only when keyboard focus is active (WCAG 2.4.7)
///   - Contrast ratio: Text approx. 12.5:1 (meets WCAG AAA), focus border approx. 3.1:1 (meets WCAG 1.4.11)
/// - **Customization**:
///   - Please modify the text, size, and `on_click` closure to suit your app.
pub fn button_1_light() -> Entity {
    let bg_ui = oklch(0.9097, 0.0, 0.0);
    let bg_ui_hover = oklch(0.8822, 0.0, 0.0);
    let bg_ui_active = oklch(0.8576, 0.0, 0.0);
    let focus_ring = oklch(0.58, 0.1155, 251.64);
    let text_high = oklch(0.2435, 0.0, 0.0);

    flex(
        ii().justify_center()
            .items_center()
            .size((96.0, 36.0))
            .r(3.0)
            .bg_color(bg_ui)
            .cursor_pointer()
            .hovered(ii().bg_color(bg_ui_hover))
            .pressed(ii().bg_color(bg_ui_active))
            .focusable_self_both()
            .focused_visible(
                ii().outline_solid(2.0)
                    .outline_offset(2.0)
                    .outline_color(focus_ring),
            ),
    )
    .tag::<a11y::AButton>()
    .label("Button 1 L", ii().font_size(14.0).text_color(text_high))
    .on_click(|| println!("Button 1 L: Clicked!"))
}

/// Simple accessible button (Dark Theme).
///
/// # Scope
/// - **Stand-alone**: Can be used by copying only this function.
///
/// # Required Assets
/// - None
///
/// # Event Loop Requirements
/// - None
///
/// # Feature Details
/// - **Keyboard**:
///   - `Tab` / `Shift + Tab`: Move focus
///   - `Space` / `Enter`: Click the button (triggers `on_click`)
/// - **Screen Reader**:
///   - Role: `Role::Button` (inferred via `tag::<a11y::AButton>()`)
///   - Label: `"Button 1 D"`
/// - **Visual / WCAG**:
///   - `focused_visible`: Outline hidden when clicked with the mouse; border displayed only when keyboard focus is active (WCAG 2.4.7)
///   - Contrast ratio: Text approx. 13.5:1 (meets WCAG AAA), focus border approx. 2.8:1 (3.8:1 against bg_app)
/// - **Customization**:
///   - Please customize the text, size, and `on_click` closure to suit your app.
pub fn button_1_dark() -> Entity {
    let bg_ui = oklch(0.2559, 0.004, 264.5);
    let bg_ui_hover = oklch(0.2856, 0.0074, 274.7);
    let bg_ui_active = oklch(0.3141, 0.009, 276.85);
    let focus_ring = oklch(0.5032, 0.1377, 266.68);
    let text_high = oklch(0.9497, 0.0027, 286.35);

    flex(
        ii().justify_center()
            .items_center()
            .size((96.0, 36.0))
            .r(3.0)
            .bg_color(bg_ui)
            .cursor_pointer()
            .hovered(ii().bg_color(bg_ui_hover))
            .pressed(ii().bg_color(bg_ui_active))
            .focusable_self_both()
            .focused_visible(
                ii().outline_solid(2.0)
                    .outline_offset(2.0)
                    .outline_color(focus_ring),
            ),
    )
    .tag::<a11y::AButton>()
    .label("Button 1 D", ii().font_size(14.0).text_color(text_high))
    .on_click(|| println!("Button 1 D: Clicked!"))
}

/// Accessible outline button (Light Theme).
///
/// # Scope
/// - **Stand-alone**: Can be used by copying only this function.
///
/// # Required Assets
/// - None
///
/// # Event Loop Requirements
/// - None
///
/// # Feature Details
/// - **Keyboard**:
///   - `Tab` / `Shift + Tab`: Move focus
///   - `Space` / `Enter`: Click the button (trigger `on_click`)
/// - **Screen Reader**:
///   - Role: `Role::Button` (inferred via `tag::<a11y::AButton>()`)
///   - Label: `"Button 2 L"`
/// - **Visual / WCAG**:
///   - **Transparent Background Notice**: This button has a transparent background by default.
///     To guarantee WCAG AA/AAA compliance, it **must be placed on light surfaces**.
///     Placing it over dark surfaces, colored banners, or images will break text and focus contrast.
///   - `focused_visible`: Outline hidden when clicked with mouse; 2px focus ring displayed on keyboard focus (WCAG 2.4.7, 2.4.11)
///   - Contrast ratio (tested against `bg_app`):
///     - Text: approx. 14.1:1 (meets WCAG AAA >= 7.0:1)
///     - Focus ring: approx. 3.5:1 (meets WCAG 1.4.11 & 2.4.11 >= 3.0:1)
///     - Hover border: approx. 4.2:1 (meets WCAG 1.4.11 >= 3.0:1)
///     - Default border: approx. 1.7:1 (subtle outline; button is primarily identified by the high-contrast label)
/// - **Customization**:
///   - If placing on non-standard backgrounds, explicitly add `.bg_color(...)` to ensure predictable contrast.
///   - Please modify the text, size, and `on_click` closure to suit your app.
pub fn button_2_light() -> Entity {
    let border = oklch(0.7952, 0.0, 0.0);
    let solid = oklch(0.556, 0.0, 0.0);
    let bg_ui_active = oklch(0.3141, 0.009, 276.85);
    let focus_ring = oklch(0.58, 0.1155, 251.64);
    let text_high = oklch(0.2435, 0.0, 0.0);

    flex(
        ii().justify_center()
            .items_center()
            .size((96.0, 36.0))
            .r(3.0)
            .border_solid(1.0)
            .border_color(border)
            .cursor_pointer()
            .hovered(ii().border_color(solid))
            .pressed(ii().bg_color(bg_ui_active.with_alpha(0.1)))
            .focusable_self_both()
            .focused_visible(
                ii().border_color(Color::TRANSPARENT)
                    .outline_solid(2.0)
                    .outline_offset(0.0)
                    .outline_color(focus_ring),
            ),
    )
    .tag::<a11y::AButton>()
    .label("Button 2 L", ii().font_size(14.0).text_color(text_high))
    .on_click(|| println!("Button 2 L: Clicked!"))
}

/// Accessible outline button (Dark Theme).
///
/// # Scope
/// - **Stand-alone**: Can be used by copying only this function.
///
/// # Required Assets
/// - None
///
/// # Event Loop Requirements
/// - None
///
/// # Feature Details
/// - **Keyboard**:
///   - `Tab` / `Shift + Tab`: Move focus
///   - `Space` / `Enter`: Click the button (trigger `on_click`)
/// - **Screen Reader**:
///   - Role: `Role::Button` (inferred via `tag::<a11y::AButton>()`)
///   - Label: `"Button 2 D"`
/// - **Visual / WCAG**:
///   - **Transparent Background Notice**: This button has a transparent background by default.
///     To guarantee WCAG AA/AAA compliance, it **must be placed on dark surfaces**.
///     Placing it over light surfaces or colored banners will break text and focus contrast.
///   - `focused_visible`: Outline hidden when clicked with mouse; 2px focus ring displayed on keyboard focus (WCAG 2.4.7, 2.4.11)
///   - Contrast ratio (tested against dark `bg_app`):
///     - Text: approx. 16.2:1 (meets WCAG AAA >= 7.0:1)
///     - Focus ring: approx. 5.1:1 (meets WCAG 1.4.11 & 2.4.11 >= 3.0:1)
///     - Default border: approx. 3.3:1 (meets WCAG 1.4.11 >= 3.0:1)
///     - Hover border: approx. 3.7:1 (meets WCAG 1.4.11 >= 3.0:1)
/// - **Customization**:
///   - If placing on non-standard backgrounds, explicitly add `.bg_color(...)` to ensure predictable contrast.
///   - Please modify the text, size, and `on_click` closure to suit your app.
pub fn button_2_dark() -> Entity {
    let border = oklch(0.4024, 0.012, 273.12);
    let solid = oklch(0.5403, 0.0172, 277.6);
    let bg_ui_active = oklch(0.3141, 0.009, 276.85);
    let focus_ring = oklch(0.5032, 0.1377, 266.68);
    let text_high = oklch(0.9497, 0.0027, 286.35);

    flex(
        ii().justify_center()
            .items_center()
            .size((96.0, 36.0))
            .r(3.0)
            .border_solid(1.0)
            .border_color(border)
            .cursor_pointer()
            .hovered(ii().border_color(solid))
            .pressed(ii().bg_color(bg_ui_active.with_alpha(0.2)))
            .focusable_self_both()
            .focused_visible(
                ii().border_color(Color::TRANSPARENT)
                    .outline_solid(2.0)
                    .outline_offset(0.0)
                    .outline_color(focus_ring),
            ),
    )
    .tag::<a11y::AButton>()
    .label("Button 2 D", ii().font_size(14.0).text_color(text_high))
    .on_click(|| println!("Button 2 D: Clicked!"))
}
