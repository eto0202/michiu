use crate::{
    AlignContent, AlignItems, AlignSelf, Backdrop, BasicLayout, BorderAlignment, BorderStyle,
    BoxShadow, BoxSizing, Color, Convert, CornerRadius, CursorIcon, Direction, Display, EdgeInsets,
    FlexDirection, FlexLayout, FlexWrap, GridAutoFlow, GridLayout, GridLine, GridPlacement,
    InteractionName, InteractionStyles, IntoCornerRadius, IntoRect, IntoSize, JustifyContent,
    LayoutOverflow, Length, LinearGradient, Overflow, Point, PointerEvents, Position,
    ScrollbarDisplay, ScrollbarMode, ScrollbarStyle, TextAlign, Transform, Transition, UserSelect,
    Val, VisualProperty, auto, bitmap::*, pct,
};
use std::{borrow::Cow, sync::Arc, time::Duration};

// コールドデータである複雑なGridトラック設定のみTaffyからそのまま拝借
use taffy::{GridTemplateArea, GridTemplateComponent, TrackSizingFunction};

#[derive(Debug, Clone, Default)]
pub struct ThisStyle {
    pub(crate) inner: Arc<StyleInner>,
}

#[derive(Clone, Default)]
pub(crate) struct StyleInner {
    pub(crate) mask: ComponentMask,
    pub(crate) basic_layout: BasicLayout,
    pub(crate) flex_layout: FlexLayout,
    pub(crate) grid_layout: Option<GridLayout>,
    pub(crate) visual_property: VisualProperty,
    pub(crate) interaction_styles: InteractionStyles,
    pub(crate) scrollbar_style: Option<ScrollbarStyle>,
}

// Debug トレイトの手動実装 (クロージャを含むため)
impl std::fmt::Debug for StyleInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StyleInner")
            .field("mask", &self.mask)
            .field("basic_layout", &self.basic_layout)
            .field("visual_property", &self.visual_property)
            .finish()
    }
}

// ComponentMask, BasicLayout, FlexLayout, GridLayout, VisualProperty はすべて単純なデータ構造でありスレッド安全
// InteractionStyles が持つ ThisStyle も内部は Arc で管理
// スレッド固有の生ポインタ、内部可変性コンテナ、非アトミックな参照カウントは一切含まれていない
// 値の書き換えは常に Arc::make_mut を通じて排他参照が得られたときのみ行われるためデータ競合のリスクはない
unsafe impl Send for StyleInner {}
unsafe impl Sync for StyleInner {}

impl ThisStyle {
    /// 新しいスタイルの起点
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// 要素の表示形態（Display）を設定します。
    #[inline]
    pub fn display(mut self, value: Display) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.display = value;
        inner.mask.set(STYLE_DISPLAY);
        self
    }

    #[inline]
    pub fn hidden(self) -> Self {
        self.display(Display::None)
    }

    #[inline]
    pub fn flex(self) -> Self {
        self.display(Display::Flex)
    }

    #[inline]
    pub fn grid(self) -> Self {
        self.display(Display::Grid)
    }

    #[inline]
    pub fn block(self) -> Self {
        self.display(Display::Block)
    }

    /// 要素がテーブルアイテムとして振る舞うかどうかを設定します。
    #[inline]
    pub fn item_is_table(mut self, value: bool) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.item_is_table = value;
        inner.mask.set(STYLE_ITEM_IS_TABLE);
        self
    }

    /// 要素が置換要素（画像やビデオなど）かどうかを設定します。
    #[inline]
    pub fn item_is_replaced(mut self, value: bool) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.item_is_replaced = value;
        inner.mask.set(STYLE_ITEM_IS_REPLACED);
        self
    }

    /// ボックスモデルの算出基準（BoxSizing）を設定します。
    #[inline]
    pub fn box_sizing(mut self, value: BoxSizing) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.box_sizing = value;
        inner.mask.set(STYLE_BOX_SIZING);
        self
    }

    #[inline]
    pub fn box_border(self) -> Self {
        self.box_sizing(BoxSizing::BorderBox)
    }

    #[inline]
    pub fn box_content(self) -> Self {
        self.box_sizing(BoxSizing::ContentBox)
    }

    /// テキストや要素のインライン方向（Direction）を設定します。
    #[inline]
    pub fn direction(mut self, value: Direction) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.direction = value;
        inner.mask.set(STYLE_DIRECTION);
        self
    }

    /// スクロールバーのスタイル（太さ、トラック、サム、および疑似クラス）を登録します。
    #[inline]
    pub fn scrollbar(mut self, style: ScrollbarStyle) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.scrollbar_style = Some(style);
        inner.mask.set(STYLE_SCROLLBAR);
        self
    }

    /// スクロールバーの太さ（物理幅/高さ）を直接指定します。
    #[inline]
    pub fn scrollbar_width(mut self, width: f32) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let sb = inner
            .scrollbar_style
            .get_or_insert_with(ScrollbarStyle::default);
        sb.width = width;
        inner.mask.set(STYLE_SCROLLBAR);
        self
    }

    /// スクロールバーの表示条件（None / Always / Auto）を直接指定します。
    #[inline]
    pub fn scrollbar_display(mut self, display: ScrollbarDisplay) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let sb = inner
            .scrollbar_style
            .get_or_insert_with(ScrollbarStyle::default);
        sb.display = display;
        inner.mask.set(STYLE_SCROLLBAR);
        self
    }

    #[inline]
    pub fn scrollbar_auto(self) -> Self {
        self.scrollbar_display(ScrollbarDisplay::Auto)
    }

    #[inline]
    pub fn scrollbar_none(self) -> Self {
        self.scrollbar_display(ScrollbarDisplay::None)
    }

    #[inline]
    pub fn scrollbar_always(self) -> Self {
        self.scrollbar_display(ScrollbarDisplay::Always)
    }
    #[inline]
    pub fn scrollbar_transient(self) -> Self {
        self.scrollbar_display(ScrollbarDisplay::Transient)
    }

    /// スクロールバーの配置モード（Layout: コンテンツ縮小 / Overlay: 前面重ね）を直接指定します。
    #[inline]
    pub fn scrollbar_mode(mut self, mode: ScrollbarMode) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let sb = inner
            .scrollbar_style
            .get_or_insert_with(ScrollbarStyle::default);
        sb.mode = mode;
        inner.mask.set(STYLE_SCROLLBAR);
        self
    }

    /// スクロールバーのレール（トラック背景）部分の装飾スタイルを直接指定します。
    #[inline]
    pub fn scrollbar_track(mut self, style: ThisStyle) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let sb = inner
            .scrollbar_style
            .get_or_insert_with(ScrollbarStyle::default);
        sb.track = Some(style);
        inner.mask.set(STYLE_SCROLLBAR);
        self
    }

    /// スクロールバーのつまみ（サム）部分の装飾スタイルを直接指定します。
    #[inline]
    pub fn scrollbar_thumb(mut self, style: ThisStyle) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let sb = inner
            .scrollbar_style
            .get_or_insert_with(ScrollbarStyle::default);
        sb.thumb = Some(style);
        inner.mask.set(STYLE_SCROLLBAR);
        self
    }

    /// コンテンツのはみ出し処理（Overflow）を設定します。
    #[inline]
    pub fn overflow(mut self, value: LayoutOverflow) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.overflow = value;
        inner.mask.set(STYLE_OVERFLOW);
        self
    }

    #[inline]
    pub fn overflow_auto(self) -> Self {
        self.overflow(LayoutOverflow {
            x: Overflow::Visible,
            y: Overflow::Visible,
        })
    }

    #[inline]
    pub fn overflow_hidden(self) -> Self {
        self.overflow(LayoutOverflow {
            x: Overflow::Hidden,
            y: Overflow::Hidden,
        })
    }

    #[inline]
    pub fn overflow_scroll(self) -> Self {
        self.overflow(LayoutOverflow {
            x: Overflow::Scroll,
            y: Overflow::Scroll,
        })
    }

    #[inline]
    pub fn overflow_clip(self) -> Self {
        self.overflow(LayoutOverflow {
            x: Overflow::Clip,
            y: Overflow::Clip,
        })
    }

    #[inline]
    pub fn overflow_x_auto(mut self) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.overflow.x = Overflow::Visible;
        inner.mask.set(STYLE_OVERFLOW);
        self
    }

    #[inline]
    pub fn overflow_x_hidden(mut self) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.overflow.x = Overflow::Hidden;
        inner.mask.set(STYLE_OVERFLOW);
        self
    }

    #[inline]
    pub fn overflow_x_scroll(mut self) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.overflow.x = Overflow::Scroll;
        inner.mask.set(STYLE_OVERFLOW);
        self
    }

    #[inline]
    pub fn overflow_x_clip(mut self) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.overflow.x = Overflow::Clip;
        inner.mask.set(STYLE_OVERFLOW);
        self
    }

    #[inline]
    pub fn overflow_y_auto(mut self) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.overflow.y = Overflow::Visible;
        inner.mask.set(STYLE_OVERFLOW);
        self
    }

    #[inline]
    pub fn overflow_y_hidden(mut self) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.overflow.y = Overflow::Hidden;
        inner.mask.set(STYLE_OVERFLOW);
        self
    }

    #[inline]
    pub fn overflow_y_scroll(mut self) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.overflow.y = Overflow::Scroll;
        inner.mask.set(STYLE_OVERFLOW);
        self
    }

    #[inline]
    pub fn overflow_y_clip(mut self) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.overflow.y = Overflow::Clip;
        inner.mask.set(STYLE_OVERFLOW);
        self
    }

    /// 要素の配置基準（Position）を設定します。
    #[inline]
    pub fn position(mut self, value: Position) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.position = value;
        inner.mask.set(STYLE_POSITION);
        self
    }

    #[inline]
    pub fn absolute(self) -> Self {
        self.position(Position::Absolute)
    }

    #[inline]
    pub fn relative(self) -> Self {
        self.position(Position::Relative)
    }

    /// 要素の配置インセット（inset：top, right, bottom, left）を設定します。
    #[inline]
    pub fn inset(mut self, value: impl IntoRect<Val>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.inset = value.into_rect();
        inner.mask.set(STYLE_INSET);
        self
    }

    /// 要素を絶対配置 (Position::Absolute) に設定し、同時に配置インセットを設定します。
    #[inline]
    pub fn absolute_inset(self, value: impl IntoRect<Val>) -> Self {
        self.position(Position::Absolute).inset(value)
    }

    /// 左右の配置インセット（left, right）を一括設定します。
    #[inline]
    pub fn inset_x(self, value: impl IntoSize<Val>) -> Self {
        let size = value.into_size();
        let current_top = self.inner.basic_layout.inset.top;
        let current_bottom = self.inner.basic_layout.inset.bottom;
        self.inset((current_top, size.width, current_bottom, size.height))
    }

    /// 上下の配置インセット（top, bottom）を一括設定します。
    #[inline]
    pub fn inset_y(self, value: impl IntoSize<Val>) -> Self {
        let size = value.into_size();
        let current_left = self.inner.basic_layout.inset.left;
        let current_right = self.inner.basic_layout.inset.right;
        self.inset((size.width, current_right, size.height, current_left))
    }

    #[inline]
    pub fn top(self, value: impl Convert<Val>) -> Self {
        let current = self.inner.basic_layout.inset;
        self.inset((value.convert(), current.right, current.bottom, current.left))
    }

    #[inline]
    pub fn right(self, value: impl Convert<Val>) -> Self {
        let current = self.inner.basic_layout.inset;
        self.inset((current.top, value.convert(), current.bottom, current.left))
    }

    #[inline]
    pub fn bottom(self, value: impl Convert<Val>) -> Self {
        let current = self.inner.basic_layout.inset;
        self.inset((current.top, current.right, value.convert(), current.left))
    }

    #[inline]
    pub fn left(self, value: impl Convert<Val>) -> Self {
        let current = self.inner.basic_layout.inset;
        self.inset((current.top, current.right, current.bottom, value.convert()))
    }

    /// 要素の基本サイズ（width, height）を設定します。
    #[inline]
    pub fn size(mut self, value: impl IntoSize<Val>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.size = value.into_size();
        inner.mask.set(STYLE_SIZE);
        self
    }

    #[inline]
    pub fn size_full(self) -> Self {
        self.size(pct(100.0))
    }

    #[inline]
    pub fn size_auto(self) -> Self {
        self.size(auto())
    }

    /// 要素の幅（width）のみを設定します（高さは既存の値を維持）。
    #[inline]
    pub fn width(self, value: impl Convert<Val>) -> Self {
        let current_h = self.inner.basic_layout.size.height;
        self.size((value.convert(), current_h))
    }

    #[inline]
    pub fn w_full(self) -> Self {
        self.width(pct(100.0))
    }

    #[inline]
    pub fn w_auto(self) -> Self {
        self.width(auto())
    }

    /// 要素の高さ（height）のみを設定します（幅は既存の値を維持）。
    #[inline]
    pub fn height(self, value: impl Convert<Val>) -> Self {
        let current_w = self.inner.basic_layout.size.width;
        self.size((current_w, value.convert()))
    }

    #[inline]
    pub fn h_full(self) -> Self {
        self.height(pct(100.0))
    }

    #[inline]
    pub fn h_auto(self) -> Self {
        self.height(auto())
    }

    /// 要素の最小サイズを設定します。
    #[inline]
    pub fn min_size(mut self, value: impl IntoSize<Val>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.min_size = value.into_size();
        inner.mask.set(STYLE_MIN_SIZE);
        self
    }

    /// 要素の最大サイズを設定します。
    #[inline]
    pub fn max_size(mut self, value: impl IntoSize<Val>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.max_size = value.into_size();
        inner.mask.set(STYLE_MAX_SIZE);
        self
    }

    /// 任意の比率（幅 / 高さ）でアスペクト比を設定します。
    #[inline]
    pub fn aspect_ratio(mut self, width: f32, height: f32) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        if height <= 0.0 {
            inner.basic_layout.aspect_ratio = None;
        } else {
            inner.basic_layout.aspect_ratio = Some(width / height);
        }
        inner.mask.set(STYLE_ASPECT_RATIO);
        self
    }

    #[inline]
    pub fn ratio_16_9(self) -> Self {
        self.aspect_ratio(16.0, 9.0)
    }

    #[inline]
    pub fn ratio_9_16(self) -> Self {
        self.aspect_ratio(9.0, 16.0)
    }

    #[inline]
    pub fn ratio_4_3(self) -> Self {
        self.aspect_ratio(4.0, 3.0)
    }

    #[inline]
    pub fn ratio_3_4(self) -> Self {
        self.aspect_ratio(3.0, 4.0)
    }

    #[inline]
    pub fn ratio_1_1(self) -> Self {
        self.aspect_ratio(1.0, 1.0)
    }

    #[inline]
    pub fn ratio_21_9(self) -> Self {
        self.aspect_ratio(21.0, 9.0)
    }

    #[inline]
    pub fn ratio_9_21(self) -> Self {
        self.aspect_ratio(9.0, 21.0)
    }

    #[inline]
    pub fn clear_aspect_ratio(self) -> Self {
        self.aspect_ratio(0.0, 0.0)
    }

    /// 外側余白（margin）を設定します。
    #[inline]
    pub fn margin(mut self, value: impl IntoRect<Val>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.margin = value.into_rect();
        inner.mask.set(STYLE_MARGIN);
        self
    }

    #[inline]
    pub fn m(self, value: impl IntoRect<Val>) -> Self {
        self.margin(value)
    }

    #[inline]
    pub fn m_0(self) -> Self {
        self.margin(0.0)
    }

    #[inline]
    pub fn m_auto(self) -> Self {
        self.margin(auto())
    }

    /// 左右の外側余白（margin-left, margin-right）を一括設定します。
    #[inline]
    pub fn m_x(self, value: impl IntoSize<Val>) -> Self {
        let size = value.into_size();
        let current_top = self.inner.basic_layout.margin.top;
        let current_bottom = self.inner.basic_layout.margin.bottom;
        self.margin((current_top, size.width, current_bottom, size.height))
    }

    /// 上下の外側余白（margin-top, margin-bottom）を一括設定します。
    #[inline]
    pub fn m_y(self, value: impl IntoSize<Val>) -> Self {
        let size = value.into_size();
        let current_left = self.inner.basic_layout.margin.left;
        let current_right = self.inner.basic_layout.margin.right;
        self.margin((size.width, current_right, size.height, current_left))
    }

    #[inline]
    pub fn m_t(self, value: impl Convert<Val>) -> Self {
        let current = self.inner.basic_layout.margin;
        self.margin((value.convert(), current.right, current.bottom, current.left))
    }

    #[inline]
    pub fn m_r(self, value: impl Convert<Val>) -> Self {
        let current = self.inner.basic_layout.margin;
        self.margin((current.top, value.convert(), current.bottom, current.left))
    }

    #[inline]
    pub fn m_b(self, value: impl Convert<Val>) -> Self {
        let current = self.inner.basic_layout.margin;
        self.margin((current.top, current.right, value.convert(), current.left))
    }

    #[inline]
    pub fn m_l(self, value: impl Convert<Val>) -> Self {
        let current = self.inner.basic_layout.margin;
        self.margin((current.top, current.right, current.bottom, value.convert()))
    }

    /// 内側余白（padding）を設定します。
    #[inline]
    pub fn padding(mut self, value: impl IntoRect<Length>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.padding = value.into_rect();
        inner.mask.set(STYLE_PADDING);
        self
    }

    #[inline]
    pub fn p(self, value: impl IntoRect<Length>) -> Self {
        self.padding(value)
    }

    #[inline]
    pub fn p_0(self) -> Self {
        self.padding(0.0)
    }

    /// 左右の内側余白（padding-left, padding-right）を一括設定します。
    #[inline]
    pub fn p_x(self, value: impl IntoSize<Length>) -> Self {
        let size = value.into_size();
        let current_top = self.inner.basic_layout.padding.top;
        let current_bottom = self.inner.basic_layout.padding.bottom;
        self.padding((current_top, size.width, current_bottom, size.height))
    }

    /// 上下の内側余白（padding-top, padding-bottom）を一括設定します。
    #[inline]
    pub fn p_y(self, value: impl IntoSize<Length>) -> Self {
        let size = value.into_size();
        let current_left = self.inner.basic_layout.padding.left;
        let current_right = self.inner.basic_layout.padding.right;
        self.padding((size.width, current_right, size.height, current_left))
    }

    #[inline]
    pub fn p_t(self, value: impl Convert<Length>) -> Self {
        let current = self.inner.basic_layout.padding;
        self.padding((value.convert(), current.right, current.bottom, current.left))
    }

    #[inline]
    pub fn p_r(self, value: impl Convert<Length>) -> Self {
        let current = self.inner.basic_layout.padding;
        self.padding((current.top, value.convert(), current.bottom, current.left))
    }

    #[inline]
    pub fn p_b(self, value: impl Convert<Length>) -> Self {
        let current = self.inner.basic_layout.padding;
        self.padding((current.top, current.right, value.convert(), current.left))
    }

    #[inline]
    pub fn p_l(self, value: impl Convert<Length>) -> Self {
        let current = self.inner.basic_layout.padding;
        self.padding((current.top, current.right, current.bottom, value.convert()))
    }

    /// 境界線の太さ（border）を設定します。
    #[inline]
    pub fn border(mut self, style: BorderStyle, width: impl IntoRect<Length>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.border = width.into_rect();
        inner.visual_property.border_styles = Some([style; 4]);
        inner.mask.set(STYLE_BORDER);
        self
    }

    /// 実線（Solid）の枠線と太さを一括設定します。
    #[inline]
    pub fn border_solid(self, width: impl IntoRect<Length>) -> Self {
        self.border(BorderStyle::Solid, width)
    }

    /// 丸点線（Dotted）の枠線と太さを一括設定します。
    #[inline]
    pub fn border_dotted(self, width: impl IntoRect<Length>) -> Self {
        self.border(BorderStyle::Dotted, width)
    }

    /// 破線（Dashed）の枠線と太さを一括設定します。
    #[inline]
    pub fn border_dashed(self, width: impl IntoRect<Length>) -> Self {
        self.border(BorderStyle::Dashed, width)
    }

    /// 二重線（Double）の枠線と太さを一括設定します。
    #[inline]
    pub fn border_double(self, width: impl IntoRect<Length>) -> Self {
        self.border(BorderStyle::Double, width)
    }

    /// 上枠線（Border Top）の種類と太さを個別に設定します。
    #[inline]
    pub fn border_top(mut self, style: BorderStyle, value: impl Convert<Length>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.border.top = value.convert();

        let mut styles = inner
            .visual_property
            .border_styles
            .unwrap_or([BorderStyle::Solid; 4]);
        styles[0] = style;
        inner.visual_property.border_styles = Some(styles);
        inner.mask.set(STYLE_BORDER);
        self
    }

    /// 右枠線（Border Right）の種類と太さを個別に設定します。
    #[inline]
    pub fn border_right(mut self, style: BorderStyle, value: impl Convert<Length>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.border.right = value.convert();

        let mut styles = inner
            .visual_property
            .border_styles
            .unwrap_or([BorderStyle::Solid; 4]);
        styles[1] = style;
        inner.visual_property.border_styles = Some(styles);
        inner.mask.set(STYLE_BORDER);
        self
    }

    /// 下枠線（Border Bottom）の種類と太さを個別に設定します。
    #[inline]
    pub fn border_bottom(mut self, style: BorderStyle, value: impl Convert<Length>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.border.bottom = value.convert();

        let mut styles = inner
            .visual_property
            .border_styles
            .unwrap_or([BorderStyle::Solid; 4]);
        styles[2] = style;
        inner.visual_property.border_styles = Some(styles);
        inner.mask.set(STYLE_BORDER);
        self
    }

    /// 左枠線（Border Left）の種類と太さを個別に設定します。
    #[inline]
    pub fn border_left(mut self, style: BorderStyle, value: impl Convert<Length>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.border.left = value.convert();

        let mut styles = inner
            .visual_property
            .border_styles
            .unwrap_or([BorderStyle::Solid; 4]);
        styles[3] = style;
        inner.visual_property.border_styles = Some(styles);
        inner.mask.set(STYLE_BORDER);
        self
    }

    /// 四辺個別の枠線の長さ比率（0.0 ~ 1.0）を設定します。
    /// 単一値、2連タプル (縦, 横)、4連タプル (上, 右, 下, 左) を受け入れます。
    #[inline]
    pub fn border_lengths(mut self, value: impl IntoRect<f32>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let rect = value.into_rect();
        inner.visual_property.border_lengths = Some(EdgeInsets {
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
            left: rect.left,
        });
        inner.mask.set(STYLE_BORDER);
        self
    }

    #[inline]
    pub fn border_top_length(mut self, value: f32) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let mut lengths = inner
            .visual_property
            .border_lengths
            .unwrap_or(EdgeInsets::px_all(1.0));
        lengths.top = value;
        inner.visual_property.border_lengths = Some(lengths);
        inner.mask.set(STYLE_BORDER);
        self
    }

    #[inline]
    pub fn border_right_length(mut self, value: f32) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let mut lengths = inner
            .visual_property
            .border_lengths
            .unwrap_or(EdgeInsets::px_all(1.0));
        lengths.right = value;
        inner.visual_property.border_lengths = Some(lengths);
        inner.mask.set(STYLE_BORDER);
        self
    }

    #[inline]
    pub fn border_bottom_length(mut self, value: f32) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let mut lengths = inner
            .visual_property
            .border_lengths
            .unwrap_or(EdgeInsets::px_all(1.0));
        lengths.bottom = value;
        inner.visual_property.border_lengths = Some(lengths);
        inner.mask.set(STYLE_BORDER);
        self
    }

    #[inline]
    pub fn border_left_length(mut self, value: f32) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let mut lengths = inner
            .visual_property
            .border_lengths
            .unwrap_or(EdgeInsets::px_all(1.0));
        lengths.left = value;
        inner.visual_property.border_lengths = Some(lengths);
        inner.mask.set(STYLE_BORDER);
        self
    }

    /// すべての辺の枠線基準点（伸縮方向）を一括設定します。
    #[inline]
    pub fn border_align(mut self, value: BorderAlignment) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.border_alignments = Some([value; 4]);
        inner.mask.set(STYLE_BORDER);
        self
    }

    /// 四辺個別の枠線基準点を設定します。[Top, Right, Bottom, Left]
    #[inline]
    pub fn border_aligns(mut self, values: [BorderAlignment; 4]) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.border_alignments = Some(values);
        inner.mask.set(STYLE_BORDER);
        self
    }

    fn set_border_align_idx(mut self, idx: usize, value: BorderAlignment) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let mut aligns = inner
            .visual_property
            .border_alignments
            .unwrap_or([BorderAlignment::Start; 4]);
        aligns[idx] = value;
        inner.visual_property.border_alignments = Some(aligns);
        inner.mask.set(STYLE_BORDER);
        self
    }

    #[inline]
    pub fn border_top_align(self, value: BorderAlignment) -> Self {
        self.set_border_align_idx(0, value)
    }
    #[inline]
    pub fn border_right_align(self, value: BorderAlignment) -> Self {
        self.set_border_align_idx(1, value)
    }
    #[inline]
    pub fn border_bottom_align(self, value: BorderAlignment) -> Self {
        self.set_border_align_idx(2, value)
    }
    #[inline]
    pub fn border_left_align(self, value: BorderAlignment) -> Self {
        self.set_border_align_idx(3, value)
    }

    /// コンテナ内の一括交差軸配置を設定します。
    #[inline]
    pub fn align_items(mut self, value: impl Into<Option<AlignItems>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.align_items = value.into();
        inner.mask.set(STYLE_ALIGN_ITEMS);
        self
    }

    #[inline]
    pub fn items_start(self) -> Self {
        self.align_items(AlignItems::Start)
    }

    #[inline]
    pub fn items_end(self) -> Self {
        self.align_items(AlignItems::End)
    }

    #[inline]
    pub fn items_flex_start(self) -> Self {
        self.align_items(AlignItems::FlexStart)
    }

    #[inline]
    pub fn items_flex_end(self) -> Self {
        self.align_items(AlignItems::FlexEnd)
    }

    #[inline]
    pub fn items_center(self) -> Self {
        self.align_items(AlignItems::Center)
    }

    #[inline]
    pub fn items_baseline(self) -> Self {
        self.align_items(AlignItems::Baseline)
    }

    #[inline]
    pub fn items_stretch(self) -> Self {
        self.align_items(AlignItems::Stretch)
    }

    #[inline]
    pub fn items_start_safe(self) -> Self {
        self.align_items(AlignItems::SafeStart)
    }

    #[inline]
    pub fn items_end_safe(self) -> Self {
        self.align_items(AlignItems::SafeEnd)
    }

    #[inline]
    pub fn items_flex_start_safe(self) -> Self {
        self.align_items(AlignItems::SafeFlexStart)
    }

    #[inline]
    pub fn items_flex_end_safe(self) -> Self {
        self.align_items(AlignItems::SafeFlexEnd)
    }

    #[inline]
    pub fn items_center_safe(self) -> Self {
        self.align_items(AlignItems::SafeCenter)
    }

    /// 個別要素の交差軸配置を設定します。
    #[inline]
    pub fn align_self(mut self, value: impl Into<Option<AlignSelf>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.align_self = value.into();
        inner.mask.set(STYLE_ALIGN_SELF);
        self
    }

    #[inline]
    pub fn self_auto(self) -> Self {
        self.align_self(None)
    }

    #[inline]
    pub fn self_start(self) -> Self {
        self.align_self(AlignSelf::Start)
    }

    #[inline]
    pub fn self_end(self) -> Self {
        self.align_self(AlignSelf::End)
    }

    #[inline]
    pub fn self_flex_start(self) -> Self {
        self.align_self(AlignSelf::FlexStart)
    }

    #[inline]
    pub fn self_flex_end(self) -> Self {
        self.align_self(AlignSelf::FlexEnd)
    }

    #[inline]
    pub fn self_center(self) -> Self {
        self.align_self(AlignSelf::Center)
    }

    #[inline]
    pub fn self_baseline(self) -> Self {
        self.align_self(AlignSelf::Baseline)
    }

    #[inline]
    pub fn self_stretch(self) -> Self {
        self.align_self(AlignSelf::Stretch)
    }

    #[inline]
    pub fn self_start_safe(self) -> Self {
        self.align_self(AlignSelf::SafeStart)
    }

    #[inline]
    pub fn self_end_safe(self) -> Self {
        self.align_self(AlignSelf::SafeEnd)
    }

    #[inline]
    pub fn self_flex_start_safe(self) -> Self {
        self.align_self(AlignSelf::SafeFlexStart)
    }

    #[inline]
    pub fn self_flex_end_safe(self) -> Self {
        self.align_self(AlignSelf::SafeFlexEnd)
    }

    #[inline]
    pub fn self_center_safe(self) -> Self {
        self.align_self(AlignSelf::SafeCenter)
    }

    /// コンテナ内の一括主軸配置を設定します。
    #[inline]
    pub fn justify_items(mut self, value: impl Into<Option<AlignItems>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.justify_items = value.into();
        inner.mask.set(STYLE_JUSTIFY_ITEMS);
        self
    }

    #[inline]
    pub fn justify_items_center(self) -> Self {
        self.justify_items(AlignItems::Center)
    }

    #[inline]
    pub fn justify_items_center_safe(self) -> Self {
        self.justify_items(AlignItems::SafeCenter)
    }

    #[inline]
    pub fn justify_items_end(self) -> Self {
        self.justify_items(AlignItems::End)
    }

    #[inline]
    pub fn justify_items_end_safe(self) -> Self {
        self.justify_items(AlignItems::SafeEnd)
    }

    #[inline]
    pub fn justify_items_start(self) -> Self {
        self.justify_items(AlignItems::Start)
    }

    #[inline]
    pub fn justify_items_start_safe(self) -> Self {
        self.justify_items(AlignItems::SafeStart)
    }

    #[inline]
    pub fn justify_items_stretch(self) -> Self {
        self.justify_items(AlignItems::Stretch)
    }

    #[inline]
    pub fn justify_items_flex_start(self) -> Self {
        self.justify_items(AlignItems::FlexStart)
    }

    #[inline]
    pub fn justify_items_flex_end(self) -> Self {
        self.justify_items(AlignItems::FlexEnd)
    }

    #[inline]
    pub fn justify_items_flex_start_safe(self) -> Self {
        self.justify_items(AlignItems::SafeFlexStart)
    }

    #[inline]
    pub fn justify_items_flex_end_safe(self) -> Self {
        self.justify_items(AlignItems::SafeFlexEnd)
    }

    #[inline]
    pub fn justify_items_baseline(self) -> Self {
        self.justify_items(AlignItems::Baseline)
    }

    /// 個別要素の主軸配置を設定します。
    #[inline]
    pub fn justify_self(mut self, value: impl Into<Option<AlignSelf>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.justify_self = value.into();
        inner.mask.set(STYLE_JUSTIFY_SELF);
        self
    }

    #[inline]
    pub fn justify_self_auto(self) -> Self {
        self.justify_self(None)
    }

    #[inline]
    pub fn justify_self_baseline(self) -> Self {
        self.justify_self(AlignSelf::Baseline)
    }

    #[inline]
    pub fn justify_self_center(self) -> Self {
        self.justify_self(AlignSelf::Center)
    }

    #[inline]
    pub fn justify_self_center_safe(self) -> Self {
        self.justify_self(AlignSelf::SafeCenter)
    }

    #[inline]
    pub fn justify_self_end(self) -> Self {
        self.justify_self(AlignSelf::End)
    }

    #[inline]
    pub fn justify_self_end_safe(self) -> Self {
        self.justify_self(AlignSelf::SafeEnd)
    }

    #[inline]
    pub fn justify_self_start(self) -> Self {
        self.justify_self(AlignSelf::Start)
    }

    #[inline]
    pub fn justify_self_start_safe(self) -> Self {
        self.justify_self(AlignSelf::SafeStart)
    }

    #[inline]
    pub fn justify_self_stretch(self) -> Self {
        self.justify_self(AlignSelf::Stretch)
    }

    #[inline]
    pub fn justify_self_flex_start(self) -> Self {
        self.justify_self(AlignSelf::FlexStart)
    }

    #[inline]
    pub fn justify_self_flex_end(self) -> Self {
        self.justify_self(AlignSelf::FlexEnd)
    }

    #[inline]
    pub fn justify_self_flex_start_safe(self) -> Self {
        self.justify_self(AlignSelf::SafeFlexStart)
    }

    #[inline]
    pub fn justify_self_flex_end_safe(self) -> Self {
        self.justify_self(AlignSelf::SafeFlexEnd)
    }

    /// 複数行にまたがる場合のコンテンツ一括配置を設定します。
    #[inline]
    pub fn align_content(mut self, value: impl Into<Option<AlignContent>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.align_content = value.into();
        inner.mask.set(STYLE_ALIGN_CONTENT);
        self
    }

    #[inline]
    pub fn content_around(self) -> Self {
        self.align_content(AlignContent::SpaceAround)
    }

    #[inline]
    pub fn content_between(self) -> Self {
        self.align_content(AlignContent::SpaceBetween)
    }

    #[inline]
    pub fn content_center(self) -> Self {
        self.align_content(AlignContent::Center)
    }

    #[inline]
    pub fn content_center_safe(self) -> Self {
        self.align_content(AlignContent::SafeCenter)
    }

    #[inline]
    pub fn content_end(self) -> Self {
        self.align_content(AlignContent::End)
    }

    #[inline]
    pub fn content_end_safe(self) -> Self {
        self.align_content(AlignContent::SafeEnd)
    }

    #[inline]
    pub fn content_evenly(self) -> Self {
        self.align_content(AlignContent::SpaceEvenly)
    }

    #[inline]
    pub fn content_start(self) -> Self {
        self.align_content(AlignContent::Start)
    }

    #[inline]
    pub fn content_start_safe(self) -> Self {
        self.align_content(AlignContent::SafeStart)
    }

    #[inline]
    pub fn content_stretch(self) -> Self {
        self.align_content(AlignContent::Stretch)
    }

    #[inline]
    pub fn content_flex_start(self) -> Self {
        self.align_content(AlignContent::FlexStart)
    }

    #[inline]
    pub fn content_flex_end(self) -> Self {
        self.align_content(AlignContent::FlexEnd)
    }

    #[inline]
    pub fn content_flex_start_safe(self) -> Self {
        self.align_content(AlignContent::SafeFlexStart)
    }

    #[inline]
    pub fn content_flex_end_safe(self) -> Self {
        self.align_content(AlignContent::SafeFlexEnd)
    }

    /// 主軸方向のコンテンツ配置を設定します。
    #[inline]
    pub fn justify_content(mut self, value: impl Into<Option<JustifyContent>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.justify_content = value.into();
        inner.mask.set(STYLE_JUSTIFY_CONTENT);
        self
    }

    #[inline]
    pub fn justify_around(self) -> Self {
        self.justify_content(JustifyContent::SpaceAround)
    }

    #[inline]
    pub fn justify_between(self) -> Self {
        self.justify_content(JustifyContent::SpaceBetween)
    }

    #[inline]
    pub fn justify_center(self) -> Self {
        self.justify_content(JustifyContent::Center)
    }

    #[inline]
    pub fn justify_center_safe(self) -> Self {
        self.justify_content(JustifyContent::SafeCenter)
    }

    #[inline]
    pub fn justify_end(self) -> Self {
        self.justify_content(JustifyContent::End)
    }

    #[inline]
    pub fn justify_end_safe(self) -> Self {
        self.justify_content(JustifyContent::SafeEnd)
    }

    #[inline]
    pub fn justify_evenly(self) -> Self {
        self.justify_content(JustifyContent::SpaceEvenly)
    }

    #[inline]
    pub fn justify_start(self) -> Self {
        self.justify_content(JustifyContent::Start)
    }

    #[inline]
    pub fn justify_start_safe(self) -> Self {
        self.justify_content(JustifyContent::SafeStart)
    }

    #[inline]
    pub fn justify_stretch(self) -> Self {
        self.justify_content(JustifyContent::Stretch)
    }

    #[inline]
    pub fn justify_flex_start(self) -> Self {
        self.justify_content(JustifyContent::FlexStart)
    }

    #[inline]
    pub fn justify_flex_end(self) -> Self {
        self.justify_content(JustifyContent::FlexEnd)
    }

    #[inline]
    pub fn justify_flex_start_safe(self) -> Self {
        self.justify_content(JustifyContent::SafeFlexStart)
    }

    #[inline]
    pub fn justify_flex_end_safe(self) -> Self {
        self.justify_content(JustifyContent::SafeFlexEnd)
    }

    /// 要素間の行・列方向の隙間（gap）を設定します。
    #[inline]
    pub fn gap(mut self, value: impl IntoSize<Val>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.gap = value.into_size();
        inner.mask.set(STYLE_GAP);
        self
    }

    #[inline]
    pub fn gap_0(self) -> Self {
        self.gap(0.0)
    }

    #[inline]
    pub fn gap_auto(self) -> Self {
        self.gap(auto())
    }

    /// 子要素同士の行方向（縦方向、row-gap）の隙間を設定します。
    #[inline]
    pub fn gap_row(self, value: impl Convert<Val>) -> Self {
        let current_column_gap = self.inner.flex_layout.gap.width;
        self.gap((current_column_gap, value.convert()))
    }

    /// 子要素同士の列方向（横方向、column-gap）の隙間を設定します。
    #[inline]
    pub fn gap_col(self, value: impl Convert<Val>) -> Self {
        let current_row_gap = self.inner.flex_layout.gap.height;
        self.gap((value.convert(), current_row_gap))
    }

    /// エイリアス：子要素同士の縦方向の隙間を設定します。
    #[inline]
    pub fn gap_y(self, value: impl Convert<Val>) -> Self {
        self.gap_row(value)
    }

    /// エイリアス：子要素同士の横方向の隙間を設定します。
    #[inline]
    pub fn gap_x(self, value: impl Convert<Val>) -> Self {
        self.gap_col(value)
    }

    /// テキストの配置揃え方向を設定します。
    #[inline]
    pub fn text_align(mut self, value: TextAlign) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.text_align = value;
        inner.mask.set(STYLE_TEXT_ALIGN);
        self
    }

    #[inline]
    pub fn text_center(self) -> Self {
        self.text_align(TextAlign::Center)
    }

    #[inline]
    pub fn text_auto(self) -> Self {
        self.text_align(TextAlign::Auto)
    }

    #[inline]
    pub fn text_left(self) -> Self {
        self.text_align(TextAlign::Left)
    }

    #[inline]
    pub fn text_right(self) -> Self {
        self.text_align(TextAlign::Right)
    }

    /// Flexコンテナ内での主軸の方向を設定します。
    #[inline]
    pub fn flex_direction(mut self, value: FlexDirection) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.flex_direction = value;
        inner.mask.set(STYLE_FLEX_DIRECTION);
        self
    }

    #[inline]
    pub fn flex_col(self) -> Self {
        self.flex_direction(FlexDirection::Column)
    }

    #[inline]
    pub fn flex_col_reverse(self) -> Self {
        self.flex_direction(FlexDirection::ColumnReverse)
    }

    #[inline]
    pub fn flex_row(self) -> Self {
        self.flex_direction(FlexDirection::Row)
    }

    #[inline]
    pub fn flex_row_reverse(self) -> Self {
        self.flex_direction(FlexDirection::RowReverse)
    }

    /// 子要素を複数行に折り返すかどうかを設定します。
    #[inline]
    pub fn flex_wrap_internal(mut self, value: FlexWrap) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.flex_wrap = value;
        inner.mask.set(STYLE_FLEX_WRAP);
        self
    }

    #[inline]
    pub fn flex_wrap(self) -> Self {
        self.flex_wrap_internal(FlexWrap::Wrap)
    }

    #[inline]
    pub fn flex_nowrap(self) -> Self {
        self.flex_wrap_internal(FlexWrap::NoWrap)
    }

    #[inline]
    pub fn flex_wrap_reverse(self) -> Self {
        self.flex_wrap_internal(FlexWrap::WrapReverse)
    }

    /// 子要素の基準となる基本寸法を設定します。
    #[inline]
    pub fn basis(mut self, value: impl Convert<Val>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.flex_basis = value.convert();
        inner.mask.set(STYLE_FLEX_BASIS);
        self
    }

    #[inline]
    pub fn basis_0(self) -> Self {
        self.basis(0.0)
    }

    #[inline]
    pub fn basis_auto(self) -> Self {
        self.basis(auto())
    }

    #[inline]
    pub fn basis_full(self) -> Self {
        self.basis(pct(100.0))
    }

    /// 要素の伸長比率（flex-grow）を直接設定します。
    /// bool（true/false）または数値（f32/i32）を受け入れます。
    #[inline]
    pub fn flex_grow(mut self, value: impl Convert<f32>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.flex_grow = value.convert();
        inner.mask.set(STYLE_FLEX_GROW);
        self
    }

    /// 要素を引き伸ばすように設定します (flex-grow: 1.0)。
    #[inline]
    pub fn grow(self) -> Self {
        self.flex_grow(1.0)
    }

    /// 要素を引き伸ばさないように設定します (flex-grow: 0.0)。
    #[inline]
    pub fn grow_0(self) -> Self {
        self.flex_grow(0.0)
    }

    /// 要素の縮小比率（flex-shrink）を直接設定します。
    /// bool（true/false）または数値（f32/i32）を受け入れます。
    #[inline]
    pub fn flex_shrink(mut self, value: impl Convert<f32>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.flex_shrink = value.convert();
        inner.mask.set(STYLE_FLEX_SHRINK);
        self
    }

    /// 要素を縮小可能に設定します (flex-shrink: 1.0)。
    #[inline]
    pub fn shrink(self) -> Self {
        self.flex_shrink(1.0)
    }

    /// 要素を絶対に縮小させない（サイズを潰さない）ように設定します (flex-shrink: 0.0)。
    #[inline]
    pub fn shrink_0(self) -> Self {
        self.flex_shrink(0.0)
    }

    /// 要素の背景色（Background Color）を設定します。
    #[inline]
    pub fn bg_color(mut self, value: Color) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.bg_color = Some(value);
        inner.mask.set(STYLE_BG_COLOR);
        self
    }

    /// 要素の境界線の色を設定します。
    #[inline]
    pub fn border_color(mut self, value: Color) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.border_color = Some(value);
        inner.mask.set(STYLE_BORDER_COLOR);
        self
    }

    /// 要素の角丸の半径を設定します。
    #[inline]
    pub fn corner_radius(mut self, value: impl IntoCornerRadius) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.corner_radius = Some(value.into_corner_radius());
        inner.mask.set(STYLE_CORNER_RADIUS);
        self
    }

    /// `corner_radius` の短縮エイリアス。要素の角丸を設定します。
    /// 単一値、2連タプル、4連タプルを受け入れます。
    #[inline]
    pub fn rounded(self, value: impl IntoCornerRadius) -> Self {
        self.corner_radius(value)
    }

    /// `corner_radius` の超短縮エイリアス。要素の角丸を設定します。
    #[inline]
    pub fn r(self, value: impl IntoCornerRadius) -> Self {
        self.corner_radius(value)
    }

    /// 要素を完全なサークル（またはカプセル型、Tailwind CSS の rounded-full 相当）にします。
    #[inline]
    pub fn rounded_full(self) -> Self {
        self.corner_radius(9999.0)
    }

    /// 上半分の角（top-left, top-right）にのみ角丸を設定します。
    #[inline]
    pub fn rounded_top(self, value: impl Convert<f32>) -> Self {
        let val = value.convert();
        let current = self
            .inner
            .visual_property
            .corner_radius
            .unwrap_or(CornerRadius::ZERO);
        self.corner_radius((val, val, current.bottom_right, current.bottom_left))
    }

    /// 下半分の角（bottom-left, bottom-right）にのみ角丸を設定します。
    #[inline]
    pub fn rounded_bottom(self, value: impl Convert<f32>) -> Self {
        let val = value.convert();
        let current = self
            .inner
            .visual_property
            .corner_radius
            .unwrap_or(CornerRadius::ZERO);
        self.corner_radius((current.top_left, current.top_right, val, val))
    }

    /// 左半分の角（top-left, bottom-left）にのみ角丸を設定します。
    #[inline]
    pub fn rounded_left(self, value: impl Convert<f32>) -> Self {
        let val = value.convert();
        let current = self
            .inner
            .visual_property
            .corner_radius
            .unwrap_or(CornerRadius::ZERO);
        self.corner_radius((val, current.top_right, current.bottom_right, val))
    }

    /// 右半分の角（top-right, bottom-right）にのみ角丸を設定します。
    #[inline]
    pub fn rounded_right(self, value: impl Convert<f32>) -> Self {
        let val = value.convert();
        let current = self
            .inner
            .visual_property
            .corner_radius
            .unwrap_or(CornerRadius::ZERO);
        self.corner_radius((current.top_left, val, val, current.bottom_left))
    }

    /// 要素全体の不透明度を設定します。
    #[inline]
    pub fn opacity(mut self, value: f32) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.opacity = Some(value);
        inner.mask.set(STYLE_OPACITY);
        self
    }

    #[inline]
    pub fn opacity_0(self) -> Self {
        self.opacity(0.0)
    }

    #[inline]
    pub fn opacity_50(self) -> Self {
        self.opacity(0.5)
    }

    #[inline]
    pub fn opacity_100(self) -> Self {
        self.opacity(1.0)
    }

    /// 要素の外側に配置する影を設定します。
    #[inline]
    pub fn box_shadow(mut self, value: BoxShadow) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.shadow_params = Some(value);
        inner.visual_property.shadow_color = Some(value.color);
        inner.mask.set(STYLE_BOX_SHADOW);
        self
    }

    /// 影の色（shadow_color）のみを設定・上書きします。
    #[inline]
    pub fn shadow_color(mut self, value: Color) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.shadow_color = Some(value);
        inner.mask.set(STYLE_BOX_SHADOW);
        self
    }

    /// 重なり順（Z-Index）を整数で設定します。
    #[inline]
    pub fn z_index(mut self, value: i32) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.z_index = Some(value);
        inner.mask.set(STYLE_Z_INDEX);
        self
    }

    #[inline]
    pub fn z(self, value: i32) -> Self {
        self.z_index(value)
    }

    #[inline]
    pub fn z_neg_1(self) -> Self {
        self.z_index(-1)
    }

    #[inline]
    pub fn z_0(self) -> Self {
        self.z_index(0)
    }

    #[inline]
    pub fn z_1(self) -> Self {
        self.z_index(1)
    }

    /// この要素の上にマウスが乗った際のマウスクラスアイコンを設定します。
    #[inline]
    pub fn cursor(mut self, value: CursorIcon) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.cursor = Some(value);
        inner.mask.set(STYLE_CURSOR);
        self
    }

    #[inline]
    pub fn cursor_default(self) -> Self {
        self.cursor(CursorIcon::Default)
    }

    #[inline]
    pub fn cursor_grab(self) -> Self {
        self.cursor(CursorIcon::Grab)
    }

    #[inline]
    pub fn cursor_grabbing(self) -> Self {
        self.cursor(CursorIcon::Grabbing)
    }

    #[inline]
    pub fn cursor_not_allowed(self) -> Self {
        self.cursor(CursorIcon::NotAllowed)
    }

    #[inline]
    pub fn cursor_pointer(self) -> Self {
        self.cursor(CursorIcon::Pointer)
    }

    #[inline]
    pub fn cursor_text(self) -> Self {
        self.cursor(CursorIcon::Text)
    }

    #[inline]
    pub fn backdrop(mut self, backdrop: Backdrop) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.backdrop = backdrop;
        inner.mask.set(STYLE_BACKDROP);
        self
    }

    #[inline]
    pub fn backdrop_acrylic(self) -> Self {
        self.backdrop(Backdrop::Acrylic)
    }

    #[inline]
    pub fn backdrop_mica(self) -> Self {
        self.backdrop(Backdrop::Mica)
    }

    #[inline]
    pub fn backdrop_mica_alt(self) -> Self {
        self.backdrop(Backdrop::MicaAlt)
    }

    #[inline]
    pub fn backdrop_none(self) -> Self {
        self.backdrop(Backdrop::None)
    }

    /// 要素内でレンダリングされるテキストの基本色を設定します。
    #[inline]
    pub fn text_color(mut self, value: Color) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.text_color = Some(value);
        inner.mask.set(STYLE_TEXT_COLOR);
        self
    }

    /// グリッドの行方向の明示的なトラックサイズ定義を設定します。
    #[inline]
    pub fn grid_template_rows(mut self, value: Vec<GridTemplateComponent<String>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
        grid.grid_template_rows = value;
        inner.mask.set(STYLE_GRID_LAYOUT);
        self
    }

    /// グリッドの列方向の明示的なトラックサイズ定義を設定します。
    #[inline]
    pub fn grid_template_columns(mut self, value: Vec<GridTemplateComponent<String>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
        grid.grid_template_columns = value;
        inner.mask.set(STYLE_GRID_LAYOUT);
        self
    }

    /// 暗黙的に生成されるグリッド行のデフォルトサイズを設定します。
    #[inline]
    pub fn grid_auto_rows(mut self, value: Vec<TrackSizingFunction>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
        grid.grid_auto_rows = value;
        inner.mask.set(STYLE_GRID_LAYOUT);
        self
    }

    /// 暗黙的に生成されるグリッド列のデフォルトサイズを設定します。
    #[inline]
    pub fn grid_auto_columns(mut self, value: Vec<TrackSizingFunction>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
        grid.grid_auto_columns = value;
        inner.mask.set(STYLE_GRID_LAYOUT);
        self
    }

    /// 自動配置アルゴリズムの制御方法を設定します。
    #[inline]
    pub fn grid_auto_flow(mut self, value: GridAutoFlow) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
        grid.grid_auto_flow = value;
        inner.mask.set(STYLE_GRID_LAYOUT);
        self
    }

    /// 名前付きグリッドエリアを定義して配置を決定します。
    #[inline]
    pub fn grid_template_areas(mut self, value: Vec<GridTemplateArea<String>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
        grid.grid_template_areas = value;
        inner.mask.set(STYLE_GRID_LAYOUT);
        self
    }

    /// 明示的に定義された各グリッド列線に対する名前のリストを設定します。
    #[inline]
    pub fn grid_template_column_names(mut self, value: Vec<Vec<String>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
        grid.grid_template_column_names = value;
        inner.mask.set(STYLE_GRID_LAYOUT);
        self
    }

    /// 明示的に定義された各グリッド行線に対する名前のリストを設定します。
    #[inline]
    pub fn grid_template_row_names(mut self, value: Vec<Vec<String>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
        grid.grid_template_row_names = value;
        inner.mask.set(STYLE_GRID_LAYOUT);
        self
    }

    /// グリッドアイテムが配置される行の開始位置と終了位置を指定します。
    #[inline]
    pub fn grid_row(mut self, value: GridLine<GridPlacement<String>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
        grid.grid_row = value;
        inner.mask.set(STYLE_GRID_LAYOUT);
        self
    }

    /// グリッドアイテムが配置される列の開始位置と終了位置を指定します。
    #[inline]
    pub fn grid_column(mut self, value: GridLine<GridPlacement<String>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
        grid.grid_column = value;
        inner.mask.set(STYLE_GRID_LAYOUT);
        self
    }

    /// マウスが要素の上に乗った（Hover）際に適用するオーバーライドスタイルを設定します。
    #[inline]
    pub fn hovered(mut self, style: ThisStyle) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.interaction_styles.hovered = Some(style);
        inner.mask.set(STATE_HOVERED);
        self
    }

    /// キーボードタブ移動などで要素にフォーカスが当たった際に適用するスタイルを設定します。
    #[inline]
    pub fn focused(mut self, style: ThisStyle) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.interaction_styles.focused = Some(style);
        inner.mask.set(STATE_FOCUSED);
        self
    }

    /// マウスの左ボタンが要素の上で押し下げられた際、またはタップ中に適用するスタイルを設定します。
    #[inline]
    pub fn pressed(mut self, style: ThisStyle) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.interaction_styles.pressed = Some(style);
        inner.mask.set(STATE_PRESSED);
        self
    }

    /// 要素が無効化された際に適用するスタイルを設定します。
    #[inline]
    pub fn disabled(mut self, style: ThisStyle) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.interaction_styles.disabled = Some(style);
        inner.mask.set(STATE_DISABLED);
        self
    }

    /// 要素がアクティブ状態の時に適用するスタイルを設定します。
    #[inline]
    pub fn actived(mut self, style: ThisStyle) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.interaction_styles.actived = Some(style);
        inner.mask.set(STATE_ACTIVED);
        self
    }

    /// 要素がトグル選択された際に適用するスタイルを設定します。
    #[inline]
    pub fn selected(mut self, style: ThisStyle) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.interaction_styles.selected = Some(style);
        inner.mask.set(STATE_SELECTED);
        self
    }

    /// 要素が現在ドラッグ操作中にある際に適用するスタイルを設定します。
    #[inline]
    pub fn dragged(mut self, style: ThisStyle) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.interaction_styles.dragged = Some(style);
        inner.mask.set(STATE_DRAGGED);
        self
    }

    /// 子孫要素のインタラクション状態に連動して親のスタイルを変化させる伝播設定
    #[inline]
    pub fn interaction_within(mut self, name: InteractionName, style: ThisStyle) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        match name {
            InteractionName::Hover => inner.interaction_styles.hovered_within = Some(style),
            InteractionName::Focus => inner.interaction_styles.focused_within = Some(style),
            InteractionName::Press => inner.interaction_styles.pressed_within = Some(style),
            InteractionName::Disable => inner.interaction_styles.disabled_within = Some(style),
            InteractionName::Active => inner.interaction_styles.actived_within = Some(style),
            InteractionName::Select => inner.interaction_styles.selected_within = Some(style),
            InteractionName::Drag => inner.interaction_styles.dragged_within = Some(style),
            InteractionName::All => inner.interaction_styles.any_within = Some(style),
        }
        // 動的withinプロパティがこのスタイルに格納されていることをビットマーク
        inner.mask.set(STYLE_INTERACTION_WITHIN);
        self
    }

    #[inline]
    pub fn hover_within(self, style: ThisStyle) -> Self {
        self.interaction_within(InteractionName::Hover, style)
    }

    #[inline]
    pub fn focus_within(self, style: ThisStyle) -> Self {
        self.interaction_within(InteractionName::Focus, style)
    }

    #[inline]
    pub fn press_within(self, style: ThisStyle) -> Self {
        self.interaction_within(InteractionName::Press, style)
    }

    #[inline]
    pub fn disable_within(self, style: ThisStyle) -> Self {
        self.interaction_within(InteractionName::Disable, style)
    }

    #[inline]
    pub fn active_within(self, style: ThisStyle) -> Self {
        self.interaction_within(InteractionName::Active, style)
    }

    #[inline]
    pub fn select_within(self, style: ThisStyle) -> Self {
        self.interaction_within(InteractionName::Select, style)
    }

    #[inline]
    pub fn drag_within(self, style: ThisStyle) -> Self {
        self.interaction_within(InteractionName::Drag, style)
    }

    #[inline]
    pub fn all_within(self, style: ThisStyle) -> Self {
        self.interaction_within(InteractionName::All, style)
    }

    /// ポインターメッセージ（マウスインタラクションなど）の透過を制御します。
    #[inline]
    pub fn pointer_events(mut self, value: PointerEvents) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.pointer_events = Some(value);
        inner.mask.set(STYLE_POINTER_EVENTS);
        self
    }

    /// 要素がマウスインタラクションを無視し、背後にある要素へイベントを透過させます。
    #[inline]
    pub fn pointer_events_none(self) -> Self {
        self.pointer_events(PointerEvents::None)
    }

    /// 要素が通常通りマウスインタラクションを受け取ります（デフォルト）。
    #[inline]
    pub fn pointer_events_auto(self) -> Self {
        self.pointer_events(PointerEvents::Auto)
    }

    /// 要素にアフィン変換（平行移動・拡大・回転）を適用します。
    #[inline]
    pub fn transform(mut self, value: Transform) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.transform = Some(value.matrix);
        inner.mask.set(STYLE_TRANSFORM);
        self
    }

    #[inline]
    pub fn transform_scale(self, x: f32, y: f32) -> Self {
        let current = self
            .inner
            .visual_property
            .transform
            .map(|m| Transform { matrix: m })
            .unwrap_or_default();
        self.transform(current.scale(x, y))
    }

    #[inline]
    pub fn transform_translate(self, x: f32, y: f32) -> Self {
        let current = self
            .inner
            .visual_property
            .transform
            .map(|m| Transform { matrix: m })
            .unwrap_or_default();
        self.transform(current.translate(x, y))
    }

    #[inline]
    pub fn transform_rotate(self, radians: f32) -> Self {
        let current = self
            .inner
            .visual_property
            .transform
            .map(|m| Transform { matrix: m })
            .unwrap_or_default();
        self.transform(current.rotate(radians))
    }

    #[inline]
    pub fn transform_origin(mut self, point: Point<f32>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.transform_origin = Some(point);
        inner.mask.set(STYLE_TRANSFORM);
        self
    }

    /// 状態遷移時のトランジション（CSS transition）を設定します。
    #[inline]
    pub fn transition(mut self, transition: Transition) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.transitions.push(transition);
        inner.mask.set(STYLE_TRANSITIONS);
        self
    }

    #[inline]
    pub fn trans_bg_color(self, duration: Duration, curve: AnimationCurve) -> Self {
        self.transition(Transition::new(
            PropertyList::BackgroundColor,
            duration,
            curve,
        ))
    }

    #[inline]
    pub fn trans_border_color(self, duration: Duration, curve: AnimationCurve) -> Self {
        self.transition(Transition::new(PropertyList::BorderColor, duration, curve))
    }

    #[inline]
    pub fn trans_box_shadow(self, duration: Duration, curve: AnimationCurve) -> Self {
        self.transition(Transition::new(PropertyList::BoxShadow, duration, curve))
    }

    #[inline]
    pub fn trans_corder_radius(self, duration: Duration, curve: AnimationCurve) -> Self {
        self.transition(Transition::new(PropertyList::CornerRadius, duration, curve))
    }

    #[inline]
    pub fn trans_opacity(self, duration: Duration, curve: AnimationCurve) -> Self {
        self.transition(Transition::new(PropertyList::Opacity, duration, curve))
    }

    #[inline]
    pub fn trans_transform(self, duration: Duration, curve: AnimationCurve) -> Self {
        self.transition(Transition::new(PropertyList::Transform, duration, curve))
    }

    #[inline]
    pub fn trans_size(self, duration: Duration, curve: AnimationCurve) -> Self {
        self.transition(Transition::new(PropertyList::Size, duration, curve))
    }

    #[inline]
    pub fn trans_width(self, duration: Duration, curve: AnimationCurve) -> Self {
        self.transition(Transition::new(PropertyList::Width, duration, curve))
    }

    #[inline]
    pub fn trans_height(self, duration: Duration, curve: AnimationCurve) -> Self {
        self.transition(Transition::new(PropertyList::Height, duration, curve))
    }

    /// キーフレームアニメーション（CSS animation）を設定します。
    #[inline]
    pub fn animation(mut self, animation: KeyframeAnimation) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.keyframe_animations.push(animation);
        inner.mask.set(STYLE_ANIMATIONS);
        self
    }

    /// 背景に 2色線形グラデーションを適用します。
    #[inline]
    pub fn bg_gradient(mut self, gradient: LinearGradient) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.bg_gradient = Some(gradient);
        inner.mask.set(STYLE_BG_COLOR); // 背景描画トリガーとしてマーク
        self
    }

    /// テキストのフォントファミリーを設定します。（例: font_family("Arial")）
    #[inline]
    pub fn font_family(mut self, family: impl Into<Cow<'static, str>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.font_family = Some(family.into());
        inner.mask.set(STYLE_EXT_PROPERTIES);
        self
    }

    /// テキストの太さを設定します（100 〜 900。標準は 400、ボールドは 700）。
    #[inline]
    pub fn font_weight(mut self, weight: u32) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.font_weight = Some(weight);
        inner.mask.set(STYLE_EXT_PROPERTIES);
        self
    }

    /// 要素内でレンダリングされるテキストの基本フォントサイズを設定します。
    #[inline]
    pub fn font_size(mut self, size: f32) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.font_size = Some(size);
        inner.mask.set(STYLE_EXT_PROPERTIES);
        self
    }

    /// フォントスタイルを設定します
    #[inline]
    pub fn font_style(mut self, style: u32) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.font_style = Some(style);
        inner.mask.set(STYLE_EXT_PROPERTIES);
        self
    }

    /// ユーザーによるテキスト選択・コピーの挙動を設定します
    #[inline]
    pub fn user_select(mut self, value: UserSelect) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.user_select = Some(value);
        inner.mask.set(STYLE_USER_SELECT);
        self
    }

    /// テキストのドラッグ範囲選択を許可します (user-select: text 相当)
    #[inline]
    pub fn select_text(self) -> Self {
        self.user_select(UserSelect::Text)
    }

    /// テキストを全選択します
    #[inline]
    pub fn select_all(self) -> Self {
        self.user_select(UserSelect::All)
    }

    /// テキストの範囲選択を禁止します
    #[inline]
    pub fn select_none(self) -> Self {
        self.user_select(UserSelect::None)
    }

    #[inline]
    pub fn select_bg_color(mut self, color: Color) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.select_bg_color = Some(color);
        inner.mask.set(STYLE_USER_SELECT);
        self
    }

    #[inline]
    pub fn select_text_color(mut self, color: Color) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.select_text_color = Some(color);
        inner.mask.set(STYLE_USER_SELECT);
        self
    }
}

/// アニメーションのイージングカーブを定義する列挙型。
/// 軽量なため Clone と Copy が可能です。
#[derive(Debug, Clone, Copy)]
pub enum AnimationCurve {
    /// イージングなし（線形 / リニア）
    Linear,
    /// 加減速をかける標準的な2次イージング
    EaseInOutQuad,
    /// 加速（2次）
    EaseInQuad,
    /// 減速（2次）
    EaseOutQuad,
    /// ユーザーが独自のイージング計算（0.0～1.0 を受け取り 0.0～1.0 を返す）を行えるエスケープハッチ
    Custom(fn(f32) -> f32),
}

impl AnimationCurve {
    /// 経過割合 t (0.0 <= t <= 1.0) に基づいて、イージングされた値を評価します
    pub fn evaluate(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match *self {
            AnimationCurve::Linear => t,
            AnimationCurve::EaseInQuad => t * t,
            AnimationCurve::EaseOutQuad => t * (2.0 - t),
            AnimationCurve::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
            AnimationCurve::Custom(f) => f(t),
        }
    }
}

impl PartialEq for AnimationCurve {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Linear, Self::Linear) => true,
            (Self::EaseInOutQuad, Self::EaseInOutQuad) => true,
            (Self::EaseInQuad, Self::EaseInQuad) => true,
            (Self::EaseOutQuad, Self::EaseOutQuad) => true,
            (Self::Custom(f1), Self::Custom(f2)) => std::ptr::fn_addr_eq(*f1, *f2),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackCount {
    Infinite,
    Count(u32),
}

/// CSS Animation 相当の設定を定義
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyframeAnimation {
    /// 何をアニメーションさせるか
    pub property: PropertyList,
    /// 1周（ループ）にかかる時間
    pub duration: Duration,
    /// ループ回数
    pub iteration_count: PlaybackCount,
    /// イージングカーブ
    pub curve: AnimationCurve,
}

impl KeyframeAnimation {
    #[inline]
    pub fn new(
        property: PropertyList,
        duration: Duration,
        iteration_count: PlaybackCount,
        curve: AnimationCurve,
    ) -> Self {
        Self {
            property,
            duration,
            iteration_count,
            curve,
        }
    }
}

#[cfg(test)]
mod tests;
