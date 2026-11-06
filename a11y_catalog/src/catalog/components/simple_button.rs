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
    .label(
        "Button 1 L",
        ii().font_size(16.0)
            .font_family("Segoe UI")
            .text_color(text_high),
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
    .label(
        "Button 1 D",
        ii().font_size(16.0)
            .font_family("Segoe UI")
            .text_color(text_high),
    )
    .on_click(|| println!("Button 1 D: Clicked!"))
}
