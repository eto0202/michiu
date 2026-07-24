use std::{borrow::Cow, sync::Arc};

use crate::*;

/// 要素がほぼ必ず持つ、基本のレイアウト情報。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BasicLayout {
    pub display: Display,
    pub item_is_table: bool,
    pub item_is_replaced: bool,
    pub box_sizing: BoxSizing,
    pub direction: Direction,
    pub overflow: LayoutOverflow,
    pub position: Position,
    pub inset: Rect<Val>,
    pub size: Size<Val>,
    pub min_size: Size<Val>,
    pub max_size: Size<Val>,
    pub aspect_ratio: Option<f32>,
    pub margin: Rect<Val>,
    pub padding: Rect<Length>,
    pub border: Rect<Length>,
    pub resizable: [bool; 4],
}

impl Default for BasicLayout {
    fn default() -> Self {
        // Taffy のデフォルトの Style から基本設定値をコピーして初期化
        let default_style: taffy::Style = taffy::Style::default();
        Self {
            display: default_style.display.into(),
            item_is_table: default_style.item_is_table,
            item_is_replaced: default_style.item_is_replaced,
            box_sizing: default_style.box_sizing.into(),
            direction: default_style.direction.into(),
            overflow: default_style.overflow.into(),
            position: default_style.position.into(),
            inset: default_style.inset.into(),
            size: default_style.size.into(),
            min_size: default_style.min_size.into(),
            max_size: default_style.max_size.into(),
            aspect_ratio: default_style.aspect_ratio,
            margin: default_style.margin.into(),
            padding: default_style.padding.into(),
            border: default_style.border.into(),
            resizable: [false; 4],
        }
    }
}

impl BasicLayout {
    /// 指定されたプロパティマスクに基づいて、自身を別のレイアウトデータで上書きします。
    pub(crate) fn override_with(&mut self, other: &Self, mask: ComponentMask) {
        if mask.has(STYLE_DISPLAY) {
            self.display = other.display;
        }
        if mask.has(STYLE_ITEM_IS_TABLE) {
            self.item_is_table = other.item_is_table;
        }
        if mask.has(STYLE_ITEM_IS_REPLACED) {
            self.item_is_replaced = other.item_is_replaced;
        }
        if mask.has(STYLE_BOX_SIZING) {
            self.box_sizing = other.box_sizing;
        }
        if mask.has(STYLE_DIRECTION) {
            self.direction = other.direction;
        }
        if mask.has(STYLE_OVERFLOW) {
            self.overflow = other.overflow;
        }
        if mask.has(STYLE_POSITION) {
            self.position = other.position;
        }
        if mask.has(STYLE_INSET) {
            self.inset = other.inset;
        }
        if mask.has(STYLE_SIZE) {
            self.size = other.size;
        }
        if mask.has(STYLE_MIN_SIZE) {
            self.min_size = other.min_size;
        }
        if mask.has(STYLE_MAX_SIZE) {
            self.max_size = other.max_size;
        }
        if mask.has(STYLE_ASPECT_RATIO) {
            self.aspect_ratio = other.aspect_ratio;
        }
        if mask.has(STYLE_MARGIN) {
            self.margin = other.margin;
        }
        if mask.has(STYLE_PADDING) {
            self.padding = other.padding;
        }
        if mask.has(STYLE_BORDER) {
            self.border = other.border;
        }
        if mask.has(STYLE_RESIZABLE) {
            self.resizable = other.resizable;
        }
    }
}

// 2. FlexLayout (Flexboxレイアウト：13プロパティ) - ホットデータ
/// Flexboxコンテナ、またはその子要素に適用される情報。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlexLayout {
    pub align_items: Option<AlignItems>,
    pub align_self: Option<AlignSelf>,
    pub justify_items: Option<AlignItems>,
    pub justify_self: Option<AlignSelf>,
    pub align_content: Option<AlignContent>,
    pub justify_content: Option<JustifyContent>,
    pub gap: Size<Val>,
    pub text_align: TextAlign,
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub flex_basis: Val,
    pub flex_grow: f32,
    pub flex_shrink: f32,
}

impl FlexLayout {
    /// 指定されたプロパティマスクに基づいて、自身を別のFlexレイアウトデータで上書きします。
    pub(crate) fn override_with(&mut self, other: &Self, mask: ComponentMask) {
        if mask.has(STYLE_ALIGN_ITEMS) {
            self.align_items = other.align_items;
        }
        if mask.has(STYLE_ALIGN_SELF) {
            self.align_self = other.align_self;
        }
        if mask.has(STYLE_JUSTIFY_ITEMS) {
            self.justify_items = other.justify_items;
        }
        if mask.has(STYLE_JUSTIFY_SELF) {
            self.justify_self = other.justify_self;
        }
        if mask.has(STYLE_ALIGN_CONTENT) {
            self.align_content = other.align_content;
        }
        if mask.has(STYLE_JUSTIFY_CONTENT) {
            self.justify_content = other.justify_content;
        }
        if mask.has(STYLE_GAP) {
            self.gap = other.gap;
        }
        if mask.has(STYLE_TEXT_ALIGN) {
            self.text_align = other.text_align;
        }
        if mask.has(STYLE_FLEX_DIRECTION) {
            self.flex_direction = other.flex_direction;
        }
        if mask.has(STYLE_FLEX_WRAP) {
            self.flex_wrap = other.flex_wrap;
        }
        if mask.has(STYLE_FLEX_BASIS) {
            self.flex_basis = other.flex_basis;
        }
        if mask.has(STYLE_FLEX_GROW) {
            self.flex_grow = other.flex_grow;
        }
        if mask.has(STYLE_FLEX_SHRINK) {
            self.flex_shrink = other.flex_shrink;
        }
    }
}

impl Default for FlexLayout {
    fn default() -> Self {
        let default_style: taffy::Style = taffy::Style::default();
        Self {
            align_items: default_style.align_items.map(|a| a.into()),
            align_self: default_style.align_self.map(|a| a.into()),
            justify_items: default_style.justify_items.map(|a| a.into()),
            justify_self: default_style.justify_self.map(|a| a.into()),
            align_content: default_style.align_content.map(|a| a.into()),
            justify_content: default_style.justify_content.map(|a| a.into()),
            gap: default_style.gap.into(),
            text_align: default_style.text_align.into(),
            flex_direction: default_style.flex_direction.into(),
            flex_wrap: default_style.flex_wrap.into(),
            flex_basis: default_style.flex_basis.into(),
            flex_grow: default_style.flex_grow,
            flex_shrink: default_style.flex_shrink,
        }
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
    pub z_index: Option<i32>,
    pub cursor: Option<CursorIcon>,
    /// 各方向 [Ns, Ew, Nesw, Nwse] のカスタムカーソル指定
    pub resizable_cursor: Option<[Option<CursorIcon>; 4]>,
    pub backdrop: Backdrop,
    pub text_color: Option<Color>,
    pub font_size: Option<f32>,
    pub font_family: Option<Cow<'static, str>>,
    pub font_weight: Option<u32>,
    pub font_style: Option<u32>,
    pub bg_gradient: Option<LinearGradient>,
    pub transitions: Vec<Transition>,
    pub keyframe_animations: Vec<KeyframeAnimation>,
    pub pointer_events: Option<PointerEvents>,
    pub user_select: Option<UserSelect>,
    pub select_bg_color: Option<Color>,
    pub select_text_color: Option<Color>,
    pub focusable: Option<Focusable>,
    pub outline_width: Option<EdgeInsets>, // アウトラインはレイアウトに影響を与えないため
    pub outline_color: Option<Color>,
    pub outline_lengths: Option<EdgeInsets>,
    pub outline_styles: Option<[BorderStyle; 4]>,
    pub outline_alignments: Option<[BorderAlignment; 4]>,
    pub outline_offset: Option<f32>,
}

impl VisualProperty {
    pub(crate) fn override_with(&mut self, other: &Self, mask: ComponentMask) {
        if mask.has(STYLE_BG_COLOR) {
            self.bg_color = other.bg_color;
            self.bg_gradient = other.bg_gradient;
        }
        if mask.has(STYLE_BORDER_COLOR) {
            self.border_color = other.border_color;
        }
        if mask.has(STYLE_BORDER) {
            self.border_lengths = other.border_lengths;
            self.border_styles = other.border_styles;
            self.border_alignments = other.border_alignments;
        }
        if mask.has(STYLE_CORNER_RADIUS) {
            self.corner_radius = other.corner_radius;
        }
        if mask.has(STYLE_OPACITY) {
            self.opacity = other.opacity;
        }
        if mask.has(STYLE_BOX_SHADOW) {
            self.shadow_params = other.shadow_params;
            self.shadow_color = other.shadow_color;
        }
        if mask.has(STYLE_TRANSFORM) {
            self.transform = other.transform;
            self.transform_origin = other.transform_origin;
        }
        if mask.has(STYLE_Z_INDEX) {
            self.z_index = other.z_index;
        }
        if mask.has(STYLE_CURSOR) {
            self.cursor = other.cursor;
        }
        if mask.has(STYLE_RESIZABLE) {
            self.resizable_cursor = other.resizable_cursor;
        }
        if mask.has(STYLE_BACKDROP) {
            self.backdrop = other.backdrop;
        }
        if mask.has(STYLE_TEXT_COLOR) {
            self.text_color = other.text_color;
        }
        if mask.has(STYLE_FONT_SIZE) {
            self.font_size = other.font_size;
        }
        if mask.has(STYLE_EXT_PROPERTIES) {
            if other.font_family.is_some() {
                self.font_family = other.font_family.clone();
            }
            if other.font_weight.is_some() {
                self.font_weight = other.font_weight;
            }
            if other.font_style.is_some() {
                self.font_style = other.font_style;
            }
        }
        if mask.has(STYLE_POINTER_EVENTS) {
            self.pointer_events = other.pointer_events;
        }
        if mask.has(STYLE_USER_SELECT) {
            self.user_select = other.user_select;
            self.select_bg_color = other.select_bg_color;
            self.select_text_color = other.select_text_color;
        }
        if mask.has(STYLE_FOCUSABLE) {
            self.focusable = other.focusable;
        }
        if mask.has(STYLE_OUTLINE) {
            self.outline_width = other.outline_width;
            self.outline_color = other.outline_color;
            self.outline_lengths = other.outline_lengths;
            self.outline_styles = other.outline_styles;
            self.outline_alignments = other.outline_alignments;
            self.outline_offset = other.outline_offset;
        }

        // 複数追加できるものは破棄せず結合
        if mask.has(STYLE_TRANSITIONS) {
            self.transitions.extend(other.transitions.clone());
        }
        if mask.has(STYLE_ANIMATIONS) {
            self.keyframe_animations
                .extend(other.keyframe_animations.clone());
        }
    }
}

/// インタラクション（動的状態）ごとにオーバーライドして適用される、追加のスタイル表現。
/// 滅多に使われない、かつ再帰的な構造を持つため、StyleInner の直下ではなくこの構造体に隠蔽して管理。
#[derive(Debug, Clone, Default)]
pub(crate) struct InteractionStyles {
    pub(crate) hovered: Option<ThisStyle>,
    pub(crate) focused: Option<ThisStyle>,
    pub(crate) pressed: Option<ThisStyle>,
    pub(crate) disabled: Option<ThisStyle>,
    pub(crate) actived: Option<ThisStyle>,
    pub(crate) selected: Option<ThisStyle>,
    pub(crate) dragged: Option<ThisStyle>,

    pub(crate) dragging: Option<ThisStyle>,
    pub(crate) drag_in: Option<ThisStyle>,
    pub(crate) drag_over: Option<ThisStyle>,

    pub(crate) hovered_within: Option<ThisStyle>,
    pub(crate) focused_within: Option<ThisStyle>,
    pub(crate) pressed_within: Option<ThisStyle>,
    pub(crate) disabled_within: Option<ThisStyle>,
    pub(crate) actived_within: Option<ThisStyle>,
    pub(crate) selected_within: Option<ThisStyle>,
    pub(crate) dragged_within: Option<ThisStyle>,
    pub(crate) any_within: Option<ThisStyle>, // All（いずれかのインタラクションがあればON）
}

impl InteractionStyles {
    /// 与えられた疑似状態（StyleTarget）に対応する Option<ThisStyle> フィールドの実体可変参照を取得します
    #[inline]
    pub(crate) fn get_style_target_mut(&mut self, target: StyleTarget) -> &mut ThisStyle {
        match target {
            StyleTarget::Hovered => self.hovered.get_or_insert_with(ThisStyle::new),
            StyleTarget::Focused => self.focused.get_or_insert_with(ThisStyle::new),
            StyleTarget::Pressed => self.pressed.get_or_insert_with(ThisStyle::new),
            StyleTarget::Disabled => self.disabled.get_or_insert_with(ThisStyle::new),
            StyleTarget::Actived => self.actived.get_or_insert_with(ThisStyle::new),
            StyleTarget::Selected => self.selected.get_or_insert_with(ThisStyle::new),
            StyleTarget::Dragged => self.dragged.get_or_insert_with(ThisStyle::new),
            StyleTarget::Dragging => self.dragging.get_or_insert_with(ThisStyle::new),
            StyleTarget::DragIn => self.drag_in.get_or_insert_with(ThisStyle::new),
            StyleTarget::DragOver => self.drag_over.get_or_insert_with(ThisStyle::new),

            StyleTarget::HoveredWithin => self.hovered_within.get_or_insert_with(ThisStyle::new),
            StyleTarget::FocusedWithin => self.focused_within.get_or_insert_with(ThisStyle::new),
            StyleTarget::PressedWithin => self.pressed_within.get_or_insert_with(ThisStyle::new),
            StyleTarget::DisabledWithin => self.disabled_within.get_or_insert_with(ThisStyle::new),
            StyleTarget::ActivedWithin => self.actived_within.get_or_insert_with(ThisStyle::new),
            StyleTarget::SelectedWithin => self.selected_within.get_or_insert_with(ThisStyle::new),
            StyleTarget::DraggedWithin => self.dragged_within.get_or_insert_with(ThisStyle::new),
            StyleTarget::AnyWithin => self.any_within.get_or_insert_with(ThisStyle::new),
            StyleTarget::Base => unreachable!("Base target must be handled individually"),
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
        merge(&mut self.pressed_within, &other.pressed_within);
        merge(&mut self.disabled_within, &other.disabled_within);
        merge(&mut self.actived_within, &other.actived_within);
        merge(&mut self.selected_within, &other.selected_within);
        merge(&mut self.dragged_within, &other.dragged_within);
        merge(&mut self.any_within, &other.any_within);
    }
}

/// 実行時にウィンドウ内で現在アクティブ（排他的）になっている、各状態の対象要素（EntityId）を管理します。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct InteractionStates {
    pub hovered: Option<EntityId>,
    pub focused: Option<EntityId>,
    pub pressed: Option<EntityId>,
    pub dragged: Option<EntityId>,
}

impl InteractionStates {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn clear_entity(&mut self, id: EntityId) {
        if self.hovered == Some(id) {
            self.hovered = None;
        }
        if self.focused == Some(id) {
            self.focused = None;
        }
        if self.pressed == Some(id) {
            self.pressed = None;
        }
        if self.dragged == Some(id) {
            self.dragged = None;
        }
    }
}
