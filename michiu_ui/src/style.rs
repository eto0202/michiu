use crate::{
    AlignContent, AlignItems, AlignSelf, AnimationCurve, Backdrop, BasicLayout, BorderAlignment,
    BorderStyle, BoxShadow, BoxSizing, Color, ComponentMask, Context, Convert, CornerRadius,
    CursorIcon, Direction, Display, DndDragPayload, DndDragPlaceholderParent, DndDragProperty,
    DndDropProperty, DndDropTarget, EdgeInsets, EntityId, FlexDirection, FlexLayout, FlexWrap,
    FocusTrigger, Focusable, GlobalCursorIcon, GridAutoFlow, GridLayout, GridLine, GridPlacement,
    InteractionName, InteractionStyles, IntoStyleConvert, IntoStyleCornerRadius, IntoStylePoint,
    IntoStyleRect, IntoStyleResizable, IntoStyleSize, IntoStyleValue, JustifyContent,
    KeyframeAnimation, LayoutOverflow, LayoutPoint, Length, LinearGradient, MichiuSoA, Overflow,
    PointerEvents, Position, PropertyList, Rect, ScrollbarDisplay, ScrollbarMode, ScrollbarStyle,
    TargetStyle, TextAlign, Transform, Transition, UserSelect, Val, VisualProperty, auto, pct,
};
use std::{borrow::Cow, sync::Arc, time::Duration};
use taffy::{GridTemplateArea, GridTemplateComponent, TrackSizingFunction};

// スレッド安全な動的クロージャをサポートする `StyleValue` の定義
pub enum StyleValue<T> {
    Static(T),
    Dynamic(Box<dyn Fn() -> T + Send + Sync + 'static>),
}

/// For detailed information on the behavior of each property and its correspondence with CSS,
/// please refer to the documentation for [`taffy::Style`] used internally.
#[derive(Debug, Clone, Default)]
pub struct ThisStyle {
    pub(crate) inner: Arc<StyleInner>,
}

#[derive(Clone, Default)]
pub struct StyleInner {
    pub mask: ComponentMask,
    pub basic_layout: BasicLayout,
    pub flex_layout: FlexLayout,
    pub grid_layout: Option<GridLayout>,
    pub visual_property: VisualProperty,
    pub interaction_styles: InteractionStyles,
    pub scrollbar_style: Option<ScrollbarStyle>,
    // 動的にスタイルプロパティを更新するためのクローン可能なセッターリスト
    pub dynamic_setters: DynamicSettersType,

    pub drag_property: Option<DndDragProperty>,
    pub drop_property: Option<DndDropProperty>,
}

impl StyleInner {
    #[inline]
    pub(crate) fn apply_visual_property(&self, target: &mut TargetStyle) {
        self.visual_property
            .apply_visual_property(target, self.mask);
    }
}

type DynamicSettersType = Vec<Arc<dyn Fn(&mut Context, EntityId, StyleTarget) + Send + Sync>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleTarget {
    Base,
    Hovered,
    Focused,
    FocusedVisible,
    Pressed,
    Disabled,
    Actived,
    Selected,
    Dragged,
    DndDragging, // ドラッグ中の元の要素
    DndDragIn,   // ドロップゾーン侵入時
    DndDragOver, // プレースホルダー（ドラッグイメージ）

    HoveredWithin,
    FocusedWithin,
    FocusedVisibleWithin,
    PressedWithin,
    DisabledWithin,
    ActivedWithin,
    SelectedWithin,
    DraggedWithin,
    AnyWithin,

    HoveredParent,
    FocusedParent,
    FocusedVisibleParent,
    PressedParent,
    DisabledParent,
    ActivedParent,
    SelectedParent,
    DraggedParent,
    AnyParent,
}

// Debug トレイトの手動実装 (クロージャを含むため)
impl std::fmt::Debug for StyleInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StyleInner")
            .field("mask", &self.mask)
            .field("basic_layout", &self.basic_layout)
            .field("flex_layout", &self.flex_layout)
            .field("grid_layout", &self.grid_layout)
            .field("visual_property", &self.visual_property)
            .field("interaction_styles", &self.interaction_styles)
            .field("scrollbar_style", &self.scrollbar_style)
            // dynamic_setters はデバッグ表示が難しいため除外
            .field("drag_property", &self.drag_property)
            .field("drop_property", &self.drop_property)
            .finish_non_exhaustive()
    }
}

// ComponentMask, BasicLayout, FlexLayout, GridLayout, VisualProperty はすべて単純なデータ構造でありスレッド安全
// InteractionStyles が持つ ThisStyle も内部は Arc で管理
// スレッド固有の生ポインタ、内部可変性コンテナ、非アトミックな参照カウントは一切含まれていない
// 値の書き換えは常に Arc::make_mut を通じて排他参照が得られたときのみ行われるためデータ競合のリスクはない
unsafe impl Send for StyleInner {}
unsafe impl Sync for StyleInner {}

impl ThisStyle {
    /// Creates a new style.
    ///
    /// Prefer using [`crate::ts`] functions for a cleaner syntax.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the border to a width 1.0, solid and Red.
    #[inline]
    #[must_use]
    pub fn debug_border_red(self) -> Self {
        self.border_solid(1.0).border_color(Color::RED)
    }

    /// Set the border to a width 1.0, solid and Bule.
    #[inline]
    #[must_use]
    pub fn debug_border_blue(self) -> Self {
        self.border_solid(1.0).border_color(Color::BLUE)
    }

    /// Set the border to a width 1.0, solid and Green.
    #[inline]
    #[must_use]
    pub fn debug_border_green(self) -> Self {
        self.border_solid(1.0).border_color(Color::GREEN)
    }

    /// Set the border to a width 1.0, solid and Yellow.
    #[inline]
    #[must_use]
    pub fn debug_border_yellow(self) -> Self {
        self.border_solid(1.0).border_color(Color::YELLOW)
    }

    /// Set the [`Display`] (Block, Flex, Grid, None).
    ///
    /// Default is [`Display::Flex`].
    #[inline]
    #[must_use]
    pub fn display(mut self, value: impl IntoStyleValue<Display>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.display = v;
                inner.mask.set(ComponentMask::STYLE_DISPLAY);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_DISPLAY);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        cx.get_basic_layout_mut(id, target).display = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Set the display to a hidden ([`Display::None`]).
    #[inline]
    #[must_use]
    pub fn hidden(self) -> Self {
        self.display(Display::None)
    }

    /// Set the display to a [`Display::Flex`].
    #[inline]
    #[must_use]
    pub fn flex(self) -> Self {
        self.display(Display::Flex)
    }

    /// Set the display to a [`Display::Grid`].
    #[inline]
    #[must_use]
    pub fn grid(self) -> Self {
        self.display(Display::Grid)
    }

    /// Set the display to a [`Display::Block`].
    #[inline]
    #[must_use]
    pub fn block(self) -> Self {
        self.display(Display::Block)
    }

    /// Normally, you don't need to change this value.
    ///
    /// This is an internal flag that specifies whether a child element of a block layout is a table element.
    ///
    /// See the Taffy documentation for detailed specifications.
    ///
    /// [`taffy::Style::item_is_table`]
    #[inline]
    #[must_use]
    pub fn item_is_table(mut self, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.item_is_table = v;
                inner.mask.set(ComponentMask::STYLE_ITEM_IS_TABLE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_ITEM_IS_TABLE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        cx.get_basic_layout_mut(id, target).item_is_table = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Set whether it is a replacement element.
    ///
    /// [`taffy::Style::item_is_replaced`]
    #[inline]
    #[must_use]
    pub fn item_is_replaced(mut self, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.item_is_replaced = v;
                inner.mask.set(ComponentMask::STYLE_ITEM_IS_REPLACED);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_ITEM_IS_REPLACED);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        cx.get_basic_layout_mut(id, target).item_is_replaced = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// This sets the basis for calculating element sizes ([`BoxSizing`]).
    ///
    /// Default is [`BoxSizing::BorderBox`].
    #[inline]
    #[must_use]
    pub fn box_sizing(mut self, value: impl IntoStyleValue<BoxSizing>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.box_sizing = v;
                inner.mask.set(ComponentMask::STYLE_BOX_SIZING);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BOX_SIZING);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        cx.get_basic_layout_mut(id, target).box_sizing = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Set the box sizing to a [`BoxSizing::BorderBox`].
    #[inline]
    #[must_use]
    pub fn box_border(self) -> Self {
        self.box_sizing(BoxSizing::BorderBox)
    }

    /// Set the box sizing to a [`BoxSizing::ContentBox`].
    #[inline]
    #[must_use]
    pub fn box_content(self) -> Self {
        self.box_sizing(BoxSizing::ContentBox)
    }

    /// Sets the inline direction for text and elements.
    ///
    /// Default is [`Direction::Ltr`]
    #[inline]
    #[must_use]
    pub fn direction(mut self, value: impl IntoStyleValue<Direction>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.direction = v;
                inner.mask.set(ComponentMask::STYLE_DIRECTION);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_DIRECTION);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        cx.get_basic_layout_mut(id, target).direction = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Register the scrollbar style ([`ScrollbarStyle`]).
    #[inline]
    #[must_use]
    pub fn scrollbar(mut self, style: impl IntoStyleValue<ScrollbarStyle>) -> Self {
        match style.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.scrollbar_style = Some(v);
                inner.mask.set(ComponentMask::STYLE_SCROLLBAR);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_SCROLLBAR);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.scrollbar.bar_styles.find_mut(id) {
                            v.style = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Directly sets the width of the scrollbar.
    #[inline]
    #[must_use]
    pub fn scrollbar_width(mut self, width: impl IntoStyleValue<f32>) -> Self {
        match width.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let sb = inner
                    .scrollbar_style
                    .get_or_insert_with(ScrollbarStyle::default);
                sb.width = v;
                inner.mask.set(ComponentMask::STYLE_SCROLLBAR);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_SCROLLBAR);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.scrollbar.bar_styles.find_mut(id) {
                            v.style.width = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Directly sets the display (None, Always, Auto, Transient) of the scroll bar.
    ///
    /// Default is [`ScrollbarDisplay::Auto`]
    #[inline]
    #[must_use]
    pub fn scrollbar_display(mut self, display: impl IntoStyleValue<ScrollbarDisplay>) -> Self {
        match display.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let sb = inner
                    .scrollbar_style
                    .get_or_insert_with(ScrollbarStyle::default);
                sb.display = v;
                inner.mask.set(ComponentMask::STYLE_SCROLLBAR);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_SCROLLBAR);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.scrollbar.bar_styles.find_mut(id) {
                            v.style.display = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Set the scrollbar display to a [`ScrollbarDisplay::Auto`].
    #[inline]
    #[must_use]
    pub fn scrollbar_auto(self) -> Self {
        self.scrollbar_display(ScrollbarDisplay::Auto)
    }

    /// Set the scrollbar display to a [`ScrollbarDisplay::None`].
    #[inline]
    #[must_use]
    pub fn scrollbar_none(self) -> Self {
        self.scrollbar_display(ScrollbarDisplay::None)
    }

    /// Set the scrollbar display to a [`ScrollbarDisplay::Always`].
    #[inline]
    #[must_use]
    pub fn scrollbar_always(self) -> Self {
        self.scrollbar_display(ScrollbarDisplay::Always)
    }

    /// Set the scrollbar display to a [`ScrollbarDisplay::Transient`].
    #[inline]
    #[must_use]
    pub fn scrollbar_transient(self) -> Self {
        self.scrollbar_display(ScrollbarDisplay::Transient)
    }

    /// Directly sets the mode (Layout, Overlay) of the scrollbar.
    ///
    /// Default is [`ScrollbarMode::Overlay`]
    #[inline]
    #[must_use]
    pub fn scrollbar_mode(mut self, mode: impl IntoStyleValue<ScrollbarMode>) -> Self {
        match mode.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let sb = inner
                    .scrollbar_style
                    .get_or_insert_with(ScrollbarStyle::default);
                sb.mode = v;
                inner.mask.set(ComponentMask::STYLE_SCROLLBAR);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_SCROLLBAR);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.scrollbar.bar_styles.find_mut(id) {
                            v.style.mode = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// This setting determines how child elements are displayed when they extend beyond the element's boundaries.
    ///
    /// ```rust
    /// pub struct LayoutOverflow {
    ///    pub x: Overflow,
    ///    pub y: Overflow,
    /// }
    /// ```
    ///
    /// Default is [`Overflow::Visible`]
    #[inline]
    #[must_use]
    pub fn overflow(mut self, value: impl IntoStyleValue<LayoutOverflow>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.overflow = v;
                inner.mask.set(ComponentMask::STYLE_OVERFLOW);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_OVERFLOW);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        cx.get_basic_layout_mut(id, target).overflow = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Sets the overflow to a auto ([`Overflow::Visible`]).
    #[inline]
    #[must_use]
    pub fn overflow_auto(self) -> Self {
        self.overflow(LayoutOverflow {
            x: Overflow::Visible,
            y: Overflow::Visible,
        })
    }

    /// Sets the overflow to a [`Overflow::Hidden`].
    #[inline]
    #[must_use]
    pub fn overflow_hidden(self) -> Self {
        self.overflow(LayoutOverflow {
            x: Overflow::Hidden,
            y: Overflow::Hidden,
        })
    }

    /// Sets the overflow to a [`Overflow::Scroll`].
    #[inline]
    #[must_use]
    pub fn overflow_scroll(self) -> Self {
        self.overflow(LayoutOverflow {
            x: Overflow::Scroll,
            y: Overflow::Scroll,
        })
    }

    /// Sets the overflow to a [`Overflow::Clip`].
    #[inline]
    #[must_use]
    pub fn overflow_clip(self) -> Self {
        self.overflow(LayoutOverflow {
            x: Overflow::Clip,
            y: Overflow::Clip,
        })
    }

    /// Set the X-axis overflow (Visible, Hidden, Scroll, Clip) individually ([`Overflow`]).
    ///
    /// Default is [`Overflow::Visible`]
    #[inline]
    #[must_use]
    pub fn overflow_x(mut self, value: impl IntoStyleValue<Overflow>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.overflow.x = v;
                inner.mask.set(ComponentMask::STYLE_OVERFLOW);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_OVERFLOW);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        cx.get_basic_layout_mut(id, target).overflow.x = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Set the Y-axis overflow (Visible, Hidden, Scroll, Clip) individually ([`Overflow`]).
    ///
    /// Default is [`Overflow::Visible`]
    #[inline]
    #[must_use]
    pub fn overflow_y(mut self, value: impl IntoStyleValue<Overflow>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.overflow.y = v;
                inner.mask.set(ComponentMask::STYLE_OVERFLOW);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_OVERFLOW);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        cx.get_basic_layout_mut(id, target).overflow.y = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Set the X-axis overflow to a auto ([`Overflow::Visible`]).
    #[inline]
    #[must_use]
    pub fn overflow_x_auto(self) -> Self {
        self.overflow_x(Overflow::Visible)
    }

    /// Set the X-axis overflow to a [`Overflow::Hidden`].
    #[inline]
    #[must_use]
    pub fn overflow_x_hidden(self) -> Self {
        self.overflow_x(Overflow::Hidden)
    }

    /// Set the X-axis overflow to a [`Overflow::Scroll`].
    #[inline]
    #[must_use]
    pub fn overflow_x_scroll(self) -> Self {
        self.overflow_x(Overflow::Scroll)
    }

    /// Set the X-axis overflow to a [`Overflow::Clip`].
    #[inline]
    #[must_use]
    pub fn overflow_x_clip(self) -> Self {
        self.overflow_x(Overflow::Clip)
    }

    /// Set the Y-axis overflow to a auto ([`Overflow::Visible`]).
    #[inline]
    #[must_use]
    pub fn overflow_y_auto(self) -> Self {
        self.overflow_y(Overflow::Visible)
    }

    /// Set the Y-axis overflow to a [`Overflow::Hidden`].
    #[inline]
    #[must_use]
    pub fn overflow_y_hidden(self) -> Self {
        self.overflow_y(Overflow::Hidden)
    }

    /// Set the Y-axis overflow to a [`Overflow::Scroll`].
    #[inline]
    #[must_use]
    pub fn overflow_y_scroll(self) -> Self {
        self.overflow_y(Overflow::Scroll)
    }

    /// Set the Y-axis overflow to a [`Overflow::Clip`].
    #[inline]
    #[must_use]
    pub fn overflow_y_clip(self) -> Self {
        self.overflow_y(Overflow::Clip)
    }

    /// Sets the position (Relative, Absolute) for elements.
    ///
    /// Default is [`Position::Relative`]
    #[inline]
    #[must_use]
    pub fn position(mut self, value: impl IntoStyleValue<Position>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.position = v;
                inner.mask.set(ComponentMask::STYLE_POSITION);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_POSITION);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        cx.get_basic_layout_mut(id, target).position = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Set the position to a [`Position::Absolute`].
    #[inline]
    #[must_use]
    pub fn absolute(self) -> Self {
        self.position(Position::Absolute)
    }

    /// Set the position to a [`Position::Relative`].
    #[inline]
    #[must_use]
    pub fn relative(self) -> Self {
        self.position(Position::Relative)
    }

    /// Sets the positioning inset (top, right, bottom, left).
    #[inline]
    #[must_use]
    pub fn inset(mut self, value: impl IntoStyleRect<Val>) -> Self {
        match value.into_style_rect() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.inset = v;
                inner.mask.set(ComponentMask::STYLE_INSET);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_INSET);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).inset = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Set the element to absolute positioning ([`Position::Absolute`]) and simultaneously set the position inset.
    #[inline]
    #[must_use]
    pub fn absolute_inset(self, value: impl IntoStyleRect<Val>) -> Self {
        self.position(Position::Absolute).inset(value)
    }

    /// Sets the left and right alignment insets all at once.
    #[inline]
    #[must_use]
    pub fn inset_x(mut self, value: impl IntoStyleSize<Val>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let current_top = self.inner.basic_layout.inset.top;
                let current_bottom = self.inner.basic_layout.inset.bottom;
                self.inset((current_top, v.width, current_bottom, v.height))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_INSET);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let size = getter();
                    let v = cx.get_basic_layout_mut(id, target);
                    v.inset.right = size.width;
                    v.inset.left = size.height;

                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Sets the top and bottom alignment insets all at once.
    #[inline]
    #[must_use]
    pub fn inset_y(mut self, value: impl IntoStyleSize<Val>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let current_left = self.inner.basic_layout.inset.left;
                let current_right = self.inner.basic_layout.inset.right;
                self.inset((v.width, current_right, v.height, current_left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_INSET);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let size = getter();
                    let v = cx.get_basic_layout_mut(id, target);
                    v.inset.top = size.width;
                    v.inset.bottom = size.height;

                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Sets the top alignment insets.
    #[inline]
    #[must_use]
    pub fn top(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.inset;
                self.inset((v, current.right, current.bottom, current.left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_INSET);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).inset.top = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Sets the right alignment insets.
    #[inline]
    #[must_use]
    pub fn right(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.inset;
                self.inset((current.top, v, current.bottom, current.left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_INSET);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).inset.right = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Sets the bottom alignment insets.
    #[inline]
    #[must_use]
    pub fn bottom(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.inset;
                self.inset((current.top, current.right, v, current.left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_INSET);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).inset.bottom = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Sets the left alignment insets.
    #[inline]
    #[must_use]
    pub fn left(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.inset;
                self.inset((current.top, current.right, current.bottom, v))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_INSET);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).inset.left = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Sets the basic size (width, height) of the element.
    #[inline]
    #[must_use]
    pub fn size(mut self, value: impl IntoStyleSize<Val>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.size = v;
                inner.mask.set(ComponentMask::STYLE_SIZE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).size = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Set both the width and height of the element to 100% (the entire parent element).
    #[inline]
    #[must_use]
    pub fn size_full(self) -> Self {
        self.size(pct(100.0))
    }

    /// Set both the width and height of the element to 50% (half the size of its parent element).
    #[inline]
    #[must_use]
    pub fn size_half(self) -> Self {
        self.size(pct(50.0))
    }

    /// Set both the width and height of the element to Auto.
    #[inline]
    #[must_use]
    pub fn size_auto(self) -> Self {
        self.size(auto())
    }

    /// Sets only the width of the element (the height will retain its existing value).
    #[inline]
    #[must_use]
    pub fn width(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current_h = self.inner.basic_layout.size.height;
                self.size((v, current_h))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).size.width = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Sets only the width of the element (the height will retain its existing value).
    #[inline]
    #[must_use]
    pub fn w(self, value: impl IntoStyleConvert<Val>) -> Self {
        self.width(value)
    }

    /// Set the width of the element to 100% (the entire parent element).
    #[inline]
    #[must_use]
    pub fn w_full(self) -> Self {
        self.width(pct(100.0))
    }

    /// Set the width of the element to 50% (half the size of its parent element).
    #[inline]
    #[must_use]
    pub fn w_half(self) -> Self {
        self.width(pct(50.0))
    }

    /// Set the width of the element to Auto.
    #[inline]
    #[must_use]
    pub fn w_auto(self) -> Self {
        self.width(auto())
    }

    /// Sets only the height of the element (the width will retain its existing value).
    #[inline]
    #[must_use]
    pub fn height(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current_w = self.inner.basic_layout.size.width;
                self.size((current_w, v))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).size.height = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Sets only the height of the element (the width will retain its existing value).
    #[inline]
    #[must_use]
    pub fn h(self, value: impl IntoStyleConvert<Val>) -> Self {
        self.height(value)
    }

    /// Set the height of the element to 100% (the entire parent element).
    #[inline]
    #[must_use]
    pub fn h_full(self) -> Self {
        self.height(pct(100.0))
    }

    /// Set the height of the element to 50% (half the size of its parent element).
    #[inline]
    #[must_use]
    pub fn h_half(self) -> Self {
        self.height(pct(50.0))
    }

    /// Set the height of the element to Auto.
    #[inline]
    #[must_use]
    pub fn h_auto(self) -> Self {
        self.height(auto())
    }

    /// Set the minimum size.
    #[inline]
    #[must_use]
    pub fn min_size(mut self, value: impl IntoStyleSize<Val>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.min_size = v;
                inner.mask.set(ComponentMask::STYLE_MIN_SIZE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_MIN_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).min_size = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Set the maximum size.
    #[inline]
    #[must_use]
    pub fn max_size(mut self, value: impl IntoStyleSize<Val>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.max_size = v;
                inner.mask.set(ComponentMask::STYLE_MAX_SIZE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_MAX_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).max_size = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Only the minimum width is set.
    #[inline]
    #[must_use]
    pub fn min_width(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current_h = self.inner.basic_layout.min_size.height;
                self.min_size((v, current_h))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_MIN_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).min_size.width = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Only the minimum height is set.
    #[inline]
    #[must_use]
    pub fn min_height(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current_w = self.inner.basic_layout.min_size.width;
                self.min_size((current_w, v))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_MIN_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).min_size.height = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Only the maximum width is set.
    #[inline]
    #[must_use]
    pub fn max_width(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current_h = self.inner.basic_layout.max_size.height;
                self.max_size((v, current_h))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_MAX_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).max_size.width = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Only the maximum height is set.
    #[inline]
    #[must_use]
    pub fn max_height(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current_w = self.inner.basic_layout.max_size.width;
                self.max_size((current_w, v))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_MAX_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).max_size.height = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Set the aspect ratio to any desired ratio (width, height).
    #[inline]
    #[must_use]
    pub fn aspect_ratio(
        mut self,
        width: impl IntoStyleValue<f32>,
        height: impl IntoStyleValue<f32>,
    ) -> Self {
        let w_val = width.into_style_value();
        let h_val = height.into_style_value();

        match (w_val, h_val) {
            (StyleValue::Static(w), StyleValue::Static(h)) => {
                let inner = Arc::make_mut(&mut self.inner);
                if h <= 0.0 {
                    inner.basic_layout.aspect_ratio = None;
                } else {
                    inner.basic_layout.aspect_ratio = Some(w / h);
                }
                inner.mask.set(ComponentMask::STYLE_ASPECT_RATIO);
            }
            (w_getter, h_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_ASPECT_RATIO);

                let get_w = match w_getter {
                    StyleValue::Static(w) => {
                        Box::new(move || w) as Box<dyn Fn() -> f32 + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };
                let get_h = match h_getter {
                    StyleValue::Static(h) => {
                        Box::new(move || h) as Box<dyn Fn() -> f32 + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };

                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let w = get_w();
                        let h = get_h();

                        let b = cx.get_basic_layout_mut(id, target);
                        if h <= 0.0 {
                            b.aspect_ratio = None;
                        } else {
                            b.aspect_ratio = Some(w / h);
                        }

                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Set the aspect ratio (16:9).
    #[inline]
    #[must_use]
    pub fn ratio_16_9(self) -> Self {
        self.aspect_ratio(16.0, 9.0)
    }

    /// Set the aspect ratio (9:16).
    #[inline]
    #[must_use]
    pub fn ratio_9_16(self) -> Self {
        self.aspect_ratio(9.0, 16.0)
    }

    /// Set the aspect ratio (4:3).
    #[inline]
    #[must_use]
    pub fn ratio_4_3(self) -> Self {
        self.aspect_ratio(4.0, 3.0)
    }

    /// Set the aspect ratio (3:4).
    #[inline]
    #[must_use]
    pub fn ratio_3_4(self) -> Self {
        self.aspect_ratio(3.0, 4.0)
    }

    /// Set the aspect ratio (1:1).
    #[inline]
    #[must_use]
    pub fn ratio_1_1(self) -> Self {
        self.aspect_ratio(1.0, 1.0)
    }

    /// Set the aspect ratio (21:9).
    #[inline]
    #[must_use]
    pub fn ratio_21_9(self) -> Self {
        self.aspect_ratio(21.0, 9.0)
    }

    /// Set the aspect ratio (9:21).
    #[inline]
    #[must_use]
    pub fn ratio_9_21(self) -> Self {
        self.aspect_ratio(9.0, 21.0)
    }

    /// Set the aspect ratio (0:0).
    #[inline]
    #[must_use]
    pub fn clear_aspect_ratio(self) -> Self {
        self.aspect_ratio(0.0, 0.0)
    }

    /// Sets the outer margin.
    #[inline]
    #[must_use]
    pub fn margin(mut self, value: impl IntoStyleRect<Val>) -> Self {
        match value.into_style_rect() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.margin = v;
                inner.mask.set(ComponentMask::STYLE_MARGIN);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_MARGIN);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).margin = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the outer margin.
    #[inline]
    #[must_use]
    pub fn m(self, value: impl IntoStyleRect<Val>) -> Self {
        self.margin(value)
    }

    /// Sets the outer margin 0.
    #[inline]
    #[must_use]
    pub fn m_0(self) -> Self {
        self.margin(0.0)
    }

    /// Sets the outer margin Auto
    #[inline]
    #[must_use]
    pub fn m_auto(self) -> Self {
        self.margin(auto())
    }

    /// Sets the left and right outer margins (margin-left, margin-right) all at once.
    #[inline]
    #[must_use]
    pub fn m_x(mut self, value: impl IntoStyleSize<Val>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let current_top = self.inner.basic_layout.margin.top;
                let current_bottom = self.inner.basic_layout.margin.bottom;
                self.margin((current_top, v.width, current_bottom, v.height))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_MARGIN);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let size = getter();
                    let b = cx.get_basic_layout_mut(id, target);
                    b.margin.right = size.width;
                    b.margin.left = size.height;

                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Sets the top and bottom outer margins (margin-top, margin-bottom) all at once.
    #[inline]
    #[must_use]
    pub fn m_y(mut self, value: impl IntoStyleSize<Val>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let current_left = self.inner.basic_layout.margin.left;
                let current_right = self.inner.basic_layout.margin.right;
                self.margin((v.width, current_right, v.height, current_left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_MARGIN);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let size = getter();
                    let b = cx.get_basic_layout_mut(id, target);
                    b.margin.top = size.width;
                    b.margin.bottom = size.height;

                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Set the top outer margin.
    #[inline]
    #[must_use]
    pub fn m_t(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.margin;
                self.margin((v, current.right, current.bottom, current.left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_MARGIN);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).margin.top = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Set the right outer margin.
    #[inline]
    #[must_use]
    pub fn m_r(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.margin;
                self.margin((current.top, v, current.bottom, current.left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_MARGIN);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).margin.right = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Set the bottom outer margin.
    #[inline]
    #[must_use]
    pub fn m_b(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.margin;
                self.margin((current.top, current.right, v, current.left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_MARGIN);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).margin.bottom = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Set the left outer margin.
    #[inline]
    #[must_use]
    pub fn m_l(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.margin;
                self.margin((current.top, current.right, current.bottom, v))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_MARGIN);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).margin.left = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Sets the inner padding.
    #[inline]
    #[must_use]
    pub fn padding(mut self, value: impl IntoStyleRect<Length>) -> Self {
        match value.into_style_rect() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.padding = v;
                inner.mask.set(ComponentMask::STYLE_PADDING);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_PADDING);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).padding = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the inner padding.
    #[inline]
    #[must_use]
    pub fn p(self, value: impl IntoStyleRect<Length>) -> Self {
        self.padding(value)
    }

    /// Sets the inner padding 0.
    #[inline]
    #[must_use]
    pub fn p_0(self) -> Self {
        self.padding(0.0)
    }

    /// Sets the left and right inner padding (padding-left, padding-right) all at once.
    #[inline]
    #[must_use]
    pub fn p_x(mut self, value: impl IntoStyleSize<Length>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let current_top = self.inner.basic_layout.padding.top;
                let current_bottom = self.inner.basic_layout.padding.bottom;
                self.padding((current_top, v.width, current_bottom, v.height))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_PADDING);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let size = getter();
                    let b = cx.get_basic_layout_mut(id, target);
                    b.padding.right = size.width;
                    b.padding.left = size.height;

                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Sets the top and bottom inner padding (padding-top, padding-bottom) all at once.
    #[inline]
    #[must_use]
    pub fn p_y(mut self, value: impl IntoStyleSize<Length>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let current_left = self.inner.basic_layout.padding.left;
                let current_right = self.inner.basic_layout.padding.right;
                self.padding((v.height, current_left, v.height, current_right))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_PADDING);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let size = getter();
                    let b = cx.get_basic_layout_mut(id, target);
                    b.padding.top = size.width;
                    b.padding.bottom = size.height;

                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Set the top inner padding.
    #[inline]
    #[must_use]
    pub fn p_t(mut self, value: impl IntoStyleConvert<Length>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.padding;
                self.padding((v, current.right, current.bottom, current.left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_PADDING);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).padding.top = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Set the right inner padding.
    #[inline]
    #[must_use]
    pub fn p_r(mut self, value: impl IntoStyleConvert<Length>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.padding;
                self.padding((current.top, v, current.bottom, current.left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_PADDING);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).padding.right = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Set the bottom inner padding.
    #[inline]
    #[must_use]
    pub fn p_b(mut self, value: impl IntoStyleConvert<Length>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.padding;
                self.padding((current.top, current.right, v, current.left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_PADDING);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).padding.bottom = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Set the left inner padding.
    #[inline]
    #[must_use]
    pub fn p_l(mut self, value: impl IntoStyleConvert<Length>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.padding;
                self.padding((current.top, current.right, current.bottom, v))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_PADDING);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).padding.left = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Sets the border style (Solid, Dotted, Dashed, Double) and width.
    ///
    /// Default style is [`BorderStyle::Solid`]
    #[inline]
    #[must_use]
    pub fn border(
        mut self,
        style: impl IntoStyleValue<BorderStyle>,
        width: impl IntoStyleRect<Length>,
    ) -> Self {
        let s_val = style.into_style_value();
        let w_val = width.into_style_rect();

        match (s_val, w_val) {
            (StyleValue::Static(s), StyleValue::Static(w)) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.border = w;
                inner.visual_property.border_styles = Some([s; 4]);
                inner.mask.set(ComponentMask::STYLE_BORDER);
            }
            (s_getter, w_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BORDER);

                let get_s = match s_getter {
                    StyleValue::Static(s) => {
                        Box::new(move || s) as Box<dyn Fn() -> BorderStyle + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };
                let get_w = match w_getter {
                    StyleValue::Static(w) => {
                        Box::new(move || w) as Box<dyn Fn() -> Rect<Length> + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };

                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let s = get_s();
                    let w = get_w();
                    cx.get_basic_layout_mut(id, target).border = w;
                    cx.get_visual_property_mut(id, target).border_styles = Some([s; 4]);
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the border and width of solid lines all at once.
    #[inline]
    #[must_use]
    pub fn border_solid(self, width: impl IntoStyleRect<Length>) -> Self {
        self.border(BorderStyle::Solid, width)
    }

    /// Sets the border and width of dotted lines all at once.
    #[inline]
    #[must_use]
    pub fn border_dotted(self, width: impl IntoStyleRect<Length>) -> Self {
        self.border(BorderStyle::Dotted, width)
    }

    /// Sets the border and width of dashed lines all at once.
    #[inline]
    #[must_use]
    pub fn border_dashed(self, width: impl IntoStyleRect<Length>) -> Self {
        self.border(BorderStyle::Dashed, width)
    }

    /// Sets the border and width of double lines all at once.
    #[inline]
    #[must_use]
    pub fn border_double(self, width: impl IntoStyleRect<Length>) -> Self {
        self.border(BorderStyle::Double, width)
    }

    /// Sets the style and width of the top border individually.
    #[inline]
    #[must_use]
    pub fn border_top(
        mut self,
        style: impl IntoStyleValue<BorderStyle>,
        value: impl IntoStyleConvert<Length>,
    ) -> Self {
        let s_val = style.into_style_value();
        let v_val = value.into_style_convert();

        match (s_val, v_val) {
            (StyleValue::Static(s), StyleValue::Static(v)) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.border.top = v;
                let mut styles = inner
                    .visual_property
                    .border_styles
                    .unwrap_or([BorderStyle::Solid; 4]);
                styles[0] = s;
                inner.visual_property.border_styles = Some(styles);
                inner.mask.set(ComponentMask::STYLE_BORDER);
            }
            (s_getter, v_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BORDER);

                let get_s = match s_getter {
                    StyleValue::Static(s) => {
                        Box::new(move || s) as Box<dyn Fn() -> BorderStyle + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };
                let get_v = match v_getter {
                    StyleValue::Static(v) => {
                        Box::new(move || v) as Box<dyn Fn() -> Length + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };

                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let s = get_s();
                    let v = get_v();
                    cx.get_basic_layout_mut(id, target).border.top = v;
                    let vis = cx.get_visual_property_mut(id, target);
                    let mut styles = vis.border_styles.unwrap_or([BorderStyle::Solid; 4]);
                    styles[0] = s;
                    vis.border_styles = Some(styles);

                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the style and width of the right border individually.
    #[inline]
    #[must_use]
    pub fn border_right(
        mut self,
        style: impl IntoStyleValue<BorderStyle>,
        value: impl IntoStyleConvert<Length>,
    ) -> Self {
        let s_val = style.into_style_value();
        let v_val = value.into_style_convert();

        match (s_val, v_val) {
            (StyleValue::Static(s), StyleValue::Static(v)) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.border.right = v;
                let mut styles = inner
                    .visual_property
                    .border_styles
                    .unwrap_or([BorderStyle::Solid; 4]);
                styles[1] = s;
                inner.visual_property.border_styles = Some(styles);
                inner.mask.set(ComponentMask::STYLE_BORDER);
            }
            (s_getter, v_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BORDER);

                let get_s = match s_getter {
                    StyleValue::Static(s) => {
                        Box::new(move || s) as Box<dyn Fn() -> BorderStyle + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };
                let get_v = match v_getter {
                    StyleValue::Static(v) => {
                        Box::new(move || v) as Box<dyn Fn() -> Length + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };

                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let s = get_s();
                    let v = get_v();
                    cx.get_basic_layout_mut(id, target).border.right = v;
                    let vis = cx.get_visual_property_mut(id, target);
                    let mut styles = vis.border_styles.unwrap_or([BorderStyle::Solid; 4]);
                    styles[1] = s;
                    vis.border_styles = Some(styles);

                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the style and width of the bottom border individually.
    #[inline]
    #[must_use]
    pub fn border_bottom(
        mut self,
        style: impl IntoStyleValue<BorderStyle>,
        value: impl IntoStyleConvert<Length>,
    ) -> Self {
        let s_val = style.into_style_value();
        let v_val = value.into_style_convert();

        match (s_val, v_val) {
            (StyleValue::Static(s), StyleValue::Static(v)) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.border.bottom = v;
                let mut styles = inner
                    .visual_property
                    .border_styles
                    .unwrap_or([BorderStyle::Solid; 4]);
                styles[2] = s;
                inner.visual_property.border_styles = Some(styles);
                inner.mask.set(ComponentMask::STYLE_BORDER);
            }
            (s_getter, v_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BORDER);

                let get_s = match s_getter {
                    StyleValue::Static(s) => {
                        Box::new(move || s) as Box<dyn Fn() -> BorderStyle + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };
                let get_v = match v_getter {
                    StyleValue::Static(v) => {
                        Box::new(move || v) as Box<dyn Fn() -> Length + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };

                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let s = get_s();
                    let v = get_v();
                    cx.get_basic_layout_mut(id, target).border.bottom = v;
                    let vis = cx.get_visual_property_mut(id, target);
                    let mut styles = vis.border_styles.unwrap_or([BorderStyle::Solid; 4]);
                    styles[2] = s;
                    vis.border_styles = Some(styles);

                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the style and width of the left border individually.
    #[inline]
    #[must_use]
    pub fn border_left(
        mut self,
        style: impl IntoStyleValue<BorderStyle>,
        value: impl IntoStyleConvert<Length>,
    ) -> Self {
        let s_val = style.into_style_value();
        let v_val = value.into_style_convert();

        match (s_val, v_val) {
            (StyleValue::Static(s), StyleValue::Static(v)) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.border.left = v;
                let mut styles = inner
                    .visual_property
                    .border_styles
                    .unwrap_or([BorderStyle::Solid; 4]);
                styles[3] = s;
                inner.visual_property.border_styles = Some(styles);
                inner.mask.set(ComponentMask::STYLE_BORDER);
            }
            (s_getter, v_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BORDER);

                let get_s = match s_getter {
                    StyleValue::Static(s) => {
                        Box::new(move || s) as Box<dyn Fn() -> BorderStyle + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };
                let get_v = match v_getter {
                    StyleValue::Static(v) => {
                        Box::new(move || v) as Box<dyn Fn() -> Length + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };

                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let s = get_s();
                    let v = get_v();
                    cx.get_basic_layout_mut(id, target).border.left = v;
                    let vis = cx.get_visual_property_mut(id, target);
                    let mut styles = vis.border_styles.unwrap_or([BorderStyle::Solid; 4]);
                    styles[3] = s;
                    vis.border_styles = Some(styles);

                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Set the length ratio (0.0 to 1.0) for each of the four individual border lines.
    #[inline]
    #[must_use]
    pub fn border_lengths(mut self, value: impl IntoStyleRect<f32>) -> Self {
        match value.into_style_rect() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.border_lengths = Some(EdgeInsets {
                    top: v.top,
                    right: v.right,
                    bottom: v.bottom,
                    left: v.left,
                });
                inner.mask.set(ComponentMask::STYLE_BORDER);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BORDER);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let v = getter();
                    let vis = cx.get_visual_property_mut(id, target);
                    vis.border_lengths = Some(EdgeInsets {
                        top: v.top,
                        right: v.right,
                        bottom: v.bottom,
                        left: v.left,
                    });
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the length ratio (0.0 to 1.0) of the top border.
    #[inline]
    #[must_use]
    pub fn border_top_length(mut self, value: impl IntoStyleValue<f32>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let mut lengths = inner
                    .visual_property
                    .border_lengths
                    .unwrap_or(EdgeInsets::px_all(1.0));
                lengths.top = v;
                inner.visual_property.border_lengths = Some(lengths);
                inner.mask.set(ComponentMask::STYLE_BORDER);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BORDER);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    let vis = cx.get_visual_property_mut(id, target);
                    let mut lengths = vis.border_lengths.unwrap_or(EdgeInsets::px_all(1.0));
                    lengths.top = val;
                    vis.border_lengths = Some(lengths);

                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the length ratio (0.0 to 1.0) of the right border.
    #[inline]
    #[must_use]
    pub fn border_right_length(mut self, value: impl IntoStyleValue<f32>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let mut lengths = inner
                    .visual_property
                    .border_lengths
                    .unwrap_or(EdgeInsets::px_all(1.0));
                lengths.right = v;
                inner.visual_property.border_lengths = Some(lengths);
                inner.mask.set(ComponentMask::STYLE_BORDER);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BORDER);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    let vis = cx.get_visual_property_mut(id, target);
                    let mut lengths = vis.border_lengths.unwrap_or(EdgeInsets::px_all(1.0));
                    lengths.right = val;
                    vis.border_lengths = Some(lengths);

                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the length ratio (0.0 to 1.0) of the bottom border.
    #[inline]
    #[must_use]
    pub fn border_bottom_length(mut self, value: impl IntoStyleValue<f32>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let mut lengths = inner
                    .visual_property
                    .border_lengths
                    .unwrap_or(EdgeInsets::px_all(1.0));
                lengths.bottom = v;
                inner.visual_property.border_lengths = Some(lengths);
                inner.mask.set(ComponentMask::STYLE_BORDER);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BORDER);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    let vis = cx.get_visual_property_mut(id, target);
                    let mut lengths = vis.border_lengths.unwrap_or(EdgeInsets::px_all(1.0));
                    lengths.bottom = val;
                    vis.border_lengths = Some(lengths);

                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the length ratio (0.0 to 1.0) of the left border.
    #[inline]
    #[must_use]
    pub fn border_left_length(mut self, value: impl IntoStyleValue<f32>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let mut lengths = inner
                    .visual_property
                    .border_lengths
                    .unwrap_or(EdgeInsets::px_all(1.0));
                lengths.left = v;
                inner.visual_property.border_lengths = Some(lengths);
                inner.mask.set(ComponentMask::STYLE_BORDER);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BORDER);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    let vis = cx.get_visual_property_mut(id, target);
                    let mut lengths = vis.border_lengths.unwrap_or(EdgeInsets::px_all(1.0));
                    lengths.left = val;
                    vis.border_lengths = Some(lengths);

                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the alignment (for expansion/contraction direction) for the border lines on all four sides at once.
    ///
    /// Default is [`BorderAlignment::Start`]
    #[inline]
    #[must_use]
    pub fn border_align(mut self, value: impl IntoStyleValue<BorderAlignment>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.border_alignments = Some([v; 4]);
                inner.mask.set(ComponentMask::STYLE_BORDER);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BORDER);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).border_alignments = Some([val; 4]);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Set individual border alignment for each of the four sides `[Top, Right, Bottom, Left]`.
    #[inline]
    #[must_use]
    pub fn border_aligns(mut self, values: impl IntoStyleValue<[BorderAlignment; 4]>) -> Self {
        match values.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.border_alignments = Some(v);
                inner.mask.set(ComponentMask::STYLE_BORDER);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BORDER);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).border_alignments = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    fn set_border_align_idx(
        mut self,
        idx: usize,
        value: impl IntoStyleValue<BorderAlignment>,
    ) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let mut aligns = inner
                    .visual_property
                    .border_alignments
                    .unwrap_or([BorderAlignment::Start; 4]);
                aligns[idx] = v;
                inner.visual_property.border_alignments = Some(aligns);
                inner.mask.set(ComponentMask::STYLE_BORDER);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BORDER);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    let vis = cx.get_visual_property_mut(id, target);
                    let mut aligns = vis.border_alignments.unwrap_or([BorderAlignment::Start; 4]);
                    aligns[idx] = val;
                    vis.border_alignments = Some(aligns);

                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the alignment for the border line on top.
    #[inline]
    #[must_use]
    pub fn border_top_align(self, value: impl IntoStyleValue<BorderAlignment>) -> Self {
        self.set_border_align_idx(0, value)
    }

    /// Sets the alignment for the border line on right.
    #[inline]
    #[must_use]
    pub fn border_right_align(self, value: impl IntoStyleValue<BorderAlignment>) -> Self {
        self.set_border_align_idx(1, value)
    }

    /// Sets the alignment for the border line on bottom.
    #[inline]
    #[must_use]
    pub fn border_bottom_align(self, value: impl IntoStyleValue<BorderAlignment>) -> Self {
        self.set_border_align_idx(2, value)
    }

    /// Sets the alignment for the border line on left.
    #[inline]
    #[must_use]
    pub fn border_left_align(self, value: impl IntoStyleValue<BorderAlignment>) -> Self {
        self.set_border_align_idx(3, value)
    }

    /// Sets the outline style (Solid, Dotted, Dashed, Double) and width all at once.
    ///
    /// Default style [`BorderStyle::Solid`]
    #[inline]
    #[must_use]
    pub fn outline(
        mut self,
        style: impl IntoStyleValue<BorderStyle>,
        width: impl IntoStyleRect<Length>,
    ) -> Self {
        let s_val = style.into_style_value();
        let w_val = width.into_style_rect();

        match (s_val, w_val) {
            (StyleValue::Static(s), StyleValue::Static(w)) => {
                let inner = Arc::make_mut(&mut self.inner);
                // 基本レイアウトには影響しないため、basic_layout にはマウントせず visual_property にのみ設定
                inner.visual_property.outline_width = Some(EdgeInsets {
                    top: w.top.into(),
                    right: w.right.into(),
                    bottom: w.bottom.into(),
                    left: w.left.into(),
                });
                inner.visual_property.outline_styles = Some([s; 4]);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);
            }
            (s_getter, w_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);
                let get_s = match s_getter {
                    StyleValue::Static(s) => {
                        Box::new(move || s) as Box<dyn Fn() -> BorderStyle + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };
                let get_w = match w_getter {
                    StyleValue::Static(w) => {
                        Box::new(move || w) as Box<dyn Fn() -> Rect<Length> + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let s = get_s();
                    let w = get_w();
                    let vis = cx.get_visual_property_mut(id, target);
                    vis.outline_width = Some(EdgeInsets {
                        top: w.top.into(),
                        right: w.right.into(),
                        bottom: w.bottom.into(),
                        left: w.left.into(),
                    });
                    vis.outline_styles = Some([s; 4]);

                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the border and width of solid lines all at once.
    #[inline]
    #[must_use]
    pub fn outline_solid(self, width: impl IntoStyleRect<Length>) -> Self {
        self.outline(BorderStyle::Solid, width)
    }

    /// Sets the border and width of dotted lines all at once.
    #[inline]
    #[must_use]
    pub fn outline_dotted(self, width: impl IntoStyleRect<Length>) -> Self {
        self.outline(BorderStyle::Dotted, width)
    }

    /// Sets the border and width of dashed lines all at once.
    #[inline]
    #[must_use]
    pub fn outline_dashed(self, width: impl IntoStyleRect<Length>) -> Self {
        self.outline(BorderStyle::Dashed, width)
    }

    /// Sets the border and width of double lines all at once.
    #[inline]
    #[must_use]
    pub fn outline_double(self, width: impl IntoStyleRect<Length>) -> Self {
        self.outline(BorderStyle::Double, width)
    }

    /// Set the style and width of the top outline.
    #[inline]
    #[must_use]
    pub fn outline_top(
        mut self,
        style: impl IntoStyleValue<BorderStyle>,
        value: impl IntoStyleConvert<Length>,
    ) -> Self {
        let s_val = style.into_style_value();
        let v_val = value.into_style_convert();

        match (s_val, v_val) {
            (StyleValue::Static(s), StyleValue::Static(v)) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner
                    .visual_property
                    .outline_width
                    .get_or_insert_default()
                    .top = v.into();
                let mut styles = inner
                    .visual_property
                    .outline_styles
                    .unwrap_or([BorderStyle::Solid; 4]);
                styles[0] = s;
                inner.visual_property.outline_styles = Some(styles);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);
            }
            (s_getter, v_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);

                let get_s = match s_getter {
                    StyleValue::Static(s) => {
                        Box::new(move || s) as Box<dyn Fn() -> BorderStyle + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };
                let get_v = match v_getter {
                    StyleValue::Static(v) => {
                        Box::new(move || v) as Box<dyn Fn() -> Length + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };

                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let s = get_s();
                    let v = get_v();
                    let vis = cx.get_visual_property_mut(id, target);
                    vis.outline_width.get_or_insert_default().top = v.into();
                    let mut styles = vis.outline_styles.unwrap_or([BorderStyle::Solid; 4]);
                    styles[0] = s;
                    vis.outline_styles = Some(styles);

                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Set the style and width of the right outline.
    #[inline]
    #[must_use]
    pub fn outline_right(
        mut self,
        style: impl IntoStyleValue<BorderStyle>,
        value: impl IntoStyleConvert<Length>,
    ) -> Self {
        let s_val = style.into_style_value();
        let v_val = value.into_style_convert();

        match (s_val, v_val) {
            (StyleValue::Static(s), StyleValue::Static(v)) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner
                    .visual_property
                    .outline_width
                    .get_or_insert_default()
                    .right = v.into();
                let mut styles = inner
                    .visual_property
                    .outline_styles
                    .unwrap_or([BorderStyle::Solid; 4]);
                styles[1] = s;
                inner.visual_property.outline_styles = Some(styles);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);
            }
            (s_getter, v_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);

                let get_s = match s_getter {
                    StyleValue::Static(s) => {
                        Box::new(move || s) as Box<dyn Fn() -> BorderStyle + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };
                let get_v = match v_getter {
                    StyleValue::Static(v) => {
                        Box::new(move || v) as Box<dyn Fn() -> Length + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };

                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let s = get_s();
                    let v = get_v();
                    let vis = cx.get_visual_property_mut(id, target);
                    vis.outline_width.get_or_insert_default().right = v.into();
                    let mut styles = vis.outline_styles.unwrap_or([BorderStyle::Solid; 4]);
                    styles[1] = s;
                    vis.outline_styles = Some(styles);

                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Set the Style and width of the bottom outline.
    #[inline]
    #[must_use]
    pub fn outline_bottom(
        mut self,
        style: impl IntoStyleValue<BorderStyle>,
        value: impl IntoStyleConvert<Length>,
    ) -> Self {
        let s_val = style.into_style_value();
        let v_val = value.into_style_convert();

        match (s_val, v_val) {
            (StyleValue::Static(s), StyleValue::Static(v)) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner
                    .visual_property
                    .outline_width
                    .get_or_insert_default()
                    .bottom = v.into();
                let mut styles = inner
                    .visual_property
                    .outline_styles
                    .unwrap_or([BorderStyle::Solid; 4]);
                styles[2] = s;
                inner.visual_property.outline_styles = Some(styles);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);
            }
            (s_getter, v_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);

                let get_s = match s_getter {
                    StyleValue::Static(s) => {
                        Box::new(move || s) as Box<dyn Fn() -> BorderStyle + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };
                let get_v = match v_getter {
                    StyleValue::Static(v) => {
                        Box::new(move || v) as Box<dyn Fn() -> Length + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };

                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let s = get_s();
                    let v = get_v();
                    let vis = cx.get_visual_property_mut(id, target);
                    vis.outline_width.get_or_insert_default().bottom = v.into();
                    let mut styles = vis.outline_styles.unwrap_or([BorderStyle::Solid; 4]);
                    styles[2] = s;
                    vis.outline_styles = Some(styles);

                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Set the Style and width of the left outline.
    #[inline]
    #[must_use]
    pub fn outline_left(
        mut self,
        style: impl IntoStyleValue<BorderStyle>,
        width: impl IntoStyleConvert<Length>,
    ) -> Self {
        let s_val = style.into_style_value();
        let v_val = width.into_style_convert();

        match (s_val, v_val) {
            (StyleValue::Static(s), StyleValue::Static(v)) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner
                    .visual_property
                    .outline_width
                    .get_or_insert_default()
                    .left = v.into();
                let mut styles = inner
                    .visual_property
                    .outline_styles
                    .unwrap_or([BorderStyle::Solid; 4]);
                styles[3] = s;
                inner.visual_property.outline_styles = Some(styles);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);
            }
            (s_getter, v_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);

                let get_s = match s_getter {
                    StyleValue::Static(s) => {
                        Box::new(move || s) as Box<dyn Fn() -> BorderStyle + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };
                let get_v = match v_getter {
                    StyleValue::Static(v) => {
                        Box::new(move || v) as Box<dyn Fn() -> Length + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };

                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let s = get_s();
                    let v = get_v();
                    let vis = cx.get_visual_property_mut(id, target);
                    vis.outline_width.get_or_insert_default().left = v.into();
                    let mut styles = vis.outline_styles.unwrap_or([BorderStyle::Solid; 4]);
                    styles[3] = s;
                    vis.outline_styles = Some(styles);

                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Set the outline color.
    #[inline]
    #[must_use]
    pub fn outline_color(mut self, value: impl IntoStyleValue<Color>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.outline_color = Some(v);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).outline_color = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the gap (offset) between the element and its outline.
    #[inline]
    #[must_use]
    pub fn outline_offset(mut self, value: impl IntoStyleValue<f32>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.outline_offset = Some(v);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).outline_offset = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Set the length of the outline of each of the four sides individually.
    #[inline]
    #[must_use]
    pub fn outline_lengths(mut self, value: impl IntoStyleRect<f32>) -> Self {
        match value.into_style_rect() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.outline_lengths = Some(EdgeInsets {
                    top: v.top,
                    right: v.right,
                    bottom: v.bottom,
                    left: v.left,
                });
                inner.mask.set(ComponentMask::STYLE_OUTLINE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let v = getter();
                    let vis = cx.get_visual_property_mut(id, target);
                    vis.outline_lengths = Some(EdgeInsets {
                        top: v.top,
                        right: v.right,
                        bottom: v.bottom,
                        left: v.left,
                    });

                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// This sets the outline alignment (direction of layout and scaling) all at once.
    #[inline]
    #[must_use]
    pub fn outline_align(mut self, value: impl IntoStyleValue<BorderAlignment>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.outline_alignments = Some([v; 4]);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).outline_alignments = Some([val; 4]);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// これなんだっけ???
    #[inline]
    #[must_use]
    pub fn outline_weight(mut self, value: impl IntoStyleValue<BorderAlignment>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.outline_alignments = Some([v; 4]);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_OUTLINE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).outline_alignments = Some([val; 4]);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the arrangement of cross axes within the container.
    ///
    /// Default is [`AlignItems::Stretch`]
    #[inline]
    #[must_use]
    pub fn align_items(mut self, value: impl IntoStyleValue<Option<AlignItems>>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.align_items = v;
                inner.mask.set(ComponentMask::STYLE_ALIGN_ITEMS);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_ALIGN_ITEMS);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_flex_layout_mut(id, target).align_items = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the arrangement of cross axes within the container to a [`AlignItems::Start`].
    #[inline]
    #[must_use]
    pub fn items_start(self) -> Self {
        self.align_items(AlignItems::Start)
    }

    /// Sets the arrangement of cross axes within the container to a [`AlignItems::End`].
    #[inline]
    #[must_use]
    pub fn items_end(self) -> Self {
        self.align_items(AlignItems::End)
    }

    /// Sets the arrangement of cross axes within the container to  [`AlignItems::FlexStart`].
    #[inline]
    #[must_use]
    pub fn items_flex_start(self) -> Self {
        self.align_items(AlignItems::FlexStart)
    }

    /// Sets the arrangement of cross axes within the container to a [`AlignItems::FlexEnd`].
    #[inline]
    #[must_use]
    pub fn items_flex_end(self) -> Self {
        self.align_items(AlignItems::FlexEnd)
    }

    /// Sets the arrangement of cross axes within the container to a [`AlignItems::Center`].
    #[inline]
    #[must_use]
    pub fn items_center(self) -> Self {
        self.align_items(AlignItems::Center)
    }

    /// Sets the arrangement of cross axes within the container to a [`AlignItems::Baseline`].
    #[inline]
    #[must_use]
    pub fn items_baseline(self) -> Self {
        self.align_items(AlignItems::Baseline)
    }

    /// Sets the arrangement of cross axes within the container to a [`AlignItems::Stretch`].
    #[inline]
    #[must_use]
    pub fn items_stretch(self) -> Self {
        self.align_items(AlignItems::Stretch)
    }

    /// Sets the arrangement of cross axes within the container to a [`AlignItems::SafeStart`].
    #[inline]
    #[must_use]
    pub fn items_start_safe(self) -> Self {
        self.align_items(AlignItems::SafeStart)
    }

    /// Sets the arrangement of cross axes within the container to a [`AlignItems::SafeEnd`].
    #[inline]
    #[must_use]
    pub fn items_end_safe(self) -> Self {
        self.align_items(AlignItems::SafeEnd)
    }

    /// Sets the arrangement of cross axes within the container to a [`AlignItems::SafeFlexStart`].
    #[inline]
    #[must_use]
    pub fn items_flex_start_safe(self) -> Self {
        self.align_items(AlignItems::SafeFlexStart)
    }

    /// Sets the arrangement of cross axes within the container to a [`AlignItems::SafeFlexEnd`].
    #[inline]
    #[must_use]
    pub fn items_flex_end_safe(self) -> Self {
        self.align_items(AlignItems::SafeFlexEnd)
    }

    /// Sets the arrangement of cross axes within the container to a [`AlignItems::SafeCenter`].
    #[inline]
    #[must_use]
    pub fn items_center_safe(self) -> Self {
        self.align_items(AlignItems::SafeCenter)
    }

    /// Sets the cross axis alignment of individual elements.
    ///
    /// Default is [`AlignSelf::Stretch`]
    #[inline]
    #[must_use]
    pub fn align_self(mut self, value: impl IntoStyleValue<Option<AlignSelf>>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.align_self = v;
                inner.mask.set(ComponentMask::STYLE_ALIGN_SELF);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_ALIGN_SELF);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_flex_layout_mut(id, target).align_self = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the cross axis alignment of individual elements to a auto ([`None`]).
    #[inline]
    #[must_use]
    pub fn self_auto(self) -> Self {
        self.align_self(None)
    }

    /// Sets the cross axis alignment of individual elements to a [`AlignSelf::Start`].
    #[inline]
    #[must_use]
    pub fn self_start(self) -> Self {
        self.align_self(AlignSelf::Start)
    }

    /// Sets the cross axis alignment of individual elements to a [`AlignSelf::End`].
    #[inline]
    #[must_use]
    pub fn self_end(self) -> Self {
        self.align_self(AlignSelf::End)
    }

    /// Sets the cross axis alignment of individual elements to a [`AlignSelf::FlexStart`].
    #[inline]
    #[must_use]
    pub fn self_flex_start(self) -> Self {
        self.align_self(AlignSelf::FlexStart)
    }

    /// Sets the cross axis alignment of individual elements to a [`AlignSelf::FlexEnd`].
    #[inline]
    #[must_use]
    pub fn self_flex_end(self) -> Self {
        self.align_self(AlignSelf::FlexEnd)
    }

    /// Sets the cross axis alignment of individual elements to a [`AlignSelf::Center`].
    #[inline]
    #[must_use]
    pub fn self_center(self) -> Self {
        self.align_self(AlignSelf::Center)
    }

    /// Sets the cross axis alignment of individual elements to a [`AlignSelf::Baseline`].
    #[inline]
    #[must_use]
    pub fn self_baseline(self) -> Self {
        self.align_self(AlignSelf::Baseline)
    }

    /// Sets the cross axis alignment of individual elements to a [`AlignSelf::Stretch`].
    #[inline]
    #[must_use]
    pub fn self_stretch(self) -> Self {
        self.align_self(AlignSelf::Stretch)
    }

    /// Sets the cross axis alignment of individual elements to a [`AlignSelf::SafeStart`].
    #[inline]
    #[must_use]
    pub fn self_start_safe(self) -> Self {
        self.align_self(AlignSelf::SafeStart)
    }

    /// Sets the cross axis alignment of individual elements to a [`AlignSelf::SafeEnd`].
    #[inline]
    #[must_use]
    pub fn self_end_safe(self) -> Self {
        self.align_self(AlignSelf::SafeEnd)
    }

    /// Sets the cross axis alignment of individual elements to a [`AlignSelf::SafeFlexStart`].
    #[inline]
    #[must_use]
    pub fn self_flex_start_safe(self) -> Self {
        self.align_self(AlignSelf::SafeFlexStart)
    }

    /// Sets the cross axis alignment of individual elements to a [`AlignSelf::SafeFlexEnd`].
    #[inline]
    #[must_use]
    pub fn self_flex_end_safe(self) -> Self {
        self.align_self(AlignSelf::SafeFlexEnd)
    }

    /// Sets the cross axis alignment of individual elements to a [`AlignSelf::SafeCenter`].
    #[inline]
    #[must_use]
    pub fn self_center_safe(self) -> Self {
        self.align_self(AlignSelf::SafeCenter)
    }

    /// Sets the central axis placement within the container.
    ///
    /// Default is [`AlignItems::Stretch`]
    #[inline]
    #[must_use]
    pub fn justify_items(mut self, value: impl IntoStyleValue<Option<AlignItems>>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.justify_items = v;
                inner.mask.set(ComponentMask::STYLE_JUSTIFY_ITEMS);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_JUSTIFY_ITEMS);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_flex_layout_mut(id, target).justify_items = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the central axis placement within the container to a [`AlignItems::Center`].
    #[inline]
    #[must_use]
    pub fn justify_items_center(self) -> Self {
        self.justify_items(AlignItems::Center)
    }

    /// Sets the central axis placement within the container to a [`AlignItems::SafeCenter`].
    #[inline]
    #[must_use]
    pub fn justify_items_center_safe(self) -> Self {
        self.justify_items(AlignItems::SafeCenter)
    }

    /// Sets the central axis placement within the container to a [`AlignItems::End`].
    #[inline]
    #[must_use]
    pub fn justify_items_end(self) -> Self {
        self.justify_items(AlignItems::End)
    }

    /// Sets the central axis placement within the container to a [`AlignItems::SafeEnd`].
    #[inline]
    #[must_use]
    pub fn justify_items_end_safe(self) -> Self {
        self.justify_items(AlignItems::SafeEnd)
    }

    /// Sets the central axis placement within the container to a [`AlignItems::Start`].
    #[inline]
    #[must_use]
    pub fn justify_items_start(self) -> Self {
        self.justify_items(AlignItems::Start)
    }

    /// Sets the central axis placement within the container to a [`AlignItems::SafeStart`].
    #[inline]
    #[must_use]
    pub fn justify_items_start_safe(self) -> Self {
        self.justify_items(AlignItems::SafeStart)
    }

    /// Sets the central axis placement within the container to a [`AlignItems::Stretch`].
    #[inline]
    #[must_use]
    pub fn justify_items_stretch(self) -> Self {
        self.justify_items(AlignItems::Stretch)
    }

    /// Sets the central axis placement within the container to a [`AlignItems::FlexStart`].
    #[inline]
    #[must_use]
    pub fn justify_items_flex_start(self) -> Self {
        self.justify_items(AlignItems::FlexStart)
    }

    /// Sets the central axis placement within the container to a [`AlignItems::FlexEnd`].
    #[inline]
    #[must_use]
    pub fn justify_items_flex_end(self) -> Self {
        self.justify_items(AlignItems::FlexEnd)
    }

    /// Sets the central axis placement within the container to a [`AlignItems::SafeFlexStart`].
    #[inline]
    #[must_use]
    pub fn justify_items_flex_start_safe(self) -> Self {
        self.justify_items(AlignItems::SafeFlexStart)
    }

    /// Sets the central axis placement within the container to a [`AlignItems::SafeFlexEnd`].
    #[inline]
    #[must_use]
    pub fn justify_items_flex_end_safe(self) -> Self {
        self.justify_items(AlignItems::SafeFlexEnd)
    }

    /// Sets the central axis placement within the container to a [`AlignItems::Baseline`].
    #[inline]
    #[must_use]
    pub fn justify_items_baseline(self) -> Self {
        self.justify_items(AlignItems::Baseline)
    }

    /// Sets the axis alignment of individual elements.
    ///
    /// Default is [`AlignSelf::Stretch`]
    #[inline]
    #[must_use]
    pub fn justify_self(mut self, value: impl IntoStyleValue<Option<AlignSelf>>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.justify_self = v;
                inner.mask.set(ComponentMask::STYLE_JUSTIFY_SELF);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_JUSTIFY_SELF);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_flex_layout_mut(id, target).justify_self = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the axis alignment of individual elements to auto ([`None`]).
    #[inline]
    #[must_use]
    pub fn justify_self_auto(self) -> Self {
        self.justify_self(None)
    }

    /// Sets the axis alignment of individual elements to a [`AlignSelf::Baseline`].
    #[inline]
    #[must_use]
    pub fn justify_self_baseline(self) -> Self {
        self.justify_self(AlignSelf::Baseline)
    }

    /// Sets the axis alignment of individual elements to a [`AlignSelf::Center`].
    #[inline]
    #[must_use]
    pub fn justify_self_center(self) -> Self {
        self.justify_self(AlignSelf::Center)
    }

    /// Sets the axis alignment of individual elements to a [`AlignSelf::SafeCenter`].
    #[inline]
    #[must_use]
    pub fn justify_self_center_safe(self) -> Self {
        self.justify_self(AlignSelf::SafeCenter)
    }

    /// Sets the axis alignment of individual elements to a [`AlignSelf::End`].
    #[inline]
    #[must_use]
    pub fn justify_self_end(self) -> Self {
        self.justify_self(AlignSelf::End)
    }

    /// Sets the axis alignment of individual elements to a [`AlignSelf::SafeEnd`].
    #[inline]
    #[must_use]
    pub fn justify_self_end_safe(self) -> Self {
        self.justify_self(AlignSelf::SafeEnd)
    }

    /// Sets the axis alignment of individual elements to a [`AlignSelf::Start`].
    #[inline]
    #[must_use]
    pub fn justify_self_start(self) -> Self {
        self.justify_self(AlignSelf::Start)
    }

    /// Sets the axis alignment of individual elements to a [`AlignSelf::SafeStart`].
    #[inline]
    #[must_use]
    pub fn justify_self_start_safe(self) -> Self {
        self.justify_self(AlignSelf::SafeStart)
    }

    /// Sets the axis alignment of individual elements to a [`AlignSelf::Stretch`].
    #[inline]
    #[must_use]
    pub fn justify_self_stretch(self) -> Self {
        self.justify_self(AlignSelf::Stretch)
    }

    /// Sets the axis alignment of individual elements to a [`AlignSelf::FlexStart`].
    #[inline]
    #[must_use]
    pub fn justify_self_flex_start(self) -> Self {
        self.justify_self(AlignSelf::FlexStart)
    }

    /// Sets the axis alignment of individual elements to a [`AlignSelf::FlexEnd`].
    #[inline]
    #[must_use]
    pub fn justify_self_flex_end(self) -> Self {
        self.justify_self(AlignSelf::FlexEnd)
    }

    /// Sets the axis alignment of individual elements to a [`AlignSelf::SafeFlexStart`].
    #[inline]
    #[must_use]
    pub fn justify_self_flex_start_safe(self) -> Self {
        self.justify_self(AlignSelf::SafeFlexStart)
    }

    /// Sets the axis alignment of individual elements to a [`AlignSelf::SafeFlexEnd`].
    #[inline]
    #[must_use]
    pub fn justify_self_flex_end_safe(self) -> Self {
        self.justify_self(AlignSelf::SafeFlexEnd)
    }

    /// Sets the bulk placement of content spanning multiple lines.
    ///
    /// Default is [`AlignContent::Stretch`]
    #[inline]
    #[must_use]
    pub fn align_content(mut self, value: impl IntoStyleValue<Option<AlignContent>>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.align_content = v;
                inner.mask.set(ComponentMask::STYLE_ALIGN_CONTENT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_ALIGN_CONTENT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_flex_layout_mut(id, target).align_content = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the bulk placement of content spanning multiple lines to a [`AlignContent::SpaceAround`].
    #[inline]
    #[must_use]
    pub fn content_around(self) -> Self {
        self.align_content(AlignContent::SpaceAround)
    }

    /// Sets the bulk placement of content spanning multiple lines to a [`AlignContent::SpaceBetween`].
    #[inline]
    #[must_use]
    pub fn content_between(self) -> Self {
        self.align_content(AlignContent::SpaceBetween)
    }

    /// Sets the bulk placement of content spanning multiple lines to a [`AlignContent::Center`].
    #[inline]
    #[must_use]
    pub fn content_center(self) -> Self {
        self.align_content(AlignContent::Center)
    }

    /// Sets the bulk placement of content spanning multiple lines to a [`AlignContent::SafeCenter`].
    #[inline]
    #[must_use]
    pub fn content_center_safe(self) -> Self {
        self.align_content(AlignContent::SafeCenter)
    }

    /// Sets the bulk placement of content spanning multiple lines to a [`AlignContent::End`].
    #[inline]
    #[must_use]
    pub fn content_end(self) -> Self {
        self.align_content(AlignContent::End)
    }

    /// Sets the bulk placement of content spanning multiple lines to a [`AlignContent::SafeEnd`].
    #[inline]
    #[must_use]
    pub fn content_end_safe(self) -> Self {
        self.align_content(AlignContent::SafeEnd)
    }

    /// Sets the bulk placement of content spanning multiple lines to a [`AlignContent::SpaceEvenly`].
    #[inline]
    #[must_use]
    pub fn content_evenly(self) -> Self {
        self.align_content(AlignContent::SpaceEvenly)
    }

    /// Sets the bulk placement of content spanning multiple lines to a [`AlignContent::Start`].
    #[inline]
    #[must_use]
    pub fn content_start(self) -> Self {
        self.align_content(AlignContent::Start)
    }

    /// Sets the bulk placement of content spanning multiple lines to a [`AlignContent::SafeStart`].
    #[inline]
    #[must_use]
    pub fn content_start_safe(self) -> Self {
        self.align_content(AlignContent::SafeStart)
    }

    /// Sets the bulk placement of content spanning multiple lines to a [`AlignContent::Stretch`].
    #[inline]
    #[must_use]
    pub fn content_stretch(self) -> Self {
        self.align_content(AlignContent::Stretch)
    }

    /// Sets the bulk placement of content spanning multiple lines to a [`AlignContent::FlexStart`].
    #[inline]
    #[must_use]
    pub fn content_flex_start(self) -> Self {
        self.align_content(AlignContent::FlexStart)
    }

    /// Sets the bulk placement of content spanning multiple lines to a [`AlignContent::FlexEnd`].
    #[inline]
    #[must_use]
    pub fn content_flex_end(self) -> Self {
        self.align_content(AlignContent::FlexEnd)
    }

    /// Sets the bulk placement of content spanning multiple lines to a [`AlignContent::SafeFlexStart`].
    #[inline]
    #[must_use]
    pub fn content_flex_start_safe(self) -> Self {
        self.align_content(AlignContent::SafeFlexStart)
    }

    /// Sets the bulk placement of content spanning multiple lines to a [`AlignContent::SafeFlexEnd`].
    #[inline]
    #[must_use]
    pub fn content_flex_end_safe(self) -> Self {
        self.align_content(AlignContent::SafeFlexEnd)
    }

    /// Set the content placement along the main axis.
    ///
    /// Default is [`JustifyContent::Stretch`]
    #[inline]
    #[must_use]
    pub fn justify_content(mut self, value: impl IntoStyleValue<Option<JustifyContent>>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.justify_content = v;
                inner.mask.set(ComponentMask::STYLE_JUSTIFY_CONTENT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_JUSTIFY_CONTENT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_flex_layout_mut(id, target).justify_content = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Set the content placement along the main axis to a [`JustifyContent::SpaceAround`].
    #[inline]
    #[must_use]
    pub fn justify_around(self) -> Self {
        self.justify_content(JustifyContent::SpaceAround)
    }

    /// Set the content placement along the main axis to a [`JustifyContent::SpaceBetween`].
    #[inline]
    #[must_use]
    pub fn justify_between(self) -> Self {
        self.justify_content(JustifyContent::SpaceBetween)
    }

    /// Set the content placement along the main axis to a [`JustifyContent::Center`].
    #[inline]
    #[must_use]
    pub fn justify_center(self) -> Self {
        self.justify_content(JustifyContent::Center)
    }

    /// Set the content placement along the main axis to a [`JustifyContent::SafeCenter`].
    #[inline]
    #[must_use]
    pub fn justify_center_safe(self) -> Self {
        self.justify_content(JustifyContent::SafeCenter)
    }

    /// Set the content placement along the main axis to a [`JustifyContent::End`].
    #[inline]
    #[must_use]
    pub fn justify_end(self) -> Self {
        self.justify_content(JustifyContent::End)
    }

    /// Set the content placement along the main axis to a [`JustifyContent::SafeEnd`].
    #[inline]
    #[must_use]
    pub fn justify_end_safe(self) -> Self {
        self.justify_content(JustifyContent::SafeEnd)
    }

    /// Set the content placement along the main axis to a [`JustifyContent::SpaceEvenly`].
    #[inline]
    #[must_use]
    pub fn justify_evenly(self) -> Self {
        self.justify_content(JustifyContent::SpaceEvenly)
    }

    /// Set the content placement along the main axis to a [`JustifyContent::Start`].
    #[inline]
    #[must_use]
    pub fn justify_start(self) -> Self {
        self.justify_content(JustifyContent::Start)
    }

    /// Set the content placement along the main axis to a [`JustifyContent::SafeStart`].
    #[inline]
    #[must_use]
    pub fn justify_start_safe(self) -> Self {
        self.justify_content(JustifyContent::SafeStart)
    }

    /// Set the content placement along the main axis to a [`JustifyContent::Stretch`].
    #[inline]
    #[must_use]
    pub fn justify_stretch(self) -> Self {
        self.justify_content(JustifyContent::Stretch)
    }

    /// Set the content placement along the main axis to a [`JustifyContent::FlexStart`].
    #[inline]
    #[must_use]
    pub fn justify_flex_start(self) -> Self {
        self.justify_content(JustifyContent::FlexStart)
    }

    /// Set the content placement along the main axis to a [`JustifyContent::FlexEnd`].
    #[inline]
    #[must_use]
    pub fn justify_flex_end(self) -> Self {
        self.justify_content(JustifyContent::FlexEnd)
    }

    /// Set the content placement along the main axis to a [`JustifyContent::SafeFlexStart`].
    #[inline]
    #[must_use]
    pub fn justify_flex_start_safe(self) -> Self {
        self.justify_content(JustifyContent::SafeFlexStart)
    }

    /// Set the content placement along the main axis to a [`JustifyContent::SafeFlexEnd`].
    #[inline]
    #[must_use]
    pub fn justify_flex_end_safe(self) -> Self {
        self.justify_content(JustifyContent::SafeFlexEnd)
    }

    /// Sets the spacing between child elements in the row and column directions.
    #[inline]
    #[must_use]
    pub fn gap(mut self, value: impl IntoStyleSize<Val>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.gap = v;
                inner.mask.set(ComponentMask::STYLE_GAP);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GAP);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_flex_layout_mut(id, target).gap = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the spacing between child elements in the row and column directions to 0.
    #[inline]
    #[must_use]
    pub fn gap_0(self) -> Self {
        self.gap(0.0)
    }

    /// Sets the spacing between child elements in the row and column directions to auto.
    #[inline]
    #[must_use]
    pub fn gap_auto(self) -> Self {
        self.gap(auto())
    }

    /// Sets the row-gap (vertical) spacing between child elements.
    #[inline]
    #[must_use]
    pub fn gap_row(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current_column_gap = self.inner.flex_layout.gap.width;
                self.gap((current_column_gap, v))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GAP);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_flex_layout_mut(id, target).gap.height = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Sets the column-gap (horizontal) spacing between child elements.
    #[inline]
    #[must_use]
    pub fn gap_col(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current_row_gap = self.inner.flex_layout.gap.height;
                self.gap((v, current_row_gap))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GAP);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_flex_layout_mut(id, target).gap.width = val;
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// Sets the row-gap (vertical) spacing between child elements.
    #[inline]
    #[must_use]
    pub fn gap_y(self, value: impl IntoStyleConvert<Val>) -> Self {
        self.gap_row(value)
    }

    /// Sets the column-gap (horizontal) spacing between child elements.
    #[inline]
    #[must_use]
    pub fn gap_x(self, value: impl IntoStyleConvert<Val>) -> Self {
        self.gap_col(value)
    }

    /// Sets the text alignment direction.
    ///
    /// Default is [`TextAlign::Auto`]
    ///
    /// When multiple lines are entered, the default setting is top alignment; for single lines, it falls back to center alignment.
    #[inline]
    #[must_use]
    pub fn text_align(mut self, value: impl IntoStyleValue<TextAlign>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.text_align = v;
                inner.mask.set(ComponentMask::STYLE_TEXT_ALIGN);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_TEXT_ALIGN);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_flex_layout_mut(id, target).text_align = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the text alignment direction to a [`TextAlign::Center`].
    #[inline]
    #[must_use]
    pub fn text_center(self) -> Self {
        self.text_align(TextAlign::Center)
    }

    /// Sets the text alignment direction to a [`TextAlign::Auto`].
    #[inline]
    #[must_use]
    pub fn text_auto(self) -> Self {
        self.text_align(TextAlign::Auto)
    }

    /// Sets the text alignment direction to a [`TextAlign::Left`].
    #[inline]
    #[must_use]
    pub fn text_left(self) -> Self {
        self.text_align(TextAlign::Left)
    }

    /// Sets the text alignment direction to a [`TextAlign::Right`].
    #[inline]
    #[must_use]
    pub fn text_right(self) -> Self {
        self.text_align(TextAlign::Right)
    }

    /// Sets the direction of the main axis within the flex container.
    ///
    /// Default is [`FlexDirection::Row`].
    ///
    /// Prefer using [`crate::h_flex`] or [`crate::v_flex`] functions for a cleaner syntax.
    ///
    /// # Examples
    ///
    /// ```rust
    /// // Recommended: Use the layout shortcuts directly
    /// h_flex(/* style */);
    /// v_flex(/* style */);
    ///
    /// // Equivalent low-level approach:
    /// div(ts().flex().flex_direction(FlexDirection::Row));
    /// div(ts().flex().flex_direction(FlexDirection::Column));
    /// ```
    #[inline]
    #[must_use]
    pub fn flex_direction(mut self, value: impl IntoStyleValue<FlexDirection>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.flex_direction = v;
                inner.mask.set(ComponentMask::STYLE_FLEX_DIRECTION);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_FLEX_DIRECTION);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        cx.get_flex_layout_mut(id, target).flex_direction = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Sets the direction of the main axis within the flex container to a [`FlexDirection::Column`].
    #[inline]
    #[must_use]
    pub fn flex_col(self) -> Self {
        self.flex_direction(FlexDirection::Column)
    }

    /// Sets the direction of the main axis within the flex container to a [`FlexDirection::ColumnReverse`].
    #[inline]
    #[must_use]
    pub fn flex_col_reverse(self) -> Self {
        self.flex_direction(FlexDirection::ColumnReverse)
    }

    /// Sets the direction of the main axis within the flex container to a [`FlexDirection::Row`].
    #[inline]
    #[must_use]
    pub fn flex_row(self) -> Self {
        self.flex_direction(FlexDirection::Row)
    }

    /// Sets the direction of the main axis within the flex container to a [`FlexDirection::RowReverse`].
    #[inline]
    #[must_use]
    pub fn flex_row_reverse(self) -> Self {
        self.flex_direction(FlexDirection::RowReverse)
    }

    /// Sets whether child elements are wrapped to multiple lines.
    ///
    /// Default is [`FlexWrap::NoWrap`]
    #[inline]
    #[must_use]
    pub fn flex_wrap_internal(mut self, value: impl IntoStyleValue<FlexWrap>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.flex_wrap = v;
                inner.mask.set(ComponentMask::STYLE_FLEX_WRAP);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_FLEX_WRAP);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        cx.get_flex_layout_mut(id, target).flex_wrap = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Sets whether child elements are wrapped to multiple lines to  [`FlexWrap::Wrap`].
    #[inline]
    #[must_use]
    pub fn flex_wrap(self) -> Self {
        self.flex_wrap_internal(FlexWrap::Wrap)
    }

    /// Sets whether child elements are wrapped to multiple lines to  [`FlexWrap::NoWrap`].
    #[inline]
    #[must_use]
    pub fn flex_nowrap(self) -> Self {
        self.flex_wrap_internal(FlexWrap::NoWrap)
    }

    /// Sets whether child elements are wrapped to multiple lines to  [`FlexWrap::WrapReverse`].
    #[inline]
    #[must_use]
    pub fn flex_wrap_reverse(self) -> Self {
        self.flex_wrap_internal(FlexWrap::WrapReverse)
    }

    /// Sets the base dimensions that serve as the basis for the child elements.
    #[inline]
    #[must_use]
    pub fn basis(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.flex_basis = v;
                inner.mask.set(ComponentMask::STYLE_FLEX_BASIS);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_FLEX_BASIS);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_flex_layout_mut(id, target).flex_basis = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the base dimensions that serve as the basis for the child elements to 0.
    #[inline]
    #[must_use]
    pub fn basis_0(self) -> Self {
        self.basis(0.0)
    }

    /// Sets the base dimensions that serve as the basis for the child elements to auto.
    #[inline]
    #[must_use]
    pub fn basis_auto(self) -> Self {
        self.basis(auto())
    }

    /// Sets the base dimensions that serve as the basis for the child elements to 100%.
    #[inline]
    #[must_use]
    pub fn basis_full(self) -> Self {
        self.basis(pct(100.0))
    }

    /// Sets the element's flex-grow ratio.
    ///
    /// Accepts a boolean (true/false) or a number (f32/i32).
    #[inline]
    #[must_use]
    pub fn flex_grow(mut self, value: impl IntoStyleConvert<f32>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.flex_grow = v;
                inner.mask.set(ComponentMask::STYLE_FLEX_GROW);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_FLEX_GROW);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_flex_layout_mut(id, target).flex_grow = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Set the element to stretch (`flex_grow(1.0)`).
    #[inline]
    #[must_use]
    pub fn grow(self) -> Self {
        self.flex_grow(1.0)
    }

    /// Sets the element not to stretch (`flex_grow(0.0)`).
    #[inline]
    #[must_use]
    pub fn grow_0(self) -> Self {
        self.flex_grow(0.0)
    }

    /// Sets the element's shrinking ratio (flex-shrink).
    /// Accepts a boolean (true/false) or a number (f32/i32).
    #[inline]
    #[must_use]
    pub fn flex_shrink(mut self, value: impl IntoStyleConvert<f32>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.flex_shrink = v;
                inner.mask.set(ComponentMask::STYLE_FLEX_SHRINK);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_FLEX_SHRINK);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_flex_layout_mut(id, target).flex_shrink = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Makes the element shrinkable (`flex_shrink(1.0)`)
    #[inline]
    #[must_use]
    pub fn shrink(self) -> Self {
        self.flex_shrink(1.0)
    }

    /// Sets the element is never scaled down (`flex_shrink(0.0)`).
    #[inline]
    #[must_use]
    pub fn shrink_0(self) -> Self {
        self.flex_shrink(0.0)
    }

    /// Sets the background color of the element.
    ///
    /// Prefer using
    /// [`crate::rgb`], [`crate::rgba`], [`crate::hex`], [`crate::hsl`], [`crate::hsla`] functions for a cleaner syntax.
    #[inline]
    #[must_use]
    pub fn bg_color(mut self, value: impl IntoStyleValue<Color>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.bg_color = Some(v);
                inner.mask.set(ComponentMask::STYLE_BG_COLOR);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BG_COLOR);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).bg_color = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the color of the element's border.
    ///
    /// Prefer using
    /// [`crate::rgb`], [`crate::rgba`], [`crate::hex`], [`crate::hsl`], [`crate::hsla`] functions for a cleaner syntax.
    #[inline]
    #[must_use]
    pub fn border_color(mut self, value: impl IntoStyleValue<Color>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.border_color = Some(v);
                inner.mask.set(ComponentMask::STYLE_BORDER_COLOR);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BORDER_COLOR);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).border_color = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the radius of the element's corner rounding.
    #[inline]
    #[must_use]
    pub fn corner_radius(mut self, value: impl IntoStyleCornerRadius) -> Self {
        match value.into_style_corner_radius() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.corner_radius = Some(v);
                inner.mask.set(ComponentMask::STYLE_CORNER_RADIUS);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_CORNER_RADIUS);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).corner_radius = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the radius of the element's corner rounding.
    #[inline]
    #[must_use]
    pub fn rounded(self, value: impl IntoStyleCornerRadius) -> Self {
        self.corner_radius(value)
    }

    /// Sets the radius of the element's corner rounding.
    #[inline]
    #[must_use]
    pub fn r(self, value: impl IntoStyleCornerRadius) -> Self {
        self.corner_radius(value)
    }

    /// Sets the radius of the element's corner rounding to (`corner_radius(9999.0)`).
    #[inline]
    #[must_use]
    pub fn r_full(self) -> Self {
        self.corner_radius(9999.0)
    }

    /// Rounded corners are applied only to the top corners (top-left, top-right).
    #[inline]
    #[must_use]
    pub fn r_top(self, value: impl Convert<f32>) -> Self {
        let val = value.convert();
        let current = self
            .inner
            .visual_property
            .corner_radius
            .unwrap_or(CornerRadius::ZERO);
        self.corner_radius((val, val, current.bottom_right, current.bottom_left))
    }

    /// Rounded corners are applied only to the bottom corners (bottom-left, bottom-right).
    #[inline]
    #[must_use]
    pub fn r_bottom(self, value: impl Convert<f32>) -> Self {
        let val = value.convert();
        let current = self
            .inner
            .visual_property
            .corner_radius
            .unwrap_or(CornerRadius::ZERO);
        self.corner_radius((current.top_left, current.top_right, val, val))
    }

    /// Rounded corners are applied only to the left corners (top-left, bottom-left).
    #[inline]
    #[must_use]
    pub fn r_left(self, value: impl Convert<f32>) -> Self {
        let val = value.convert();
        let current = self
            .inner
            .visual_property
            .corner_radius
            .unwrap_or(CornerRadius::ZERO);
        self.corner_radius((val, current.top_right, current.bottom_right, val))
    }

    /// Rounded corners are applied only to the right corners (top-right, bottom-right).
    #[inline]
    #[must_use]
    pub fn r_right(self, value: impl Convert<f32>) -> Self {
        let val = value.convert();
        let current = self
            .inner
            .visual_property
            .corner_radius
            .unwrap_or(CornerRadius::ZERO);
        self.corner_radius((current.top_left, val, val, current.bottom_left))
    }

    /// Sets the opacity (0 to 1) of the entire element.
    #[inline]
    #[must_use]
    pub fn opacity(mut self, value: impl IntoStyleValue<f32>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.opacity = Some(v);
                inner.mask.set(ComponentMask::STYLE_OPACITY);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_OPACITY);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).opacity = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the opacityof the entire element to 0%.
    #[inline]
    #[must_use]
    pub fn opacity_0(self) -> Self {
        self.opacity(0.0)
    }

    /// Sets the opacityof the entire element to 50%.
    #[inline]
    #[must_use]
    pub fn opacity_50(self) -> Self {
        self.opacity(0.5)
    }

    /// Sets the opacityof the entire element to 100%.
    #[inline]
    #[must_use]
    pub fn opacity_100(self) -> Self {
        self.opacity(1.0)
    }

    /// Sets a shadow ([`BoxShadow`]) to be placed outside the element.
    #[inline]
    #[must_use]
    pub fn box_shadow(mut self, value: impl IntoStyleValue<BoxShadow>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.shadow_params = Some(v);
                inner.visual_property.shadow_color = Some(v.color);
                inner.mask.set(ComponentMask::STYLE_BOX_SHADOW);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BOX_SHADOW);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    let v = cx.get_visual_property_mut(id, target);
                    v.shadow_params = Some(val);
                    v.shadow_color = Some(val.color);

                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets and overrides only the shadow color.
    #[inline]
    #[must_use]
    pub fn shadow_color(mut self, value: impl IntoStyleValue<Color>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.shadow_color = Some(v);
                inner.mask.set(ComponentMask::STYLE_BOX_SHADOW);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BOX_SHADOW);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).shadow_color = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets and overrides only the shadow offset.
    #[inline]
    #[must_use]
    pub fn shadow_offset(mut self, value: impl IntoStylePoint<f32>) -> Self {
        match value.into_style_point() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let shadow = inner.visual_property.shadow_params.unwrap_or_default();
                inner.visual_property.shadow_params = Some(BoxShadow {
                    offset: LayoutPoint { x: v.x, y: v.y },
                    blur: shadow.blur,
                    spread: shadow.spread,
                    color: shadow.color,
                });
                inner.mask.set(ComponentMask::STYLE_BOX_SHADOW);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BOX_SHADOW);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    let v = cx.get_visual_property_mut(id, target);
                    let shadow = v.shadow_params.unwrap_or_default();
                    v.shadow_params = Some(BoxShadow {
                        offset: LayoutPoint { x: val.x, y: val.y },
                        blur: shadow.blur,
                        spread: shadow.spread,
                        color: shadow.color,
                    });

                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets and overrides only the shadow blur.
    #[inline]
    #[must_use]
    pub fn shadow_blur(mut self, value: impl IntoStyleConvert<f32>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let shadow = inner.visual_property.shadow_params.unwrap_or_default();
                inner.visual_property.shadow_params = Some(BoxShadow {
                    offset: shadow.offset,
                    blur: v,
                    spread: shadow.spread,
                    color: shadow.color,
                });
                inner.mask.set(ComponentMask::STYLE_BOX_SHADOW);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BOX_SHADOW);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    let v = cx.get_visual_property_mut(id, target);
                    let shadow = v.shadow_params.unwrap_or_default();
                    v.shadow_params = Some(BoxShadow {
                        offset: shadow.offset,
                        blur: val,
                        spread: shadow.spread,
                        color: shadow.color,
                    });

                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets and overrides only the shadow spread.
    #[inline]
    #[must_use]
    pub fn shadow_spread(mut self, value: impl IntoStyleConvert<f32>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let shadow = inner.visual_property.shadow_params.unwrap_or_default();
                inner.visual_property.shadow_params = Some(BoxShadow {
                    offset: shadow.offset,
                    blur: shadow.blur,
                    spread: v,
                    color: shadow.color,
                });
                inner.mask.set(ComponentMask::STYLE_BOX_SHADOW);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BOX_SHADOW);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    let v = cx.get_visual_property_mut(id, target);
                    let shadow = v.shadow_params.unwrap_or_default();
                    v.shadow_params = Some(BoxShadow {
                        offset: shadow.offset,
                        blur: shadow.blur,
                        spread: val,
                        color: shadow.color,
                    });

                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Set the stacking order (Z-Index) as an integer.
    #[inline]
    #[must_use]
    pub fn z_index(mut self, value: impl IntoStyleValue<i32>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.z_index = Some(v);
                inner.mask.set(ComponentMask::STYLE_Z_INDEX);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_Z_INDEX);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).z_index = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Set the stacking order (Z-Index) as an integer.
    #[inline]
    #[must_use]
    pub fn z(self, value: impl IntoStyleValue<i32>) -> Self {
        self.z_index(value)
    }

    /// Set the stacking order to `z_index(-1)`.
    #[inline]
    #[must_use]
    pub fn z_neg_1(self) -> Self {
        self.z_index(-1)
    }

    /// Set the stacking order to `z_index(0)`.
    #[inline]
    #[must_use]
    pub fn z_0(self) -> Self {
        self.z_index(0)
    }

    /// Set the stacking order to `z_index(1)`.
    #[inline]
    #[must_use]
    pub fn z_1(self) -> Self {
        self.z_index(1)
    }

    /// Sets the cursor icon ([`CursorIcon`]) that appears when the cursor hovers over an element.
    #[inline]
    #[must_use]
    pub fn cursor(mut self, value: impl IntoStyleValue<CursorIcon>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.cursor = Some(v);
                inner.mask.set(ComponentMask::STYLE_CURSOR);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_CURSOR);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).cursor = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the cursor icon that appears when the cursor hovers over an element to `CursorIcon::Default(None)`.
    #[inline]
    #[must_use]
    pub fn cursor_default(self) -> Self {
        self.cursor(CursorIcon::Default(None))
    }

    /// Sets the cursor icon that appears when the cursor hovers over an element to `CursorIcon::Grab(None)`.
    #[inline]
    #[must_use]
    pub fn cursor_grab(self) -> Self {
        self.cursor(CursorIcon::Grab(None))
    }

    /// Sets the cursor icon that appears when the cursor hovers over an element to `CursorIcon::Grabbing(None)`.
    #[inline]
    #[must_use]
    pub fn cursor_grabbing(self) -> Self {
        self.cursor(CursorIcon::Grabbing(None))
    }

    /// Sets the cursor icon that appears when the cursor hovers over an element to `CursorIcon::NotAllowed(None)`.
    #[inline]
    #[must_use]
    pub fn cursor_not_allowed(self) -> Self {
        self.cursor(CursorIcon::NotAllowed(None))
    }

    /// Sets the cursor icon that appears when the cursor hovers over an element to `CursorIcon::Pointer(None)`.
    #[inline]
    #[must_use]
    pub fn cursor_pointer(self) -> Self {
        self.cursor(CursorIcon::Pointer(None))
    }

    /// Sets the cursor icon that appears when the cursor hovers over an element to `CursorIcon::Text(None)`.
    #[inline]
    #[must_use]
    pub fn cursor_text(self) -> Self {
        self.cursor(CursorIcon::Text(None))
    }

    /// Sets a global cursor icon ([`GlobalCursorIcon`]) that is passed down to descendants.
    #[inline]
    #[must_use]
    pub fn cursor_global(self, value: impl IntoStyleValue<GlobalCursorIcon>) -> Self {
        let cursor_val = match value.into_style_value() {
            StyleValue::Static(v) => StyleValue::Static(CursorIcon::Global(v)),
            StyleValue::Dynamic(getter) => {
                StyleValue::Dynamic(Box::new(move || CursorIcon::Global(getter())))
            }
        };
        self.cursor(cursor_val)
    }

    /// Sets a global cursor icon that is passed down to descendants to `GlobalCursorIcon::Default(None)`.
    #[inline]
    #[must_use]
    pub fn cursor_global_default(self) -> Self {
        self.cursor_global(GlobalCursorIcon::Default(None))
    }

    /// Sets a global cursor icon that is passed down to descendants to `GlobalCursorIcon::Pointer(None)`.
    #[inline]
    #[must_use]
    pub fn cursor_global_pointer(self) -> Self {
        self.cursor_global(GlobalCursorIcon::Pointer(None))
    }

    /// Sets a global cursor icon that is passed down to descendants to `GlobalCursorIcon::Text(None)`.
    #[inline]
    #[must_use]
    pub fn cursor_global_text(self) -> Self {
        self.cursor_global(GlobalCursorIcon::Text(None))
    }

    /// Sets a global cursor icon that is passed down to descendants to `GlobalCursorIcon::Grab(None)`.
    #[inline]
    #[must_use]
    pub fn cursor_global_grab(self) -> Self {
        self.cursor_global(GlobalCursorIcon::Grab(None))
    }

    /// Sets a global cursor icon that is passed down to descendants to `GlobalCursorIcon::Grabbing(None)`.
    #[inline]
    #[must_use]
    pub fn cursor_global_grabbing(self) -> Self {
        self.cursor_global(GlobalCursorIcon::Grabbing(None))
    }

    /// Applies a backdrop effect (such as an acrylic effect) to the element.
    ///
    /// For this effect to work, transparency must be enabled in your OS settings,
    /// and the background color of this element must be set to semi-transparent (or transparent).
    ///
    /// Default is [`Backdrop::None`]
    #[inline]
    #[must_use]
    pub fn backdrop(mut self, backdrop: impl IntoStyleValue<Backdrop>) -> Self {
        match backdrop.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.backdrop = v;
                inner.mask.set(ComponentMask::STYLE_BACKDROP);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BACKDROP);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).backdrop = val;
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Applies a [`Backdrop::Acrylic`] effect to the element.
    #[inline]
    #[must_use]
    pub fn backdrop_acrylic(self) -> Self {
        self.backdrop(Backdrop::Acrylic)
    }

    /// Applies a [`Backdrop::Mica`] effect to the element.
    #[inline]
    #[must_use]
    pub fn backdrop_mica(self) -> Self {
        self.backdrop(Backdrop::Mica)
    }

    /// Applies a [`Backdrop::MicaAlt`] effect to the element.
    #[inline]
    #[must_use]
    pub fn backdrop_mica_alt(self) -> Self {
        self.backdrop(Backdrop::MicaAlt)
    }

    /// Applies a [`Backdrop::None`] effect to the element.
    #[inline]
    #[must_use]
    pub fn backdrop_none(self) -> Self {
        self.backdrop(Backdrop::None)
    }

    /// Sets the base color of the text rendered within the element.
    #[inline]
    #[must_use]
    pub fn text_color(mut self, value: impl IntoStyleValue<Color>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.text_color = Some(v);
                inner.mask.set(ComponentMask::STYLE_TEXT_COLOR);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_TEXT_COLOR);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).text_color = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_template_rows(
        mut self,
        value: impl IntoStyleValue<Vec<GridTemplateComponent<String>>>,
    ) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_template_rows = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_template_rows = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_template_columns(
        mut self,
        value: impl IntoStyleValue<Vec<GridTemplateComponent<String>>>,
    ) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_template_columns = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_template_columns = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_auto_rows(mut self, value: impl IntoStyleValue<Vec<TrackSizingFunction>>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_auto_rows = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_auto_rows = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_auto_columns(
        mut self,
        value: impl IntoStyleValue<Vec<TrackSizingFunction>>,
    ) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_auto_columns = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_auto_columns = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_auto_flow(mut self, value: impl IntoStyleValue<GridAutoFlow>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_auto_flow = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_auto_flow = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_template_areas(
        mut self,
        value: impl IntoStyleValue<Vec<GridTemplateArea<String>>>,
    ) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_template_areas = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_template_areas = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_template_column_names(
        mut self,
        value: impl IntoStyleValue<Vec<Vec<String>>>,
    ) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_template_column_names = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_template_column_names = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_template_row_names(mut self, value: impl IntoStyleValue<Vec<Vec<String>>>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_template_row_names = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_template_row_names = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_row(mut self, value: impl IntoStyleValue<GridLine<GridPlacement<String>>>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_row = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_row = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_column(
        mut self,
        value: impl IntoStyleValue<GridLine<GridPlacement<String>>>,
    ) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_column = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_column = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Sets the style that is applied when the cursor hovers over an element.
    #[inline]
    #[must_use]
    pub fn hovered(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(style, ComponentMask::STATE_HOVERED, StyleTarget::Hovered)
    }

    /// Sets the style to be applied when an element receives focus.
    #[inline]
    #[must_use]
    pub fn focused(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(style, ComponentMask::STATE_FOCUSED, StyleTarget::Focused)
    }

    /// Sets a style that is applied only when the focus is accessed via the keyboard.
    #[inline]
    #[must_use]
    pub fn focused_visible(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(
            style,
            ComponentMask::STATE_FOCUSED_VISIBLE,
            StyleTarget::FocusedVisible,
        )
    }

    /// Sets the style to be applied when the left mouse button is pressed down over an element.
    #[inline]
    #[must_use]
    pub fn pressed(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(style, ComponentMask::STATE_PRESSED, StyleTarget::Pressed)
    }

    /// Sets the style to apply when an element is disabled.
    #[inline]
    #[must_use]
    pub fn disabled(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(style, ComponentMask::STATE_DISABLED, StyleTarget::Disabled)
    }

    /// Sets the style to be applied when the element is active.
    #[inline]
    #[must_use]
    pub fn actived(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(style, ComponentMask::STATE_ACTIVED, StyleTarget::Actived)
    }

    /// Sets the style to be applied when an element is toggled and selected.
    #[inline]
    #[must_use]
    pub fn selected(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(style, ComponentMask::STATE_SELECTED, StyleTarget::Selected)
    }

    /// Sets the style to apply when an element is currently being dragged.
    #[inline]
    #[must_use]
    pub fn dragged(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(style, ComponentMask::STATE_DRAGGED, StyleTarget::Dragged)
    }

    // 子孫要素のインタラクション状態に連動して自身のスタイルを変化させる伝播設定
    #[inline]
    fn interaction_within(
        self,
        name: InteractionName,
        style: impl IntoStyleValue<ThisStyle>,
    ) -> Self {
        let (state_flag, target) = match name {
            InteractionName::Hover => (
                ComponentMask::STYLE_INTERACTION_WITHIN,
                StyleTarget::HoveredWithin,
            ),
            InteractionName::Focus => (
                ComponentMask::STYLE_INTERACTION_WITHIN,
                StyleTarget::FocusedWithin,
            ),
            InteractionName::FocusVisible => (
                ComponentMask::STYLE_INTERACTION_WITHIN,
                StyleTarget::FocusedVisibleWithin,
            ),
            InteractionName::Press => (
                ComponentMask::STYLE_INTERACTION_WITHIN,
                StyleTarget::PressedWithin,
            ),
            InteractionName::Disable => (
                ComponentMask::STYLE_INTERACTION_WITHIN,
                StyleTarget::DisabledWithin,
            ),
            InteractionName::Active => (
                ComponentMask::STYLE_INTERACTION_WITHIN,
                StyleTarget::ActivedWithin,
            ),
            InteractionName::Select => (
                ComponentMask::STYLE_INTERACTION_WITHIN,
                StyleTarget::SelectedWithin,
            ),
            InteractionName::Drag => (
                ComponentMask::STYLE_INTERACTION_WITHIN,
                StyleTarget::DraggedWithin,
            ),
            InteractionName::All => (
                ComponentMask::STYLE_INTERACTION_WITHIN,
                StyleTarget::AnyWithin,
            ),
        };
        self.apply_interaction_style(style, state_flag, target)
    }

    /// Sets the style that is applied when a descendant element is in hover state.
    #[inline]
    #[must_use]
    pub fn hovered_within(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_within(InteractionName::Hover, style)
    }

    /// Sets the style that is applied when a descendant element is in focus state.
    #[inline]
    #[must_use]
    pub fn focused_within(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_within(InteractionName::Focus, style)
    }

    /// Sets a style that is applied only when the descendant element has keyboard focus.
    #[inline]
    #[must_use]
    pub fn focused_visible_within(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_within(InteractionName::FocusVisible, style)
    }

    /// Sets the style that is applied when a descendant element is in press state.
    #[inline]
    #[must_use]
    pub fn pressed_within(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_within(InteractionName::Press, style)
    }

    /// Sets the style that is applied when a descendant element is in disable state.
    #[inline]
    #[must_use]
    pub fn disabled_within(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_within(InteractionName::Disable, style)
    }

    /// Sets the style that is applied when a descendant element is in active state.
    #[inline]
    #[must_use]
    pub fn actived_within(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_within(InteractionName::Active, style)
    }

    /// Sets the style that is applied when a descendant element is in select state.
    #[inline]
    #[must_use]
    pub fn selected_within(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_within(InteractionName::Select, style)
    }

    /// Sets the style that is applied when a descendant element is in drag state.
    #[inline]
    #[must_use]
    pub fn dragged_within(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_within(InteractionName::Drag, style)
    }

    /// Sets the style that is applied when a descendant element is in interactive state.
    #[inline]
    #[must_use]
    pub fn all_within(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_within(InteractionName::All, style)
    }

    // 直近の親要素のインタラクション状態に連動して自身のスタイルを変化させる
    #[inline]
    #[must_use]
    fn interaction_parent(
        self,
        name: InteractionName,
        style: impl IntoStyleValue<ThisStyle>,
    ) -> Self {
        let (state_flag, target) = match name {
            InteractionName::Hover => (
                ComponentMask::STYLE_INTERACTION_PARENT,
                StyleTarget::HoveredParent,
            ),
            InteractionName::Focus => (
                ComponentMask::STYLE_INTERACTION_PARENT,
                StyleTarget::FocusedParent,
            ),
            InteractionName::FocusVisible => (
                ComponentMask::STYLE_INTERACTION_PARENT,
                StyleTarget::FocusedVisibleParent,
            ),
            InteractionName::Press => (
                ComponentMask::STYLE_INTERACTION_PARENT,
                StyleTarget::PressedParent,
            ),
            InteractionName::Disable => (
                ComponentMask::STYLE_INTERACTION_PARENT,
                StyleTarget::DisabledParent,
            ),
            InteractionName::Active => (
                ComponentMask::STYLE_INTERACTION_PARENT,
                StyleTarget::ActivedParent,
            ),
            InteractionName::Select => (
                ComponentMask::STYLE_INTERACTION_PARENT,
                StyleTarget::SelectedParent,
            ),
            InteractionName::Drag => (
                ComponentMask::STYLE_INTERACTION_PARENT,
                StyleTarget::DraggedParent,
            ),
            InteractionName::All => (
                ComponentMask::STYLE_INTERACTION_PARENT,
                StyleTarget::AnyParent,
            ),
        };
        self.apply_interaction_style(style, state_flag, target)
    }

    /// Sets the style that is applied when a most recent parent element is in hover state.
    #[inline]
    #[must_use]
    pub fn hovered_parent(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_parent(InteractionName::Hover, style)
    }

    /// Sets the style that is applied when a most recent parent element is in focus state.
    #[inline]
    #[must_use]
    pub fn focused_parent(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_parent(InteractionName::Focus, style)
    }

    /// Sets the style that is applied when a most recent parent element is in keyboard focus state.
    #[inline]
    #[must_use]
    pub fn focused_visible_parent(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_parent(InteractionName::FocusVisible, style)
    }
    /// Sets the style that is applied when a most recent parent element is in press state.
    #[inline]
    #[must_use]
    pub fn pressed_parent(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_parent(InteractionName::Press, style)
    }
    /// Sets the style that is applied when a most recent parent element is in disable state.
    #[inline]
    #[must_use]
    pub fn disabled_parent(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_parent(InteractionName::Disable, style)
    }
    /// Sets the style that is applied when a most recent parent element is in active state.
    #[inline]
    #[must_use]
    pub fn actived_parent(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_parent(InteractionName::Active, style)
    }
    /// Sets the style that is applied when a most recent parent element is in select state.
    #[inline]
    #[must_use]
    pub fn selected_parent(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_parent(InteractionName::Select, style)
    }
    /// Sets the style that is applied when a most recent parent element is in drag state.
    #[inline]
    #[must_use]
    pub fn dragged_parent(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_parent(InteractionName::Drag, style)
    }
    /// Sets the style that is applied when a most recent parent element is in interactive state.
    #[inline]
    #[must_use]
    pub fn all_parent(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_parent(InteractionName::All, style)
    }

    /// Sets the resizing permissions for the element's top, right, bottom, and left
    #[inline]
    #[must_use]
    pub fn resizable(mut self, value: impl IntoStyleResizable) -> Self {
        match value.into_style_resizable() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.resizable = v;
                inner.mask.set(ComponentMask::STYLE_RESIZABLE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_RESIZABLE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).resizable = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Set resizing permissions for all four directions at once.
    #[inline]
    #[must_use]
    pub fn resizable_all(mut self, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.resizable = [v; 4];
                inner.mask.set(ComponentMask::STYLE_RESIZABLE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_RESIZABLE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).resizable = [val; 4];
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets whether resizing is possible left and right (X-axis direction) all at once.
    #[inline]
    #[must_use]
    pub fn resizable_x(mut self, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.resizable[1] = v; // right
                inner.basic_layout.resizable[3] = v; // left
                inner.mask.set(ComponentMask::STYLE_RESIZABLE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_RESIZABLE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    let v = cx.get_basic_layout_mut(id, target);
                    v.resizable[1] = val;
                    v.resizable[3] = val;

                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets whether resizing is possible top and bottom (Y-axis direction) all at once.
    #[inline]
    #[must_use]
    pub fn resizable_y(mut self, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.resizable[0] = v; // top
                inner.basic_layout.resizable[2] = v; // bottom
                inner.mask.set(ComponentMask::STYLE_RESIZABLE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_RESIZABLE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    let v = cx.get_basic_layout_mut(id, target);
                    v.resizable[0] = val;
                    v.resizable[2] = val;

                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Individually configure whether the top section resized.
    #[inline]
    #[must_use]
    pub fn resizable_top(self, value: impl IntoStyleValue<bool>) -> Self {
        self.set_resizable_edge_idx(0, value)
    }

    /// Individually configure whether the right section resized.
    #[inline]
    #[must_use]
    pub fn resizable_right(self, value: impl IntoStyleValue<bool>) -> Self {
        self.set_resizable_edge_idx(1, value)
    }

    /// Individually configure whether the bottom section resized.
    #[inline]
    #[must_use]
    pub fn resizable_bottom(self, value: impl IntoStyleValue<bool>) -> Self {
        self.set_resizable_edge_idx(2, value)
    }

    /// Individually configure whether the left section resized.
    #[inline]
    #[must_use]
    pub fn resizable_left(self, value: impl IntoStyleValue<bool>) -> Self {
        self.set_resizable_edge_idx(3, value)
    }

    // 辺インデックス (0:top, 1:right, 2:bottom, 3:left) を指定した更新処理
    fn set_resizable_edge_idx(mut self, idx: usize, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.resizable[idx] = v;
                inner.mask.set(ComponentMask::STYLE_RESIZABLE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_RESIZABLE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_basic_layout_mut(id, target).resizable[idx] = val;
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets custom cursors for each resize direction all at once. (Ns, Ew, Nesw, Nwse)
    ///
    /// If None is specified for each direction,
    /// the library's automatic cursor mapping (OS standard resize cursor) will be applied.
    #[inline]
    #[must_use]
    pub fn resizable_cursor(
        mut self,
        ns: Option<CursorIcon>,
        ew: Option<CursorIcon>,
        nesw: Option<CursorIcon>,
        nwse: Option<CursorIcon>,
    ) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.resizable_cursor = Some([ns, ew, nesw, nwse]);
        inner.mask.set(ComponentMask::STYLE_RESIZABLE);
        self
    }

    /// Sets custom cursors for each resize direction all at once to default.
    ///
    /// `resizable_cursor(None, None, None, None)`
    #[inline]
    #[must_use]
    pub fn resizable_cursor_default(self) -> Self {
        self.resizable_cursor(None, None, None, None)
    }

    /// During dnd dragging,
    /// attach the placeholder as a child of the root element and have it follow the cursor using absolute positioning.
    ///
    /// The second argument determines whether or not to perform coordinate updates.
    #[inline]
    #[must_use]
    pub fn dnd_draggable_root(
        mut self,
        mode: impl IntoStyleValue<DndDragPayload>,
        update_position: impl IntoStyleValue<bool>,
    ) -> Self {
        let m_val = mode.into_style_value();
        let u_val = update_position.into_style_value();

        match (m_val, u_val) {
            (StyleValue::Static(m), StyleValue::Static(u)) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.drag_property = Some(DndDragProperty {
                    placeholder_parent: DndDragPlaceholderParent::Root,
                    drag_mode: m,
                    update_position: u,
                });
                inner.mask.set(ComponentMask::STYLE_DND_DRAGGABLE);
            }
            (m_getter, u_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_DND_DRAGGABLE);

                let get_m = match m_getter {
                    StyleValue::Static(m) => {
                        Box::new(move || m) as Box<dyn Fn() -> DndDragPayload + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };
                let get_u = match u_getter {
                    StyleValue::Static(u) => {
                        Box::new(move || u) as Box<dyn Fn() -> bool + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };

                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let m = get_m();
                        let u = get_u();
                        cx.states.dnd.dnd_drag_properties.insert(
                            id,
                            DndDragProperty {
                                placeholder_parent: DndDragPlaceholderParent::Root,
                                drag_mode: m,
                                update_position: u,
                            },
                        );
                        cx.mark_render_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// During dnd dragging,
    /// attach the placeholder as a child of a specific parent element and have it follow using absolute positioning.
    ///
    /// The third argument determines whether or not to perform coordinate updates.
    #[inline]
    #[must_use]
    pub fn dnd_draggable_parent(
        mut self,
        parent_id: impl IntoStyleValue<EntityId>,
        mode: impl IntoStyleValue<DndDragPayload>,
        update_position: impl IntoStyleValue<bool>,
    ) -> Self {
        let p_val = parent_id.into_style_value();
        let m_val = mode.into_style_value();
        let u_val = update_position.into_style_value();

        match (p_val, m_val, u_val) {
            (StyleValue::Static(p), StyleValue::Static(m), StyleValue::Static(u)) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.drag_property = Some(DndDragProperty {
                    placeholder_parent: DndDragPlaceholderParent::Custom(p),
                    drag_mode: m,
                    update_position: u,
                });
                inner.mask.set(ComponentMask::STYLE_DND_DRAGGABLE);
            }
            (p_getter, m_getter, u_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_DND_DRAGGABLE);

                let get_p = match p_getter {
                    StyleValue::Static(p) => {
                        Box::new(move || p) as Box<dyn Fn() -> EntityId + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };
                let get_m = match m_getter {
                    StyleValue::Static(m) => {
                        Box::new(move || m) as Box<dyn Fn() -> DndDragPayload + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };
                let get_u = match u_getter {
                    StyleValue::Static(u) => {
                        Box::new(move || u) as Box<dyn Fn() -> bool + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };

                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let p = get_p();
                        let m = get_m();
                        let u = get_u();
                        cx.states.dnd.dnd_drag_properties.insert(
                            id,
                            DndDragProperty {
                                placeholder_parent: DndDragPlaceholderParent::Custom(p),
                                drag_mode: m,
                                update_position: u,
                            },
                        );
                        cx.mark_render_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Indicates that the element is accepting drag-and-drop operations, and sets the ingestion method and behavior.
    #[inline]
    #[must_use]
    pub fn dnd_droppable(
        mut self,
        target: impl IntoStyleValue<DndDropTarget>,
        mode: impl IntoStyleValue<DndDragPayload>,
    ) -> Self {
        let t_val = target.into_style_value();
        let m_val = mode.into_style_value();

        match (t_val, m_val) {
            (StyleValue::Static(t), StyleValue::Static(m)) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.drop_property = Some(DndDropProperty {
                    target: t,
                    drag_mode: m,
                });
                inner.mask.set(ComponentMask::STYLE_DND_DROPPABLE);
            }
            (t_getter, m_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_DND_DROPPABLE);

                let get_t = match t_getter {
                    StyleValue::Static(t) => {
                        Box::new(move || t) as Box<dyn Fn() -> DndDropTarget + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };
                let get_m = match m_getter {
                    StyleValue::Static(m) => {
                        Box::new(move || m) as Box<dyn Fn() -> DndDragPayload + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };

                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let t = get_t();
                        let m = get_m();
                        cx.states.dnd.dnd_drop_properties.insert(
                            id,
                            DndDropProperty {
                                target: t,
                                drag_mode: m,
                            },
                        );
                        cx.mark_render_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Sets the style to apply to the original element while it is being dnd dragged.
    #[inline]
    #[must_use]
    pub fn dnd_draggable_original(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(
            style,
            ComponentMask::STATE_DND_DRAGGING,
            StyleTarget::DndDragging,
        )
    }

    /// Sets the style to apply to the placeholder while it is being dnd dragged.
    #[inline]
    #[must_use]
    pub fn dnd_draggable_placeholder(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(
            style,
            ComponentMask::STATE_DND_DRAG_OVER,
            StyleTarget::DndDragOver,
        )
    }

    /// Sets the style to be applied to the dnd drop zone when a dnd dragged element is hovering over it.
    #[inline]
    #[must_use]
    pub fn dnd_drag_over(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(
            style,
            ComponentMask::STATE_DND_DRAG_IN,
            StyleTarget::DndDragIn,
        )
    }

    /// Sets the transparency of pointer events (such as mouse interactions).
    ///
    /// Default is [`PointerEvents::Auto`]
    #[inline]
    #[must_use]
    pub fn pointer_events(mut self, value: impl IntoStyleValue<PointerEvents>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.pointer_events = Some(v);
                inner.mask.set(ComponentMask::STYLE_POINTER_EVENTS);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_POINTER_EVENTS);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        cx.get_visual_property_mut(id, target).pointer_events = Some(val);
                        cx.mark_render_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Sets the transparency of pointer events to a [`PointerEvents::None`].
    #[inline]
    #[must_use]
    pub fn pointer_events_none(self) -> Self {
        self.pointer_events(PointerEvents::None)
    }

    /// Sets the transparency of pointer events to a [`PointerEvents::Auto`].
    #[inline]
    #[must_use]
    pub fn pointer_events_auto(self) -> Self {
        self.pointer_events(PointerEvents::Auto)
    }

    /// Apply an affine transformation (translation, scaling, rotation) to the element.
    #[inline]
    #[must_use]
    pub fn transform(mut self, value: impl IntoStyleValue<Transform>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.transform = Some(v.matrix);
                inner.mask.set(ComponentMask::STYLE_TRANSFORM);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_TRANSFORM);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).transform = Some(val.matrix);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the reference point for when an element transforms.
    #[inline]
    #[must_use]
    pub fn transform_origin(mut self, point: impl IntoStylePoint<f32>) -> Self {
        match point.into_style_point() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.transform_origin = Some(v);
                inner.mask.set(ComponentMask::STYLE_TRANSFORM);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_TRANSFORM);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).transform_origin = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Apply an affine transformation to the element to a scale.
    #[inline]
    #[must_use]
    pub fn transform_scale(self, x: f32, y: f32) -> Self {
        let current = self
            .inner
            .visual_property
            .transform
            .map(|m| Transform { matrix: m })
            .unwrap_or_default();
        self.transform(current.scale(x, y))
    }

    /// Apply an affine transformation to the element to a translate.
    #[inline]
    #[must_use]
    pub fn transform_translate(self, x: f32, y: f32) -> Self {
        let current = self
            .inner
            .visual_property
            .transform
            .map(|m| Transform { matrix: m })
            .unwrap_or_default();
        self.transform(current.translate(x, y))
    }

    /// Apply an affine transformation to the element to a rotate.
    #[inline]
    #[must_use]
    pub fn transform_rotate(self, radians: f32) -> Self {
        let current = self
            .inner
            .visual_property
            .transform
            .map(|m| Transform { matrix: m })
            .unwrap_or_default();
        self.transform(current.rotate(radians))
    }

    /// Sets whether to inherit the parent's transform.
    #[inline]
    #[must_use]
    pub fn transform_inherit(mut self, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.transform_inherit = Some(v);
                inner.mask.set(ComponentMask::STYLE_TRANSFORM_INHERIT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_TRANSFORM_INHERIT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).transform_inherit = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the transitions that occur when the state changes.
    #[inline]
    #[must_use]
    pub fn transition(mut self, transition: impl IntoStyleValue<Transition>) -> Self {
        match transition.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.transitions.push(v);
                inner.mask.set(ComponentMask::STYLE_TRANSITIONS);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_TRANSITIONS);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        cx.get_visual_property_mut(id, target).transitions.push(val);
                        cx.mark_render_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Sets the transitions that occur when the background color state changes.
    #[inline]
    #[must_use]
    pub fn trans_bg_color(self, duration: Duration, curve: AnimationCurve) -> Self {
        self.transition(Transition::new(
            PropertyList::BackgroundColor,
            duration,
            curve,
        ))
    }

    /// Sets the transitions that occur when the border color state changes.
    #[inline]
    #[must_use]
    pub fn trans_border_color(self, duration: Duration, curve: AnimationCurve) -> Self {
        self.transition(Transition::new(PropertyList::BorderColor, duration, curve))
    }

    /// Sets the transitions that occur when the box shadow state changes.
    #[inline]
    #[must_use]
    pub fn trans_box_shadow(self, duration: Duration, curve: AnimationCurve) -> Self {
        self.transition(Transition::new(PropertyList::BoxShadow, duration, curve))
    }

    /// Sets the transitions that occur when the corner radius state changes.
    #[inline]
    #[must_use]
    pub fn trans_corner_radius(self, duration: Duration, curve: AnimationCurve) -> Self {
        self.transition(Transition::new(PropertyList::CornerRadius, duration, curve))
    }

    /// Sets the transitions that occur when the opacity state changes.
    #[inline]
    #[must_use]
    pub fn trans_opacity(self, duration: Duration, curve: AnimationCurve) -> Self {
        self.transition(Transition::new(PropertyList::Opacity, duration, curve))
    }

    /// Sets the transitions that occur when the transform state changes.
    #[inline]
    #[must_use]
    pub fn trans_transform(self, duration: Duration, curve: AnimationCurve) -> Self {
        self.transition(Transition::new(PropertyList::Transform, duration, curve))
    }

    /// Sets the transitions that occur when the size state changes.
    #[inline]
    #[must_use]
    pub fn trans_size(self, duration: Duration, curve: AnimationCurve) -> Self {
        self.transition(Transition::new(PropertyList::Size, duration, curve))
    }

    /// Sets the transitions that occur when the width state changes.
    #[inline]
    #[must_use]
    pub fn trans_width(self, duration: Duration, curve: AnimationCurve) -> Self {
        self.transition(Transition::new(PropertyList::Width, duration, curve))
    }

    /// Sets the transitions that occur when the height state changes.
    #[inline]
    #[must_use]
    pub fn trans_height(self, duration: Duration, curve: AnimationCurve) -> Self {
        self.transition(Transition::new(PropertyList::Height, duration, curve))
    }

    /// Set up keyframe animation.
    #[inline]
    #[must_use]
    pub fn animation(mut self, animation: impl IntoStyleValue<KeyframeAnimation>) -> Self {
        match animation.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.keyframe_animations.push(v);
                inner.mask.set(ComponentMask::STYLE_ANIMATIONS);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_ANIMATIONS);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        cx.get_visual_property_mut(id, target)
                            .keyframe_animations
                            .push(val);
                        cx.mark_render_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Set a two-color linear gradient as the background.
    #[inline]
    #[must_use]
    pub fn bg_gradient(mut self, gradient: impl IntoStyleValue<LinearGradient>) -> Self {
        match gradient.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.bg_gradient = Some(v);
                inner.mask.set(ComponentMask::STYLE_BG_COLOR);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_BG_COLOR);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).bg_gradient = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the font family for the text.
    ///
    /// Default is [`cosmic_text::Family::SansSerif`]
    #[inline]
    #[must_use]
    pub fn font_family(mut self, family: impl IntoStyleValue<Cow<'static, str>>) -> Self {
        match family.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.font.family = Some(v);
                inner.mask.set(ComponentMask::STYLE_FONT_STYLE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_FONT_STYLE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).font.family = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the font weight.
    ///
    /// Default is 400
    #[inline]
    #[must_use]
    pub fn font_weight(mut self, weight: impl IntoStyleValue<u32>) -> Self {
        match weight.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.font.weight = Some(v);
                inner.mask.set(ComponentMask::STYLE_FONT_STYLE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_FONT_STYLE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).font.weight = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the font size.
    ///
    /// Default is 16.0
    #[inline]
    #[must_use]
    pub fn font_size(mut self, size: impl IntoStyleValue<f32>) -> Self {
        match size.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.font.size = Some(v);
                inner.mask.set(ComponentMask::STYLE_FONT_SIZE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_FONT_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).font.size = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the font style.
    ///
    /// 1: [`cosmic_text::Style::Italic`]
    ///
    /// 2: [`cosmic_text::Style::Oblique`]
    ///
    /// other (default): [`cosmic_text::Style::Normal`]
    #[inline]
    #[must_use]
    pub fn font_style(mut self, style: impl IntoStyleValue<u32>) -> Self {
        match style.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.font.style = Some(v);
                inner.mask.set(ComponentMask::STYLE_FONT_STYLE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_FONT_STYLE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).font.style = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Set up automatic wrapping.
    #[inline]
    #[must_use]
    pub fn auto_wrap(mut self, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.auto_wrap = Some(v);
                inner.mask.set(ComponentMask::STYLE_AUTO_WRAP);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_AUTO_WRAP);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).auto_wrap = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the behavior of text selection by the user.
    ///
    /// Default is [`UserSelect::None`]
    #[inline]
    #[must_use]
    pub fn user_select(mut self, value: impl IntoStyleValue<UserSelect>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.user_select = Some(v);
                inner.mask.set(ComponentMask::STYLE_USER_SELECT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_USER_SELECT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).user_select = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Sets the behavior of text selection by the user to a [`UserSelect::Text`].
    #[inline]
    #[must_use]
    pub fn select_text(self) -> Self {
        self.user_select(UserSelect::Text)
    }

    /// Sets the behavior of text selection by the user to a [`UserSelect::All`].
    #[inline]
    #[must_use]
    pub fn select_all(self) -> Self {
        self.user_select(UserSelect::All)
    }

    /// Sets the behavior of text selection by the user to a [`UserSelect::None`].
    #[inline]
    #[must_use]
    pub fn select_none(self) -> Self {
        self.user_select(UserSelect::None)
    }

    /// Sets the highlight color when text is selected.
    #[inline]
    #[must_use]
    pub fn select_bg_color(mut self, color: impl IntoStyleValue<Color>) -> Self {
        match color.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.select_bg_color = Some(v);
                inner.mask.set(ComponentMask::STYLE_USER_SELECT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_USER_SELECT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).select_bg_color = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// This sets the text color when text is selected.
    #[inline]
    #[must_use]
    pub fn select_text_color(mut self, color: impl IntoStyleValue<Color>) -> Self {
        match color.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.select_text_color = Some(v);
                inner.mask.set(ComponentMask::STYLE_USER_SELECT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_USER_SELECT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).select_text_color = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Allow focus on the element and set the behavior of style inheritance.
    ///
    /// Default is [`Focusable::None`]
    ///
    /// Default is [`FocusTrigger::Both`]
    #[inline]
    #[must_use]
    pub fn focusable(mut self, value: impl IntoStyleValue<Focusable>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.focusable = Some(v);
                inner.mask.set(ComponentMask::STYLE_FOCUSABLE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_FOCUSABLE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).focusable = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// Makes the element focusable and inherits the focus style of its parent.
    ///
    /// Default is [`FocusTrigger::Both`]
    #[inline]
    #[must_use]
    pub fn focusable_inherit(self, trigger: FocusTrigger) -> Self {
        self.focusable(Focusable::Inherit(trigger))
    }

    /// Makes the element focusable and inherits the focus style of its parent.
    #[inline]
    #[must_use]
    pub fn focusable_inherit_both(self) -> Self {
        self.focusable(Focusable::Inherit(FocusTrigger::Both))
    }

    /// Makes the element mouse focusable and inherits the focus style of its parent.
    #[inline]
    #[must_use]
    pub fn focusable_inherit_mouse(self) -> Self {
        self.focusable(Focusable::Inherit(FocusTrigger::Mouse))
    }

    /// Makes the element keyboard focusable and inherits the focus style of its parent.
    #[inline]
    #[must_use]
    pub fn focusable_inherit_keyboard(self) -> Self {
        self.focusable(Focusable::Inherit(FocusTrigger::Keyboard))
    }

    /// Makes the element focusable and uses its own focus style.
    ///
    /// /// Default is [`FocusTrigger::Both`]
    #[inline]
    #[must_use]
    pub fn focusable_self(self, trigger: FocusTrigger) -> Self {
        self.focusable(Focusable::SelfStyle(trigger))
    }

    /// Makes the element focusable and uses its own focus style.
    #[inline]
    #[must_use]
    pub fn focusable_self_both(self) -> Self {
        self.focusable(Focusable::SelfStyle(FocusTrigger::Both))
    }

    /// Makes the element mouse focusable and uses its own focus style.
    #[inline]
    #[must_use]
    pub fn focusable_self_mouse(self) -> Self {
        self.focusable(Focusable::SelfStyle(FocusTrigger::Mouse))
    }

    /// Makes the element keyboard focusable and uses its own focus style.
    #[inline]
    #[must_use]
    pub fn focusable_self_keyboard(self) -> Self {
        self.focusable(Focusable::SelfStyle(FocusTrigger::Keyboard))
    }

    /// Makes the element unfocusable.
    #[inline]
    #[must_use]
    pub fn focusable_none(self) -> Self {
        self.focusable(Focusable::None)
    }

    /// This setting prevents the element from taking focus from the currently active element when it is clicked.
    #[inline]
    #[must_use]
    pub fn prevent_focus_steal(mut self, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.prevent_focus_steal = Some(v);
                inner.mask.set(ComponentMask::STYLE_PREVENT_FOCUS_STEAL);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_PREVENT_FOCUS_STEAL);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target).prevent_focus_steal = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// This setting prevents a descendant element from stealing focus
    /// from the currently active element when it is clicked.
    #[inline]
    #[must_use]
    pub fn prevent_focus_steal_within(mut self, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.prevent_focus_steal_within = Some(v);
                inner
                    .mask
                    .set(ComponentMask::STYLE_PREVENT_FOCUS_STEAL_WITHIN);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner
                    .mask
                    .set(ComponentMask::STYLE_PREVENT_FOCUS_STEAL_WITHIN);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    cx.get_visual_property_mut(id, target)
                        .prevent_focus_steal_within = Some(val);
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    // すべての疑似クラスおよび within 伝播系のスタイルと動的セッターを統合する共通ヘルパー
    #[track_caller]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::unreachable)]
    fn apply_interaction_style(
        mut self,
        style: impl IntoStyleValue<ThisStyle>,
        state_flag: u128,
        target: StyleTarget,
    ) -> Self {
        match style.into_style_value() {
            // 静的に構築されたスタイルが渡された場合
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);

                // ネストされた子スタイルが持つ動的セッター群を引き上げ、
                // 実行時に親ターゲットに補正して転送するセッターを登録。
                if !v.inner.dynamic_setters.is_empty() {
                    let v_setters = v.inner.dynamic_setters.clone();
                    inner
                        .dynamic_setters
                        .push(Arc::new(move |cx, id, _parent_target| {
                            for setter in &v_setters {
                                setter(cx, id, target);
                            }
                        }));
                }

                // interaction_styles の該当疑似クラススロットへアタッチ
                let interaction = &mut inner.interaction_styles;
                match target {
                    StyleTarget::Hovered => interaction.hovered = Some(v),
                    StyleTarget::Focused => interaction.focused = Some(v),
                    StyleTarget::FocusedVisible => interaction.focused_visible = Some(v),
                    StyleTarget::Pressed => interaction.pressed = Some(v),
                    StyleTarget::Disabled => interaction.disabled = Some(v),
                    StyleTarget::Actived => interaction.actived = Some(v),
                    StyleTarget::Selected => interaction.selected = Some(v),
                    StyleTarget::Dragged => interaction.dragged = Some(v),
                    StyleTarget::DndDragging => interaction.dragging = Some(v),
                    StyleTarget::DndDragIn => interaction.drag_in = Some(v),
                    StyleTarget::DndDragOver => interaction.drag_over = Some(v),

                    StyleTarget::HoveredWithin => interaction.hovered_within = Some(v),
                    StyleTarget::FocusedWithin => interaction.focused_within = Some(v),
                    StyleTarget::FocusedVisibleWithin => {
                        interaction.focused_visible_within = Some(v);
                    }
                    StyleTarget::PressedWithin => interaction.pressed_within = Some(v),
                    StyleTarget::DisabledWithin => interaction.disabled_within = Some(v),
                    StyleTarget::ActivedWithin => interaction.actived_within = Some(v),
                    StyleTarget::SelectedWithin => interaction.selected_within = Some(v),
                    StyleTarget::DraggedWithin => interaction.dragged_within = Some(v),
                    StyleTarget::AnyWithin => interaction.any_within = Some(v),
                    StyleTarget::Base => {
                        unreachable!(
                            "\n\
                            Internal invariant violated. This is a bug in michiu_ui.\n\
                            Please report this issue at https://github.com/eto0202/michiu/issues\n\
                             [StyleValue Static]\n\
                             [Style]       : {v:?}\n\
                             [flag]        : {state_flag},\n\
                             [target]      : {target:?},\n\
                             [interaction] : {interaction:?}\n\
                             [loc]         : {}\n\
                            ",
                            std::panic::Location::caller()
                        )
                    }

                    StyleTarget::HoveredParent => interaction.hovered_parent = Some(v),
                    StyleTarget::FocusedParent => interaction.focused_parent = Some(v),
                    StyleTarget::FocusedVisibleParent => {
                        interaction.focused_visible_parent = Some(v);
                    }
                    StyleTarget::PressedParent => interaction.pressed_parent = Some(v),
                    StyleTarget::DisabledParent => interaction.disabled_parent = Some(v),
                    StyleTarget::ActivedParent => interaction.actived_parent = Some(v),
                    StyleTarget::SelectedParent => interaction.selected_parent = Some(v),
                    StyleTarget::DraggedParent => interaction.dragged_parent = Some(v),
                    StyleTarget::AnyParent => interaction.any_parent = Some(v),
                }
                inner.mask.set(state_flag);
            }

            // 疑似クラス自体が consume 等の遅延ゲッターで上書きされた場合
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(state_flag);
                inner
                    .dynamic_setters
                    .push(Arc::new(move |cx, id, _parent_target| {
                        let val = getter();

                        if !cx.renders.rnd_interaction.contains_key(id) {
                            cx.renders
                                .rnd_interaction
                                .insert(id, InteractionStyles::default());
                        }
                        // 上で入れたばっかなので Some のはず
                        let styles = cx.renders.rnd_interaction.at_mut(id);

                        // 動的に解決されたスタイルを対応する疑似フィールドへ上書きマウント
                        match target {
                            StyleTarget::Hovered => styles.hovered = Some(val.clone()),
                            StyleTarget::Focused => styles.focused = Some(val.clone()),
                            StyleTarget::FocusedVisible => {
                                styles.focused_visible = Some(val.clone());
                            }
                            StyleTarget::Pressed => styles.pressed = Some(val.clone()),
                            StyleTarget::Disabled => styles.disabled = Some(val.clone()),
                            StyleTarget::Actived => styles.actived = Some(val.clone()),
                            StyleTarget::Selected => styles.selected = Some(val.clone()),
                            StyleTarget::Dragged => styles.dragged = Some(val.clone()),
                            StyleTarget::DndDragging => styles.dragging = Some(val.clone()),
                            StyleTarget::DndDragIn => styles.drag_in = Some(val.clone()),
                            StyleTarget::DndDragOver => styles.drag_over = Some(val.clone()),

                            StyleTarget::HoveredWithin => styles.hovered_within = Some(val.clone()),
                            StyleTarget::FocusedWithin => styles.focused_within = Some(val.clone()),
                            StyleTarget::FocusedVisibleWithin => {
                                styles.focused_visible_within = Some(val.clone());
                            }
                            StyleTarget::PressedWithin => styles.pressed_within = Some(val.clone()),
                            StyleTarget::DisabledWithin => {
                                styles.disabled_within = Some(val.clone());
                            }
                            StyleTarget::ActivedWithin => styles.actived_within = Some(val.clone()),
                            StyleTarget::SelectedWithin => {
                                styles.selected_within = Some(val.clone());
                            }
                            StyleTarget::DraggedWithin => styles.dragged_within = Some(val.clone()),
                            StyleTarget::AnyWithin => styles.any_within = Some(val.clone()),
                            StyleTarget::Base => {
                                unreachable!(
                                    "\n\
                                    Internal invariant violated. This is a bug in michiu_ui.\n\
                                    Please report this issue at https://github.com/eto0202/michiu/issues\n\
                                     [StyleValue Dynamic]\n\
                                     [style]       : {val:?}\n\
                                     [flag]        : {state_flag},\n\
                                     [target]      : {target:?},\n\
                                     [interaction] : {styles:?}\n\
                                     [loc]         : {}\n\
                                    ",
                                    std::panic::Location::caller()
                                )
                            }

                            StyleTarget::HoveredParent => styles.hovered_parent = Some(val.clone()),
                            StyleTarget::FocusedParent => styles.focused_parent = Some(val.clone()),
                            StyleTarget::FocusedVisibleParent => {
                                styles.focused_visible_parent = Some(val.clone());
                            }
                            StyleTarget::PressedParent => styles.pressed_parent = Some(val.clone()),
                            StyleTarget::DisabledParent => {
                                styles.disabled_parent = Some(val.clone());
                            }
                            StyleTarget::ActivedParent => styles.actived_parent = Some(val.clone()),
                            StyleTarget::SelectedParent => {
                                styles.selected_parent = Some(val.clone());
                            }
                            StyleTarget::DraggedParent => styles.dragged_parent = Some(val.clone()),
                            StyleTarget::AnyParent => styles.any_parent = Some(val.clone()),
                        }

                        // 動的スタイルの内部に存在するセッターも、その場で即時にターゲット解決を実行
                        for setter in &val.inner.dynamic_setters {
                            setter(cx, id, target);
                        }

                        cx.mark_render_dirty(id);
                    }));
            }
        }
        self
    }

    /// Creates and returns a new [`ThisStyle`] that merges the other styles.
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        let mut merged = self.clone();
        let inner_mut = Arc::make_mut(&mut merged.inner);
        let other_inner = &other.inner;

        // ビットマスクのマージ
        inner_mut.mask.merge(other_inner.mask);

        // 基本レイアウトプロパティの上書き
        if other_inner.mask.has_basic_layout() {
            inner_mut
                .basic_layout
                .override_with(&other_inner.basic_layout, other_inner.mask);
        }

        // Flexレイアウトプロパティの上書き
        if other_inner.mask.has_flex_layout() {
            inner_mut
                .flex_layout
                .override_with(&other_inner.flex_layout, other_inner.mask);
        }

        // ビジュアルプロパティの上書き
        if other_inner.mask.has_visual_property() {
            inner_mut
                .visual_property
                .override_with(&other_inner.visual_property, other_inner.mask);
        }

        // 疑似クラスの上書き
        if other_inner.mask.has_interaction_property()
            || other_inner
                .mask
                .has(ComponentMask::STYLE_INTERACTION_WITHIN)
            || other_inner
                .mask
                .has(ComponentMask::STYLE_INTERACTION_PARENT)
        {
            inner_mut
                .interaction_styles
                .override_with(&other_inner.interaction_styles, other_inner.mask);
        }

        // コールドデータのコピー
        if other_inner.mask.has_grid_layout()
            && let Some(ref g) = other_inner.grid_layout
        {
            inner_mut.grid_layout = Some(g.clone());
        }
        if let Some(ref sb) = other_inner.scrollbar_style {
            inner_mut.scrollbar_style = Some(sb.clone());
        }

        // 動的セッターの結合
        inner_mut
            .dynamic_setters
            .extend(other_inner.dynamic_setters.clone());

        // D&D設定
        if other_inner.drag_property.is_some() {
            inner_mut.drag_property = other_inner.drag_property;
        }
        if other_inner.drop_property.is_some() {
            inner_mut.drop_property = other_inner.drop_property;
        }

        merged
    }
}

#[cfg(test)]
mod tests;
