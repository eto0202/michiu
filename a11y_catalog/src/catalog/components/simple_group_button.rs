use michiu::{accessibility::*, prelude::*};

/// Accessible grouped buttons / segmented controls (Light Theme).
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
///   - `Tab` / `Shift + Tab`: Move focus between buttons within the group
///   - `Space` / `Enter`: Activate the focused button
/// - **Screen Reader**:
///   - Role: `Role::Button` for each child item
///   - Separator lines are explicitly marked with `NO_A11Y_NODE` to avoid cluttering announcements
/// - **Visual / WCAG**:
///   - Target size: 28x28px (exceeds WCAG 2.5.8 minimum target size of 24x24px)
///   - `focused_visible`: 2px focus ring displayed on keyboard navigation (WCAG 2.4.7, 2.4.11)
///   - Contrast ratio:
///     - Text: approx. 12.3:1 against `bg_ui` (meets WCAG AAA >= 7.0:1)
///     - Focus ring: approx. 3.05:1 against `bg_ui` (meets WCAG 1.4.11 & 2.4.11 >= 3.0:1)
/// - **Customization**:
///   - Modify labels, add/remove items in the `children` array, or toggle `DIRECTION` to `FlexDirection::Column`.
pub fn group_button_1_light() -> Entity {
    use ui::FlexDirection;

    const DIRECTION: FlexDirection = FlexDirection::Row;

    let bg_ui = oklch(0.9097, 0.0, 0.0);
    let bg_ui_hover = oklch(0.8822, 0.0, 0.0);
    let bg_ui_active = oklch(0.8576, 0.0, 0.0);
    let border = oklch(0.7952, 0.0, 0.0);
    let focus_ring = oklch(0.58, 0.1155, 251.64);
    let text_high = oklch(0.2435, 0.0, 0.0);

    let btn_modifier = ii()
        .justify_center()
        .items_center()
        .size(28.0)
        .r(3.0)
        .cursor_pointer()
        .bg_color(Color::TRANSPARENT)
        .hovered(ii().bg_color(bg_ui_hover))
        .pressed(ii().bg_color(bg_ui_active))
        .focusable_self_both()
        .focused_visible(
            ii().border_color(Color::TRANSPARENT)
                .outline_solid(2.0)
                .outline_offset(0.0)
                .outline_color(focus_ring),
        );
    let btn = |text: &'static str| {
        flex(&btn_modifier)
            .tag::<a11y::AButton>()
            .label(text, ii().font_size(14.0).text_color(text_high))
            .on_click(move || println!("Group Button 1 L: Clicked {text}!"))
    };

    let separator_size = match DIRECTION {
        FlexDirection::Row | FlexDirection::RowReverse => ii().size((1.0, pct(80.0))).m_x(1.0),
        _ => ii().size((pct(80.0), 1.0)).m_y(1.0),
    };
    let separator = || div(separator_size.clone().bg_color(border)).a11y(NO_A11Y_NODE);

    let children = [
        btn("1"),
        separator(),
        btn("2"),
        btn("3"),
        btn("4"),
        separator(),
        btn("5"),
    ];

    flex(
        ii().flex_direction(DIRECTION)
            .justify_center()
            .items_center()
            .p(2.0)
            .gap(2.0)
            .r(3.0)
            .bg_color(bg_ui)
            .border_solid(1.0)
            .border_color(border),
    )
    .children(children)
}

/// Accessible grouped buttons / segmented controls (Dark Theme).
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
/// - **Direction**: Switch between horizontal and vertical layouts simply by changing `DIRECTION`.
/// - **Keyboard**:
///   - `Tab` / `Shift + Tab`: Move focus between buttons within the group
///   - `Space` / `Enter`: Activate the focused button
/// - **Screen Reader**:
///   - Role: `Role::Button` for each child item
///   - Separator lines are explicitly marked with `NO_A11Y_NODE` to avoid cluttering announcements
/// - **Visual / WCAG**:
///   - Target size: 28x28px (exceeds WCAG 2.5.8 minimum target size of 24x24px)
///   - `focused_visible`: 2px focus ring displayed on keyboard navigation (WCAG 2.4.7, 2.4.11)
///   - Contrast ratio:
///     - Text: approx. 12.2:1 against `bg_ui` (meets WCAG AAA >= 7.0:1)
///     - Focus ring: approx. 3.84:1 against `bg_ui` (meets WCAG 1.4.11 & 2.4.11 >= 3.0:1)
///     - Border: approx. 3.25:1 against dark `bg_app` (meets WCAG 1.4.11 >= 3.0:1)
/// - **Customization**:
///   - Modify labels, add/remove items in the `children` array, or toggle `DIRECTION` to `FlexDirection::Column`.
pub fn group_button_1_dark() -> Entity {
    use ui::FlexDirection;

    const DIRECTION: FlexDirection = FlexDirection::Row;

    let bg_ui = oklch(0.2559, 0.004, 264.5);
    let bg_ui_hover = oklch(0.2856, 0.0074, 274.7);
    let bg_ui_active = oklch(0.3141, 0.009, 276.85);
    let border = oklch(0.4024, 0.012, 273.12);
    let focus_ring = oklch(0.5032, 0.1377, 266.68);
    let text_high = oklch(0.9497, 0.0027, 286.35);

    let btn_modifier = ii()
        .justify_center()
        .items_center()
        .size(28.0)
        .r(3.0)
        .cursor_pointer()
        .bg_color(Color::TRANSPARENT)
        .hovered(ii().bg_color(bg_ui_hover))
        .pressed(ii().bg_color(bg_ui_active))
        .focusable_self_both()
        .focused_visible(
            ii().border_color(Color::TRANSPARENT)
                .outline_solid(2.0)
                .outline_offset(0.0)
                .outline_color(focus_ring),
        );

    let btn = |text: &'static str| {
        flex(&btn_modifier)
            .tag::<a11y::AButton>()
            .label(text, ii().font_size(14.0).text_color(text_high))
            .on_click(move || println!("Group Button 1 D: Clicked {text}!"))
    };

    let separator_size = match DIRECTION {
        FlexDirection::Row | FlexDirection::RowReverse => ii().size((1.0, pct(80.0))).m_x(1.0),
        _ => ii().size((pct(80.0), 1.0)).m_y(1.0),
    };
    let separator = || div(separator_size.clone().bg_color(border)).a11y(NO_A11Y_NODE);

    let children = [
        btn("1"),
        separator(),
        btn("2"),
        btn("3"),
        btn("4"),
        separator(),
        btn("5"),
    ];

    flex(
        ii().flex_direction(DIRECTION)
            .justify_center()
            .items_center()
            .p(2.0)
            .gap(2.0)
            .r(3.0)
            .bg_color(bg_ui)
            .border_solid(1.0)
            .border_color(border),
    )
    .children(children)
}
