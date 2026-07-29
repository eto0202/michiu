use crate::*;
use std::{borrow::Cow, sync::Arc, time::Duration};

// コールドデータである複雑なGridトラック設定のみTaffyからそのまま拝借
use taffy::{GridTemplateArea, GridTemplateComponent, TrackSizingFunction};

/// スレッド安全な動的クロージャをサポートする StyleValue の定義
pub enum StyleValue<T> {
    Static(T),
    Dynamic(Box<dyn Fn() -> T + Send + Sync + 'static>),
}

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
    // 動的にスタイルプロパティを更新するためのクローン可能なセッターリスト
    pub(crate) dynamic_setters: DynamicSettersType,

    pub(crate) drag_property: Option<DragProperty>,
    pub(crate) drop_property: Option<DropProperty>,
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
    Dragging, // ドラッグ中の元の要素
    DragIn,   // ドロップゾーン侵入時
    DragOver, // プレースホルダー（ドラッグイメージ）

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

    #[inline]
    pub fn debug_border_red(self) -> Self {
        self.border_solid(1.0).border_color(Color::RED)
    }

    /// 要素の表示形態（Display）を設定します。
    #[inline]
    pub fn display(mut self, value: impl IntoStyleValue<Display>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.display = v;
                inner.mask.set(STYLE_DISPLAY);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_DISPLAY);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.basic_layouts.get_mut(id) {
                            v.display = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
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
    pub fn item_is_table(mut self, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.item_is_table = v;
                inner.mask.set(STYLE_ITEM_IS_TABLE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_ITEM_IS_TABLE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.basic_layouts.get_mut(id) {
                            v.item_is_table = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// 要素が置換要素（画像やビデオなど）かどうかを設定します。
    #[inline]
    pub fn item_is_replaced(mut self, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.item_is_replaced = v;
                inner.mask.set(STYLE_ITEM_IS_REPLACED);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_ITEM_IS_REPLACED);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.basic_layouts.get_mut(id) {
                            v.item_is_replaced = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// ボックスモデルの算出基準（BoxSizing）を設定します。
    #[inline]
    pub fn box_sizing(mut self, value: impl IntoStyleValue<BoxSizing>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.box_sizing = v;
                inner.mask.set(STYLE_BOX_SIZING);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BOX_SIZING);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.basic_layouts.get_mut(id) {
                            v.box_sizing = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
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
    pub fn direction(mut self, value: impl IntoStyleValue<Direction>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.direction = v;
                inner.mask.set(STYLE_DIRECTION);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_DIRECTION);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.basic_layouts.get_mut(id) {
                            v.direction = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// スクロールバーのスタイル（太さ、トラック、サム、および疑似クラス）を登録します。
    #[inline]
    pub fn scrollbar(mut self, style: impl IntoStyleValue<ScrollbarStyle>) -> Self {
        match style.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.scrollbar_style = Some(v);
                inner.mask.set(STYLE_SCROLLBAR);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_SCROLLBAR);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.scrollbar_styles.get_mut(id) {
                            v.style = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// スクロールバーの太さ（物理幅/高さ）を直接指定します。
    #[inline]
    pub fn scrollbar_width(mut self, width: impl IntoStyleValue<f32>) -> Self {
        match width.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let sb = inner
                    .scrollbar_style
                    .get_or_insert_with(ScrollbarStyle::default);
                sb.width = v;
                inner.mask.set(STYLE_SCROLLBAR);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_SCROLLBAR);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.scrollbar_styles.get_mut(id) {
                            v.style.width = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// スクロールバーの表示条件（None / Always / Auto）を直接指定します。
    #[inline]
    pub fn scrollbar_display(mut self, display: impl IntoStyleValue<ScrollbarDisplay>) -> Self {
        match display.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let sb = inner
                    .scrollbar_style
                    .get_or_insert_with(ScrollbarStyle::default);
                sb.display = v;
                inner.mask.set(STYLE_SCROLLBAR);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_SCROLLBAR);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.scrollbar_styles.get_mut(id) {
                            v.style.display = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
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

    /// スクロールバーの配置モード（Layout / Overlay）を直接指定します。
    #[inline]
    pub fn scrollbar_mode(mut self, mode: impl IntoStyleValue<ScrollbarMode>) -> Self {
        match mode.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let sb = inner
                    .scrollbar_style
                    .get_or_insert_with(ScrollbarStyle::default);
                sb.mode = v;
                inner.mask.set(STYLE_SCROLLBAR);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_SCROLLBAR);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.scrollbar_styles.get_mut(id) {
                            v.style.mode = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// コンテンツのはみ出し処理（Overflow）を設定します。
    #[inline]
    pub fn overflow(mut self, value: impl IntoStyleValue<LayoutOverflow>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.overflow = v;
                inner.mask.set(STYLE_OVERFLOW);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_OVERFLOW);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.basic_layouts.get_mut(id) {
                            v.overflow = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
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

    /// x軸方向のはみ出し処理を個別に設定します（動的セッター対応）
    #[inline]
    pub fn overflow_x(mut self, value: impl IntoStyleValue<Overflow>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.overflow.x = v;
                inner.mask.set(STYLE_OVERFLOW);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_OVERFLOW);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.basic_layouts.get_mut(id) {
                            v.overflow.x = val; // x軸のみを安全に更新（y軸の動的設定を破壊しない）
                        }
                        cx.mark_layout_dirty(id); // クリック境界が動くため必須
                    }
                }));
            }
        }
        self
    }

    /// y軸方向のはみ出し処理を個別に設定します（動的セッター対応）
    #[inline]
    pub fn overflow_y(mut self, value: impl IntoStyleValue<Overflow>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.overflow.y = v;
                inner.mask.set(STYLE_OVERFLOW);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_OVERFLOW);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.basic_layouts.get_mut(id) {
                            v.overflow.y = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    #[inline]
    pub fn overflow_x_auto(self) -> Self {
        self.overflow_x(Overflow::Visible)
    }

    #[inline]
    pub fn overflow_x_hidden(self) -> Self {
        self.overflow_x(Overflow::Hidden)
    }

    #[inline]
    pub fn overflow_x_scroll(self) -> Self {
        self.overflow_x(Overflow::Scroll)
    }

    #[inline]
    pub fn overflow_x_clip(self) -> Self {
        self.overflow_x(Overflow::Clip)
    }

    #[inline]
    pub fn overflow_y_auto(self) -> Self {
        self.overflow_y(Overflow::Visible)
    }

    #[inline]
    pub fn overflow_y_hidden(self) -> Self {
        self.overflow_y(Overflow::Hidden)
    }

    #[inline]
    pub fn overflow_y_scroll(self) -> Self {
        self.overflow_y(Overflow::Scroll)
    }

    #[inline]
    pub fn overflow_y_clip(self) -> Self {
        self.overflow_y(Overflow::Clip)
    }

    /// 要素の配置基準（Position）を設定します。
    #[inline]
    pub fn position(mut self, value: impl IntoStyleValue<Position>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.position = v;
                inner.mask.set(STYLE_POSITION);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_POSITION);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.basic_layouts.get_mut(id) {
                            v.position = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
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
    pub fn inset(mut self, value: impl IntoStyleRect<Val>) -> Self {
        match value.into_style_rect() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.inset = v;
                inner.mask.set(STYLE_INSET);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_INSET);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.inset = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// 要素を絶対配置 (Position::Absolute) に設定し、同時に配置インセットを設定します。
    #[inline]
    pub fn absolute_inset(self, value: impl IntoStyleRect<Val>) -> Self {
        self.position(Position::Absolute).inset(value)
    }

    /// 左右の配置インセット（left, right）を一括設定します。
    #[inline]
    pub fn inset_x(mut self, value: impl IntoStyleSize<Val>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let current_top = self.inner.basic_layout.inset.top;
                let current_bottom = self.inner.basic_layout.inset.bottom;
                self.inset((current_top, v.width, current_bottom, v.height))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_INSET);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let size = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.inset.right = size.width;
                        v.inset.left = size.height;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// 上下の配置インセット（top, bottom）を一括設定します。
    #[inline]
    pub fn inset_y(mut self, value: impl IntoStyleSize<Val>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let current_left = self.inner.basic_layout.inset.left;
                let current_right = self.inner.basic_layout.inset.right;
                self.inset((v.width, current_right, v.height, current_left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_INSET);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let size = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.inset.top = size.width;
                        v.inset.bottom = size.height;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    #[inline]
    pub fn top(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.inset;
                self.inset((v, current.right, current.bottom, current.left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_INSET);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.inset.top = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    #[inline]
    pub fn right(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.inset;
                self.inset((current.top, v, current.bottom, current.left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_INSET);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.inset.right = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    #[inline]
    pub fn bottom(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.inset;
                self.inset((current.top, current.right, v, current.left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_INSET);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.inset.bottom = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    #[inline]
    pub fn left(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.inset;
                self.inset((current.top, current.right, current.bottom, v))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_INSET);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.inset.left = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// 要素の基本サイズ（width, height）を設定します。
    #[inline]
    pub fn size(mut self, value: impl IntoStyleSize<Val>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.size = v;
                inner.mask.set(STYLE_SIZE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.size = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    #[inline]
    pub fn size_full(self) -> Self {
        self.size(pct(100.0))
    }

    #[inline]
    pub fn size_half(self) -> Self {
        self.size(pct(50.0))
    }

    #[inline]
    pub fn size_auto(self) -> Self {
        self.size(auto())
    }

    /// 要素の幅（width）のみを設定します（高さは既存の値を維持）。
    #[inline]
    pub fn width(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current_h = self.inner.basic_layout.size.height;
                self.size((v, current_h))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.size.width = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    #[inline]
    pub fn w(self, value: impl IntoStyleConvert<Val>) -> Self {
        self.width(value)
    }

    #[inline]
    pub fn w_full(self) -> Self {
        self.width(pct(100.0))
    }

    #[inline]
    pub fn w_half(self) -> Self {
        self.width(pct(50.0))
    }

    #[inline]
    pub fn w_auto(self) -> Self {
        self.width(auto())
    }

    /// 要素の高さ（height）のみを設定します（幅は既存の値を維持）。
    #[inline]
    pub fn height(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current_w = self.inner.basic_layout.size.width;
                self.size((current_w, v))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.size.height = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    #[inline]
    pub fn h(self, value: impl IntoStyleConvert<Val>) -> Self {
        self.height(value)
    }

    #[inline]
    pub fn h_full(self) -> Self {
        self.height(pct(100.0))
    }

    #[inline]
    pub fn h_half(self) -> Self {
        self.height(pct(50.0))
    }

    #[inline]
    pub fn h_auto(self) -> Self {
        self.height(auto())
    }

    /// 要素の最小サイズを設定します。
    #[inline]
    pub fn min_size(mut self, value: impl IntoStyleSize<Val>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.min_size = v;
                inner.mask.set(STYLE_MIN_SIZE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_MIN_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.min_size = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// 要素の最大サイズを設定します。
    #[inline]
    pub fn max_size(mut self, value: impl IntoStyleSize<Val>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.max_size = v;
                inner.mask.set(STYLE_MAX_SIZE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_MAX_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.max_size = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// 要素の最小幅（min_width）のみを設定します。
    #[inline]
    pub fn min_width(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current_h = self.inner.basic_layout.min_size.height;
                self.min_size((v, current_h))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_MIN_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.min_size.width = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// 要素の最小高さ（min_height）のみを設定します。
    #[inline]
    pub fn min_height(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current_w = self.inner.basic_layout.min_size.width;
                self.min_size((current_w, v))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_MIN_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.min_size.height = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// 要素の最大幅（max_width）のみを設定します。
    #[inline]
    pub fn max_width(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current_h = self.inner.basic_layout.max_size.height;
                self.max_size((v, current_h))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_MAX_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.max_size.width = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// 要素の最大高さ（max_height）のみを設定します。
    #[inline]
    pub fn max_height(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current_w = self.inner.basic_layout.max_size.width;
                self.max_size((current_w, v))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_MAX_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.max_size.height = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// 任意の比率（幅 / 高さ）でアスペクト比を設定します。
    #[inline]
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
                inner.mask.set(STYLE_ASPECT_RATIO);
            }
            (w_getter, h_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_ASPECT_RATIO);

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
                        if let Some(v) = cx.layouts.basic_layouts.get_mut(id) {
                            if h <= 0.0 {
                                v.aspect_ratio = None;
                            } else {
                                v.aspect_ratio = Some(w / h);
                            }
                        }
                        cx.mark_layout_dirty(id); // レイアウト Dirty マーク
                    }
                }));
            }
        }
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
    pub fn margin(mut self, value: impl IntoStyleRect<Val>) -> Self {
        match value.into_style_rect() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.margin = v;
                inner.mask.set(STYLE_MARGIN);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_MARGIN);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.margin = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    #[inline]
    pub fn m(self, value: impl IntoStyleRect<Val>) -> Self {
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
    pub fn m_x(mut self, value: impl IntoStyleSize<Val>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let current_top = self.inner.basic_layout.margin.top;
                let current_bottom = self.inner.basic_layout.margin.bottom;
                self.margin((current_top, v.width, current_bottom, v.height))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_MARGIN);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let size = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.margin.right = size.width;
                        v.margin.left = size.height;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// 上下の外側余白（margin-top, margin-bottom）を一括設定します。
    #[inline]
    pub fn m_y(mut self, value: impl IntoStyleSize<Val>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let current_left = self.inner.basic_layout.margin.left;
                let current_right = self.inner.basic_layout.margin.right;
                self.margin((v.width, current_right, v.height, current_left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_MARGIN);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let size = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.margin.top = size.width;
                        v.margin.bottom = size.height;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    pub fn m_t(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.margin;
                self.margin((v, current.right, current.bottom, current.left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_MARGIN);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.margin.top = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    #[inline]
    pub fn m_r(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.margin;
                self.margin((current.top, v, current.bottom, current.left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_MARGIN);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.margin.right = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    #[inline]
    pub fn m_b(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.margin;
                self.margin((current.top, current.right, v, current.left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_MARGIN);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.margin.bottom = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    #[inline]
    pub fn m_l(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.margin;
                self.margin((current.top, current.right, current.bottom, v))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_MARGIN);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.margin.left = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// 内側余白（padding）を設定します。
    #[inline]
    pub fn padding(mut self, value: impl IntoStyleRect<Length>) -> Self {
        match value.into_style_rect() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.padding = v;
                inner.mask.set(STYLE_PADDING);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_PADDING);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.padding = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    #[inline]
    pub fn p(self, value: impl IntoStyleRect<Length>) -> Self {
        self.padding(value)
    }

    #[inline]
    pub fn p_0(self) -> Self {
        self.padding(0.0)
    }

    /// 左右の内側余白（padding-left, padding-right）を一括設定します。
    #[inline]
    pub fn p_x(mut self, value: impl IntoStyleSize<Length>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let current_top = self.inner.basic_layout.padding.top;
                let current_bottom = self.inner.basic_layout.padding.bottom;
                self.padding((current_top, v.width, current_bottom, v.height))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_PADDING);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let size = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.padding.right = size.width;
                        v.padding.left = size.height;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// 上下の内側余白（padding-top, padding-bottom）を一括設定します。
    #[inline]
    pub fn p_y(mut self, value: impl IntoStyleSize<Length>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let current_left = self.inner.basic_layout.padding.left;
                let current_right = self.inner.basic_layout.padding.right;
                self.padding((v.height, current_left, v.height, current_right))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_PADDING);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let size = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.padding.top = size.width;
                        v.padding.bottom = size.height;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    #[inline]
    pub fn p_t(mut self, value: impl IntoStyleConvert<Length>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.padding;
                self.padding((v, current.right, current.bottom, current.left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_PADDING);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.padding.top = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    #[inline]
    pub fn p_r(mut self, value: impl IntoStyleConvert<Length>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.padding;
                self.padding((current.top, v, current.bottom, current.left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_PADDING);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.padding.right = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    #[inline]
    pub fn p_b(mut self, value: impl IntoStyleConvert<Length>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.padding;
                self.padding((current.top, current.right, v, current.left))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_PADDING);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.padding.bottom = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    #[inline]
    pub fn p_l(mut self, value: impl IntoStyleConvert<Length>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current = self.inner.basic_layout.padding;
                self.padding((current.top, current.right, current.bottom, v))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_PADDING);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.padding.left = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// 境界線の太さ（border）を設定します。
    #[inline]
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
                inner.mask.set(STYLE_BORDER);
            }
            (s_getter, w_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BORDER);

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
                    if let Some(layout) = cx.get_basic_layout_mut(id, target) {
                        layout.border = w;
                    }
                    if let Some(vis) = cx.get_visual_property_mut(id, target) {
                        vis.border_styles = Some([s; 4]);
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// 実線（Solid）の枠線と太さを一括設定します。
    #[inline]
    pub fn border_solid(self, width: impl IntoStyleRect<Length>) -> Self {
        self.border(BorderStyle::Solid, width)
    }

    /// 丸点線（Dotted）の枠線と太さを一括設定します。
    #[inline]
    pub fn border_dotted(self, width: impl IntoStyleRect<Length>) -> Self {
        self.border(BorderStyle::Dotted, width)
    }

    /// 破線（Dashed）の枠線と太さを一括設定します。
    #[inline]
    pub fn border_dashed(self, width: impl IntoStyleRect<Length>) -> Self {
        self.border(BorderStyle::Dashed, width)
    }

    /// 二重線（Double）の枠線と太さを一括設定します。
    #[inline]
    pub fn border_double(self, width: impl IntoStyleRect<Length>) -> Self {
        self.border(BorderStyle::Double, width)
    }

    /// 上枠線（Border Top）の種類と太さを個別に設定します。
    #[inline]
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
                inner.mask.set(STYLE_BORDER);
            }
            (s_getter, v_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BORDER);

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
                    if let Some(layout) = cx.get_basic_layout_mut(id, target) {
                        layout.border.top = v;
                    }
                    if let Some(vis) = cx.get_visual_property_mut(id, target) {
                        let mut styles = vis.border_styles.unwrap_or([BorderStyle::Solid; 4]);
                        styles[0] = s;
                        vis.border_styles = Some(styles);
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// 右枠線（Border Right）の種類と太さを個別に設定します。
    #[inline]
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
                inner.mask.set(STYLE_BORDER);
            }
            (s_getter, v_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BORDER);

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
                    if let Some(layout) = cx.get_basic_layout_mut(id, target) {
                        layout.border.right = v;
                    }
                    if let Some(vis) = cx.get_visual_property_mut(id, target) {
                        let mut styles = vis.border_styles.unwrap_or([BorderStyle::Solid; 4]);
                        styles[1] = s;
                        vis.border_styles = Some(styles);
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// 下枠線（Border Bottom）の種類と太さを個別に設定します。
    #[inline]
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
                inner.mask.set(STYLE_BORDER);
            }
            (s_getter, v_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BORDER);

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
                    if let Some(layout) = cx.get_basic_layout_mut(id, target) {
                        layout.border.bottom = v;
                    }
                    if let Some(vis) = cx.get_visual_property_mut(id, target) {
                        let mut styles = vis.border_styles.unwrap_or([BorderStyle::Solid; 4]);
                        styles[2] = s;
                        vis.border_styles = Some(styles);
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// 左枠線（Border Left）の種類と太さを個別に設定します。
    #[inline]
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
                inner.mask.set(STYLE_BORDER);
            }
            (s_getter, v_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BORDER);

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
                    if let Some(layout) = cx.get_basic_layout_mut(id, target) {
                        layout.border.left = v;
                    }
                    if let Some(vis) = cx.get_visual_property_mut(id, target) {
                        let mut styles = vis.border_styles.unwrap_or([BorderStyle::Solid; 4]);
                        styles[3] = s;
                        vis.border_styles = Some(styles);
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// 四辺個別の枠線の長さ比率（0.0 ~ 1.0）を設定します。
    /// 単一値、2連タプル (縦, 横)、4連タプル (上, 右, 下, 左) を受け入れます。
    #[inline]
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
                inner.mask.set(STYLE_BORDER);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BORDER);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let v = getter();
                    if let Some(vis) = cx.get_visual_property_mut(id, target) {
                        vis.border_lengths = Some(EdgeInsets {
                            top: v.top,
                            right: v.right,
                            bottom: v.bottom,
                            left: v.left,
                        });
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    #[inline]
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
                inner.mask.set(STYLE_BORDER);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BORDER);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(vis) = cx.get_visual_property_mut(id, target) {
                        let mut lengths = vis.border_lengths.unwrap_or(EdgeInsets::px_all(1.0));
                        lengths.top = val;
                        vis.border_lengths = Some(lengths);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    #[inline]
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
                inner.mask.set(STYLE_BORDER);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BORDER);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(vis) = cx.get_visual_property_mut(id, target) {
                        let mut lengths = vis.border_lengths.unwrap_or(EdgeInsets::px_all(1.0));
                        lengths.right = val;
                        vis.border_lengths = Some(lengths);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    #[inline]
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
                inner.mask.set(STYLE_BORDER);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BORDER);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(vis) = cx.get_visual_property_mut(id, target) {
                        let mut lengths = vis.border_lengths.unwrap_or(EdgeInsets::px_all(1.0));
                        lengths.bottom = val;
                        vis.border_lengths = Some(lengths);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    #[inline]
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
                inner.mask.set(STYLE_BORDER);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BORDER);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(vis) = cx.get_visual_property_mut(id, target) {
                        let mut lengths = vis.border_lengths.unwrap_or(EdgeInsets::px_all(1.0));
                        lengths.left = val;
                        vis.border_lengths = Some(lengths);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// すべての辺の枠線基準点（伸縮方向）を一括設定します。
    #[inline]
    pub fn border_align(mut self, value: impl IntoStyleValue<BorderAlignment>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.border_alignments = Some([v; 4]);
                inner.mask.set(STYLE_BORDER);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BORDER);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(vis) = cx.get_visual_property_mut(id, target) {
                        vis.border_alignments = Some([val; 4]);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// 四辺個別の枠線基準点を設定します。[Top, Right, Bottom, Left]
    #[inline]
    pub fn border_aligns(mut self, values: impl IntoStyleValue<[BorderAlignment; 4]>) -> Self {
        match values.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.border_alignments = Some(v);
                inner.mask.set(STYLE_BORDER);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BORDER);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(vis) = cx.get_visual_property_mut(id, target) {
                        vis.border_alignments = Some(val);
                    }
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
                inner.mask.set(STYLE_BORDER);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BORDER);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(vis) = cx.get_visual_property_mut(id, target) {
                        let mut aligns =
                            vis.border_alignments.unwrap_or([BorderAlignment::Start; 4]);
                        aligns[idx] = val;
                        vis.border_alignments = Some(aligns);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    #[inline]
    pub fn border_top_align(self, value: impl IntoStyleValue<BorderAlignment>) -> Self {
        self.set_border_align_idx(0, value)
    }
    #[inline]
    pub fn border_right_align(self, value: impl IntoStyleValue<BorderAlignment>) -> Self {
        self.set_border_align_idx(1, value)
    }
    #[inline]
    pub fn border_bottom_align(self, value: impl IntoStyleValue<BorderAlignment>) -> Self {
        self.set_border_align_idx(2, value)
    }
    #[inline]
    pub fn border_left_align(self, value: impl IntoStyleValue<BorderAlignment>) -> Self {
        self.set_border_align_idx(3, value)
    }

    /// アウトラインの太さとスタイルを一括指定します。
    #[inline]
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
                inner.mask.set(STYLE_OUTLINE);
            }
            (s_getter, w_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_OUTLINE);
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
                    if let Some(vis) = cx.get_visual_property_mut(id, target) {
                        vis.outline_width = Some(EdgeInsets {
                            top: w.top.into(),
                            right: w.right.into(),
                            bottom: w.bottom.into(),
                            left: w.left.into(),
                        });
                        vis.outline_styles = Some([s; 4]);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// 実線（Solid）の枠線と太さを一括設定します。
    #[inline]
    pub fn outline_solid(self, width: impl IntoStyleRect<Length>) -> Self {
        self.outline(BorderStyle::Solid, width)
    }

    /// 丸点線（Dotted）の枠線と太さを一括設定します。
    #[inline]
    pub fn outline_dotted(self, width: impl IntoStyleRect<Length>) -> Self {
        self.outline(BorderStyle::Dotted, width)
    }

    /// 破線（Dashed）の枠線と太さを一括設定します。
    #[inline]
    pub fn outline_dashed(self, width: impl IntoStyleRect<Length>) -> Self {
        self.outline(BorderStyle::Dashed, width)
    }

    /// 二重線（Double）の枠線と太さを一括設定します。
    #[inline]
    pub fn outline_double(self, width: impl IntoStyleRect<Length>) -> Self {
        self.outline(BorderStyle::Double, width)
    }

    /// アウトラインの色を設定します。
    #[inline]
    pub fn outline_color(mut self, value: impl IntoStyleValue<Color>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.outline_color = Some(v);
                inner.mask.set(STYLE_OUTLINE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_OUTLINE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.outline_color = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// 要素とアウトラインとの「隙間（Offset）」を物理ピクセルで設定します。
    #[inline]
    pub fn outline_offset(mut self, value: impl IntoStyleValue<f32>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.outline_offset = Some(v);
                inner.mask.set(STYLE_OUTLINE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_OUTLINE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.outline_offset = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// 四辺のアウトライン個別長さを設定します。
    #[inline]
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
                inner.mask.set(STYLE_OUTLINE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_OUTLINE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let v = getter();
                    if let Some(vis) = cx.get_visual_property_mut(id, target) {
                        vis.outline_lengths = Some(EdgeInsets {
                            top: v.top,
                            right: v.right,
                            bottom: v.bottom,
                            left: v.left,
                        });
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// アウトラインの基準（配置伸縮の方向）を一括設定します。
    #[inline]
    pub fn outline_align(mut self, value: impl IntoStyleValue<BorderAlignment>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.outline_alignments = Some([v; 4]);
                inner.mask.set(STYLE_OUTLINE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_OUTLINE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(vis) = cx.get_visual_property_mut(id, target) {
                        vis.outline_alignments = Some([val; 4]);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    #[inline]
    pub fn outline_weight(mut self, value: impl IntoStyleValue<BorderAlignment>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.outline_alignments = Some([v; 4]);
                inner.mask.set(STYLE_OUTLINE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_OUTLINE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(vis) = cx.get_visual_property_mut(id, target) {
                        vis.outline_alignments = Some([val; 4]);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// コンテナ内の一括交差軸配置を設定します。
    #[inline]
    pub fn align_items(mut self, value: impl IntoStyleValue<Option<AlignItems>>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.align_items = v;
                inner.mask.set(STYLE_ALIGN_ITEMS);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_ALIGN_ITEMS);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_flex_layout_mut(id, target) {
                        v.align_items = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
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
    pub fn align_self(mut self, value: impl IntoStyleValue<Option<AlignSelf>>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.align_self = v;
                inner.mask.set(STYLE_ALIGN_SELF);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_ALIGN_SELF);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_flex_layout_mut(id, target) {
                        v.align_self = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
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
    pub fn justify_items(mut self, value: impl IntoStyleValue<Option<AlignItems>>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.justify_items = v;
                inner.mask.set(STYLE_JUSTIFY_ITEMS);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_JUSTIFY_ITEMS);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_flex_layout_mut(id, target) {
                        v.justify_items = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
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
    pub fn justify_self(mut self, value: impl IntoStyleValue<Option<AlignSelf>>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.justify_self = v;
                inner.mask.set(STYLE_JUSTIFY_SELF);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_JUSTIFY_SELF);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_flex_layout_mut(id, target) {
                        v.justify_self = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
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
    pub fn align_content(mut self, value: impl IntoStyleValue<Option<AlignContent>>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.align_content = v;
                inner.mask.set(STYLE_ALIGN_CONTENT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_ALIGN_CONTENT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_flex_layout_mut(id, target) {
                        v.align_content = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
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
    pub fn justify_content(mut self, value: impl IntoStyleValue<Option<JustifyContent>>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.justify_content = v;
                inner.mask.set(STYLE_JUSTIFY_CONTENT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_JUSTIFY_CONTENT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_flex_layout_mut(id, target) {
                        v.justify_content = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
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
    pub fn gap(mut self, value: impl IntoStyleSize<Val>) -> Self {
        match value.into_style_size() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.gap = v;
                inner.mask.set(STYLE_GAP);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_GAP);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_flex_layout_mut(id, target) {
                        v.gap = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
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
    pub fn gap_row(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current_column_gap = self.inner.flex_layout.gap.width;
                self.gap((current_column_gap, v))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_GAP);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_flex_layout_mut(id, target) {
                        v.gap.height = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// 子要素同士の列方向（横方向、column-gap）の隙間を設定します。
    #[inline]
    pub fn gap_col(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let current_row_gap = self.inner.flex_layout.gap.height;
                self.gap((v, current_row_gap))
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_GAP);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_flex_layout_mut(id, target) {
                        v.gap.width = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
                self
            }
        }
    }

    /// エイリアス：子要素同士の縦方向の隙間を設定します。
    #[inline]
    pub fn gap_y(self, value: impl IntoStyleConvert<Val>) -> Self {
        self.gap_row(value)
    }

    /// エイリアス：子要素同士の横方向の隙間を設定します。
    #[inline]
    pub fn gap_x(self, value: impl IntoStyleConvert<Val>) -> Self {
        self.gap_col(value)
    }

    /// テキストの配置揃え方向を設定します。
    #[inline]
    pub fn text_align(mut self, value: impl IntoStyleValue<TextAlign>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.text_align = v;
                inner.mask.set(STYLE_TEXT_ALIGN);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_TEXT_ALIGN);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_flex_layout_mut(id, target) {
                        v.text_align = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
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
    pub fn flex_direction(mut self, value: impl IntoStyleValue<FlexDirection>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.flex_direction = v;
                inner.mask.set(STYLE_FLEX_DIRECTION);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_FLEX_DIRECTION);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.flex_layouts.get_mut(id) {
                            v.flex_direction = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
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
    pub fn flex_wrap_internal(mut self, value: impl IntoStyleValue<FlexWrap>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.flex_wrap = v;
                inner.mask.set(STYLE_FLEX_WRAP);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_FLEX_WRAP);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.layouts.flex_layouts.get_mut(id) {
                            v.flex_wrap = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
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
    pub fn basis(mut self, value: impl IntoStyleConvert<Val>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.flex_basis = v;
                inner.mask.set(STYLE_FLEX_BASIS);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_FLEX_BASIS);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_flex_layout_mut(id, target) {
                        v.flex_basis = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
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
    pub fn flex_grow(mut self, value: impl IntoStyleConvert<f32>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.flex_grow = v;
                inner.mask.set(STYLE_FLEX_GROW);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_FLEX_GROW);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_flex_layout_mut(id, target) {
                        v.flex_grow = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
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
    pub fn flex_shrink(mut self, value: impl IntoStyleConvert<f32>) -> Self {
        match value.into_style_convert() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.flex_layout.flex_shrink = v;
                inner.mask.set(STYLE_FLEX_SHRINK);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_FLEX_SHRINK);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_flex_layout_mut(id, target) {
                        v.flex_shrink = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
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
    pub fn bg_color(mut self, value: impl IntoStyleValue<Color>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.bg_color = Some(v);
                inner.mask.set(STYLE_BG_COLOR);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BG_COLOR);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.bg_color = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// 要素の境界線の色を設定します。
    #[inline]
    pub fn border_color(mut self, value: impl IntoStyleValue<Color>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.border_color = Some(v);
                inner.mask.set(STYLE_BORDER_COLOR);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BORDER_COLOR);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.border_color = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// 要素の角丸の半径を設定します。
    #[inline]
    pub fn corner_radius(mut self, value: impl IntoStyleCornerRadius) -> Self {
        match value.into_style_corner_radius() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.corner_radius = Some(v);
                inner.mask.set(STYLE_CORNER_RADIUS);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_CORNER_RADIUS);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.corner_radius = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// `corner_radius` の短縮エイリアス。要素の角丸を設定します。
    /// 単一値、2連タプル、4連タプルを受け入れます。
    #[inline]
    pub fn rounded(self, value: impl IntoStyleCornerRadius) -> Self {
        self.corner_radius(value)
    }

    /// `corner_radius` の超短縮エイリアス。要素の角丸を設定します。
    #[inline]
    pub fn r(self, value: impl IntoStyleCornerRadius) -> Self {
        self.corner_radius(value)
    }

    /// 要素を完全なサークル（またはカプセル型、Tailwind CSS の rounded-full 相当）にします。
    #[inline]
    pub fn r_full(self) -> Self {
        self.corner_radius(9999.0)
    }

    /// 上半分の角（top-left, top-right）にのみ角丸を設定します。
    #[inline]
    pub fn r_top(self, value: impl Convert<f32>) -> Self {
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
    pub fn r_bottom(self, value: impl Convert<f32>) -> Self {
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
    pub fn r_left(self, value: impl Convert<f32>) -> Self {
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
    pub fn r_right(self, value: impl Convert<f32>) -> Self {
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
    pub fn opacity(mut self, value: impl IntoStyleValue<f32>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.opacity = Some(v);
                inner.mask.set(STYLE_OPACITY);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_OPACITY);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.opacity = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
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
    pub fn box_shadow(mut self, value: impl IntoStyleValue<BoxShadow>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.shadow_params = Some(v);
                inner.visual_property.shadow_color = Some(v.color);
                inner.mask.set(STYLE_BOX_SHADOW);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BOX_SHADOW);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.shadow_params = Some(val);
                        v.shadow_color = Some(val.color);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// 影の色（shadow_color）のみを設定・上書きします。
    #[inline]
    pub fn shadow_color(mut self, value: impl IntoStyleValue<Color>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.shadow_color = Some(v);
                inner.mask.set(STYLE_BOX_SHADOW);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BOX_SHADOW);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.shadow_color = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// 重なり順（Z-Index）を整数で設定します。
    #[inline]
    pub fn z_index(mut self, value: impl IntoStyleValue<i32>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.z_index = Some(v);
                inner.mask.set(STYLE_Z_INDEX);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_Z_INDEX);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.z_index = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    #[inline]
    pub fn z(self, value: impl IntoStyleValue<i32>) -> Self {
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
    pub fn cursor(mut self, value: impl IntoStyleValue<CursorIcon>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.cursor = Some(v);
                inner.mask.set(STYLE_CURSOR);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_CURSOR);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.cursor = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    #[inline]
    pub fn cursor_default(self) -> Self {
        self.cursor(CursorIcon::Default(None))
    }

    #[inline]
    pub fn cursor_grab(self) -> Self {
        self.cursor(CursorIcon::Grab(None))
    }

    #[inline]
    pub fn cursor_grabbing(self) -> Self {
        self.cursor(CursorIcon::Grabbing(None))
    }

    #[inline]
    pub fn cursor_not_allowed(self) -> Self {
        self.cursor(CursorIcon::NotAllowed(None))
    }

    #[inline]
    pub fn cursor_pointer(self) -> Self {
        self.cursor(CursorIcon::Pointer(None))
    }

    #[inline]
    pub fn cursor_text(self) -> Self {
        self.cursor(CursorIcon::Text(None))
    }

    /// 親先祖へ伝播するグローバルカーソルアイコンを設定します。
    #[inline]
    pub fn cursor_global(self, value: impl IntoStyleValue<GlobalCursorIcon>) -> Self {
        let cursor_val = match value.into_style_value() {
            StyleValue::Static(v) => StyleValue::Static(CursorIcon::Global(v)),
            StyleValue::Dynamic(getter) => {
                StyleValue::Dynamic(Box::new(move || CursorIcon::Global(getter())))
            }
        };
        self.cursor(cursor_val)
    }

    #[inline]
    pub fn cursor_global_default(self) -> Self {
        self.cursor_global(GlobalCursorIcon::Default(None))
    }

    #[inline]
    pub fn cursor_global_pointer(self) -> Self {
        self.cursor_global(GlobalCursorIcon::Pointer(None))
    }

    #[inline]
    pub fn cursor_global_text(self) -> Self {
        self.cursor_global(GlobalCursorIcon::Text(None))
    }

    #[inline]
    pub fn cursor_global_grab(self) -> Self {
        self.cursor_global(GlobalCursorIcon::Grab(None))
    }

    #[inline]
    pub fn cursor_global_grabbing(self) -> Self {
        self.cursor_global(GlobalCursorIcon::Grabbing(None))
    }

    #[inline]
    pub fn backdrop(mut self, backdrop: impl IntoStyleValue<Backdrop>) -> Self {
        match backdrop.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.backdrop = v;
                inner.mask.set(STYLE_BACKDROP);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BACKDROP);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.backdrop = val;
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
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
    pub fn text_color(mut self, value: impl IntoStyleValue<Color>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.text_color = Some(v);
                inner.mask.set(STYLE_TEXT_COLOR);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_TEXT_COLOR);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.text_color = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// グリッドの行方向の明示的なトラックサイズ定義を設定します。
    #[inline]
    pub fn grid_template_rows(
        mut self,
        value: impl IntoStyleValue<Vec<GridTemplateComponent<String>>>,
    ) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_template_rows = v;
                inner.mask.set(STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.grid_layouts.contains_key(id) {
                            cx.layouts.grid_layouts.insert(id, GridLayout::default());
                        }
                        if let Some(grid) = cx.layouts.grid_layouts.get_mut(id) {
                            grid.grid_template_rows = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// グリッドの列方向の明示的なトラックサイズ定義を設定します。
    #[inline]
    pub fn grid_template_columns(
        mut self,
        value: impl IntoStyleValue<Vec<GridTemplateComponent<String>>>,
    ) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_template_columns = v;
                inner.mask.set(STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.grid_layouts.contains_key(id) {
                            cx.layouts.grid_layouts.insert(id, GridLayout::default());
                        }
                        if let Some(grid) = cx.layouts.grid_layouts.get_mut(id) {
                            grid.grid_template_columns = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// 暗黙的に生成されるグリッド行のデフォルトサイズを設定します。
    #[inline]
    pub fn grid_auto_rows(mut self, value: impl IntoStyleValue<Vec<TrackSizingFunction>>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_auto_rows = v;
                inner.mask.set(STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.grid_layouts.contains_key(id) {
                            cx.layouts.grid_layouts.insert(id, GridLayout::default());
                        }
                        if let Some(grid) = cx.layouts.grid_layouts.get_mut(id) {
                            grid.grid_auto_rows = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// 暗黙的に生成されるグリッド列のデフォルトサイズを設定します。
    #[inline]
    pub fn grid_auto_columns(
        mut self,
        value: impl IntoStyleValue<Vec<TrackSizingFunction>>,
    ) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_auto_columns = v;
                inner.mask.set(STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.grid_layouts.contains_key(id) {
                            cx.layouts.grid_layouts.insert(id, GridLayout::default());
                        }
                        if let Some(grid) = cx.layouts.grid_layouts.get_mut(id) {
                            grid.grid_auto_columns = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// 自動配置アルゴリズムの制御方法を設定します。
    #[inline]
    pub fn grid_auto_flow(mut self, value: impl IntoStyleValue<GridAutoFlow>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_auto_flow = v;
                inner.mask.set(STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.grid_layouts.contains_key(id) {
                            cx.layouts.grid_layouts.insert(id, GridLayout::default());
                        }
                        if let Some(grid) = cx.layouts.grid_layouts.get_mut(id) {
                            grid.grid_auto_flow = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// 名前付きグリッドエリアを定義して配置を決定します。
    #[inline]
    pub fn grid_template_areas(
        mut self,
        value: impl IntoStyleValue<Vec<GridTemplateArea<String>>>,
    ) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_template_areas = v;
                inner.mask.set(STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.grid_layouts.contains_key(id) {
                            cx.layouts.grid_layouts.insert(id, GridLayout::default());
                        }
                        if let Some(grid) = cx.layouts.grid_layouts.get_mut(id) {
                            grid.grid_template_areas = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// 明示的に定義された各グリッド列線に対する名前のリストを設定します。
    #[inline]
    pub fn grid_template_column_names(
        mut self,
        value: impl IntoStyleValue<Vec<Vec<String>>>,
    ) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_template_column_names = v;
                inner.mask.set(STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.grid_layouts.contains_key(id) {
                            cx.layouts.grid_layouts.insert(id, GridLayout::default());
                        }
                        if let Some(grid) = cx.layouts.grid_layouts.get_mut(id) {
                            grid.grid_template_column_names = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// 明示的に定義された各グリッド行線に対する名前のリストを設定します。
    #[inline]
    pub fn grid_template_row_names(mut self, value: impl IntoStyleValue<Vec<Vec<String>>>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_template_row_names = v;
                inner.mask.set(STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.grid_layouts.contains_key(id) {
                            cx.layouts.grid_layouts.insert(id, GridLayout::default());
                        }
                        if let Some(grid) = cx.layouts.grid_layouts.get_mut(id) {
                            grid.grid_template_row_names = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// グリッドアイテムが配置される行の開始位置と終了位置を指定します。
    #[inline]
    pub fn grid_row(mut self, value: impl IntoStyleValue<GridLine<GridPlacement<String>>>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_row = v;
                inner.mask.set(STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.grid_layouts.contains_key(id) {
                            cx.layouts.grid_layouts.insert(id, GridLayout::default());
                        }
                        if let Some(grid) = cx.layouts.grid_layouts.get_mut(id) {
                            grid.grid_row = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// グリッドアイテムが配置される列の開始位置と終了位置を指定します。
    #[inline]
    pub fn grid_column(
        mut self,
        value: impl IntoStyleValue<GridLine<GridPlacement<String>>>,
    ) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_column = v;
                inner.mask.set(STYLE_GRID_LAYOUT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if !cx.layouts.grid_layouts.contains_key(id) {
                            cx.layouts.grid_layouts.insert(id, GridLayout::default());
                        }
                        if let Some(grid) = cx.layouts.grid_layouts.get_mut(id) {
                            grid.grid_column = val;
                        }
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// マウスが要素の上に乗った（Hover）際に適用するオーバーライドスタイルを設定します。
    #[inline]
    pub fn hovered(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(style, STATE_HOVERED, StyleTarget::Hovered)
    }

    /// キーボードタブ移動などで要素にフォーカスが当たった際に適用するスタイルを設定します。
    #[inline]
    pub fn focused(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(style, STATE_FOCUSED, StyleTarget::Focused)
    }

    /// キーボード経由のフォーカス時のみ（focus-visible相当）適用するスタイルを設定します。
    #[inline]
    pub fn focused_visible(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(style, STATE_FOCUSED_VISIBLE, StyleTarget::FocusedVisible)
    }

    /// マウスの左ボタンが要素の上で押し下げられた際、またはタップ中に適用するスタイルを設定します。
    #[inline]
    pub fn pressed(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(style, STATE_PRESSED, StyleTarget::Pressed)
    }

    /// 要素が無効化された際に適用するスタイルを設定します。
    #[inline]
    pub fn disabled(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(style, STATE_DISABLED, StyleTarget::Disabled)
    }

    /// 要素がアクティブ状態の時に適用するスタイルを設定します。
    #[inline]
    pub fn actived(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(style, STATE_ACTIVED, StyleTarget::Actived)
    }

    /// 要素がトグル選択された際に適用するスタイルを設定します。
    #[inline]
    pub fn selected(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(style, STATE_SELECTED, StyleTarget::Selected)
    }

    /// 要素が現在ドラッグ操作中にある際に適用するスタイルを設定します。
    #[inline]
    pub fn dragged(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(style, STATE_DRAGGED, StyleTarget::Dragged)
    }

    /// 子孫要素のインタラクション状態に連動して自身のスタイルを変化させる伝播設定
    #[inline]
    pub fn interaction_within(
        self,
        name: InteractionName,
        style: impl IntoStyleValue<ThisStyle>,
    ) -> Self {
        let (state_flag, target) = match name {
            InteractionName::Hover => (STYLE_INTERACTION_WITHIN, StyleTarget::HoveredWithin),
            InteractionName::Focus => (STYLE_INTERACTION_WITHIN, StyleTarget::FocusedWithin),
            InteractionName::FocusVisible => {
                (STYLE_INTERACTION_WITHIN, StyleTarget::FocusedVisibleWithin)
            }
            InteractionName::Press => (STYLE_INTERACTION_WITHIN, StyleTarget::PressedWithin),
            InteractionName::Disable => (STYLE_INTERACTION_WITHIN, StyleTarget::DisabledWithin),
            InteractionName::Active => (STYLE_INTERACTION_WITHIN, StyleTarget::ActivedWithin),
            InteractionName::Select => (STYLE_INTERACTION_WITHIN, StyleTarget::SelectedWithin),
            InteractionName::Drag => (STYLE_INTERACTION_WITHIN, StyleTarget::DraggedWithin),
            InteractionName::All => (STYLE_INTERACTION_WITHIN, StyleTarget::AnyWithin),
        };
        self.apply_interaction_style(style, state_flag, target)
    }

    #[inline]
    pub fn hovered_within(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_within(InteractionName::Hover, style)
    }

    #[inline]
    pub fn focused_within(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_within(InteractionName::Focus, style)
    }

    /// 子孫要素がキーボードフォーカスされている場合のみ適用するスタイルを設定します。
    #[inline]
    pub fn focused_visible_within(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_within(InteractionName::FocusVisible, style)
    }

    #[inline]
    pub fn pressed_within(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_within(InteractionName::Press, style)
    }

    #[inline]
    pub fn disabled_within(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_within(InteractionName::Disable, style)
    }

    #[inline]
    pub fn actived_within(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_within(InteractionName::Active, style)
    }

    #[inline]
    pub fn selected_within(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_within(InteractionName::Select, style)
    }

    #[inline]
    pub fn dragged_within(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_within(InteractionName::Drag, style)
    }

    #[inline]
    pub fn all_within(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_within(InteractionName::All, style)
    }

    /// 直近の親要素のインタラクション状態に連動して自身のスタイルを変化させる
    #[inline]
    pub fn interaction_parent(
        self,
        name: InteractionName,
        style: impl IntoStyleValue<ThisStyle>,
    ) -> Self {
        let (state_flag, target) = match name {
            InteractionName::Hover => (STYLE_INTERACTION_PARENT, StyleTarget::HoveredParent),
            InteractionName::Focus => (STYLE_INTERACTION_PARENT, StyleTarget::FocusedParent),
            InteractionName::FocusVisible => {
                (STYLE_INTERACTION_PARENT, StyleTarget::FocusedVisibleParent)
            }
            InteractionName::Press => (STYLE_INTERACTION_PARENT, StyleTarget::PressedParent),
            InteractionName::Disable => (STYLE_INTERACTION_PARENT, StyleTarget::DisabledParent),
            InteractionName::Active => (STYLE_INTERACTION_PARENT, StyleTarget::ActivedParent),
            InteractionName::Select => (STYLE_INTERACTION_PARENT, StyleTarget::SelectedParent),
            InteractionName::Drag => (STYLE_INTERACTION_PARENT, StyleTarget::DraggedParent),
            InteractionName::All => (STYLE_INTERACTION_PARENT, StyleTarget::AnyParent),
        };
        self.apply_interaction_style(style, state_flag, target)
    }

    #[inline]
    pub fn hovered_parent(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_parent(InteractionName::Hover, style)
    }

    #[inline]
    pub fn focused_parent(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_parent(InteractionName::Focus, style)
    }

    #[inline]
    pub fn focused_visible_parent(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_parent(InteractionName::FocusVisible, style)
    }

    #[inline]
    pub fn pressed_parent(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_parent(InteractionName::Press, style)
    }

    #[inline]
    pub fn disabled_parent(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_parent(InteractionName::Disable, style)
    }

    #[inline]
    pub fn actived_parent(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_parent(InteractionName::Active, style)
    }

    #[inline]
    pub fn selected_parent(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_parent(InteractionName::Select, style)
    }

    #[inline]
    pub fn dragged_parent(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_parent(InteractionName::Drag, style)
    }

    #[inline]
    pub fn all_parent(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.interaction_parent(InteractionName::All, style)
    }

    /// 要素の上下左右のリサイズ許可を設定します。
    /// 引数にはタプル `(top, right, bottom, left)`、配列 `[top, right, bottom, left]`、
    /// またはそれらを解決するシグナル、動的クロージャを指定できます。
    #[inline]
    pub fn resizable(mut self, value: impl IntoStyleResizable) -> Self {
        match value.into_style_resizable() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.resizable = v;
                inner.mask.set(STYLE_RESIZABLE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_RESIZABLE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.resizable = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// 四方向（上下左右）すべてのリサイズ可否を一括設定します。
    #[inline]
    pub fn resizable_all(mut self, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.resizable = [v; 4];
                inner.mask.set(STYLE_RESIZABLE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_RESIZABLE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.resizable = [val; 4];
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// 左右（X軸方向）のリサイズ可否を一括設定します。
    #[inline]
    pub fn resizable_x(mut self, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.resizable[1] = v; // right
                inner.basic_layout.resizable[3] = v; // left
                inner.mask.set(STYLE_RESIZABLE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_RESIZABLE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.resizable[1] = val;
                        v.resizable[3] = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// 上下（Y軸方向）のリサイズ可否を一括設定します。
    #[inline]
    pub fn resizable_y(mut self, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.resizable[0] = v; // top
                inner.basic_layout.resizable[2] = v; // bottom
                inner.mask.set(STYLE_RESIZABLE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_RESIZABLE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.resizable[0] = val;
                        v.resizable[2] = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// 上側のリサイズ可否を個別に設定します。
    #[inline]
    pub fn resizable_top(self, value: impl IntoStyleValue<bool>) -> Self {
        self.set_resizable_edge_idx(0, value)
    }

    /// 右側のリサイズ可否を個別に設定します。
    #[inline]
    pub fn resizable_right(self, value: impl IntoStyleValue<bool>) -> Self {
        self.set_resizable_edge_idx(1, value)
    }

    /// 下側のリサイズ可否を個別に設定します。
    #[inline]
    pub fn resizable_bottom(self, value: impl IntoStyleValue<bool>) -> Self {
        self.set_resizable_edge_idx(2, value)
    }

    /// 左側のリサイズ可否を個別に設定します。
    #[inline]
    pub fn resizable_left(self, value: impl IntoStyleValue<bool>) -> Self {
        self.set_resizable_edge_idx(3, value)
    }

    /// 辺インデックス (0:top, 1:right, 2:bottom, 3:left) を指定した更新処理
    fn set_resizable_edge_idx(mut self, idx: usize, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.basic_layout.resizable[idx] = v;
                inner.mask.set(STYLE_RESIZABLE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_RESIZABLE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_basic_layout_mut(id, target) {
                        v.resizable[idx] = val;
                    }
                    cx.mark_layout_dirty(id);
                }));
            }
        }
        self
    }

    /// リサイズ方向ごとのカスタムカーソルを一括設定します。[Ns, Ew, Nesw, Nwse]
    /// 各方向に対して None を指定した場合は、ライブラリの自動カーソルマッピングが適用されます。
    #[inline]
    pub fn resizable_cursor(
        mut self,
        ns: Option<CursorIcon>,
        ew: Option<CursorIcon>,
        nesw: Option<CursorIcon>,
        nwse: Option<CursorIcon>,
    ) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.resizable_cursor = Some([ns, ew, nesw, nwse]);
        inner.mask.set(STYLE_RESIZABLE);
        self
    }

    #[inline]
    pub fn resizable_cursor_default(self) -> Self {
        self.resizable_cursor(None, None, None, None)
    }

    /// ドラッグ時にプレースホルダーを最上位ルート要素の子としてアタッチし、絶対配置追従させます。
    #[inline]
    pub fn draggable_root(
        mut self,
        mode: impl IntoStyleValue<DragPayload>,
        update_position: impl IntoStyleValue<bool>,
    ) -> Self {
        let m_val = mode.into_style_value();
        let u_val = update_position.into_style_value();

        match (m_val, u_val) {
            (StyleValue::Static(m), StyleValue::Static(u)) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.drag_property = Some(DragProperty {
                    placeholder_parent: DragPlaceholderParent::Root,
                    drag_mode: m,
                    update_position: u,
                });
                inner.mask.set(STYLE_DRAGGABLE);
            }
            (m_getter, u_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_DRAGGABLE);

                let get_m = match m_getter {
                    StyleValue::Static(m) => {
                        Box::new(move || m) as Box<dyn Fn() -> DragPayload + Send + Sync>
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
                        cx.events.drag_properties.insert(
                            id,
                            DragProperty {
                                placeholder_parent: DragPlaceholderParent::Root,
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

    /// ドラッグ時にプレースホルダーを特定の親要素の子としてアタッチし（範囲制限）、絶対配置追従させます。
    #[inline]
    pub fn draggable_parent(
        mut self,
        parent_id: impl IntoStyleValue<EntityId>,
        mode: impl IntoStyleValue<DragPayload>,
        update_position: impl IntoStyleValue<bool>,
    ) -> Self {
        let p_val = parent_id.into_style_value();
        let m_val = mode.into_style_value();
        let u_val = update_position.into_style_value();

        match (p_val, m_val, u_val) {
            (StyleValue::Static(p), StyleValue::Static(m), StyleValue::Static(u)) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.drag_property = Some(DragProperty {
                    placeholder_parent: DragPlaceholderParent::Custom(p),
                    drag_mode: m,
                    update_position: u,
                });
                inner.mask.set(STYLE_DRAGGABLE);
            }
            (p_getter, m_getter, u_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_DRAGGABLE);

                let get_p = match p_getter {
                    StyleValue::Static(p) => {
                        Box::new(move || p) as Box<dyn Fn() -> EntityId + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };
                let get_m = match m_getter {
                    StyleValue::Static(m) => {
                        Box::new(move || m) as Box<dyn Fn() -> DragPayload + Send + Sync>
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
                        cx.events.drag_properties.insert(
                            id,
                            DragProperty {
                                placeholder_parent: DragPlaceholderParent::Custom(p),
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

    /// 要素がドロップの受け入れ可能であることを示し、取り込み方式と動作を指定します。
    #[inline]
    pub fn droppable(
        mut self,
        target: impl IntoStyleValue<DropTarget>,
        mode: impl IntoStyleValue<DragPayload>,
    ) -> Self {
        let t_val = target.into_style_value();
        let m_val = mode.into_style_value();

        match (t_val, m_val) {
            (StyleValue::Static(t), StyleValue::Static(m)) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.drop_property = Some(DropProperty {
                    target: t,
                    drag_mode: m,
                });
                inner.mask.set(STYLE_DROPPABLE);
            }
            (t_getter, m_getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_DROPPABLE);

                let get_t = match t_getter {
                    StyleValue::Static(t) => {
                        Box::new(move || t) as Box<dyn Fn() -> DropTarget + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };
                let get_m = match m_getter {
                    StyleValue::Static(m) => {
                        Box::new(move || m) as Box<dyn Fn() -> DragPayload + Send + Sync>
                    }
                    StyleValue::Dynamic(g) => g,
                };

                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let t = get_t();
                        let m = get_m();
                        cx.events.drop_properties.insert(
                            id,
                            DropProperty {
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

    /// ドラッグ中の元の要素に適用する疑似クラススタイルを指定します。
    #[inline]
    pub fn draggable_original(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(style, STATE_DRAGGING, StyleTarget::Dragging)
    }

    /// ドラッグ中のプレースホルダー（ドラッグイメージ）に適用する疑似クラススタイルを指定します。
    #[inline]
    pub fn draggable_placeholder(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(style, STATE_DRAG_OVER, StyleTarget::DragOver)
    }

    /// ドロップゾーンにドラッグ要素がホバー侵入している際に、ドロップゾーン側に適用するスタイルを指定します。
    #[inline]
    pub fn drag_over(self, style: impl IntoStyleValue<ThisStyle>) -> Self {
        self.apply_interaction_style(style, STATE_DRAG_IN, StyleTarget::DragIn)
    }

    /// ポインターメッセージ（マウスインタラクションなど）の透過を制御します。
    #[inline]
    pub fn pointer_events(mut self, value: impl IntoStyleValue<PointerEvents>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.pointer_events = Some(v);
                inner.mask.set(STYLE_POINTER_EVENTS);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_POINTER_EVENTS);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.renders.base_visual_properties.get_mut(id) {
                            v.pointer_events = Some(val);
                        }
                        cx.mark_render_dirty(id);
                    }
                }));
            }
        }
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
    pub fn transform(mut self, value: impl IntoStyleValue<Transform>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.transform = Some(v.matrix);
                inner.mask.set(STYLE_TRANSFORM);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_TRANSFORM);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.transform = Some(val.matrix);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    #[inline]
    pub fn transform_origin(mut self, point: impl IntoStylePoint<f32>) -> Self {
        match point.into_style_point() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.transform_origin = Some(v);
                inner.mask.set(STYLE_TRANSFORM);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_TRANSFORM);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.transform_origin = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
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
    pub fn transform_inherit(mut self, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.transform_inherit = Some(v);
                inner.mask.set(STYLE_TRANSFORM_INHERIT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_TRANSFORM_INHERIT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.transform_inherit = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// 状態遷移時のトランジション（CSS transition）を設定します。
    #[inline]
    pub fn transition(mut self, transition: impl IntoStyleValue<Transition>) -> Self {
        match transition.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.transitions.push(v);
                inner.mask.set(STYLE_TRANSITIONS);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_TRANSITIONS);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.renders.base_visual_properties.get_mut(id) {
                            v.transitions.push(val);
                        }
                        cx.mark_render_dirty(id);
                    }
                }));
            }
        }
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
    pub fn animation(mut self, animation: impl IntoStyleValue<KeyframeAnimation>) -> Self {
        match animation.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.keyframe_animations.push(v);
                inner.mask.set(STYLE_ANIMATIONS);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_ANIMATIONS);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == StyleTarget::Base {
                        let val = getter();
                        if let Some(v) = cx.renders.base_visual_properties.get_mut(id) {
                            v.keyframe_animations.push(val);
                        }
                        cx.mark_render_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// 背景に 2色線形グラデーションを適用します。
    #[inline]
    pub fn bg_gradient(mut self, gradient: impl IntoStyleValue<LinearGradient>) -> Self {
        match gradient.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.bg_gradient = Some(v);
                inner.mask.set(STYLE_BG_COLOR);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_BG_COLOR);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.bg_gradient = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// テキストのフォントファミリーを設定します。（例: font_family("Arial")）
    #[inline]
    pub fn font_family(mut self, family: impl IntoStyleValue<Cow<'static, str>>) -> Self {
        match family.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.font_family = Some(v);
                inner.mask.set(STYLE_EXT_PROPERTIES);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_EXT_PROPERTIES);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.font_family = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// テキストの太さを設定します。
    #[inline]
    pub fn font_weight(mut self, weight: impl IntoStyleValue<u32>) -> Self {
        match weight.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.font_weight = Some(v);
                inner.mask.set(STYLE_EXT_PROPERTIES);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_EXT_PROPERTIES);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.font_weight = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// 要素内でレンダリングされるテキストの基本フォントサイズを設定します。
    #[inline]
    pub fn font_size(mut self, size: impl IntoStyleValue<f32>) -> Self {
        match size.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.font_size = Some(v);
                inner.mask.set(STYLE_FONT_SIZE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_FONT_SIZE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.font_size = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// フォントスタイルを設定します
    #[inline]
    pub fn font_style(mut self, style: impl IntoStyleValue<u32>) -> Self {
        match style.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.font_style = Some(v);
                inner.mask.set(STYLE_EXT_PROPERTIES);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_EXT_PROPERTIES);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.font_style = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// ユーザーによるテキスト選択・コピーの挙動を設定します
    #[inline]
    pub fn user_select(mut self, value: impl IntoStyleValue<UserSelect>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.user_select = Some(v);
                inner.mask.set(STYLE_USER_SELECT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_USER_SELECT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.user_select = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
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
    pub fn select_bg_color(mut self, color: impl IntoStyleValue<Color>) -> Self {
        match color.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.select_bg_color = Some(v);
                inner.mask.set(STYLE_USER_SELECT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_USER_SELECT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.select_bg_color = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    #[inline]
    pub fn select_text_color(mut self, color: impl IntoStyleValue<Color>) -> Self {
        match color.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.select_text_color = Some(v);
                inner.mask.set(STYLE_USER_SELECT);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_USER_SELECT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.select_text_color = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// 要素にフォーカスを許可し、スタイル継承ポリシーを指定します。
    /// 引数には `bool`（true の場合は自動的に親スタイル継承を有効化）または `Focusable` を指定できます。
    #[inline]
    pub fn focusable(mut self, value: impl IntoStyleValue<Focusable>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.focusable = Some(v);
                inner.mask.set(STYLE_FOCUSABLE);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_FOCUSABLE);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.focusable = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    #[inline]
    pub fn focusable_inherit(self, trigger: FocusTrigger) -> Self {
        self.focusable(Focusable::Inherit(trigger))
    }

    #[inline]
    pub fn focusable_inherit_both(self) -> Self {
        self.focusable(Focusable::Inherit(FocusTrigger::Both))
    }

    #[inline]
    pub fn focusable_inherit_mouse(self) -> Self {
        self.focusable(Focusable::Inherit(FocusTrigger::Mouse))
    }

    #[inline]
    pub fn focusable_inherit_keyboard(self) -> Self {
        self.focusable(Focusable::Inherit(FocusTrigger::Keyboard))
    }

    #[inline]
    pub fn focusable_self(self, trigger: FocusTrigger) -> Self {
        self.focusable(Focusable::SelfStyle(trigger))
    }

    #[inline]
    pub fn focusable_self_both(self) -> Self {
        self.focusable(Focusable::SelfStyle(FocusTrigger::Both))
    }

    #[inline]
    pub fn focusable_self_mouse(self) -> Self {
        self.focusable(Focusable::SelfStyle(FocusTrigger::Mouse))
    }

    #[inline]
    pub fn focusable_self_keyboard(self) -> Self {
        self.focusable(Focusable::SelfStyle(FocusTrigger::Keyboard))
    }

    #[inline]
    pub fn focusable_none(self) -> Self {
        self.focusable(Focusable::None)
    }

    /// 自身がクリックされた際に、現在アクティブなフォーカス要素からフォーカスを奪わないように設定します。
    #[inline]
    pub fn prevent_focus_steal(mut self, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.prevent_focus_steal = Some(v);
                inner.mask.set(STYLE_PREVENT_FOCUS_STEAL);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_PREVENT_FOCUS_STEAL);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.prevent_focus_steal = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// 子孫要素がクリックされた際に、現在アクティブなフォーカス要素からフォーカスを奪わないように設定します。
    #[inline]
    pub fn prevent_focus_steal_within(mut self, value: impl IntoStyleValue<bool>) -> Self {
        match value.into_style_value() {
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.visual_property.prevent_focus_steal_within = Some(v);
                inner.mask.set(STYLE_PREVENT_FOCUS_STEAL_WITHIN);
            }
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(STYLE_PREVENT_FOCUS_STEAL_WITHIN);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    let val = getter();
                    if let Some(v) = cx.get_visual_property_mut(id, target) {
                        v.prevent_focus_steal_within = Some(val);
                    }
                    cx.mark_render_dirty(id);
                }));
            }
        }
        self
    }

    /// すべての疑似クラスおよび within 伝播系のスタイルと動的セッターを統合する共通コアヘルパー
    fn apply_interaction_style(
        mut self,
        style: impl IntoStyleValue<ThisStyle>,
        state_flag: u128,
        target: StyleTarget,
    ) -> Self {
        match style.into_style_value() {
            // パターン A: 静的に構築されたスタイルが渡された場合
            StyleValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);

                // ネストされた子スタイルが持つ動的セッター群を引き上げ、
                // 実行時に親ターゲット（例: Hovered）に補正して転送するセッターを登録します。
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
                    StyleTarget::Dragging => interaction.dragging = Some(v),
                    StyleTarget::DragIn => interaction.drag_in = Some(v),
                    StyleTarget::DragOver => interaction.drag_over = Some(v),

                    StyleTarget::HoveredWithin => interaction.hovered_within = Some(v),
                    StyleTarget::FocusedWithin => interaction.focused_within = Some(v),
                    StyleTarget::FocusedVisibleWithin => {
                        interaction.focused_visible_within = Some(v)
                    }
                    StyleTarget::PressedWithin => interaction.pressed_within = Some(v),
                    StyleTarget::DisabledWithin => interaction.disabled_within = Some(v),
                    StyleTarget::ActivedWithin => interaction.actived_within = Some(v),
                    StyleTarget::SelectedWithin => interaction.selected_within = Some(v),
                    StyleTarget::DraggedWithin => interaction.dragged_within = Some(v),
                    StyleTarget::AnyWithin => interaction.any_within = Some(v),
                    StyleTarget::Base => unreachable!(),

                    StyleTarget::HoveredParent => interaction.hovered_parent = Some(v),
                    StyleTarget::FocusedParent => interaction.focused_parent = Some(v),
                    StyleTarget::FocusedVisibleParent => {
                        interaction.focused_visible_parent = Some(v)
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

            // パターン B: 疑似クラス自体が consume 等の遅延ゲッターで上書きされた場合
            StyleValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(state_flag);
                inner
                    .dynamic_setters
                    .push(Arc::new(move |cx, id, _parent_target| {
                        let val = getter(); // ThisStyle の動的評価結果

                        if !cx.renders.interaction_properties.contains_key(id) {
                            cx.renders
                                .interaction_properties
                                .insert(id, InteractionStyles::default());
                        }
                        let styles = cx.renders.interaction_properties.get_mut(id).unwrap();

                        // 動的に解決されたスタイルを対応する疑似フィールドへ上書きマウント
                        match target {
                            StyleTarget::Hovered => styles.hovered = Some(val.clone()),
                            StyleTarget::Focused => styles.focused = Some(val.clone()),
                            StyleTarget::FocusedVisible => {
                                styles.focused_visible = Some(val.clone())
                            }
                            StyleTarget::Pressed => styles.pressed = Some(val.clone()),
                            StyleTarget::Disabled => styles.disabled = Some(val.clone()),
                            StyleTarget::Actived => styles.actived = Some(val.clone()),
                            StyleTarget::Selected => styles.selected = Some(val.clone()),
                            StyleTarget::Dragged => styles.dragged = Some(val.clone()),
                            StyleTarget::Dragging => styles.dragging = Some(val.clone()),
                            StyleTarget::DragIn => styles.drag_in = Some(val.clone()),
                            StyleTarget::DragOver => styles.drag_over = Some(val.clone()),

                            StyleTarget::HoveredWithin => styles.hovered_within = Some(val.clone()),
                            StyleTarget::FocusedWithin => styles.focused_within = Some(val.clone()),
                            StyleTarget::FocusedVisibleWithin => {
                                styles.focused_visible_within = Some(val.clone())
                            }
                            StyleTarget::PressedWithin => styles.pressed_within = Some(val.clone()),
                            StyleTarget::DisabledWithin => {
                                styles.disabled_within = Some(val.clone())
                            }
                            StyleTarget::ActivedWithin => styles.actived_within = Some(val.clone()),
                            StyleTarget::SelectedWithin => {
                                styles.selected_within = Some(val.clone())
                            }
                            StyleTarget::DraggedWithin => styles.dragged_within = Some(val.clone()),
                            StyleTarget::AnyWithin => styles.any_within = Some(val.clone()),
                            StyleTarget::Base => unreachable!(),

                            StyleTarget::HoveredParent => styles.hovered_parent = Some(val.clone()),
                            StyleTarget::FocusedParent => styles.focused_parent = Some(val.clone()),
                            StyleTarget::FocusedVisibleParent => {
                                styles.focused_visible_parent = Some(val.clone())
                            }
                            StyleTarget::PressedParent => styles.pressed_parent = Some(val.clone()),
                            StyleTarget::DisabledParent => {
                                styles.disabled_parent = Some(val.clone())
                            }
                            StyleTarget::ActivedParent => styles.actived_parent = Some(val.clone()),
                            StyleTarget::SelectedParent => {
                                styles.selected_parent = Some(val.clone())
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

    /// 別のスタイルを上に重ねてマージした新しい ThisStyle を生成して返します。
    pub fn merge(&self, other: &Self) -> Self {
        let mut merged = self.clone();
        let inner_mut = Arc::make_mut(&mut merged.inner);
        let other_inner = &other.inner;

        // 1. ビットマスクのマージ
        inner_mut.mask.merge(other_inner.mask);

        // 2. 基本レイアウトプロパティのオーバーライド
        if other_inner.mask.has_basic_layout() {
            inner_mut
                .basic_layout
                .override_with(&other_inner.basic_layout, other_inner.mask);
        }

        // 3. Flexレイアウトプロパティのオーバーライド
        if other_inner.mask.has_flex_layout() {
            inner_mut
                .flex_layout
                .override_with(&other_inner.flex_layout, other_inner.mask);
        }

        // 4. ビジュアルプロパティのオーバーライド
        if other_inner.mask.has_visual_property() {
            inner_mut
                .visual_property
                .override_with(&other_inner.visual_property, other_inner.mask);
        }

        // 5. 疑似クラス（インタラクションプロパティ）のオーバーライド
        if other_inner.mask.has_interaction_property()
            || other_inner.mask.has(STYLE_INTERACTION_WITHIN)
            || other_inner.mask.has(STYLE_INTERACTION_PARENT)
        {
            inner_mut
                .interaction_styles
                .override_with(&other_inner.interaction_styles, other_inner.mask);
        }

        // 6. コールドデータのコピー
        if other_inner.mask.has_grid_layout()
            && let Some(ref g) = other_inner.grid_layout
        {
            inner_mut.grid_layout = Some(g.clone());
        }
        if let Some(ref sb) = other_inner.scrollbar_style {
            inner_mut.scrollbar_style = Some(sb.clone());
        }

        // 7. 動的セッターの結合
        inner_mut
            .dynamic_setters
            .extend(other_inner.dynamic_setters.clone());

        // 8. D&D設定
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
