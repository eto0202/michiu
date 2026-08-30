use crate::{
    BoxShadow, Color, Convert, Element, ExternalTexture, FlexDirection, ImageSource, InputContents,
    IntoHexColor, IntoLayoutPoint, MovieProperty, Prop, ReadSignal, StyleValue, ThisStyle,
    WebView2Contents, WriteSignal, with_context,
};
use std::borrow::Cow;

#[inline]
#[must_use]
pub fn ts() -> ThisStyle {
    ThisStyle::new()
}

/// 新しいシグナルを構築します。必ず `build_ui` のスコープ内で呼び出す必要があります。
pub fn create_signal<T: Send + 'static>(initial_value: T) -> (ReadSignal<T>, WriteSignal<T>) {
    with_context(|cx| cx.create_signal(initial_value))
}

/// 現在有効な動的リアクティブコンテキスト（またはアクティブなイベントハンドラ）から、
/// 親ツリー（トポロジー）を遡って自動解決された型 T の Context（ReadSignal）を取得します。
#[inline]
#[must_use]
pub fn use_provided<T: Clone + 'static>() -> ReadSignal<T> {
    with_context(|cx| cx.use_provided::<T>())
}

/// 現在有効な動的リアクティブコンテキスト（またはアクティブなイベントハンドラ）から、
/// 親ツリーを自動的に遡って解決した型 T のシグナルに対する同期書き込み用端（WriteSignal）を取得します。
#[inline]
#[must_use]
pub fn use_provided_setter<T: Send + 'static>() -> WriteSignal<T> {
    with_context(|cx| cx.use_provided_setter::<T>())
}

/// プロバイダーの型 `P` から、クロージャ `F` を通して値 `V` を解決する動的なスタイル値を生成
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

/// 0~255 の整数値（u8）で、不透明な RGB カラーを生成します
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

/// 0~255 の整数値（u8）でRGBを、0.0~1.0（f32）で不透明度（Alpha）を指定して RGBA カラーを生成します
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

#[inline]
pub fn hex(value: impl IntoHexColor) -> Color {
    value.into_hex_color()
}

/// HSL（Hue: 0..360, Saturation: 0.0..100.0%, Lightness: 0.0..100.0%）カラーを生成するショートハンド
#[inline]
#[must_use]
pub fn hsl(h: f32, s: f32, l: f32) -> Color {
    Color::hsl(h, s, l)
}

/// HSL にアルファ（0.0..1.0）を付与して HSLA カラーを生成するショートハンド
#[inline]
#[must_use]
pub fn hsla(h: f32, s: f32, l: f32, a: f32) -> Color {
    Color::hsla(h, s, l, a)
}

/// スタイルを適用して生成するコンテナ。
/// 静的な ThisStyle、ReadSignal<ThisStyle>、クロージャ、または None (Option) を受け入れます。
#[inline]
pub fn div(style: impl Into<Prop<ThisStyle>>) -> Element {
    let el = Element::new();
    el.style(style)
}

pub const NO_STYLE: Option<ThisStyle> = None;

/// 現時点ではスタイルを適用しないことを明示したコンテナ。
#[inline]
#[must_use]
pub fn div_n() -> Element {
    div(NO_STYLE)
}

/// 横方向のフレックスコンテナを生成します。
#[inline]
pub fn h_flex(style: impl Into<Prop<ThisStyle>>) -> Element {
    let el = Element::new();
    el.style(ts().flex().flex_direction(FlexDirection::Row))
        .style(style)
}

/// 縦方向のフレックスコンテナ を生成します。
#[inline]
pub fn v_flex(style: impl Into<Prop<ThisStyle>>) -> Element {
    let el = Element::new();
    el.style(ts().flex().flex_direction(FlexDirection::Column))
        .style(style)
}

/// グリッド配置を行うコンテナを生成します。
#[inline]
pub fn grid_box(style: impl Into<Prop<ThisStyle>>) -> Element {
    let el = Element::new();
    el.style(ts().grid()).style(style)
}

/// ブロック流し込み配置を行うコンテナを生成します。
#[inline]
pub fn block_box(style: impl Into<Prop<ThisStyle>>) -> Element {
    let el = Element::new();
    el.style(ts().block()).style(style)
}

#[inline]
pub fn hidden_box(style: impl Into<Prop<ThisStyle>>) -> Element {
    let el = Element::new();
    el.style(ts().hidden()).style(style)
}

#[inline]
pub fn text(content: impl Into<Prop<Cow<'static, str>>>) -> Element {
    div_n().text(content)
}

#[inline]
pub fn input(contents: impl Into<Prop<InputContents>>) -> Element {
    div_n().input(contents)
}

#[inline]
pub fn input_area(contents: impl Into<Prop<InputContents>>) -> Element {
    div_n().input_area(contents)
}

#[inline]
pub fn external_texture(texture: impl ExternalTexture + 'static) -> Element {
    div_n().external_texture(texture)
}

#[inline]
pub fn webview2(contents: impl Into<Prop<WebView2Contents>>) -> Element {
    div_n().webview2(contents)
}

/// プロバイダー `P` から動的に `ThisStyle` を解決してスタイルを適用する汎用コンテナ
#[inline]
pub fn div_d<P, F>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> ThisStyle + Send + Sync + 'static,
{
    Element::new().style_d(f)
}

/// プロバイダー `P` から動的に `ThisStyle` を解決してスタイルを適用する横フレックスコンテナ
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

/// プロバイダー `P` から動的に `ThisStyle` を解決してスタイルを適用する縦フレックスコンテナ
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

/// プロバイダー `P` から動的にテキストを解決してテキスト要素を生成します。
#[inline]
pub fn text_d<P, F, S>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> S + Send + Sync + 'static,
    S: Into<Cow<'static, str>>,
{
    div_n().text_d(f)
}

/// プロバイダー `P` から動的に設定を解決して入力フィールド要素を生成します。
#[inline]
pub fn input_d<P, F>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> InputContents + Send + Sync + 'static,
{
    div_n().input_d(f)
}

/// プロバイダー `P` から動的に設定を解決して複数行入力フィールド（テキストエリア）要素を生成します。
#[inline]
pub fn input_area_d<P, F>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> InputContents + Send + Sync + 'static,
{
    div_n().input_area_d(f)
}

/// プロバイダー `P` `から動的に解決されたWebView2要素を生成します`。
#[inline]
pub fn webview2_d<P, F>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> WebView2Contents + Send + Sync + 'static,
{
    div_n().webview2_d(f)
}

#[inline]
#[must_use]
pub fn shadow() -> BoxShadow {
    BoxShadow::new()
}

/// ぼかし幅（blur）から始まる影設定を生成します。
#[inline]
pub fn blur(value: impl Convert<f32>) -> BoxShadow {
    BoxShadow::new().blur(value)
}

/// 影のオフセット（x, y）から始まる影設定を生成します。
#[inline]
pub fn offset(value: impl IntoLayoutPoint) -> BoxShadow {
    BoxShadow::new().offset(value)
}

/// 影の広がり（spread）幅から始まる影設定を生成します。
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

#[inline]
#[must_use]
pub fn px(val: f32) -> Pixel {
    Pixel(val)
}

/// パーセント値（%）を生成します
#[inline]
#[must_use]
pub fn pct(val: f32) -> Percent {
    Percent(val)
}

/// 自動計算（Auto）を生成します
#[inline]
#[must_use]
pub fn auto() -> Auto {
    Auto
}

/// `Win32API` `のクリップボードへテキスト（CF_UNICODETEXT）をコピーします`。
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

/// `Win32API` `のクリップボードからテキスト（CF_UNICODETEXT）を取得します`。
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

/// Windows のマウスホイール生 delta 値（120の倍数）を、
/// OSのスクロール行数設定に準拠した「論理ピクセル単位」の移動量に変換します。
///
/// * `raw_delta`: `WM_MOUSEWHEEL` 等から得られる生値 (前: プラス, 後: マイナス)
///
/// 戻り値はスクロールさせたい論理ピクセル移動量です (手前に引いた際 = 下にスクロール = プラス加算)。
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
