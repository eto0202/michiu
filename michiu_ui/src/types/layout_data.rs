use crate::{
    AlignContent, AlignItems, AlignSelf, Backdrop, BaseVisualPropertiesSecondary, BorderAlignment,
    BorderStyle, BoxShadow, BoxSizing, Color, ComponentMask, CornerRadius, CursorIcon, Direction,
    Display, EdgeInsets, EntityId, FlexDirection, FlexWrap, Focusable, FontDate, GridAutoFlow,
    GridLine, GridPlacement, IDENTITY_MATRIX, JustifyContent, KeyframeAnimation, LayoutOverflow,
    Length, LinearGradient, MichiuSoA, Point, PointerEvents, Position, Rect, Size, StyleTarget,
    TextAlign, ThisStyle, Transition, UserSelect, Val, VisualPropertiesSecondary,
};
use std::{
    borrow::Cow,
    sync::{Arc, LazyLock},
};

#[derive(Debug, Clone, Copy, Default)]
pub struct NormalLayout {
    pub basic: BasicLayout,
    pub flex: FlexLayout,
}

impl NormalLayout {
    #[inline]
    #[must_use]
    pub fn split(&self) -> (&BasicLayout, &FlexLayout) {
        (&self.basic, &self.flex)
    }

    #[inline]
    #[must_use]
    pub fn split_mut(&mut self) -> (&mut BasicLayout, &mut FlexLayout) {
        (&mut self.basic, &mut self.flex)
    }

    #[inline]
    pub fn override_with(&mut self, basic: &BasicLayout, flex: &FlexLayout, mask: ComponentMask) {
        self.basic.override_with(basic, mask);
        self.flex.override_with(flex, mask);
    }
}

/// 要素がほぼ必ず持つ、基本のレイアウト情報。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BasicLayout {
    pub display: Display,
    pub box_sizing: BoxSizing,
    pub direction: Direction,
    pub position: Position,
    pub overflow: LayoutOverflow, // 2バイト
    pub item_is_table: bool,      // 1バイト
    pub item_is_replaced: bool,   // 1バイト
    pub resizable: [bool; 4],     // 4バイト

    pub size: Size<Val>,           // Val(8バイト) * 2 = 16バイト
    pub min_size: Size<Val>,       // 16バイト
    pub max_size: Size<Val>,       // 16バイト
    pub aspect_ratio: Option<f32>, // f32 + タグ = 8バイト

    pub inset: Rect<Val>,      // Val(8バイト) * 4 = 32バイト
    pub margin: Rect<Val>,     // 32バイト
    pub padding: Rect<Length>, // Length(8バイト) * 4 = 32バイト
    pub border: Rect<Length>,
}

pub(crate) static DEFAULT_BASIC: LazyLock<BasicLayout> = LazyLock::new(|| {
    let default_style: taffy::Style = taffy::Style::default();
    BasicLayout {
        display: default_style.display.into(),
        box_sizing: default_style.box_sizing.into(),
        direction: default_style.direction.into(),
        position: default_style.position.into(),
        overflow: default_style.overflow.into(),
        item_is_table: default_style.item_is_table,
        item_is_replaced: default_style.item_is_replaced,
        resizable: [false; 4],
        size: default_style.size.into(),
        min_size: default_style.min_size.into(),
        max_size: default_style.max_size.into(),
        aspect_ratio: default_style.aspect_ratio,
        inset: default_style.inset.into(),
        margin: default_style.margin.into(),
        padding: default_style.padding.into(),
        border: default_style.border.into(),
    }
});

impl Default for BasicLayout {
    fn default() -> Self {
        *DEFAULT_BASIC
    }
}

impl BasicLayout {
    /// 指定されたプロパティマスクに基づいて、自身を別のレイアウトデータで上書きします。
    #[inline]
    pub(crate) fn override_with(&mut self, other: &Self, mask: ComponentMask) {
        if mask.has(ComponentMask::STYLE_DISPLAY) {
            self.display = other.display;
        }
        if mask.has(ComponentMask::STYLE_ITEM_IS_TABLE) {
            self.item_is_table = other.item_is_table;
        }
        if mask.has(ComponentMask::STYLE_ITEM_IS_REPLACED) {
            self.item_is_replaced = other.item_is_replaced;
        }
        if mask.has(ComponentMask::STYLE_BOX_SIZING) {
            self.box_sizing = other.box_sizing;
        }
        if mask.has(ComponentMask::STYLE_DIRECTION) {
            self.direction = other.direction;
        }
        if mask.has(ComponentMask::STYLE_OVERFLOW) {
            self.overflow = other.overflow;
        }
        if mask.has(ComponentMask::STYLE_POSITION) {
            self.position = other.position;
        }
        if mask.has(ComponentMask::STYLE_INSET) {
            self.inset = other.inset;
        }
        if mask.has(ComponentMask::STYLE_SIZE) {
            self.size = other.size;
        }
        if mask.has(ComponentMask::STYLE_MIN_SIZE) {
            self.min_size = other.min_size;
        }
        if mask.has(ComponentMask::STYLE_MAX_SIZE) {
            self.max_size = other.max_size;
        }
        if mask.has(ComponentMask::STYLE_ASPECT_RATIO) {
            self.aspect_ratio = other.aspect_ratio;
        }
        if mask.has(ComponentMask::STYLE_MARGIN) {
            self.margin = other.margin;
        }
        if mask.has(ComponentMask::STYLE_PADDING) {
            self.padding = other.padding;
        }
        if mask.has(ComponentMask::STYLE_BORDER) {
            self.border = other.border;
        }
        if mask.has(ComponentMask::STYLE_RESIZABLE) {
            self.resizable = other.resizable;
        }
    }
}

// 2. FlexLayout (Flexboxレイアウト：13プロパティ) - ホットデータ
/// Flexboxコンテナ、またはその子要素に適用される情報。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlexLayout {
    pub flex_grow: f32,   // 4バイト
    pub flex_shrink: f32, // 4バイト
    pub flex_basis: Val,  // 8バイト (タグ1 + パディング3 + f32 4)
    pub gap: Size<Val>,   // 16バイト (Val 8バイト * 2)

    pub align_items: Option<AlignItems>,
    pub align_self: Option<AlignSelf>,
    pub justify_items: Option<AlignItems>,
    pub justify_self: Option<AlignSelf>,
    pub align_content: Option<AlignContent>,
    pub justify_content: Option<JustifyContent>,

    pub text_align: TextAlign,
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
}

impl FlexLayout {
    /// 指定されたプロパティマスクに基づいて、自身を別のFlexレイアウトデータで上書きします。
    #[inline]
    pub(crate) fn override_with(&mut self, other: &Self, mask: ComponentMask) {
        if mask.has(ComponentMask::STYLE_ALIGN_ITEMS) {
            self.align_items = other.align_items;
        }
        if mask.has(ComponentMask::STYLE_ALIGN_SELF) {
            self.align_self = other.align_self;
        }
        if mask.has(ComponentMask::STYLE_JUSTIFY_ITEMS) {
            self.justify_items = other.justify_items;
        }
        if mask.has(ComponentMask::STYLE_JUSTIFY_SELF) {
            self.justify_self = other.justify_self;
        }
        if mask.has(ComponentMask::STYLE_ALIGN_CONTENT) {
            self.align_content = other.align_content;
        }
        if mask.has(ComponentMask::STYLE_JUSTIFY_CONTENT) {
            self.justify_content = other.justify_content;
        }
        if mask.has(ComponentMask::STYLE_GAP) {
            self.gap = other.gap;
        }
        if mask.has(ComponentMask::STYLE_TEXT_ALIGN) {
            self.text_align = other.text_align;
        }
        if mask.has(ComponentMask::STYLE_FLEX_DIRECTION) {
            self.flex_direction = other.flex_direction;
        }
        if mask.has(ComponentMask::STYLE_FLEX_WRAP) {
            self.flex_wrap = other.flex_wrap;
        }
        if mask.has(ComponentMask::STYLE_FLEX_BASIS) {
            self.flex_basis = other.flex_basis;
        }
        if mask.has(ComponentMask::STYLE_FLEX_GROW) {
            self.flex_grow = other.flex_grow;
        }
        if mask.has(ComponentMask::STYLE_FLEX_SHRINK) {
            self.flex_shrink = other.flex_shrink;
        }
    }
}

pub(crate) static DEFAULT_FLEX: LazyLock<FlexLayout> = LazyLock::new(|| {
    let default_style: taffy::Style = taffy::Style::default();
    FlexLayout {
        flex_grow: default_style.flex_grow,
        flex_shrink: default_style.flex_shrink,
        flex_basis: default_style.flex_basis.into(),
        gap: default_style.gap.into(),
        align_items: default_style.align_items.map(std::convert::Into::into),
        align_self: default_style.align_self.map(std::convert::Into::into),
        justify_items: default_style.justify_items.map(std::convert::Into::into),
        justify_self: default_style.justify_self.map(std::convert::Into::into),
        align_content: default_style.align_content.map(std::convert::Into::into),
        justify_content: default_style.justify_content.map(std::convert::Into::into),
        text_align: default_style.text_align.into(),
        flex_direction: default_style.flex_direction.into(),
        flex_wrap: default_style.flex_wrap.into(),
    }
});

impl Default for FlexLayout {
    fn default() -> Self {
        *DEFAULT_FLEX
    }
}

// 3. GridLayout (Gridレイアウト：10プロパティ) - コールドデータ
/// Gridコンテナ、およびGrid子要素に適用される複雑な情報。
/// `Vec`（動的配列）を多数含み、メモリ上で重いため `Copy` は不可能（`Clone` のみ）。
/// 滅多に使われないため、`SparseSecondaryMap` で隔離し、未使用の要素には一切メモリを消費させない。
#[derive(Debug, Clone, PartialEq)]
pub struct GridLayout {
    pub grid_template_rows: Vec<taffy::GridTemplateComponent<String>>,
    pub grid_template_columns: Vec<taffy::GridTemplateComponent<String>>,
    pub grid_auto_rows: Vec<taffy::TrackSizingFunction>,
    pub grid_auto_columns: Vec<taffy::TrackSizingFunction>,
    pub grid_auto_flow: GridAutoFlow,
    pub grid_template_areas: Vec<taffy::GridTemplateArea<String>>,
    pub grid_template_column_names: Vec<Vec<String>>,
    pub grid_template_row_names: Vec<Vec<String>>,
    pub grid_row: GridLine<GridPlacement<String>>,
    pub grid_column: GridLine<GridPlacement<String>>,
}

unsafe impl Send for GridLayout {}
unsafe impl Sync for GridLayout {}

impl Default for GridLayout {
    fn default() -> Self {
        // Taffyデフォルト値と完全に一致するように空のVecで初期化
        Self {
            grid_template_rows: Vec::new(),
            grid_template_columns: Vec::new(),
            grid_auto_rows: Vec::new(),
            grid_auto_columns: Vec::new(),
            grid_auto_flow: GridAutoFlow::Row,
            grid_template_areas: Vec::new(),
            grid_template_column_names: Vec::new(),
            grid_template_row_names: Vec::new(),
            grid_row: GridLine {
                start: GridPlacement::Auto,
                end: GridPlacement::Auto,
            },
            grid_column: GridLine {
                start: GridPlacement::Auto,
                end: GridPlacement::Auto,
            },
        }
    }
}

/// 要素の描画（ペイント）に関する一括情報。
/// wgpu への高速インスタンスバッファ転送時に、時間・空間的キャッシュ局所性を最大化するために一塊で管理。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VisualProperty {
    pub bg_color: Option<Color>,
    pub border_color: Option<Color>,
    pub border_lengths: Option<EdgeInsets>,
    pub border_styles: Option<[BorderStyle; 4]>,
    pub border_alignments: Option<[BorderAlignment; 4]>,
    pub corner_radius: Option<CornerRadius>,
    pub opacity: Option<f32>,
    pub shadow_params: Option<BoxShadow>,
    pub shadow_color: Option<Color>,
    pub transform: Option<[[f32; 4]; 4]>,
    pub transform_origin: Option<Point<f32>>,
    pub transform_inherit: Option<bool>,
    pub z_index: Option<i32>,
    pub cursor: Option<CursorIcon>,
    /// 各方向 [Ns, Ew, Nesw, Nwse] のカスタムカーソル指定
    pub resizable_cursor: Option<[Option<CursorIcon>; 4]>,
    pub backdrop: Backdrop,
    pub text_color: Option<Color>,
    pub font: FontDate,
    pub auto_wrap: Option<bool>,
    pub bg_gradient: Option<LinearGradient>,
    pub transitions: Vec<Transition>,
    pub keyframe_animations: Vec<KeyframeAnimation>,
    pub pointer_events: Option<PointerEvents>,
    pub user_select: Option<UserSelect>,
    pub select_bg_color: Option<Color>,
    pub select_text_color: Option<Color>,
    pub focusable: Option<Focusable>,
    pub prevent_focus_steal: Option<bool>,
    pub prevent_focus_steal_within: Option<bool>,
    pub outline_width: Option<EdgeInsets>,
    pub outline_color: Option<Color>,
    pub outline_lengths: Option<EdgeInsets>,
    pub outline_styles: Option<[BorderStyle; 4]>,
    pub outline_alignments: Option<[BorderAlignment; 4]>,
    pub outline_offset: Option<f32>,
}

#[derive(Debug, Clone)]
pub(crate) struct CurrentStyle {
    pub(crate) bg_color: Color,
    pub(crate) border_color: Color,
    pub(crate) outline_width: EdgeInsets,
    pub(crate) outline_color: Color,
    pub(crate) outline_offset: f32,
    pub(crate) opacity: f32,
    pub(crate) transform: [[f32; 4]; 4],
    pub(crate) transform_origin: Point<f32>,
    pub(crate) corner_radius: CornerRadius,
    pub(crate) shadow_params: BoxShadow,
    pub(crate) text_color: Color,
    pub(crate) font_size: f32,
    pub(crate) font_family: Option<Cow<'static, str>>,
    pub(crate) font_weight: u32,
    pub(crate) font_style: u32,
    pub(crate) auto_wrap: bool,
    pub(crate) pointer_events: PointerEvents,
}

impl Default for CurrentStyle {
    fn default() -> Self {
        CurrentStyle {
            bg_color: Color::TRANSPARENT,
            border_color: Color::TRANSPARENT,
            outline_width: EdgeInsets::ZERO,
            outline_color: Color::TRANSPARENT,
            outline_offset: 0.0,
            opacity: 1.0,
            transform: IDENTITY_MATRIX,
            transform_origin: Point::ORIGIN,
            corner_radius: CornerRadius::ZERO,
            shadow_params: BoxShadow::none(),
            text_color: Color::WHITE,
            font_size: 16.0,
            font_family: None,
            font_weight: 400,
            font_style: 0,
            auto_wrap: false,
            pointer_events: PointerEvents::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TargetStyle {
    pub(crate) pointer_events: Option<PointerEvents>,
    pub(crate) cursor: Option<CursorIcon>,
    pub(crate) resizable_cursor: Option<[Option<CursorIcon>; 4]>,
    pub(crate) bg_color: Option<Color>,
    pub(crate) border_color: Option<Color>,
    pub(crate) opacity: Option<f32>,
    pub(crate) transform: Option<[[f32; 4]; 4]>,
    pub(crate) transform_origin: Option<Point<f32>>,
    pub(crate) transform_inherit: Option<bool>,
    pub(crate) corner_radius: Option<CornerRadius>,
    pub(crate) shadow_params: Option<BoxShadow>,
    pub(crate) shadow_color: Option<Color>,
    pub(crate) text_color: Option<Color>,
    pub(crate) select_bg_color: Option<Color>,
    pub(crate) select_text_color: Option<Color>,
    pub(crate) border_lengths: Option<EdgeInsets>,
    pub(crate) border_styles: Option<[BorderStyle; 4]>,
    pub(crate) border_alignments: Option<[BorderAlignment; 4]>,
    pub(crate) outline_width: Option<EdgeInsets>,
    pub(crate) outline_color: Option<Color>,
    pub(crate) outline_lengths: Option<EdgeInsets>,
    pub(crate) outline_styles: Option<[BorderStyle; 4]>,
    pub(crate) outline_alignments: Option<[BorderAlignment; 4]>,
    pub(crate) outline_offset: Option<f32>,
    pub(crate) font: FontDate,
    pub(crate) auto_wrap: Option<bool>,
}

impl VisualProperty {
    pub(crate) fn override_with(&mut self, other: &Self, mask: ComponentMask) {
        if mask.has(ComponentMask::STYLE_BG_COLOR) {
            self.bg_color = other.bg_color;
            self.bg_gradient = other.bg_gradient;
        }
        if mask.has(ComponentMask::STYLE_BORDER_COLOR) {
            self.border_color = other.border_color;
        }
        if mask.has(ComponentMask::STYLE_BORDER) {
            self.border_lengths = other.border_lengths;
            self.border_styles = other.border_styles;
            self.border_alignments = other.border_alignments;
        }
        if mask.has(ComponentMask::STYLE_CORNER_RADIUS) {
            self.corner_radius = other.corner_radius;
        }
        if mask.has(ComponentMask::STYLE_OPACITY) {
            self.opacity = other.opacity;
        }
        if mask.has(ComponentMask::STYLE_BOX_SHADOW) {
            self.shadow_params = other.shadow_params;
            self.shadow_color = other.shadow_color;
        }
        if mask.has(ComponentMask::STYLE_TRANSFORM) {
            self.transform = other.transform;
            self.transform_origin = other.transform_origin;
        }
        if mask.has(ComponentMask::STYLE_TRANSFORM_INHERIT) {
            self.transform_inherit = other.transform_inherit;
        }
        if mask.has(ComponentMask::STYLE_Z_INDEX) {
            self.z_index = other.z_index;
        }
        if mask.has(ComponentMask::STYLE_CURSOR) {
            self.cursor = other.cursor;
        }
        if mask.has(ComponentMask::STYLE_RESIZABLE) {
            self.resizable_cursor = other.resizable_cursor;
        }
        if mask.has(ComponentMask::STYLE_BACKDROP) {
            self.backdrop = other.backdrop;
        }
        if mask.has(ComponentMask::STYLE_TEXT_COLOR) {
            self.text_color = other.text_color;
        }
        if mask.has(ComponentMask::STYLE_FONT_SIZE) {
            self.font.size = other.font.size;
        }
        if mask.has(ComponentMask::STYLE_FONT_STYLE) {
            if other.font.family.is_some() {
                self.font.family.clone_from(&other.font.family);
            }
            if other.font.weight.is_some() {
                self.font.weight = other.font.weight;
            }
            if other.font.style.is_some() {
                self.font.style = other.font.style;
            }
        }
        if mask.has(ComponentMask::STYLE_AUTO_WRAP) {
            self.auto_wrap = other.auto_wrap;
        }
        if mask.has(ComponentMask::STYLE_POINTER_EVENTS) {
            self.pointer_events = other.pointer_events;
        }
        if mask.has(ComponentMask::STYLE_USER_SELECT) {
            self.user_select = other.user_select;
            self.select_bg_color = other.select_bg_color;
            self.select_text_color = other.select_text_color;
        }
        if mask.has(ComponentMask::STYLE_FOCUSABLE) {
            self.focusable = other.focusable;
        }
        if mask.has(ComponentMask::STYLE_PREVENT_FOCUS_STEAL) {
            self.prevent_focus_steal = other.prevent_focus_steal;
        }
        if mask.has(ComponentMask::STYLE_PREVENT_FOCUS_STEAL_WITHIN) {
            self.prevent_focus_steal_within = other.prevent_focus_steal_within;
        }
        if mask.has(ComponentMask::STYLE_OUTLINE) {
            self.outline_width = other.outline_width;
            self.outline_color = other.outline_color;
            self.outline_lengths = other.outline_lengths;
            self.outline_styles = other.outline_styles;
            self.outline_alignments = other.outline_alignments;
            self.outline_offset = other.outline_offset;
        }

        // 複数追加できるものは破棄せず結合
        if mask.has(ComponentMask::STYLE_TRANSITIONS) {
            self.transitions.extend(other.transitions.clone());
        }
        if mask.has(ComponentMask::STYLE_ANIMATIONS) {
            self.keyframe_animations
                .extend(other.keyframe_animations.clone());
        }
    }

    /// 現在の描画用データを取得 (Copy可能なプリミティブのみ)
    #[inline]
    pub(crate) fn get_current_style(
        id: EntityId,
        rnd_visual: &VisualPropertiesSecondary,
    ) -> CurrentStyle {
        rnd_visual
            .find(id)
            .map(|v| CurrentStyle {
                bg_color: v.bg_color.unwrap_or(Color::TRANSPARENT),
                border_color: v.border_color.unwrap_or(Color::TRANSPARENT),
                outline_width: v.outline_width.unwrap_or(EdgeInsets::ZERO),
                outline_color: v.outline_color.unwrap_or(Color::TRANSPARENT),
                outline_offset: v.outline_offset.unwrap_or(0.0),
                opacity: v.opacity.unwrap_or(1.0),
                transform: v.transform.unwrap_or(IDENTITY_MATRIX),
                transform_origin: v.transform_origin.unwrap_or(Point::ORIGIN),
                corner_radius: v.corner_radius.unwrap_or(CornerRadius::ZERO),
                shadow_params: v.shadow_params.unwrap_or(BoxShadow::none()),
                text_color: v.text_color.unwrap_or(Color::WHITE),
                font_size: v.font.size.unwrap_or(16.0),
                font_family: v.font.family.clone(),
                font_weight: v.font.weight.unwrap_or(400),
                font_style: v.font.style.unwrap_or(0),
                auto_wrap: v.auto_wrap.unwrap_or(false),
                pointer_events: v.pointer_events.unwrap_or_default(),
            })
            .unwrap_or_default()
    }

    /// 目標値を参照経由で構築
    #[inline]
    pub(crate) fn get_target_style(
        id: EntityId,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
    ) -> TargetStyle {
        rnd_base_visual
            .find(id)
            .map(|v| TargetStyle {
                pointer_events: v.pointer_events,
                cursor: v.cursor,
                resizable_cursor: v.resizable_cursor,
                bg_color: v.bg_color,
                border_color: v.border_color,
                opacity: v.opacity,
                transform: v.transform,
                transform_origin: v.transform_origin,
                transform_inherit: v.transform_inherit,
                corner_radius: v.corner_radius,
                shadow_params: v.shadow_params,
                shadow_color: v.shadow_color,
                text_color: v.text_color,
                select_bg_color: v.select_bg_color,
                select_text_color: v.select_text_color,
                border_lengths: v.border_lengths,
                border_styles: v.border_styles,
                border_alignments: v.border_alignments,
                outline_width: v.outline_width,
                outline_color: v.outline_color,
                outline_lengths: v.outline_lengths,
                outline_styles: v.outline_styles,
                outline_alignments: v.outline_alignments,
                outline_offset: v.outline_offset,
                font: v.font.clone(),
                auto_wrap: v.auto_wrap,
            })
            .unwrap_or_default()
    }

    /// 指定された `VisualProperty` と `ComponentMask` を基に自身のスタイルをマージ。
    #[inline]
    pub(crate) fn apply_visual_property(
        &self,
        target: &mut TargetStyle,
        inner_mask: ComponentMask,
    ) {
        if inner_mask.has(ComponentMask::STYLE_BG_COLOR) {
            target.bg_color = self.bg_color;
        }
        if inner_mask.has(ComponentMask::STYLE_BORDER_COLOR) {
            target.border_color = self.border_color;
        }
        if inner_mask.has(ComponentMask::STYLE_OPACITY) {
            target.opacity = self.opacity;
        }
        if inner_mask.has(ComponentMask::STYLE_TRANSFORM) {
            target.transform = self.transform;
            target.transform_origin = self.transform_origin;
        }

        if inner_mask.has(ComponentMask::STYLE_TRANSFORM_INHERIT) {
            target.transform_inherit = self.transform_inherit;
        }
        if inner_mask.has(ComponentMask::STYLE_CORNER_RADIUS) {
            target.corner_radius = self.corner_radius;
        }
        if inner_mask.has(ComponentMask::STYLE_POINTER_EVENTS) {
            target.pointer_events = self.pointer_events;
        }
        if inner_mask.has(ComponentMask::STYLE_BOX_SHADOW) {
            if self.shadow_params.is_some() {
                target.shadow_params = self.shadow_params;
            }
            if self.shadow_color.is_some() {
                target.shadow_color = self.shadow_color;
            }
        }
        if inner_mask.has(ComponentMask::STYLE_TEXT_COLOR) {
            target.text_color = self.text_color;
        }
        if inner_mask.has(ComponentMask::STYLE_USER_SELECT) {
            if self.select_bg_color.is_some() {
                target.select_bg_color = self.select_bg_color;
            }
            if self.select_text_color.is_some() {
                target.select_text_color = self.select_text_color;
            }
        }
        if inner_mask.has(ComponentMask::STYLE_BORDER) {
            if self.border_lengths.is_some() {
                target.border_lengths = self.border_lengths;
            }
            if self.border_styles.is_some() {
                target.border_styles = self.border_styles;
            }
            if self.border_alignments.is_some() {
                target.border_alignments = self.border_alignments;
            }
        }
        if inner_mask.has(ComponentMask::STYLE_OUTLINE) {
            if self.outline_width.is_some() {
                target.outline_width = self.outline_width;
            }
            if self.outline_color.is_some() {
                target.outline_color = self.outline_color;
            }
            if self.outline_lengths.is_some() {
                target.outline_lengths = self.outline_lengths;
            }
            if self.outline_styles.is_some() {
                target.outline_styles = self.outline_styles;
            }
            if self.outline_alignments.is_some() {
                target.outline_alignments = self.outline_alignments;
            }
            if self.outline_offset.is_some() {
                target.outline_offset = self.outline_offset;
            }
        }
        if inner_mask.has(ComponentMask::STYLE_CURSOR) {
            target.cursor = self.cursor;
        }
        if inner_mask.has(ComponentMask::STYLE_RESIZABLE) {
            target.resizable_cursor = self.resizable_cursor;
        }
        if inner_mask.has(ComponentMask::STYLE_FONT_SIZE) {
            target.font.size = self.font.size;
        }
        if inner_mask.has(ComponentMask::STYLE_FONT_STYLE) {
            if self.font.family.is_some() {
                target.font.family.clone_from(&self.font.family);
            }
            if self.font.weight.is_some() {
                target.font.weight = self.font.weight;
            }
            if self.font.style.is_some() {
                target.font.style = self.font.style;
            }
        }
        if inner_mask.has(ComponentMask::STYLE_AUTO_WRAP) {
            target.auto_wrap = self.auto_wrap;
        }
    }
}

/// インタラクション（動的状態）ごとにオーバーライドして適用される、追加のスタイル表現。
/// 滅多に使われない、かつ再帰的な構造を持つため、StyleInner の直下ではなくこの構造体に隠蔽して管理。
#[derive(Debug, Clone, Default)]
pub struct InteractionStyles {
    pub hovered: Option<ThisStyle>,
    pub focused: Option<ThisStyle>,
    pub focused_visible: Option<ThisStyle>,
    pub pressed: Option<ThisStyle>,
    pub disabled: Option<ThisStyle>,
    pub actived: Option<ThisStyle>,
    pub selected: Option<ThisStyle>,
    pub dragged: Option<ThisStyle>,

    pub dragging: Option<ThisStyle>,
    pub drag_in: Option<ThisStyle>,
    pub drag_over: Option<ThisStyle>,

    pub hovered_within: Option<ThisStyle>,
    pub focused_within: Option<ThisStyle>,
    pub focused_visible_within: Option<ThisStyle>,
    pub pressed_within: Option<ThisStyle>,
    pub disabled_within: Option<ThisStyle>,
    pub actived_within: Option<ThisStyle>,
    pub selected_within: Option<ThisStyle>,
    pub dragged_within: Option<ThisStyle>,
    pub any_within: Option<ThisStyle>, // All（いずれかのインタラクションがあればON）

    pub hovered_parent: Option<ThisStyle>,
    pub focused_parent: Option<ThisStyle>,
    pub focused_visible_parent: Option<ThisStyle>,
    pub pressed_parent: Option<ThisStyle>,
    pub disabled_parent: Option<ThisStyle>,
    pub actived_parent: Option<ThisStyle>,
    pub selected_parent: Option<ThisStyle>,
    pub dragged_parent: Option<ThisStyle>,
    pub any_parent: Option<ThisStyle>,
}

impl InteractionStyles {
    /// 与えられた疑似状態（StyleTarget）に対応する Option<ThisStyle> フィールドの実体可変参照を取得します
    #[track_caller]
    #[allow(clippy::unreachable)]
    #[inline]
    pub(crate) fn get_style_target_mut(&mut self, target: StyleTarget) -> &mut ThisStyle {
        match target {
            StyleTarget::Hovered => self.hovered.get_or_insert_with(ThisStyle::new),
            StyleTarget::Focused => self.focused.get_or_insert_with(ThisStyle::new),
            StyleTarget::FocusedVisible => self.focused_visible.get_or_insert_with(ThisStyle::new),
            StyleTarget::Pressed => self.pressed.get_or_insert_with(ThisStyle::new),
            StyleTarget::Disabled => self.disabled.get_or_insert_with(ThisStyle::new),
            StyleTarget::Actived => self.actived.get_or_insert_with(ThisStyle::new),
            StyleTarget::Selected => self.selected.get_or_insert_with(ThisStyle::new),
            StyleTarget::Dragged => self.dragged.get_or_insert_with(ThisStyle::new),
            StyleTarget::DndDragging => self.dragging.get_or_insert_with(ThisStyle::new),
            StyleTarget::DndDragIn => self.drag_in.get_or_insert_with(ThisStyle::new),
            StyleTarget::DndDragOver => self.drag_over.get_or_insert_with(ThisStyle::new),

            StyleTarget::HoveredWithin => self.hovered_within.get_or_insert_with(ThisStyle::new),
            StyleTarget::FocusedWithin => self.focused_within.get_or_insert_with(ThisStyle::new),
            StyleTarget::FocusedVisibleWithin => self
                .focused_visible_within
                .get_or_insert_with(ThisStyle::new),
            StyleTarget::PressedWithin => self.pressed_within.get_or_insert_with(ThisStyle::new),
            StyleTarget::DisabledWithin => self.disabled_within.get_or_insert_with(ThisStyle::new),
            StyleTarget::ActivedWithin => self.actived_within.get_or_insert_with(ThisStyle::new),
            StyleTarget::SelectedWithin => self.selected_within.get_or_insert_with(ThisStyle::new),
            StyleTarget::DraggedWithin => self.dragged_within.get_or_insert_with(ThisStyle::new),
            StyleTarget::AnyWithin => self.any_within.get_or_insert_with(ThisStyle::new),
            StyleTarget::Base => {
                unreachable!(
                    "\n\
                    Internal invariant violated. This is a bug in michiu_ui.\n\
                    Please report this issue at https://github.com/eto0202/michiu/issues\n\
                     [InteractionStyles]\n\
                     [target]      : {target:?},\n\
                     [loc]         : {}\n\
                    ",
                    std::panic::Location::caller()
                )
            }

            StyleTarget::HoveredParent => self.hovered_parent.get_or_insert_with(ThisStyle::new),
            StyleTarget::FocusedParent => self.focused_parent.get_or_insert_with(ThisStyle::new),
            StyleTarget::FocusedVisibleParent => self
                .focused_visible_parent
                .get_or_insert_with(ThisStyle::new),
            StyleTarget::PressedParent => self.pressed_parent.get_or_insert_with(ThisStyle::new),
            StyleTarget::DisabledParent => self.disabled_parent.get_or_insert_with(ThisStyle::new),
            StyleTarget::ActivedParent => self.actived_parent.get_or_insert_with(ThisStyle::new),
            StyleTarget::SelectedParent => self.selected_parent.get_or_insert_with(ThisStyle::new),
            StyleTarget::DraggedParent => self.dragged_parent.get_or_insert_with(ThisStyle::new),
            StyleTarget::AnyParent => self.any_parent.get_or_insert_with(ThisStyle::new),
        }
    }

    pub(crate) fn override_with(&mut self, other: &Self, _mask: ComponentMask) {
        let merge = |target: &mut Option<ThisStyle>, source: &Option<ThisStyle>| {
            if let Some(src) = source {
                if let Some(dst) = target {
                    let dst_inner = Arc::make_mut(&mut dst.inner);
                    let src_inner = &src.inner;

                    dst_inner.mask.0 |= src_inner.mask.0;
                    dst_inner
                        .basic_layout
                        .override_with(&src_inner.basic_layout, src_inner.mask);
                    dst_inner
                        .flex_layout
                        .override_with(&src_inner.flex_layout, src_inner.mask);
                    dst_inner
                        .visual_property
                        .override_with(&src_inner.visual_property, src_inner.mask);

                    if src_inner.mask.has_grid_layout()
                        && let Some(ref g) = src_inner.grid_layout
                    {
                        dst_inner.grid_layout = Some(g.clone());
                    }
                    dst_inner
                        .dynamic_setters
                        .extend(src_inner.dynamic_setters.clone());
                } else {
                    *target = Some(src.clone());
                }
            }
        };

        merge(&mut self.hovered, &other.hovered);
        merge(&mut self.focused, &other.focused);
        merge(&mut self.focused_visible, &other.focused_visible);
        merge(&mut self.pressed, &other.pressed);
        merge(&mut self.disabled, &other.disabled);
        merge(&mut self.actived, &other.actived);
        merge(&mut self.selected, &other.selected);
        merge(&mut self.dragged, &other.dragged);
        merge(&mut self.dragging, &other.dragging);
        merge(&mut self.drag_in, &other.drag_in);
        merge(&mut self.drag_over, &other.drag_over);

        merge(&mut self.hovered_within, &other.hovered_within);
        merge(&mut self.focused_within, &other.focused_within);
        merge(
            &mut self.focused_visible_within,
            &other.focused_visible_within,
        );
        merge(&mut self.pressed_within, &other.pressed_within);
        merge(&mut self.disabled_within, &other.disabled_within);
        merge(&mut self.actived_within, &other.actived_within);
        merge(&mut self.selected_within, &other.selected_within);
        merge(&mut self.dragged_within, &other.dragged_within);
        merge(&mut self.any_within, &other.any_within);

        merge(&mut self.hovered_parent, &other.hovered_parent);
        merge(&mut self.focused_parent, &other.focused_parent);
        merge(
            &mut self.focused_visible_parent,
            &other.focused_visible_parent,
        );
        merge(&mut self.pressed_parent, &other.pressed_parent);
        merge(&mut self.disabled_parent, &other.disabled_parent);
        merge(&mut self.actived_parent, &other.actived_parent);
        merge(&mut self.selected_parent, &other.selected_parent);
        merge(&mut self.dragged_parent, &other.dragged_parent);
        merge(&mut self.any_parent, &other.any_parent);
    }
}
