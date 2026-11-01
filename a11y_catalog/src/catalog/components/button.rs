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
///   - Contrast ratio: Text approx. 9.5:1 (meets WCAG AAA), focus border approx. 3.5:1 (meets WCAG 1.4.11)
/// - **Customization**:
///   - Please modify the text, size, and `on_click` closure to suit your app.
pub fn button_1_light() -> Entity {
    let bg_normal = oklch(0.965, 0.004, 260.0);
    let bg_hover = oklch(0.925, 0.006, 260.0);
    let bg_press = oklch(0.875, 0.008, 260.0);
    let text_col = oklch(0.220, 0.015, 260.0);
    let focus_col = oklch(0.600, 0.180, 250.0);

    flex(
        ii().justify_center()
            .items_center()
            .size((96.0, 36.0))
            .r(3.0)
            .bg_color(bg_normal)
            .cursor_pointer()
            .hovered(ii().bg_color(bg_hover))
            .pressed(ii().bg_color(bg_press))
            .focusable_self_both()
            .focused_visible(
                ii().outline_solid(2.0)
                    .outline_offset(2.0)
                    .outline_color(focus_col),
            ),
    )
    .tag::<a11y::AButton>()
    .label(
        "Button 1 L",
        ii().font_size(16.0)
            .font_family("Segoe UI")
            .text_color(text_col),
    )
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
///   - Contrast ratio: Text approx. 9.5:1 (meets WCAG AAA), focus border 3:1 or higher (meets WCAG 1.4.11)
/// - **Customization**:
///   - Please customize the text, size, and `on_click` closure to suit your app.
pub fn button_1_dark() -> Entity {
    let bg_normal = oklch(0.280, 0.012, 260.0);
    let bg_hover = oklch(0.340, 0.015, 260.0);
    let bg_press = oklch(0.240, 0.010, 260.0);
    let text_col = oklch(0.940, 0.005, 260.0);
    let focus_col = oklch(0.700, 0.160, 250.0);

    flex(
        ii().justify_center()
            .items_center()
            .size((96.0, 36.0))
            .r(3.0)
            .bg_color(bg_normal)
            .cursor_pointer()
            .hovered(ii().bg_color(bg_hover))
            .pressed(ii().bg_color(bg_press))
            .focusable_self_both()
            .focused_visible(
                ii().outline_solid(2.0)
                    .outline_offset(2.0)
                    .outline_color(focus_col),
            ),
    )
    .tag::<a11y::AButton>()
    .label(
        "Button 1 D",
        ii().font_size(16.0)
            .font_family("Segoe UI")
            .text_color(text_col),
    )
    .on_click(|| println!("Button 1 D: Clicked!"))
}
