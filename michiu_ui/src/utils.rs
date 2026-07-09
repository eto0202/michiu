use crate::{
    BoxShadow, CornerRadius, Element, FlexDirection, ImageSource, InputContents, LayoutPoint,
    Length, MovieProperty, Point, Prop, Rect, Size, StyleValue, ThisStyle, Val, WebView2Contents,
};
use std::borrow::Cow;

#[inline]
pub fn ts() -> ThisStyle {
    ThisStyle::new()
}

/// プロバイダーの型 `P` から、クロージャ `F` を通して値 `V` を解決する動的なスタイル値を生成
pub fn consume<P, V, F>(selector: F) -> StyleValue<V>
where
    P: Clone + 'static,
    V: 'static,
    F: Fn(&P) -> V + Send + Sync + 'static,
{
    StyleValue::Dynamic(Box::new(move || {
        // ここでプロバイダーの ReadSignal に対する `.get()` を実行することで、
        // 呼び出し元のスタイルエフェクトに自動的に依存関係が購読（Subscribe）されます
        let signal = crate::use_provided::<P>();
        let val = signal.get();
        selector(&val)
    }))
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
pub fn img(source: impl Into<Prop<ImageSource>>) -> Element {
    div_n().image(source)
}

#[inline]
pub fn video(property: impl Into<Prop<MovieProperty>>) -> Element {
    div_n().movie(property)
}

#[inline]
pub fn webview2(contents: impl Into<Prop<WebView2Contents>>) -> Element {
    div_n().webview2(contents)
}

/// プロバイダー `P` から動的に `ThisStyle` を解決してスタイルを適用する汎用コンテナ
#[inline]
pub fn div_c<P, F>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> ThisStyle + Send + Sync + 'static,
{
    Element::new().style_c(f)
}

/// プロバイダー `P` から動的に `ThisStyle` を解決してスタイルを適用する横フレックスコンテナ
#[inline]
pub fn h_flex_c<P, F>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> ThisStyle + Send + Sync + 'static,
{
    // レイアウトの基本形式（フレックス）は静的に適用し、
    // プロバイダー依存の残りのスタイルは style_c で一括解決します
    Element::new()
        .style(ts().flex().flex_direction(FlexDirection::Row))
        .style_c(f)
}

/// プロバイダー `P` から動的に `ThisStyle` を解決してスタイルを適用する縦フレックスコンテナ
#[inline]
pub fn v_flex_c<P, F>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> ThisStyle + Send + Sync + 'static,
{
    Element::new()
        .style(ts().flex().flex_direction(FlexDirection::Column))
        .style_c(f)
}

/// プロバイダー `P` から動的にテキストを解決してテキスト要素を生成します。
#[inline]
pub fn text_c<P, F, S>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> S + Send + Sync + 'static,
    S: Into<Cow<'static, str>>,
{
    div_n().text_c(f)
}

/// プロバイダー `P` から動的に設定を解決して入力フィールド要素を生成します。
#[inline]
pub fn input_c<P, F>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> InputContents + Send + Sync + 'static,
{
    div_n().input_c(f)
}

/// プロバイダー `P` から動的に設定を解決して複数行入力フィールド（テキストエリア）要素を生成します。
#[inline]
pub fn input_area_c<P, F>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> InputContents + Send + Sync + 'static,
{
    div_n().input_area_c(f)
}

/// プロバイダー `P` から動的に解決された画像要素を生成します。
#[inline]
pub fn img_c<P, F>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> ImageSource + Send + Sync + 'static,
{
    div_n().image_c(f)
}

/// プロバイダー `P` から動的に解決されたビデオ再生要素を生成します。
#[inline]
pub fn video_c<P, F>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> MovieProperty + Send + Sync + 'static,
{
    div_n().movie_c(f)
}

/// プロバイダー `P` から動的に解決されたWebView2要素を生成します。
#[inline]
pub fn webview2_c<P, F>(f: F) -> Element
where
    P: Clone + 'static,
    F: Fn(&P) -> WebView2Contents + Send + Sync + 'static,
{
    div_n().webview2_c(f)
}

#[inline]
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
pub fn px(val: f32) -> Pixel {
    Pixel(val)
}

/// パーセント値（%）を生成します
#[inline]
pub fn pct(val: f32) -> Percent {
    Percent(val)
}

/// 自動計算（Auto）を生成します
#[inline]
pub fn auto() -> Auto {
    Auto
}

/// Win32API のクリップボードへテキスト（CF_UNICODETEXT）をコピーします。
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
        let h_mem = match GlobalAlloc(GMEM_MOVEABLE, size) {
            Ok(h) => h,
            _ => return false,
        };
        let ptr = GlobalLock(h_mem);
        if ptr.is_null() {
            return false;
        }
        std::ptr::copy_nonoverlapping(text_u16.as_ptr(), ptr as *mut u16, text_u16.len());
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

/// Win32API のクリップボードからテキスト（CF_UNICODETEXT）を取得します。
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
/// * `raw_delta`: WM_MOUSEWHEEL 等から得られる生値 (前: プラス, 後: マイナス)
///
/// 戻り値はスクロールさせたい論理ピクセル移動量です (手前に引いた際 = 下にスクロール = プラス加算)。
pub fn raw_wheel_delta_to_logical_pixels(raw_delta: f32) -> f32 {
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_GETWHEELSCROLLLINES, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    };

    const WHEEL_PAGESCROLL: u32 = 0xFFFFFFFF;

    let mut scroll_lines: u32 = 3; // OSデフォルト3行をフォールバック値にする
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETWHEELSCROLLLINES,
            0,
            Some(&mut scroll_lines as *mut u32 as *mut _),
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

// 単位の相互キャスト用の中間トレイト
pub trait Convert<T> {
    fn convert(self) -> T;
}

// 具象型からターゲット単位へのキャスト実装
impl Convert<Val> for f32 {
    #[inline]
    fn convert(self) -> Val {
        Val::Px(self)
    }
}
impl Convert<Val> for i32 {
    #[inline]
    fn convert(self) -> Val {
        Val::Px(self as f32)
    }
}
impl Convert<Val> for Pixel {
    #[inline]
    fn convert(self) -> Val {
        Val::Px(self.0)
    }
}
impl Convert<Val> for Percent {
    #[inline]
    fn convert(self) -> Val {
        Val::Percent(self.0)
    }
}
impl Convert<Val> for Auto {
    #[inline]
    fn convert(self) -> Val {
        Val::Auto
    }
}

impl Convert<Length> for f32 {
    #[inline]
    fn convert(self) -> Length {
        Length::Px(self)
    }
}
impl Convert<Length> for i32 {
    #[inline]
    fn convert(self) -> Length {
        Length::Px(self as f32)
    }
}
impl Convert<Length> for Pixel {
    #[inline]
    fn convert(self) -> Length {
        Length::Px(self.0)
    }
}
impl Convert<Length> for Percent {
    #[inline]
    fn convert(self) -> Length {
        Length::Percent(self.0)
    }
}

impl Convert<f32> for f32 {
    #[inline]
    fn convert(self) -> f32 {
        self
    }
}
impl Convert<f32> for i32 {
    #[inline]
    fn convert(self) -> f32 {
        self as f32
    }
}

// Size<T> 用
pub trait IntoSize<T> {
    fn into_size(self) -> Size<T>;
}

// 具象型ごとの単一値(all)実装
impl<T> IntoSize<T> for f32
where
    f32: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_size(self) -> Size<T> {
        let v = self.convert();
        Size {
            width: v.clone(),
            height: v,
        }
    }
}
impl<T> IntoSize<T> for i32
where
    i32: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_size(self) -> Size<T> {
        let v = self.convert();
        Size {
            width: v.clone(),
            height: v,
        }
    }
}
impl<T> IntoSize<T> for Pixel
where
    Pixel: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_size(self) -> Size<T> {
        let v = self.convert();
        Size {
            width: v.clone(),
            height: v,
        }
    }
}
impl<T> IntoSize<T> for Percent
where
    Percent: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_size(self) -> Size<T> {
        let v = self.convert();
        Size {
            width: v.clone(),
            height: v,
        }
    }
}
impl<T> IntoSize<T> for Auto
where
    Auto: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_size(self) -> Size<T> {
        let v = self.convert();
        Size {
            width: v.clone(),
            height: v,
        }
    }
}

// 2連タプル (width, height)
impl<W, H, T> IntoSize<T> for (W, H)
where
    W: Convert<T>,
    H: Convert<T>,
{
    #[inline]
    fn into_size(self) -> Size<T> {
        Size {
            width: self.0.convert(),
            height: self.1.convert(),
        }
    }
}

// Rect<T> 用
pub trait IntoRect<T> {
    fn into_rect(self) -> Rect<T>;
}

// 具象型ごとの単一値(all)実装
impl<T> IntoRect<T> for f32
where
    f32: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_rect(self) -> Rect<T> {
        let v = self.convert();
        Rect {
            top: v.clone(),
            right: v.clone(),
            bottom: v.clone(),
            left: v,
        }
    }
}
impl<T> IntoRect<T> for i32
where
    i32: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_rect(self) -> Rect<T> {
        let v = self.convert();
        Rect {
            top: v.clone(),
            right: v.clone(),
            bottom: v.clone(),
            left: v,
        }
    }
}
impl<T> IntoRect<T> for Pixel
where
    Pixel: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_rect(self) -> Rect<T> {
        let v = self.convert();
        Rect {
            top: v.clone(),
            right: v.clone(),
            bottom: v.clone(),
            left: v,
        }
    }
}
impl<T> IntoRect<T> for Percent
where
    Percent: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_rect(self) -> Rect<T> {
        let v = self.convert();
        Rect {
            top: v.clone(),
            right: v.clone(),
            bottom: v.clone(),
            left: v,
        }
    }
}

// Auto 単一値 ➔ Rect (4方向すべてを Auto 一括設定)
impl<T> IntoRect<T> for Auto
where
    Auto: Convert<T> + Clone,
    T: Clone,
{
    #[inline]
    fn into_rect(self) -> Rect<T> {
        let val = self.convert();
        Rect {
            top: val.clone(),
            right: val.clone(),
            bottom: val.clone(),
            left: val,
        }
    }
}

// 2連タプル (vertical, horizontal)
impl<V, H, T> IntoRect<T> for (V, H)
where
    V: Convert<T> + Clone,
    H: Convert<T> + Clone,
    T: Clone,
{
    #[inline]
    fn into_rect(self) -> Rect<T> {
        let vert = self.0.convert();
        let horiz = self.1.convert();
        Rect {
            top: vert.clone(),
            right: horiz.clone(),
            bottom: vert,
            left: horiz,
        }
    }
}

// 4連タプル (top, right, bottom, left)
impl<Top, Right, Bottom, Left, T> IntoRect<T> for (Top, Right, Bottom, Left)
where
    Top: Convert<T>,
    Right: Convert<T>,
    Bottom: Convert<T>,
    Left: Convert<T>,
{
    #[inline]
    fn into_rect(self) -> Rect<T> {
        Rect {
            top: self.0.convert(),
            right: self.1.convert(),
            bottom: self.2.convert(),
            left: self.3.convert(),
        }
    }
}

// Point<T> 用
pub trait IntoPoint<T> {
    fn into_point(self) -> Point<T>;
}

impl<T> IntoPoint<T> for f32
where
    f32: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_point(self) -> Point<T> {
        let v = self.convert();
        Point { x: v.clone(), y: v }
    }
}
impl<T> IntoPoint<T> for i32
where
    i32: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_point(self) -> Point<T> {
        let v = self.convert();
        Point { x: v.clone(), y: v }
    }
}
impl<T> IntoPoint<T> for Pixel
where
    Pixel: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_point(self) -> Point<T> {
        let v = self.convert();
        Point { x: v.clone(), y: v }
    }
}
impl<T> IntoPoint<T> for Percent
where
    Percent: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_point(self) -> Point<T> {
        let v = self.convert();
        Point { x: v.clone(), y: v }
    }
}

impl<X, Y, T> IntoPoint<T> for (X, Y)
where
    X: Convert<T>,
    Y: Convert<T>,
{
    #[inline]
    fn into_point(self) -> Point<T> {
        Point {
            x: self.0.convert(),
            y: self.1.convert(),
        }
    }
}

// CornerRadius 用
pub trait IntoCornerRadius {
    fn into_corner_radius(self) -> CornerRadius;
}

impl IntoCornerRadius for f32 {
    #[inline]
    fn into_corner_radius(self) -> CornerRadius {
        CornerRadius::all(self)
    }
}
impl IntoCornerRadius for i32 {
    #[inline]
    fn into_corner_radius(self) -> CornerRadius {
        CornerRadius::all(self as f32)
    }
}

impl<V, H> IntoCornerRadius for (V, H)
where
    V: Convert<f32>,
    H: Convert<f32>,
{
    #[inline]
    fn into_corner_radius(self) -> CornerRadius {
        CornerRadius::symmetric(self.0.convert(), self.1.convert())
    }
}

impl<TL, TR, BR, BL> IntoCornerRadius for (TL, TR, BR, BL)
where
    TL: Convert<f32>,
    TR: Convert<f32>,
    BR: Convert<f32>,
    BL: Convert<f32>,
{
    #[inline]
    fn into_corner_radius(self) -> CornerRadius {
        CornerRadius {
            top_left: self.0.convert(),
            top_right: self.1.convert(),
            bottom_right: self.2.convert(),
            bottom_left: self.3.convert(),
        }
    }
}

// LayoutPoint（BoxShadow 等のオフセット）用
pub trait IntoLayoutPoint {
    fn into_layout_point(self) -> LayoutPoint;
}

impl IntoLayoutPoint for f32 {
    #[inline]
    fn into_layout_point(self) -> LayoutPoint {
        LayoutPoint { x: self, y: self }
    }
}
impl IntoLayoutPoint for i32 {
    #[inline]
    fn into_layout_point(self) -> LayoutPoint {
        LayoutPoint {
            x: self as f32,
            y: self as f32,
        }
    }
}

impl<X, Y> IntoLayoutPoint for (X, Y)
where
    X: Convert<f32>,
    Y: Convert<f32>,
{
    #[inline]
    fn into_layout_point(self) -> LayoutPoint {
        LayoutPoint {
            x: self.0.convert(),
            y: self.1.convert(),
        }
    }
}

// Val 自身から Val への同一変換を実装
impl Convert<Val> for Val {
    #[inline]
    fn convert(self) -> Val {
        self
    }
}

// Length 自身から Length への同一変換を実装
impl Convert<Length> for Length {
    #[inline]
    fn convert(self) -> Length {
        self
    }
}

// bool から f32 への変換 (true ➔ 1.0f32, false ➔ 0.0f32)
impl Convert<f32> for bool {
    #[inline]
    fn convert(self) -> f32 {
        if self { 1.0 } else { 0.0 }
    }
}
