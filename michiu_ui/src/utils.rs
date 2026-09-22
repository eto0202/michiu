use crate::{
    BoxShadow, Color, Convert, Element, ExternalTexture, ExternalVisual, FlexDirection,
    InputContents, IntoHexColor, IntoLayoutPoint, Prop, ReadSignal, StyleValue, ThisStyle,
    WebView2Contents, WriteSignal, with_context,
};
use std::borrow::Cow;

/// Creates a new style.
#[inline]
#[must_use]
pub fn ts() -> ThisStyle {
    ThisStyle::new()
}

/// Generates signals directly from the `Context`.
///
/// This function must be called within the scope of `build_ui`.
pub fn create_signal<T: Send + 'static>(initial_value: T) -> (ReadSignal<T>, WriteSignal<T>) {
    with_context(|cx| cx.create_signal(initial_value))
}

/// Identify the target element from the current thread-local `Context`
/// and resolve the [`ReadSignal`] of type `T` by traversing the parent tree.
#[inline]
#[must_use]
pub fn use_provided<T: Clone + 'static>() -> ReadSignal<T> {
    with_context(|cx| cx.use_provided::<T>())
}

/// From the current thread-local `Context`,
/// we obtain a [`WriteSignal`] for a signal of type `T`, automatically resolving it by traversing the parent tree.
#[inline]
#[must_use]
pub fn use_provided_setter<T: Send + 'static>() -> WriteSignal<T> {
    with_context(|cx| cx.use_provided_setter::<T>())
}

/// Generate a dynamic style value by resolving the value `V` from the provider type `P` through the closure `F`
pub fn dynamic<P, V, F>(selector: F) -> StyleValue<V>
where
    P: Clone + 'static,
    V: 'static,
    F: Fn(&P) -> V + Send + Sync + 'static,
{
    StyleValue::Dynamic(Box::new(move || {
        // ここでプロバイダーの ReadSignal に対する `.get()` を実行することで、
        // 呼び出し元のスタイルエフェクトに自動的に依存関係が購読される
        let signal = with_context(|cx| cx.use_provided::<P>());
        let val = signal.get();
        selector(&val)
    }))
}

/// Generates an opaque RGB color using an integer value (`u8`) between 0 and 255.
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
/// ```rust
/// use crate::hex;
///
/// hex("#ff0000");
/// hex(0x00_FF00);
/// hex(0x00_00ff);
/// hex("00000080");
/// hex("0xFFFFFF80");
///
/// ```
#[inline]
pub fn hex(value: impl IntoHexColor) -> Color {
    value.into_hex_color()
}

/// Generates HSL (Hue: 0..360, Saturation: 0.0..100.0%, Lightness: 0.0..100.0%) colors.
#[inline]
#[must_use]
pub fn hsl(h: f32, s: f32, l: f32) -> Color {
    Color::hsl(h, s, l)
}

/// Generate HSLA colors by applying an alpha value (0.0..1.0) to HSL
#[inline]
#[must_use]
pub fn hsla(h: f32, s: f32, l: f32, a: f32) -> Color {
    Color::hsla(h, s, l, a)
}

/// A container that applies a style and generates content.
///
/// It is the same as `Element::new().style(style)`.
#[inline]
pub fn div(style: impl Into<Prop<ThisStyle>>) -> Element {
    let el = Element::new();
    el.style(style)
}

pub const NO_STYLE: Option<ThisStyle> = None;

/// An empty container with no style.
#[inline]
#[must_use]
pub fn div_n() -> Element {
    div(NO_STYLE)
}

/// Horizontal flex container.
#[inline]
pub fn h_flex(style: impl Into<Prop<ThisStyle>>) -> Element {
    let el = Element::new();
    el.style(ts().flex().flex_direction(FlexDirection::Row))
        .style(style)
}

/// Vertical flex container.
#[inline]
pub fn v_flex(style: impl Into<Prop<ThisStyle>>) -> Element {
    let el = Element::new();
    el.style(ts().flex().flex_direction(FlexDirection::Column))
        .style(style)
}

/// Not Implemented
#[inline]
pub fn grid_box(style: impl Into<Prop<ThisStyle>>) -> Element {
    let el = Element::new();
    el.style(ts().grid()).style(style)
}

/// A container used for block placement.
#[inline]
pub fn block_box(style: impl Into<Prop<ThisStyle>>) -> Element {
    let el = Element::new();
    el.style(ts().block()).style(style)
}

/// Hidden Container
#[inline]
pub fn hidden_box(style: impl Into<Prop<ThisStyle>>) -> Element {
    let el = Element::new();
    el.style(ts().hidden()).style(style)
}

/// Text Container
#[inline]
pub fn text(content: impl Into<Prop<Cow<'static, str>>>) -> Element {
    div_n().text(content)
}

/// Input Container
#[inline]
pub fn input(contents: impl Into<Prop<InputContents>>) -> Element {
    div_n().input(contents)
}

/// Input Area (multiline) Container
#[inline]
pub fn input_area(contents: impl Into<Prop<InputContents>>) -> Element {
    div_n().input_area(contents)
}

/// External Texture Container
#[inline]
pub fn external_texture(texture: impl ExternalTexture + 'static) -> Element {
    div_n().external_texture(texture)
}

/// External Visual Container
#[inline]
pub fn external_visual(visual: impl ExternalVisual + 'static) -> Element {
    div_n().external_visual(visual)
}

/*
* /// Webview2 Container
#[inline]
pub fn webview2(contents: impl Into<Prop<WebView2Contents>>) -> Element {
    div_n().webview2(contents)
}
/// A container that creates and applies WebView2 elements dynamically resolved from provider `P`.
#[inline]
pub fn webview2_d<P, F>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> WebView2Contents + Send + Sync + 'static,
{
    div_n().webview2_d(f)
}
*/

/// A generic container that dynamically resolves `ThisStyle` from provider `P` and applies the style
#[inline]
pub fn div_d<P, F>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> ThisStyle + Send + Sync + 'static,
{
    Element::new().style_d(f)
}

/// A horizontal flex container that dynamically resolves `ThisStyle` from provider `P` and applies the style
#[inline]
pub fn h_flex_d<P, F>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> ThisStyle + Send + Sync + 'static,
{
    // レイアウトの基本形式（フレックス）は静的に適用し、
    // プロバイダー依存の残りのスタイルは style_c で一括解決します
    Element::new()
        .style(ts().flex().flex_direction(FlexDirection::Row))
        .style_d(f)
}

/// A vertical flex container that dynamically resolves `ThisStyle` from provider `P` and applies the style
#[inline]
pub fn v_flex_d<P, F>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> ThisStyle + Send + Sync + 'static,
{
    Element::new()
        .style(ts().flex().flex_direction(FlexDirection::Column))
        .style_d(f)
}

/// A container that dynamically resolves and applies text from provider `P`.
#[inline]
pub fn text_d<P, F, S>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> S + Send + Sync + 'static,
    S: Into<Cow<'static, str>>,
{
    div_n().text_d(f)
}

/// A container that dynamically resolves, generates, and applies input field elements from provider `P`.
#[inline]
pub fn input_d<P, F>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> InputContents + Send + Sync + 'static,
{
    div_n().input_d(f)
}

/// A container that dynamically resolves, generates, and applies input area elements from provider `P`.
#[inline]
pub fn input_area_d<P, F>(f: F) -> Element
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

/// Generates actual values.
#[inline]
#[must_use]
pub fn px(val: f32) -> Pixel {
    Pixel(val)
}

/// Generates a percentage value.
#[inline]
#[must_use]
pub fn pct(val: f32) -> Percent {
    Percent(val)
}

/// Generate `Auto`
#[inline]
#[must_use]
pub fn auto() -> Auto {
    Auto
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
