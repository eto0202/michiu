use lightningcss::{
    printer::PrinterOptions,
    properties::{Property, position::ZIndex, size::Size},
    rules::CssRule,
    selector::{Component, PseudoClass},
    stylesheet::{ParserOptions, StyleSheet},
    traits::ToCss,
    values::{
        color::FloatColor,
        length::{LengthPercentage, LengthPercentageOrAuto},
        percentage::DimensionPercentage,
    },
};
use michiu::ui::{self, ExternalData, ExternalDataSet, Modifier, ReadSignal};
use std::{collections::HashMap, path::Path};

#[derive(Debug, Default, Clone, derive_more::Deref, derive_more::DerefMut)]
pub struct CssMap(HashMap<String, Modifier>);

impl CssMap {
    /// Retrieves a style based on the class name. If the style does not exist, returns an empty style.
    #[inline]
    #[must_use]
    pub fn class(&self, name: &str) -> Modifier {
        self.0.get(name).cloned().unwrap_or_default()
    }
}

pub trait CssSignalExt {
    /// Retrieves the styles for a specified class based on the signal's current state.
    fn class(&self, name: &str) -> Modifier;
}

impl CssSignalExt for ReadSignal<CssMap> {
    #[inline]
    fn class(&self, name: &str) -> Modifier {
        self.get().class(name)
    }
}

pub type CssMapSet = ExternalDataSet<CssMap>;

pub trait CssMapSetExt {
    /// Retrieve the specified `CssMap`.
    fn sheet(&self, key: &str) -> CssMap;
}

impl CssMapSetExt for CssMapSet {
    /// Retrieve the specified `CssMap`.
    #[inline]
    fn sheet(&self, key: &str) -> CssMap {
        self.get(key).cloned().unwrap_or_default()
    }
}

pub trait CssSetSignalExt {
    /// Retrieve styles based on the sheet name and class name.
    fn class(&self, sheet_key: &str, class_name: &str) -> Modifier;

    /// Retrieve the specified `CssMap` from the key.
    fn sheet(&self, sheet_key: &str) -> CssMap;
}

impl CssSetSignalExt for ReadSignal<CssMapSet> {
    #[inline]
    fn class(&self, sheet_key: &str, class_name: &str) -> Modifier {
        self.get()
            .get(sheet_key)
            .map_or_default(|css_map| css_map.class(class_name))
    }

    #[inline]
    fn sheet(&self, sheet_key: &str) -> CssMap {
        self.get().sheet(sheet_key)
    }
}

/// A loader that parses CSS and maps it to a `HashMap<String, Modifier>`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CssLoader;

impl ExternalData for CssLoader {
    type Output = CssMap;

    /// Load the CSS file.
    #[inline]
    fn load(&self, _path: &Path, content: &str) -> ui::MichiuUiResult<Self::Output> {
        Ok(parse_css_to_classes(content))
    }
}

/// Scan the abstract syntax tree of lightningcss to generate a map of class names and `Modifier` values.
#[expect(clippy::print_stderr, clippy::expect_used)]
#[must_use]
pub fn parse_css_to_classes(css_content: &str) -> CssMap {
    let mut classes: HashMap<String, Modifier> = HashMap::new();

    let stylesheet = StyleSheet::parse(css_content, ParserOptions::default()).unwrap_or_else(|e| {
        // ホットリロード中はタイピング途中で文法が崩れることは普通なので、エラーは握りつぶしつつ警告ログのみ表示する。
        if cfg!(debug_assertions) {
            eprintln!("[WARN] Failed to parse CSS: {e}");
        }

        StyleSheet::parse("", ParserOptions::default())
            .expect("parsing an empty stylesheet should never fail")
    });

    for rule in &stylesheet.rules.0 {
        if let CssRule::Style(style_rule) = rule {
            for selector in &style_rule.selectors.0 {
                let mut class_name = None;
                let mut target = ui::ModTarget::Base;

                for component in selector.iter() {
                    match component {
                        Component::Class(ident) => {
                            class_name = Some(ident.0.to_string());
                        }
                        Component::NonTSPseudoClass(pseudo) => {
                            target = match pseudo {
                                PseudoClass::Hover => ui::ModTarget::Hovered,
                                PseudoClass::Focus => ui::ModTarget::Focused,
                                PseudoClass::Active => ui::ModTarget::Pressed,
                                PseudoClass::Disabled => ui::ModTarget::Disabled,
                                PseudoClass::Checked => ui::ModTarget::Selected,
                                PseudoClass::FocusVisible => ui::ModTarget::FocusedVisible,
                                PseudoClass::FocusWithin => ui::ModTarget::FocusedWithin,
                                _ => ui::ModTarget::Base,
                            };
                        }
                        _ => {}
                    }
                }

                if let Some(class) = class_name {
                    let mut style = classes.remove(&class).unwrap_or_default();
                    style = apply_declarations_to_style(
                        style,
                        &style_rule.declarations.declarations,
                        target,
                    );
                    classes.insert(class, style);
                }
            }
        }
    }

    CssMap(classes)
}

fn apply_declarations_to_style(
    mut style: Modifier,
    declarations: &[Property],
    target: ui::ModTarget,
) -> Modifier {
    for decl in declarations {
        match decl {
            Property::BackgroundColor(color) => {
                if let Some(c) = parse_css_color(color) {
                    style = map_style_prop(style, target, |s| s.bg_color(c));
                }
            }
            Property::Color(color) => {
                if let Some(c) = parse_css_color(color) {
                    style = map_style_prop(style, target, |s| s.text_color(c));
                }
            }
            Property::BorderColor(border) => {
                if let Some(top_color) = parse_css_color(&border.top) {
                    style = map_style_prop(style, target, |s| s.border_color(top_color));
                }
            }
            Property::Opacity(op) => {
                style = map_style_prop(style, target, |s| s.opacity(op.0));
            }
            Property::BorderRadius(radius, _) => {
                if let Some(tl) = parse_css_length_f32(&radius.top_left.0)
                    && let Some(tr) = parse_css_length_f32(&radius.top_right.0)
                    && let Some(br) = parse_css_length_f32(&radius.bottom_right.0)
                    && let Some(bl) = parse_css_length_f32(&radius.bottom_left.0)
                {
                    let r = ui::CornerRadius::radius(tl, tr, br, bl);
                    style = map_style_prop(style, target, |s| s.corner_radius(r));
                }
            }

            Property::Width(size) => {
                if let Some(v) = parse_css_size(size) {
                    style = map_style_prop(style, target, |s| s.width(v));
                }
            }
            Property::Height(size) => {
                if let Some(v) = parse_css_size(size) {
                    style = map_style_prop(style, target, |s| s.height(v));
                }
            }
            Property::MinWidth(size) => {
                if let Some(v) = parse_css_size(size) {
                    style = map_style_prop(style, target, |s| s.min_width(v));
                }
            }
            Property::MinHeight(size) => {
                if let Some(v) = parse_css_size(size) {
                    style = map_style_prop(style, target, |s| s.min_height(v));
                }
            }
            Property::MaxWidth(max_size) => {
                if let Some(v) = parse_css_max_size(max_size) {
                    style = map_style_prop(style, target, |s| s.max_width(v));
                }
            }
            Property::MaxHeight(max_size) => {
                if let Some(v) = parse_css_max_size(max_size) {
                    style = map_style_prop(style, target, |s| s.max_height(v));
                }
            }

            Property::Top(val) => {
                if let Some(v) = parse_length_percentage_or_auto(val) {
                    style = map_style_prop(style, target, |s| s.top(v));
                }
            }
            Property::Right(val) => {
                if let Some(v) = parse_length_percentage_or_auto(val) {
                    style = map_style_prop(style, target, |s| s.right(v));
                }
            }
            Property::Bottom(val) => {
                if let Some(v) = parse_length_percentage_or_auto(val) {
                    style = map_style_prop(style, target, |s| s.bottom(v));
                }
            }
            Property::Left(val) => {
                if let Some(v) = parse_length_percentage_or_auto(val) {
                    style = map_style_prop(style, target, |s| s.left(v));
                }
            }

            Property::Margin(margin) => {
                if let Some(t) = parse_length_percentage_or_auto(&margin.top)
                    && let Some(r) = parse_length_percentage_or_auto(&margin.right)
                    && let Some(b) = parse_length_percentage_or_auto(&margin.bottom)
                    && let Some(l) = parse_length_percentage_or_auto(&margin.left)
                {
                    let m_rect = ui::Rect::new(t, r, b, l);
                    style = map_style_prop(style, target, |s| s.margin(m_rect));
                }
            }
            Property::MarginTop(val) => {
                if let Some(v) = parse_length_percentage_or_auto(val) {
                    style = map_style_prop(style, target, |s| s.m_t(v));
                }
            }
            Property::MarginRight(val) => {
                if let Some(v) = parse_length_percentage_or_auto(val) {
                    style = map_style_prop(style, target, |s| s.m_r(v));
                }
            }
            Property::MarginBottom(val) => {
                if let Some(v) = parse_length_percentage_or_auto(val) {
                    style = map_style_prop(style, target, |s| s.m_b(v));
                }
            }
            Property::MarginLeft(val) => {
                if let Some(v) = parse_length_percentage_or_auto(val) {
                    style = map_style_prop(style, target, |s| s.m_l(v));
                }
            }
            Property::PaddingTop(lp) => {
                if let Some(v) = parse_length_percentage_or_auto_to_length(lp) {
                    style = map_style_prop(style, target, |s| s.p_t(v));
                }
            }
            Property::PaddingRight(lp) => {
                if let Some(v) = parse_length_percentage_or_auto_to_length(lp) {
                    style = map_style_prop(style, target, |s| s.p_r(v));
                }
            }
            Property::PaddingBottom(lp) => {
                if let Some(v) = parse_length_percentage_or_auto_to_length(lp) {
                    style = map_style_prop(style, target, |s| s.p_b(v));
                }
            }
            Property::PaddingLeft(lp) => {
                if let Some(v) = parse_length_percentage_or_auto_to_length(lp) {
                    style = map_style_prop(style, target, |s| s.p_l(v));
                }
            }
            Property::Padding(rect) => {
                if let (Some(t), Some(r), Some(b), Some(l)) = (
                    parse_length_percentage_or_auto_to_length(&rect.top),
                    parse_length_percentage_or_auto_to_length(&rect.right),
                    parse_length_percentage_or_auto_to_length(&rect.bottom),
                    parse_length_percentage_or_auto_to_length(&rect.left),
                ) {
                    style = map_style_prop(style, target, |s| s.padding((t, r, b, l)));
                }
            }
            Property::Gap(gap) => {
                if let (Some(row), Some(col)) = (
                    parse_css_gap_value(&gap.row),
                    parse_css_gap_value(&gap.column),
                ) {
                    style = map_style_prop(style, target, |s| s.gap_row(row).gap_col(col));
                }
            }

            Property::FlexGrow(grow, _) => {
                style = map_style_prop(style, target, |s| s.flex_grow(*grow));
            }
            Property::FlexShrink(shrink, _) => {
                style = map_style_prop(style, target, |s| s.flex_shrink(*shrink));
            }
            Property::FlexBasis(basis, _) => {
                let val = match basis {
                    LengthPercentageOrAuto::Auto => Some(ui::Val::Auto),
                    LengthPercentageOrAuto::LengthPercentage(lp) => match lp {
                        DimensionPercentage::Dimension(d) => {
                            Some(ui::Val::Px(d.to_px().unwrap_or(0.0)))
                        }
                        DimensionPercentage::Percentage(p) => Some(ui::Val::Percent(p.0 * 100.0)),
                        DimensionPercentage::Calc(_) => None,
                    },
                };
                if let Some(v) = val {
                    style = map_style_prop(style, target, |s| s.basis(v));
                }
            }
            Property::FlexWrap(fw, _) => {
                use lightningcss::properties::flex::FlexWrap as CssFw;
                style = match fw {
                    CssFw::Wrap => map_style_prop(style, target, Modifier::flex_wrap),
                    CssFw::WrapReverse => {
                        map_style_prop(style, target, Modifier::flex_wrap_reverse)
                    }
                    CssFw::NoWrap => map_style_prop(style, target, Modifier::flex_nowrap),
                };
            }

            Property::BorderWidth(width) => {
                let t = parse_border_side_width(&width.top);
                let r = parse_border_side_width(&width.right);
                let b = parse_border_side_width(&width.bottom);
                let l = parse_border_side_width(&width.left);

                let b_rect = ui::Rect::new(t, r, b, l);
                style = map_style_prop(style, target, |s| s.border_solid(b_rect));
            }
            Property::Border(border) => {
                let w = parse_border_side_width(&border.width);
                if let Some(c) = parse_css_color(&border.color) {
                    let b_style = parse_line_style(border.style);
                    let b_rect = ui::Rect::all(w);
                    style = map_style_prop(style, target, |s| {
                        s.border(b_style, b_rect).border_color(c)
                    });
                }
            }
            Property::BorderTop(border) => {
                let w = parse_border_side_width(&border.width);
                if let Some(c) = parse_css_color(&border.color) {
                    let b_style = parse_line_style(border.style);
                    style =
                        map_style_prop(style, target, |s| s.border_top(b_style, w).border_color(c));
                }
            }
            Property::BorderRight(border) => {
                let w = parse_border_side_width(&border.width);
                if let Some(c) = parse_css_color(&border.color) {
                    let b_style = parse_line_style(border.style);
                    style = map_style_prop(style, target, |s| {
                        s.border_right(b_style, w).border_color(c)
                    });
                }
            }
            Property::BorderBottom(border) => {
                let w = parse_border_side_width(&border.width);
                if let Some(c) = parse_css_color(&border.color) {
                    let b_style = parse_line_style(border.style);
                    style = map_style_prop(style, target, |s| {
                        s.border_bottom(b_style, w).border_color(c)
                    });
                }
            }
            Property::BorderLeft(border) => {
                let w = parse_border_side_width(&border.width);
                if let Some(c) = parse_css_color(&border.color) {
                    let b_style = parse_line_style(border.style);
                    style = map_style_prop(style, target, |s| {
                        s.border_left(b_style, w).border_color(c)
                    });
                }
            }
            Property::BorderTopLeftRadius(radius, _) => {
                if let Some(val) = parse_css_length_f32(&radius.0) {
                    style = map_style_prop(style, target, |s| s.r_top(val));
                }
            }
            Property::BorderTopRightRadius(radius, _) => {
                if let Some(val) = parse_css_length_f32(&radius.0) {
                    style = map_style_prop(style, target, |s| s.r_right(val));
                }
            }
            Property::BorderBottomRightRadius(radius, _) => {
                if let Some(val) = parse_css_length_f32(&radius.0) {
                    style = map_style_prop(style, target, |s| s.r_bottom(val));
                }
            }
            Property::BorderBottomLeftRadius(radius, _) => {
                if let Some(val) = parse_css_length_f32(&radius.0) {
                    style = map_style_prop(style, target, |s| s.r_left(val));
                }
            }

            Property::ZIndex(ZIndex::Integer(val)) => {
                style = map_style_prop(style, target, |s| s.z_index(*val));
            }

            Property::FontSize(fs) => {
                use lightningcss::properties::font::FontSize;
                let val = match fs {
                    FontSize::Length(len) => Some(parse_css_length_f32(len).unwrap_or(16.0)),
                    _ => None, // キーワード（medium, smallなど）は16.0相当で無視
                };
                if let Some(v) = val {
                    style = map_style_prop(style, target, |s| s.font_size(v));
                }
            }
            Property::FontFamily(ff_list) => {
                if let Some(first_family) = ff_list.first()
                    && let Ok(family_str) = first_family.to_css_string(PrinterOptions::default())
                {
                    let family_str = family_str.trim_matches('"').trim_matches('\'').to_string();
                    style = map_style_prop(style, target, |s| s.font_family(family_str));
                }
            }
            Property::FontWeight(fw) => {
                use lightningcss::properties::font::{AbsoluteFontWeight, FontWeight};
                let weight_val = match fw {
                    FontWeight::Absolute(val) => match val {
                        // 小数点以下を切り捨てした場合、`399.9` のような値が来た際に `399`になってしまう。本来なら `400` にしたいはず。
                        // また CSS の仕様範囲は 1〜1000 のため clamp で制限。
                        #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                        AbsoluteFontWeight::Weight(w) => {
                            Some((*w).clamp(1.0, 1000.0).round() as u16)
                        }
                        AbsoluteFontWeight::Normal => Some(400),
                        AbsoluteFontWeight::Bold => Some(700),
                    },
                    _ => None,
                };
                if let Some(w) = weight_val {
                    style = map_style_prop(style, target, |s| s.font_weight(w));
                }
            }
            Property::FontStyle(fs) => {
                use lightningcss::properties::font::FontStyle;
                let style_val = match fs {
                    FontStyle::Italic => Some(ui::FontStyle::Italic), // 2 = Italic
                    FontStyle::Oblique(_) => Some(ui::FontStyle::Oblique),
                    FontStyle::Normal => Some(ui::FontStyle::Normal), // 0 = Normal
                };
                if let Some(st) = style_val {
                    style = map_style_prop(style, target, |s| s.font_style(st));
                }
            }

            Property::BoxShadow(shadow_list, _) => {
                if let Some(shadow) = shadow_list.first() {
                    let offset_x = shadow.x_offset.to_px().unwrap_or(0.0);
                    let offset_y = shadow.y_offset.to_px().unwrap_or(0.0);
                    let blur = shadow.blur.to_px().unwrap_or(0.0);
                    let spread = shadow.spread.to_px().unwrap_or(0.0);

                    if let Some(c) = parse_css_color(&shadow.color) {
                        let b_shadow = ui::BoxShadow::new()
                            .offset((offset_x, offset_y))
                            .blur(blur)
                            .spread(spread)
                            .color(c);
                        style = map_style_prop(style, target, |s| s.box_shadow(b_shadow));
                    }
                }
            }

            Property::Transform(transform_list, _) => {
                use lightningcss::properties::transform::Transform;
                let mut dcomp_transform = ui::Transform::new();
                for t in &transform_list.0 {
                    match t {
                        Transform::Translate(tx, ty) => {
                            let x = parse_css_length_f32(tx).unwrap_or(0.0);
                            let y = parse_css_length_f32(ty).unwrap_or(0.0);
                            dcomp_transform = dcomp_transform.translate(x, y);
                        }
                        Transform::Scale(sx, sy) => {
                            let x = parse_number_or_percentage(sx);
                            let y = parse_number_or_percentage(sy);
                            dcomp_transform = dcomp_transform.scale(x, y);
                        }
                        Transform::Rotate(angle) => {
                            let rad = angle.to_radians();
                            dcomp_transform = dcomp_transform.rotate(rad);
                        }
                        _ => {}
                    }
                }
                style = map_style_prop(style, target, |s| s.transform(dcomp_transform));
            }

            Property::Transition(transitions, _) => {
                for t in transitions {
                    if let Some(property_list) = parse_property_id_to_list(&t.property) {
                        let duration = parse_css_time(&t.duration);
                        let curve = parse_css_easing(&t.timing_function);

                        let dcomp_transition = ui::Transition::new(property_list, duration, curve);

                        // 疑似状態（Hovered等）であっても、その状態がアクティブになった際のトランジションとして安全マウント
                        style = map_style_prop(style, target, |s| s.transition(dcomp_transition));
                    }
                }
            }
            Property::Filter(filters, _) => {
                use lightningcss::properties::effects::{Filter, FilterList};
                if let FilterList::Filters(list) = filters {
                    for f in list {
                        match f {
                            // filter: opacity() ➔ そのまま Modifier::opacity にバインド
                            Filter::Opacity(val) => {
                                let opacity_f32 = parse_number_or_percentage(val);
                                style = map_style_prop(style, target, |s| s.opacity(opacity_f32));
                            }
                            // filter: brightness(0.85) ➔ 擬似的に Modifier::opacity にフォールバック適用
                            Filter::Brightness(val) => {
                                let brightness_f32 = parse_number_or_percentage(val);
                                style =
                                    map_style_prop(style, target, |s| s.opacity(brightness_f32));
                            }
                            _ => {}
                        }
                    }
                }
            }

            Property::Display(display_value) => {
                use lightningcss::properties::display::{Display, DisplayInside, DisplayKeyword};
                let val = match display_value {
                    Display::Pair(pair) => match pair.inside {
                        DisplayInside::Flex(_) => ui::Display::Flex,
                        DisplayInside::Grid => ui::Display::Grid,
                        _ => ui::Display::Block,
                    },
                    Display::Keyword(keyword) => match keyword {
                        DisplayKeyword::None => ui::Display::None,
                        _ => ui::Display::Block,
                    },
                };
                style = map_style_prop(style, target, |s| s.display(val));
            }

            Property::Position(position_value) => {
                let val = match position_value {
                    lightningcss::properties::position::Position::Absolute => {
                        ui::Position::Absolute
                    }
                    lightningcss::properties::position::Position::Fixed => ui::Position::Fixed,
                    lightningcss::properties::position::Position::Static => ui::Position::Static,
                    lightningcss::properties::position::Position::Sticky(_) => ui::Position::Sticky,
                    lightningcss::properties::position::Position::Relative => {
                        ui::Position::Relative
                    }
                };
                style = map_style_prop(style, target, |s| s.position(val));
            }

            Property::FlexDirection(direction_value, _) => {
                use lightningcss::properties::flex::FlexDirection;
                let val = match direction_value {
                    FlexDirection::Column => ui::FlexDirection::Column,
                    FlexDirection::ColumnReverse => ui::FlexDirection::ColumnReverse,
                    FlexDirection::RowReverse => ui::FlexDirection::RowReverse,
                    FlexDirection::Row => ui::FlexDirection::Row,
                };
                style = map_style_prop(style, target, |s| s.flex_direction(val));
            }

            Property::Cursor(c) => {
                use lightningcss::properties::ui::CursorKeyword;
                let cursor = match c.keyword {
                    CursorKeyword::Pointer => ui::CursorIcon::Pointer(None),
                    CursorKeyword::Grab => ui::CursorIcon::Grab(None),
                    CursorKeyword::Grabbing => ui::CursorIcon::Grabbing(None),
                    CursorKeyword::NotAllowed => ui::CursorIcon::NotAllowed(None),
                    CursorKeyword::Text => ui::CursorIcon::Text(None),
                    _ => ui::CursorIcon::Default(None),
                };
                style = map_style_prop(style, target, |s| s.cursor(cursor));
            }

            Property::AlignItems(a, _) => {
                use lightningcss::properties::align::{AlignItems, OverflowPosition, SelfPosition};
                let align = match a {
                    AlignItems::BaselinePosition(_) => ui::AlignItems::Baseline,
                    AlignItems::SelfPosition { overflow, value } => match overflow {
                        Some(OverflowPosition::Safe) => match value {
                            SelfPosition::Center => ui::AlignItems::SafeCenter,
                            SelfPosition::Start => ui::AlignItems::Start,
                            SelfPosition::SelfStart => ui::AlignItems::SelfStart,
                            SelfPosition::End => ui::AlignItems::End,
                            SelfPosition::SelfEnd => ui::AlignItems::SelfEnd,
                            SelfPosition::FlexStart => ui::AlignItems::SafeFlexStart,
                            SelfPosition::FlexEnd => ui::AlignItems::SafeFlexEnd,
                        },
                        _ => match value {
                            SelfPosition::Center => ui::AlignItems::Center,
                            SelfPosition::Start => ui::AlignItems::Start,
                            SelfPosition::SelfStart => ui::AlignItems::SafeStart,
                            SelfPosition::End => ui::AlignItems::End,
                            SelfPosition::SelfEnd => ui::AlignItems::SelfEnd,
                            SelfPosition::FlexStart => ui::AlignItems::FlexStart,
                            SelfPosition::FlexEnd => ui::AlignItems::FlexEnd,
                        },
                    },
                    _ => ui::AlignItems::Stretch,
                };
                style = map_style_prop(style, target, |s| s.align_items(align));
            }

            Property::AlignSelf(a, _) => {
                use lightningcss::properties::align::{AlignSelf, OverflowPosition, SelfPosition};
                let align = match a {
                    AlignSelf::BaselinePosition(_) => Some(ui::AlignSelf::Baseline),
                    AlignSelf::SelfPosition { overflow, value } => match overflow {
                        Some(OverflowPosition::Safe) => match value {
                            SelfPosition::Center => Some(ui::AlignSelf::SafeCenter),
                            SelfPosition::Start => Some(ui::AlignSelf::SafeStart),
                            SelfPosition::SelfStart => Some(ui::AlignSelf::SelfStart),
                            SelfPosition::End => Some(ui::AlignSelf::SafeEnd),
                            SelfPosition::SelfEnd => Some(ui::AlignSelf::SelfEnd),
                            SelfPosition::FlexStart => Some(ui::AlignSelf::SafeFlexStart),
                            SelfPosition::FlexEnd => Some(ui::AlignSelf::SafeFlexEnd),
                        },
                        _ => match value {
                            SelfPosition::Center => Some(ui::AlignSelf::Center),
                            SelfPosition::Start => Some(ui::AlignSelf::Start),
                            SelfPosition::SelfStart => Some(ui::AlignSelf::SelfStart),
                            SelfPosition::End => Some(ui::AlignSelf::End),
                            SelfPosition::SelfEnd => Some(ui::AlignSelf::SelfEnd),
                            SelfPosition::FlexStart => Some(ui::AlignSelf::FlexStart),
                            SelfPosition::FlexEnd => Some(ui::AlignSelf::FlexEnd),
                        },
                    },
                    _ => Some(ui::AlignSelf::Stretch),
                };
                style = map_style_prop(style, target, |s| s.align_self(align));
            }

            Property::JustifyContent(j, _) => {
                use lightningcss::properties::align::{
                    ContentDistribution, ContentPosition, JustifyContent, OverflowPosition,
                };
                let jus = match j {
                    JustifyContent::ContentDistribution(c) => match c {
                        ContentDistribution::SpaceBetween => ui::JustifyContent::SpaceBetween,
                        ContentDistribution::SpaceAround => ui::JustifyContent::SpaceAround,
                        ContentDistribution::SpaceEvenly => ui::JustifyContent::SpaceEvenly,
                        ContentDistribution::Stretch => ui::JustifyContent::Stretch,
                    },
                    JustifyContent::ContentPosition { value, overflow } => match overflow {
                        Some(OverflowPosition::Safe) => match value {
                            ContentPosition::Center => ui::JustifyContent::SafeCenter,
                            ContentPosition::Start => ui::JustifyContent::SafeStart,
                            ContentPosition::End => ui::JustifyContent::SafeEnd,
                            ContentPosition::FlexStart => ui::JustifyContent::SafeFlexStart,
                            ContentPosition::FlexEnd => ui::JustifyContent::SafeFlexEnd,
                        },
                        _ => match value {
                            ContentPosition::Center => ui::JustifyContent::Center,
                            ContentPosition::Start => ui::JustifyContent::Start,
                            ContentPosition::End => ui::JustifyContent::End,
                            ContentPosition::FlexStart => ui::JustifyContent::FlexStart,
                            ContentPosition::FlexEnd => ui::JustifyContent::FlexEnd,
                        },
                    },
                    JustifyContent::Left { overflow } => match overflow {
                        Some(OverflowPosition::Safe) => ui::JustifyContent::SafeStart,
                        _ => ui::JustifyContent::Start,
                    },
                    JustifyContent::Right { overflow } => match overflow {
                        Some(OverflowPosition::Safe) => ui::JustifyContent::SafeEnd,
                        _ => ui::JustifyContent::End,
                    },
                    JustifyContent::Normal => ui::JustifyContent::Stretch,
                };
                style = map_style_prop(style, target, |s| s.justify_content(jus));
            }
            Property::AlignContent(a, _) => {
                use lightningcss::properties::align::{
                    AlignContent, ContentDistribution, ContentPosition, OverflowPosition,
                };
                let align = match a {
                    AlignContent::Normal | AlignContent::BaselinePosition(_) => {
                        ui::AlignContent::Stretch
                    }
                    AlignContent::ContentDistribution(c) => match c {
                        ContentDistribution::SpaceBetween => ui::AlignContent::SpaceBetween,
                        ContentDistribution::SpaceAround => ui::AlignContent::SpaceAround,
                        ContentDistribution::SpaceEvenly => ui::AlignContent::SpaceEvenly,
                        ContentDistribution::Stretch => ui::AlignContent::Stretch,
                    },
                    AlignContent::ContentPosition { overflow, value } => match overflow {
                        Some(OverflowPosition::Safe) => match value {
                            ContentPosition::Center => ui::AlignContent::SafeCenter,
                            ContentPosition::Start => ui::AlignContent::SafeStart,
                            ContentPosition::End => ui::AlignContent::SafeEnd,
                            ContentPosition::FlexStart => ui::AlignContent::SafeFlexStart,
                            ContentPosition::FlexEnd => ui::AlignContent::SafeFlexEnd,
                        },
                        _ => match value {
                            ContentPosition::Center => ui::AlignContent::Center,
                            ContentPosition::Start => ui::AlignContent::Start,
                            ContentPosition::End => ui::AlignContent::End,
                            ContentPosition::FlexStart => ui::AlignContent::FlexStart,
                            ContentPosition::FlexEnd => ui::AlignContent::FlexEnd,
                        },
                    },
                };
                style = map_style_prop(style, target, |s| s.align_content(align));
            }

            _ => {}
        }
    }
    style
}

/// `Size` を Val に展開 (Width / Height 等)
fn parse_css_size(size: &Size) -> Option<ui::Val> {
    match size {
        Size::Auto => Some(ui::Val::Auto),
        Size::LengthPercentage(lp) => match lp {
            LengthPercentage::Dimension(d) => Some(ui::Val::Px(d.to_px().unwrap_or(0.0))),
            LengthPercentage::Percentage(p) => Some(ui::Val::Percent(p.0 * 100.0)),
            LengthPercentage::Calc(_) => None,
        },
        Size::Stretch(_) => Some(ui::Val::Stretch),
        _ => None,
    }
}

/// `MaxSize` を Val に展開 (`MaxWidth` / `MaxHeight` 等)
fn parse_css_max_size(max_size: &lightningcss::properties::size::MaxSize) -> Option<ui::Val> {
    use lightningcss::properties::size::MaxSize;
    match max_size {
        MaxSize::None => Some(ui::Val::Auto),
        MaxSize::LengthPercentage(lp) => match lp {
            LengthPercentage::Dimension(d) => Some(ui::Val::Px(d.to_px().unwrap_or(0.0))),
            LengthPercentage::Percentage(p) => Some(ui::Val::Percent(p.0 * 100.0)),
            LengthPercentage::Calc(_) => None,
        },
        MaxSize::Stretch(_) => Some(ui::Val::Stretch),
        _ => None,
    }
}

/// `LengthPercentageOrAuto` を Val に展開 (Top / Margin 等)
fn parse_length_percentage_or_auto(val: &LengthPercentageOrAuto) -> Option<ui::Val> {
    match val {
        LengthPercentageOrAuto::Auto => Some(ui::Val::Auto),
        LengthPercentageOrAuto::LengthPercentage(lp) => match lp {
            LengthPercentage::Dimension(d) => Some(ui::Val::Px(d.to_px().unwrap_or(0.0))),
            LengthPercentage::Percentage(p) => Some(ui::Val::Percent(p.0 * 100.0)),
            LengthPercentage::Calc(_) => None,
        },
    }
}

fn parse_length_percentage_or_auto_to_length(val: &LengthPercentageOrAuto) -> Option<ui::Length> {
    use lightningcss::values::percentage::DimensionPercentage;
    match val {
        LengthPercentageOrAuto::Auto => None,
        LengthPercentageOrAuto::LengthPercentage(lp) => match lp {
            DimensionPercentage::Dimension(d) => Some(ui::Length::Px(d.to_px().unwrap_or(0.0))),
            DimensionPercentage::Percentage(p) => Some(ui::Length::Percent(p.0 * 100.0)),
            DimensionPercentage::Calc(_) => None,
        },
    }
}

/// `GapValue` から Val への変換 (`LengthPercentage` 解決へ修正)
fn parse_css_gap_value(val: &lightningcss::properties::align::GapValue) -> Option<ui::Val> {
    use lightningcss::properties::align::GapValue;
    match val {
        GapValue::Normal => Some(ui::Val::Auto),
        GapValue::LengthPercentage(lp) => match lp {
            LengthPercentage::Dimension(d) => Some(ui::Val::Px(d.to_px().unwrap_or(0.0))),
            LengthPercentage::Percentage(p) => Some(ui::Val::Percent(p.0 * 100.0)),
            LengthPercentage::Calc(_) => None,
        },
    }
}

/// `BorderSideWidth` から Length への変換
fn parse_border_side_width(val: &lightningcss::properties::border::BorderSideWidth) -> ui::Length {
    use lightningcss::properties::border::BorderSideWidth;
    match val {
        BorderSideWidth::Length(len) => ui::Length::Px(len.to_px().unwrap_or(0.0)),
        BorderSideWidth::Thin => ui::Length::Px(1.0),
        BorderSideWidth::Medium => ui::Length::Px(3.0),
        BorderSideWidth::Thick => ui::Length::Px(5.0),
    }
}

// モノモルファイズの削減
#[expect(clippy::expect_used)]
#[inline]
fn map_style_prop<F>(style: Modifier, target: ui::ModTarget, f: F) -> Modifier
where
    F: FnOnce(Modifier) -> Modifier,
{
    let mut f = Some(f);

    map_style_prop_impl(style, target, &mut |s| {
        f.take()
            .expect("Internal bug: map_style_prop_impl invoked a FnOnce closure multiple times")(
            s
        )
    })
}

// map_style_prop_impl は1度しかコンパイルされない
fn map_style_prop_impl(
    style: Modifier,
    target: ui::ModTarget,
    f: &mut dyn FnMut(Modifier) -> Modifier,
) -> Modifier {
    match target {
        ui::ModTarget::Base => f(style),
        ui::ModTarget::Hovered => style.hovered(f(Modifier::new())),
        ui::ModTarget::Focused => style.focused(f(Modifier::new())),
        ui::ModTarget::Pressed => style.pressed(f(Modifier::new())),
        ui::ModTarget::Disabled => style.disabled(f(Modifier::new())),
        ui::ModTarget::FocusedWithin => style.focused_within(f(Modifier::new())),
        _ => style,
    }
}

fn parse_css_color(color: &lightningcss::values::color::CssColor) -> Option<ui::Color> {
    use lightningcss::values::color::CssColor;
    match color {
        CssColor::CurrentColor => Some(ui::Color::WHITE),
        CssColor::Float(f) => match &**f {
            FloatColor::HSL(h) => Some(ui::Color::hsla(h.h, h.s * 100.0, h.l * 100.0, h.alpha)),
            _ => None,
        },
        CssColor::RGBA(rgba) => Some(ui::Color::rgba_f32(
            f32::from(rgba.red) / 255.0,
            f32::from(rgba.green) / 255.0,
            f32::from(rgba.blue) / 255.0,
            f32::from(rgba.alpha) / 255.0,
        )),
        _ => None,
    }
}

fn parse_css_length_f32(lp: &lightningcss::values::length::LengthPercentage) -> Option<f32> {
    use lightningcss::values::percentage::DimensionPercentage;
    match lp {
        DimensionPercentage::Dimension(d) => d.to_px(),
        DimensionPercentage::Percentage(p) => Some(p.0 * 100.0),
        DimensionPercentage::Calc(_) => None,
    }
}

fn parse_line_style(style: lightningcss::properties::border::LineStyle) -> ui::BorderStyle {
    use lightningcss::properties::border::LineStyle;
    match style {
        LineStyle::Dotted => ui::BorderStyle::Dotted,
        LineStyle::Dashed => ui::BorderStyle::Dashed,
        LineStyle::Double => ui::BorderStyle::Double,
        _ => ui::BorderStyle::Solid, // solid, inset, groove等も含めてSolidにフォールバック
    }
}

fn parse_number_or_percentage(val: &lightningcss::values::percentage::NumberOrPercentage) -> f32 {
    use lightningcss::values::percentage::NumberOrPercentage;
    match val {
        NumberOrPercentage::Number(n) => *n,
        NumberOrPercentage::Percentage(p) => p.0,
    }
}

/// lightningcss の `PropertyId` を `michiu_ui` の PropertyList（ビット対象）にマッピング
fn parse_property_id_to_list(
    prop_id: &lightningcss::properties::PropertyId,
) -> Option<ui::PropertyList> {
    use lightningcss::properties::PropertyId;
    match prop_id {
        PropertyId::BackgroundColor => Some(ui::PropertyList::BackgroundColor),
        PropertyId::BorderColor => Some(ui::PropertyList::BorderColor),
        PropertyId::Opacity | PropertyId::Filter(_) => Some(ui::PropertyList::Opacity),
        PropertyId::TransformBox => Some(ui::PropertyList::Transform),
        PropertyId::BorderRadius(_) => Some(ui::PropertyList::CornerRadius),
        PropertyId::Width => Some(ui::PropertyList::Width),
        PropertyId::Height => Some(ui::PropertyList::Height),
        PropertyId::BoxShadow(_) => Some(ui::PropertyList::BoxShadow),
        _ => None, // 未対応のプロパティはアニメーション対象外として無視
    }
}

/// CSS の Time（s / ms）を `std::time::Duration` に安全変換
fn parse_css_time(time: &lightningcss::values::time::Time) -> std::time::Duration {
    use lightningcss::values::time::Time;
    match time {
        Time::Seconds(s) => std::time::Duration::from_secs_f32(*s),
        Time::Milliseconds(ms) => std::time::Duration::from_secs_f32(*ms / 1000.0),
    }
}

/// CSS のタイミング関数（Easing）を `AnimationCurve` にマッピング
fn parse_css_easing(easing: &lightningcss::values::easing::EasingFunction) -> ui::AnimationCurve {
    use lightningcss::values::easing::EasingFunction;
    match easing {
        EasingFunction::Linear => ui::AnimationCurve::Linear,
        EasingFunction::EaseIn => ui::AnimationCurve::EaseInQuad,
        EasingFunction::EaseOut => ui::AnimationCurve::EaseOutQuad,
        // ease, ease-in-out, cubic-bezier 等は EaseInOut 基準にフォールバックマッピング
        _ => ui::AnimationCurve::EaseInOutQuad,
    }
}
