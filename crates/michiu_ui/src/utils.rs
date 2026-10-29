use crate::{
    BoxShadow, Color, Convert, Entity, ExternalTexture, ExternalVisual, FlexDirection,
    InputContents, IntoHexColor, IntoLayoutPoint, ModValue, Modifier, Prop, ReadSignal,
    WriteSignal, with_context,
};
use std::borrow::Cow;

/// Creates a new style.
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// # fn example() -> Entity {
///     div(
///         ii().size_full()
///             .p(8.0)
///             .bg_color(rgb(209, 92, 174))
///     )
/// # }
/// ```
///
/// # Why `ii()` ?
///
/// - It is an abbreviation for `Mod “i” f “i” er`.
/// - Because `m()` is prone to namespace conflicts.
/// - Because it’s highly legible (the von Restorff effect).
/// - Because it’s faster to type.
/// - Because `michiu_ui` contains a lot of `i`s.
///
/// For those who don't like `ii()` -> [`m!`] , [`Modifier::new()`]
#[inline]
#[must_use]
pub fn ii() -> Modifier {
    Modifier::new()
}

/// Creates a new style.
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// # fn example() -> Entity {
///     div(
///         m!().size_full()
///             .p(8.0)
///             .bg_color(rgb(209, 92, 174))
///     )
/// # }
/// ```
#[macro_export]
macro_rules! m {
    () => {
        $crate::Modifier::new()
    };
}

/// Generates signals directly from the `Context`.
///
/// This function must be called within the scope of `build_ui`.
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// # fn example() -> Entity {
///     let (count, set_count) = create_signal(0u32);
///     div(
///         ii().size_full()
///             .p(8.0)
///             .bg_color(rgb(209, 92, 174))
///     )
///     .on_click(move || set_count.set(count.get() + 1))
///     .label(move || format!("Count: {}", count.get()), ii())
/// # }
/// ```
pub fn create_signal<T: Send + 'static>(initial_value: T) -> (ReadSignal<T>, WriteSignal<T>) {
    with_context(|cx| cx.create_signal(initial_value))
}

/// Retrieves context signals ([`ReadSignal<T>`]) provided by ancestor entities, including itself.
///
/// Travers the UI tree upward and searches for and returns signals of type `T` provided via `.provide()`.
///
/// Allows state to be shared among descendant components at deep levels without passing arguments through a bucket relay.
///
/// Since the return value is [`ReadSignal<T>`], calling `.get()` within a reactive closure automatically subscribes to the dependency,
/// causing the component to be redrawn when the value is updated by a parent.
///
/// # Panics
/// Panics if type `T` is not provided via `.provide()` anywhere in the ancestor tree.
///
/// # Related Features
/// - [`use_provided_setter`]: Retrieves a [`WriteSignal<T>`] to update the context’s value.
/// - [`dynamic`](crate::dynamic): Directly binds the context’s value to a single style property.
/// - [`modify_d`](crate::Entity::modify_d): Dynamically applies an entire style block based on the context’s value.
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// #[derive(Debug, Clone)]
/// struct Theme {
///     color: Color,
/// }
///
/// fn root() -> Entity {
///     let (theme, _) = create_signal(Theme { color: Color::BLUE });
///     div_n().provide(theme)
///            .child(
///                 div(|| {
///                     let theme = use_provided::<Theme>().get();
///                     ii().bg_color(theme.color)
///                 })
///                 .on_click(|| {
///                     let set_theme = use_provided_setter::<Theme>();
///                     set_theme.set(Theme { color: Color::RED });
///                 })
///             )
/// }
/// ```
#[inline]
#[must_use]
#[track_caller]
#[expect(clippy::panic)]
pub fn use_provided<T: Clone + 'static>() -> ReadSignal<T> {
    let signal = with_context(|cx| cx.try_use_provided::<T>());

    let Some(signal) = signal else {
        panic!(
            "\n=======================================================\n\
            {RED_BOLD}[Michiu UI Error: Context Not Found]{RESET}\n\
            Called at: {UNDERLINE}{caller}{RESET}\n\
            \n\
            Failed to find context of type:\n\
              {CYAN}{BOLD}`{type_name}`{RESET}\n\
            \n\
            {YELLOW}Possible causes & Solutions:{RESET}\n\
            1. The context was not provided by any ancestor entity:\n\
               - Make sure an ancestor calls\n\
                 {GREEN}`div_n().provide::<{type_name}>()`{RESET}.\n\
            2. Called outside of an entity builder or reactive context:\n\
               - `use_provided` must be called inside an Entity's closure, such as\n\
                 {GREEN}`div(|| {{ let _ = use_provided::<{type_name}>(); ... }})`{RESET},\n\
                 or inside an active event handler.\n\
               - If using inside a `Modifier` method, use\n\
                 {GREEN}`bg_color(dynamic(|t: &{type_name}| t.color))`{RESET}.\n\
            =======================================================",
            caller = std::panic::Location::caller(),
            type_name = std::any::type_name::<T>()
        );
    };

    signal
}

/// From the current thread-local `Context`, we obtain a [`WriteSignal`] for a signal of type `T`,
/// automatically resolving it by traversing the parent tree.
///
/// # Panics
/// Panics if type `T` is not provided via `.provide()` anywhere in the ancestor tree.
///
/// # Related Features
/// - [`use_provided_setter`]: Retrieves a [`WriteSignal<T>`] to update the context’s value.
/// - [`dynamic`](crate::dynamic): Directly binds the context’s value to a single style property.
/// - [`modify_d`](crate::Entity::modify_d): Dynamically applies an entire style block based on the context’s value.
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// #[derive(Debug, Clone)]
/// struct Theme {
///     color: Color,
/// }
///
/// fn root() -> Entity {
///     let (theme, _) = create_signal(Theme { color: Color::BLUE });
///     div_n().provide(theme)
///            .child(
///                 div(|| {
///                     let theme = use_provided::<Theme>().get();
///                     ii().bg_color(theme.color)
///                 })
///                 .on_click(|| {
///                     let set_theme = use_provided_setter::<Theme>();
///                     set_theme.set(Theme { color: Color::RED });
///                 })
///             )
/// }
/// ```
#[inline]
#[must_use]
#[track_caller]
#[expect(clippy::panic)]
pub fn use_provided_setter<T: Send + 'static>() -> WriteSignal<T> {
    let signal = with_context(|cx| cx.try_use_provided_setter::<T>());

    let Some(signal) = signal else {
        panic!(
            "\n=======================================================\n\
            {RED_BOLD}[Michiu UI Error: Context Not Found]{RESET}\n\
            Called at: {UNDERLINE}{caller}{RESET}\n\
            \n\
            Failed to find context of type:\n\
              {CYAN}{BOLD}`{type_name}`{RESET}\n\
            \n\
            {YELLOW}Possible causes & Solutions:{RESET}\n\
            1. The context was not provided by any ancestor entity:\n\
               - Make sure an ancestor calls\n\
                 {GREEN}`div_n().provide::<{type_name}>()`{RESET}.\n\
            2. Called outside of an entity builder or reactive context:\n\
               - `use_provided_setter` must be called inside an Entity's closure, such as\n\
                 {GREEN}`div(|| {{ let _ = use_provided_setter::<{type_name}>(); ... }})`{RESET},\n\
                 or inside an active event handler.\n\
            =======================================================",
            caller = std::panic::Location::caller(),
            type_name = std::any::type_name::<T>()
        );
    };

    signal
}

/// Generates dynamic style values that reactively change based on the value of a provider ([`use_provided`](crate::use_provided)).
///
/// Because automatic subscription to signals occurs internally,
/// only this style property is updated when the provider’s value is updated.
///
/// # Tip
/// By adding a type annotation such as `|t: &Theme|` to the arguments of a selector function,
/// the system automatically infers which context type to retrieve.
///
/// # Panics
/// Panics if type `T` is not provided via `.provide()` anywhere in the ancestor tree.
///
/// # Related Features
/// - [`use_provided_setter`]: Retrieves a [`WriteSignal<T>`] to update the context’s value.
/// - [`dynamic`](crate::dynamic): Directly binds the context’s value to a single style property.
/// - [`modify_d`](crate::Entity::modify_d): Dynamically applies an entire style block based on the context’s value.
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// #[derive(Debug, Clone)]
/// struct Theme {
///     color: Color,
/// }
///
/// fn root() -> Entity {
///     let (theme, _) = create_signal(Theme { color: Color::BLUE });
///     div_n().provide(theme)
///            .child(
///                 div(ii().bg_color(dynamic(|t: &Theme| t.color)))
///             )
/// }
/// ```
pub fn dynamic<P, V, F>(selector: F) -> ModValue<V>
where
    P: Clone + 'static,
    V: 'static,
    F: Fn(&P) -> V + Send + Sync + 'static,
{
    ModValue::Dynamic(Box::new(move || {
        // ここでプロバイダーの ReadSignal に対する `.get()` を実行することで、
        // 呼び出し元のスタイルエフェクトに自動的に依存関係が購読される
        let signal = with_context(|cx| cx.use_provided::<P>());
        let val = signal.get();
        selector(&val)
    }))
}

/// Generates an opaque RGB color using an integer value (`u8`) between 0 and 255.
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// # fn examples() -> Modifier {
///    ii().bg_color(rgb(209, 92, 174))
/// # }
/// ```
#[inline]
#[must_use]
pub fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: f32::from(r) / 255.0,
        g: f32::from(g) / 255.0,
        b: f32::from(b) / 255.0,
        a: 1.0,
    }
}

/// Generates an RGBA color by specifying RGB values as integers from 0 to 255 (`u8`)
/// and opacity (alpha) values from 0.0 to 1.0 (`f32`).
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// # fn examples() -> Modifier {
///    ii().bg_color(rgba(209, 92, 174, 0.5))
/// # }
/// ```
#[inline]
#[must_use]
pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color {
        r: f32::from(r) / 255.0,
        g: f32::from(g) / 255.0,
        b: f32::from(b) / 255.0,
        a,
    }
}

/// Generate a HEX.
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// # fn examples() -> Modifier {
///    ii().bg_color(hex("#ff0000"))
///        .border_color(hex(0x00_FF00))
///        .text_color(hex(0x00_00ff))
///        .outline_color(hex("00000080"))
///        .shadow_color(hex("0xFFFFFF80"))
/// # }
/// ```
#[inline]
pub fn hex(value: impl IntoHexColor) -> Color {
    value.into_hex_color()
}

/// Generates HSL (Hue: 0..360, Saturation: 0.0..100.0%, Lightness: 0.0..100.0%) colors.
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// # fn examples() -> Modifier {
///    ii().bg_color(hsl(318.0, 56.0, 59.0))
/// # }
/// ```
#[inline]
#[must_use]
pub fn hsl(h: f32, s: f32, l: f32) -> Color {
    Color::hsl(h, s, l)
}

/// Generate HSLA colors by applying an alpha value (0.0..1.0) to HSL
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// # fn examples() -> Modifier {
///    ii().bg_color(hsla(318.0, 56.0, 59.0, 0.5))
/// # }
/// ```
#[inline]
#[must_use]
pub fn hsla(h: f32, s: f32, l: f32, a: f32) -> Color {
    Color::hsla(h, s, l, a)
}

/// Generate from Oklab (L: 0.0..1.0, a: -0.4..0.4, b: -0.4..0.4)
#[inline]
#[must_use]
pub fn oklab(l: f32, a: f32, b: f32) -> Color {
    Color::oklab(l, a, b)
}

/// Generate it by adding an alpha value to Oklab
#[inline]
#[must_use]
pub fn oklaba(l: f32, a: f32, b: f32, alpha: f32) -> Color {
    Color::oklaba(l, a, b, alpha)
}

/// Generate from Oklch (L: 0.0..1.0, Chroma: 0.0..0.4+, Hue: 0..360°)
#[inline]
#[must_use]
pub fn oklch(l: f32, c: f32, h: f32) -> Color {
    Color::oklch(l, c, h)
}

/// Generate it by adding an alpha value to Oklch
#[inline]
#[must_use]
pub fn oklcha(l: f32, c: f32, h: f32, alpha: f32) -> Color {
    Color::oklcha(l, c, h, alpha)
}

/// A container that applies a style and generates content.
///
/// It is the same as `Entity::new().modify(style)`.
///
/// Default is Flex Box.
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// # fn examples() -> Entity {
///    flex(ii().justify_center())
/// # }
/// ```
#[inline]
pub fn flex(m: impl Into<Prop<Modifier>>) -> Entity {
    let e = Entity::new();
    e.modify(m)
}

/// A block container that applies a style and generates content.
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// # fn examples() -> Entity {
///    div(ii().size(100.0))
/// # }
/// ```
#[inline]
pub fn div(m: impl Into<Prop<Modifier>>) -> Entity {
    let e = Entity::new();
    e.modify(ii().block()).modify(m)
}

pub const NO_STYLE: Option<Modifier> = None;

/// An empty container with no style.
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// # fn examples() -> Entity {
///    div_n().modify(ii())
/// # }
/// ```
#[inline]
#[must_use]
pub fn div_n() -> Entity {
    flex(NO_STYLE)
}

/// Horizontal flex container.
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// # fn examples() -> Entity {
///    h_flex(ii().border_solid(1.0))
/// # }
/// ```
#[inline]
pub fn h_flex(m: impl Into<Prop<Modifier>>) -> Entity {
    let e = Entity::new();
    e.modify(ii().flex().flex_direction(FlexDirection::Row))
        .modify(m)
}

/// Vertical flex container.
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// # fn examples() -> Entity {
///    h_flex(ii().outline_solid(1.0))
/// # }
/// ```
#[inline]
pub fn v_flex(m: impl Into<Prop<Modifier>>) -> Entity {
    let e = Entity::new();
    e.modify(ii().flex().flex_direction(FlexDirection::Column))
        .modify(m)
}

/// Not Implemented
#[inline]
pub fn grid_box(m: impl Into<Prop<Modifier>>) -> Entity {
    let e = Entity::new();
    e.modify(ii().grid()).modify(m)
}

/// Hidden Container
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// # fn examples() -> Entity {
///    hidden_box(ii().size_full())
/// # }
/// ```
#[inline]
pub fn hidden_box(m: impl Into<Prop<Modifier>>) -> Entity {
    let e = Entity::new();
    e.modify(ii().hidden()).modify(m)
}

/// Text Container
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// # fn examples() -> Entity {
///    text("examples").modify(ii().font_size(20.0))
/// # }
/// ```
#[inline]
pub fn text(content: impl Into<Prop<Cow<'static, str>>>) -> Entity {
    div_n().text(content)
}

/// Input Container
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// # fn examples() -> Entity {
///    let (read_text, write_text) = create_signal(String::new());
///
///    input(
///         InputContents::new((read_text, write_text))
///             .numeric_only(true)
///    )
/// # }
/// ```
#[inline]
pub fn input(contents: impl Into<Prop<InputContents>>) -> Entity {
    div_n().input(contents)
}

/// Input Area (multiline) Container
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// # fn examples() -> Entity {
///    let (text, set_text) = create_signal(String::new());
///
///    input_area(
///         InputContents::new((text, set_text))
///             .is_ime(true)
///    )
/// # }
/// ```
#[inline]
pub fn input_area(contents: impl Into<Prop<InputContents>>) -> Entity {
    div_n().input_area(contents)
}

/// External Texture Container
#[inline]
pub fn external_texture(texture: impl ExternalTexture + 'static) -> Entity {
    div_n().external_texture(texture)
}

/// External Visual Container
#[inline]
pub fn external_visual(visual: impl ExternalVisual + 'static) -> Entity {
    div_n().external_visual(visual)
}

/// A generic container that dynamically resolves `Modifier` from provider `P` and applies the style
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// #[derive(Debug, Clone)]
/// struct Theme {
///     color: Color,
/// }
///
/// fn root() -> Entity {
///     let (theme, _) = create_signal(Theme { color: Color::BLUE });
///     div_n().provide(theme)
///            .child(
///                 flex_d(|t: &Theme| ii().bg_color(t.color))
///             )
/// }
/// ```
#[inline]
pub fn flex_d<P, F>(f: F) -> Entity
where
    P: Clone + 'static,
    F: Fn(&P) -> Modifier + Send + Sync + 'static,
{
    Entity::new().modify_d(f)
}

/// A block container that dynamically resolves `Modifier` from provider `P` and applies the style
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// #[derive(Debug, Clone)]
/// struct Theme {
///     color: Color,
/// }
///
/// fn root() -> Entity {
///     let (theme, _) = create_signal(Theme { color: Color::BLUE });
///     div_n().provide(theme)
///            .child(
///                 div_d(|t: &Theme| ii().bg_color(t.color))
///             )
/// }
/// ```
#[inline]
pub fn div_d<P, F>(f: F) -> Entity
where
    P: Clone + 'static,
    F: Fn(&P) -> Modifier + Send + Sync + 'static,
{
    Entity::new().modify(ii().block()).modify_d(f)
}

/// A horizontal flex container that dynamically resolves `Modifier` from provider `P` and applies the style
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// #[derive(Debug, Clone)]
/// struct Theme {
///     color: Color,
/// }
///
/// fn root() -> Entity {
///     let (theme, _) = create_signal(Theme { color: Color::BLUE });
///     div_n().provide(theme)
///            .child(
///                 h_flex_d(|t: &Theme| ii().bg_color(t.color))
///             )
/// }
/// ```
#[inline]
pub fn h_flex_d<P, F>(f: F) -> Entity
where
    P: Clone + 'static,
    F: Fn(&P) -> Modifier + Send + Sync + 'static,
{
    // レイアウトの基本形式（フレックス）は静的に適用し、
    // プロバイダー依存の残りのスタイルは style_c で一括解決します
    Entity::new()
        .modify(ii().flex().flex_direction(FlexDirection::Row))
        .modify_d(f)
}

/// A vertical flex container that dynamically resolves `Modifier` from provider `P` and applies the style
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// #[derive(Debug, Clone)]
/// struct Theme {
///     color: Color,
/// }
///
/// fn root() -> Entity {
///     let (theme, _) = create_signal(Theme { color: Color::BLUE });
///     div_n().provide(theme)
///            .child(
///                 v_flex_d(|t: &Theme| ii().bg_color(t.color))
///             )
/// }
/// ```
#[inline]
pub fn v_flex_d<P, F>(f: F) -> Entity
where
    P: Clone + 'static,
    F: Fn(&P) -> Modifier + Send + Sync + 'static,
{
    Entity::new()
        .modify(ii().flex().flex_direction(FlexDirection::Column))
        .modify_d(f)
}

/// A container that dynamically resolves and applies text from provider `P`.
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// pub enum MichiuOrder {
///    Small,
///    Big,
/// }
///
/// fn root() -> Entity {
///     let (order, _) = create_signal(MichiuOrder::Small);
///     div_n().provide(order)
///            .child(
///                 text_d(|order: &MichiuOrder| match order {
///                     MichiuOrder::Small => "small...",
///                     MichiuOrder::Big => "BIG!!!",
///                 })
///             )
/// }
/// ```
#[inline]
pub fn text_d<P, F, S>(f: F) -> Entity
where
    P: Clone + 'static,
    F: Fn(&P) -> S + Send + Sync + 'static,
    S: Into<Cow<'static, str>>,
{
    div_n().text_d(f)
}

/// A container that dynamically resolves, generates, and applies input field entities from provider `P`.
///
/// # Examples
/// ```no_run
/// # use michiu_ui::prelude::*;
/// #[derive(Debug, Clone)]
/// struct Theme {
///     color: Color,
/// }
///
/// fn root() -> Entity {
///     let (theme, _) = create_signal(Theme { color: Color::BLUE });
///     let (read_text, write_text) = create_signal(String::new());
///
///     div_n().provide(theme)
///            .child(
///                 input_d(move |t: &Theme| {
///                     InputContents::new((read_text, write_text))
///                         .placeholder_color(t.color)
/// })
///             )
/// }
/// ```
#[inline]
pub fn input_d<P, F>(f: F) -> Entity
where
    P: Clone + 'static,
    F: Fn(&P) -> InputContents + Send + Sync + 'static,
{
    div_n().input_d(f)
}

/// A container that dynamically resolves, generates, and applies input area entities from provider `P`.
#[inline]
pub fn input_area_d<P, F>(f: F) -> Entity
where
    P: Clone + 'static,
    F: Fn(&P) -> InputContents + Send + Sync + 'static,
{
    div_n().input_area_d(f)
}

/// Creates a `BoxShadow`.
#[inline]
#[must_use]
pub fn shadow() -> BoxShadow {
    BoxShadow::new()
}

/// Creates a `BoxShadow` starting with the blur width.
#[inline]
pub fn blur(value: impl Convert<f32>) -> BoxShadow {
    BoxShadow::new().blur(value)
}

/// Creates a `BoxShadow` starting with the offset.
#[inline]
pub fn offset(value: impl IntoLayoutPoint) -> BoxShadow {
    BoxShadow::new().offset(value)
}

/// Creates a `BoxShadow` starting with the spread.
#[inline]
pub fn spread(value: impl Convert<f32>) -> BoxShadow {
    BoxShadow::new().spread(value)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pixel(pub f32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Percent(pub f32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Auto;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stretch;

/// Generates actual values.
///
/// # Examples
/// ```no_run,ignore
/// # use michiu_ui::prelude::*;
/// # fn examples() {
/// // All Directions (f32, i32, px, pct, auto, fill)
/// ii().p(10.0);
/// ii().p(10);
/// ii().p(pct(5.0));
/// ii().p(auto());
/// ii().p(fill());
///
/// // (vertical, horizontal)
/// ii().p((10.0, 20.0));
/// ii().p((px(10.0), pct(50.0)));
///
/// // (top, right, bottom, left)
/// ii().p((10.0, 20.0, 30.0, 40.0));
/// ii().p((px(10.0), pct(20.0), auto(), fill()));
/// # }
/// ```
#[inline]
#[must_use]
pub fn px(val: f32) -> Pixel {
    Pixel(val)
}

/// Generates a percentage value.
///
/// # Examples
/// ```no_run,ignore
/// # use michiu_ui::prelude::*;
/// # fn examples() {
/// // All Directions (f32, i32, px, pct, auto, fill)
/// ii().p(10.0);
/// ii().p(10);
/// ii().p(pct(5.0));
/// ii().p(auto());
/// ii().p(fill());
///
/// // (vertical, horizontal)
/// ii().p((10.0, 20.0));
/// ii().p((px(10.0), pct(50.0)));
///
/// // (top, right, bottom, left)
/// ii().p((10.0, 20.0, 30.0, 40.0));
/// ii().p((px(10.0), pct(20.0), auto(), fill()));
/// # }
/// ```
#[inline]
#[must_use]
pub fn pct(val: f32) -> Percent {
    Percent(val)
}

/// Generate `Auto`
///
/// # Examples
/// ```no_run,ignore
/// # use michiu_ui::prelude::*;
/// # fn examples() {
/// // All Directions (f32, i32, px, pct, auto, fill)
/// ii().p(10.0);
/// ii().p(10);
/// ii().p(pct(5.0));
/// ii().p(auto());
/// ii().p(fill());
///
/// // (vertical, horizontal)
/// ii().p((10.0, 20.0));
/// ii().p((px(10.0), pct(50.0)));
///
/// // (top, right, bottom, left)
/// ii().p((10.0, 20.0, 30.0, 40.0));
/// ii().p((px(10.0), pct(20.0), auto(), fill()));
/// # }
/// ```
#[inline]
#[must_use]
pub fn auto() -> Auto {
    Auto
}

/// Generate `Stretch`
///
/// # Examples
/// ```no_run,ignore
/// # use michiu_ui::prelude::*;
/// # fn examples() {
/// // All Directions (f32, i32, px, pct, auto, fill)
/// ii().p(10.0);
/// ii().p(10);
/// ii().p(pct(5.0));
/// ii().p(auto());
/// ii().p(fill());
///
/// // (vertical, horizontal)
/// ii().p((10.0, 20.0));
/// ii().p((px(10.0), pct(50.0)));
///
/// // (top, right, bottom, left)
/// ii().p((10.0, 20.0, 30.0, 40.0));
/// ii().p((px(10.0), pct(20.0), auto(), fill()));
/// # }
/// ```
#[inline]
#[must_use]
pub fn fill() -> Stretch {
    Stretch
}

/// Copies text to the `Win32API` clipboard.
#[must_use]
pub fn set_win32_clipboard(text: &str) -> bool {
    unsafe {
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::DataExchange::{
            CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
        };
        use windows::Win32::System::Memory::{
            GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock,
        };

        let text_u16: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let size = text_u16.len() * 2;
        let Ok(h_mem) = GlobalAlloc(GMEM_MOVEABLE, size) else {
            return false;
        };
        let ptr = GlobalLock(h_mem);
        if ptr.is_null() {
            return false;
        }
        std::ptr::copy_nonoverlapping(text_u16.as_ptr(), ptr.cast::<u16>(), text_u16.len());
        let _ = GlobalUnlock(h_mem);

        let mut success = false;
        if OpenClipboard(None).is_ok() {
            let _ = EmptyClipboard();
            if SetClipboardData(13, Some(HANDLE(h_mem.0))).is_ok() {
                // 13 = CF_UNICODETEXT
                success = true;
            }
            let _ = CloseClipboard();
        }
        success
    }
}

/// Retrieves text from the `Win32API` clipboard.
#[must_use]
pub fn get_win32_clipboard() -> Option<String> {
    unsafe {
        use windows::Win32::Foundation::HGLOBAL;
        use windows::Win32::System::DataExchange::{
            CloseClipboard, GetClipboardData, OpenClipboard,
        };
        use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};

        let mut result = None;
        if OpenClipboard(None).is_ok() {
            if let Ok(h_mem) = GetClipboardData(13) {
                // 13 = CF_UNICODETEXT
                if !h_mem.is_invalid() {
                    let ptr = GlobalLock(HGLOBAL(h_mem.0));
                    if !ptr.is_null() {
                        let u16_ptr = ptr as *const u16;
                        let mut len = 0;
                        while *u16_ptr.add(len) != 0 {
                            len += 1;
                        }
                        let slice = std::slice::from_raw_parts(u16_ptr, len);
                        result = Some(String::from_utf16_lossy(slice));
                        let _ = GlobalUnlock(HGLOBAL(h_mem.0));
                    }
                }
            }
            let _ = CloseClipboard();
        }
        result
    }
}

/// Converts the raw mouse wheel delta value on Windows (a multiple of 120)
/// into a logical pixel displacement that conforms to the OS's scroll line setting.
///
/// `raw_delta`: Raw value obtained from `WM_MOUSEWHEEL`, etc. (forward: positive, backward: negative)
///
/// The return value is the logical pixel distance to scroll (scrolling forward = scrolling down = positive value).
#[expect(clippy::cast_precision_loss)]
#[must_use]
pub fn raw_wheel_delta_to_logical_pixels(raw_delta: f32) -> f32 {
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_GETWHEELSCROLLLINES, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    };

    const WHEEL_PAGESCROLL: u32 = 0xFFFF_FFFF;

    let mut scroll_lines: u32 = 3; // OSデフォルト3行をフォールバック値にする
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETWHEELSCROLLLINES,
            0,
            Some((&raw mut scroll_lines).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }

    if scroll_lines == WHEEL_PAGESCROLL {
        // 1画面（ページスクロール）設定時
        // 1ノッチ(120)あたり、一般的な基準サイズ 350px 相当にスクロール
        let notches = raw_delta / 120.0;
        -notches * 350.0
    } else {
        // 通常の行スクロール設定時
        // 1行あたり 30.0px（論理ピクセル）として移動量を算出
        let notches = raw_delta / 120.0;
        -notches * (scroll_lines as f32) * 30.0
    }
}

pub(crate) const RED_BOLD: &str = "\x1b[1;31m";
pub(crate) const CYAN: &str = "\x1b[36m";
pub(crate) const YELLOW: &str = "\x1b[33m";
pub(crate) const UNDERLINE: &str = "\x1b[4m";
pub(crate) const BOLD: &str = "\x1b[1m";
pub(crate) const RESET: &str = "\x1b[0m";
pub(crate) const GREEN: &str = "\x1b[32m";
