use crate::{
    AlignContent, AlignItems, AlignSelf, BasicLayout, BoxShadow, BoxSizing, Clear, Color,
    CornerRadius, CursorIcon, Direction, Display, FlexDirection, FlexLayout, FlexWrap, Float,
    FromPercent, GridAutoFlow, GridLayout, GridLine, GridPlacement, InteractionStyles,
    JustifyContent, LayoutOverflow, Length, LinearGradient, Point, Position, Rect, Size, TextAlign,
    Transform, Transition, Val, VisualProperty, bitmap::*,
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
    pub fn hidden(mut self) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.display = Display::None;
        inner.mask.set(STYLE_DISPLAY);
        self
    }

    #[inline]
    pub fn flex(mut self) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.display = Display::Flex;
        inner.mask.set(STYLE_DISPLAY);
        self
    }

    #[inline]
    pub fn grid(mut self) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.display = Display::Grid;
        inner.mask.set(STYLE_DISPLAY);
        self
    }

    #[inline]
    pub fn block(mut self) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.display = Display::Block;
        inner.mask.set(STYLE_DISPLAY);
        self
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
    pub fn box_border(mut self) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.box_sizing = BoxSizing::BorderBox;
        inner.mask.set(STYLE_BOX_SIZING);
        self
    }

    #[inline]
    pub fn box_content(mut self) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.box_sizing = BoxSizing::ContentBox;
        inner.mask.set(STYLE_BOX_SIZING);
        self
    }

    /// テキストや要素のインライン方向（Direction）を設定します。
    #[inline]
    pub fn direction(mut self, value: Direction) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.direction = value;
        inner.mask.set(STYLE_DIRECTION);
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

    /// スクロールバーの幅を物理的なピクセル数などで設定します。
    #[inline]
    pub fn scrollbar_width(mut self, value: f32) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.scrollbar_width = value;
        inner.mask.set(STYLE_SCROLLBAR_WIDTH);
        self
    }

    /// 要素を左右のどちらに回り込ませるか（Float）を設定します。
    #[inline]
    pub fn float(mut self, value: Float) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.float = value;
        inner.mask.set(STYLE_FLOAT);
        self
    }

    /// 回り込み（Float）を解除するかどうか（Clear）を設定します。
    #[inline]
    pub fn clear(mut self, value: Clear) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.clear = value;
        inner.mask.set(STYLE_CLEAR);
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

    /// 要素の配置インセット（inset：top, right, bottom, left）を設定します。
    #[inline]
    pub fn inset(mut self, value: Rect<Val>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.inset = value;
        inner.mask.set(STYLE_INSET);
        self
    }

    /// 要素の基本サイズ（width, height）を設定します。
    #[inline]
    pub fn size(mut self, value: Size<Val>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.size = value;
        inner.mask.set(STYLE_SIZE);
        self
    }

    /// 要素の最小サイズを設定します。
    #[inline]
    pub fn min_size(mut self, value: Size<Val>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.min_size = value;
        inner.mask.set(STYLE_MIN_SIZE);
        self
    }

    /// 要素の最大サイズを設定します。
    #[inline]
    pub fn max_size(mut self, value: Size<Val>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.max_size = value;
        inner.mask.set(STYLE_MAX_SIZE);
        self
    }

    /// アスペクト比を設定します。
    #[inline]
    pub fn aspect_ratio(mut self, value: Option<f32>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.aspect_ratio = value;
        inner.mask.set(STYLE_ASPECT_RATIO);
        self
    }

    /// 外側余白（margin）を設定します。
    #[inline]
    pub fn margin(mut self, value: Rect<Val>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.margin = value;
        inner.mask.set(STYLE_MARGIN);
        self
    }

    /// 内側余白（padding）を設定します。
    #[inline]
    pub fn padding(mut self, value: Rect<Length>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.padding = value;
        inner.mask.set(STYLE_PADDING);
        self
    }

    /// 境界線の太さ（border）を設定します。
    #[inline]
    pub fn border(mut self, value: Rect<Length>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.basic_layout.border = value;
        inner.mask.set(STYLE_BORDER);
        self
    }

    /// コンテナ内の一括交差軸配置を設定します。
    #[inline]
    pub fn align_items(mut self, value: impl Into<Option<AlignItems>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.align_items = value.into();
        inner.mask.set(STYLE_ALIGN_ITEMS);
        self
    }

    /// 個別要素の交差軸配置を設定します。
    #[inline]
    pub fn align_self(mut self, value: impl Into<Option<AlignSelf>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.align_self = value.into();
        inner.mask.set(STYLE_ALIGN_SELF);
        self
    }

    /// コンテナ内の一括主軸配置を設定します。
    #[inline]
    pub fn justify_items(mut self, value: impl Into<Option<AlignItems>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.justify_items = value.into();
        inner.mask.set(STYLE_JUSTIFY_ITEMS);
        self
    }

    /// 個別要素の主軸配置を設定します。
    #[inline]
    pub fn justify_self(mut self, value: impl Into<Option<AlignSelf>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.justify_self = value.into();
        inner.mask.set(STYLE_JUSTIFY_SELF);
        self
    }

    /// 複数行にまたがる場合のコンテンツ一括配置を設定します。
    #[inline]
    pub fn align_content(mut self, value: impl Into<Option<AlignContent>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.align_content = value.into();
        inner.mask.set(STYLE_ALIGN_CONTENT);
        self
    }

    /// 主軸方向のコンテンツ配置を設定します。
    #[inline]
    pub fn justify_content(mut self, value: impl Into<Option<JustifyContent>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.justify_content = value.into();
        inner.mask.set(STYLE_JUSTIFY_CONTENT);
        self
    }

    /// 要素間の行・列方向の隙間（gap）を設定します。
    #[inline]
    pub fn gap(mut self, value: Size<Val>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.gap = value;
        inner.mask.set(STYLE_GAP);
        self
    }

    /// テキストの配置揃え方向を設定します。
    #[inline]
    pub fn text_align(mut self, value: TextAlign) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.text_align = value;
        inner.mask.set(STYLE_TEXT_ALIGN);
        self
    }

    /// Flexコンテナ内での主軸の方向を設定します。
    #[inline]
    pub fn flex_direction(mut self, value: FlexDirection) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.flex_direction = value;
        inner.mask.set(STYLE_FLEX_DIRECTION);
        self
    }

    /// 子要素を複数行に折り返すかどうかを設定します。
    #[inline]
    pub fn flex_wrap(mut self, value: FlexWrap) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.flex_wrap = value;
        inner.mask.set(STYLE_FLEX_WRAP);
        self
    }

    /// 子要素の基準となる基本寸法を設定します。
    #[inline]
    pub fn flex_basis(mut self, value: Val) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.flex_basis = value;
        inner.mask.set(STYLE_FLEX_BASIS);
        self
    }

    /// 要素の伸長係数を設定します。
    #[inline]
    pub fn flex_grow(mut self, value: f32) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.flex_grow = value;
        inner.mask.set(STYLE_FLEX_GROW);
        self
    }

    /// 要素の縮小係数を設定します。
    #[inline]
    pub fn flex_shrink(mut self, value: f32) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.flex_layout.flex_shrink = value;
        inner.mask.set(STYLE_FLEX_SHRINK);
        self
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
    pub fn corner_radius(mut self, value: CornerRadius) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.corner_radius = Some(value);
        inner.mask.set(STYLE_CORNER_RADIUS);
        self
    }

    /// 要素全体の不透明度を設定します。
    #[inline]
    pub fn opacity(mut self, value: f32) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.opacity = Some(value);
        inner.mask.set(STYLE_OPACITY);
        self
    }

    /// 要素の外側に配置する影を設定します。
    #[inline]
    pub fn box_shadow(mut self, value: BoxShadow) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.box_shadow = Some(value);
        inner.mask.set(STYLE_BOX_SHADOW);
        self
    }

    /// クリップパスの定義文字列を設定します。
    #[inline]
    pub fn clip_path(mut self, path: impl Into<Cow<'static, str>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.clip_path = Some(path.into());
        inner.mask.set(STYLE_CLIP_PATH);
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

    /// この要素の上にマウスが乗った際のマウスクラスアイコンを設定します。
    #[inline]
    pub fn cursor(mut self, value: CursorIcon) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.cursor = Some(value);
        inner.mask.set(STYLE_CURSOR);
        self
    }

    /// 描画フィルター効果を設定します。
    #[inline]
    pub fn filter(mut self, filter: impl Into<Cow<'static, str>>) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.filter = Some(filter.into());
        inner.mask.set(STYLE_FILTER);
        self
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

    /// 要素にアフィン変換（平行移動・拡大・回転）を適用します。
    #[inline]
    pub fn transform(mut self, value: Transform) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        inner.visual_property.transform = Some(value.matrix);
        inner.mask.set(STYLE_TRANSFORM);
        self
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
}

pub fn ts() -> ThisStyle {
    ThisStyle::new()
}

/// 呼び出し側の文脈（代入先）に応じて、自動的に `Length::Px` または `Val::Px` に解決される物理ピクセル値を生成します。
#[inline]
pub fn px<T>(val: f32) -> T
where
    T: From<f32>,
{
    T::from(val)
}

/// 呼び出し側の文脈に応じて、自動的に `Length::Percent` または `Val::Percent` に解決されるパーセント値を生成します。
#[inline]
pub fn pct<T>(val: f32) -> T
where
    T: FromPercent,
{
    T::percent(val)
}

/// 呼び出し側の文脈が `Val` を期待している場合に、自動的に `Val::Auto` に解決して生成します。
/// (※ `Length` はAutoを許容しないため、Lengthを期待する文脈では安全にコンパイルエラーになります)
#[inline]
pub fn auto<T>() -> T
where
    T: FromAuto,
{
    T::auto()
}

/// `auto()` 関数の自動解決を支援するための補助トレイト
pub trait FromAuto {
    fn auto() -> Self;
}

impl FromAuto for Val {
    #[inline]
    fn auto() -> Self {
        Self::Auto
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

#[inline]
pub fn linear() -> AnimationCurve {
    AnimationCurve::Linear
}

#[inline]
pub fn ease_in_out_quad() -> AnimationCurve {
    AnimationCurve::EaseInOutQuad
}

#[inline]
pub fn ease_in_quad() -> AnimationCurve {
    AnimationCurve::EaseInQuad
}

#[inline]
pub fn ease_out_quad() -> AnimationCurve {
    AnimationCurve::EaseOutQuad
}

#[inline]
pub fn custom_curve(f: fn(f32) -> f32) -> AnimationCurve {
    AnimationCurve::Custom(f)
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

#[inline]
pub fn playback_count_inf() -> PlaybackCount {
    PlaybackCount::Infinite
}

#[inline]
pub fn playback_count(count: u32) -> PlaybackCount {
    PlaybackCount::Count(count)
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
