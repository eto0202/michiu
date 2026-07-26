#![allow(unused)]
use crate::{Context, ReadSignal, ThisStyle};
use lightningcss::{
    printer::PrinterOptions,
    properties::{Property, PropertyId, display::DisplayPair, position::ZIndex, size::Size},
    rules::CssRule,
    selector::{Component, PseudoClass},
    stylesheet::{ParserOptions, StyleSheet},
    traits::ToCss,
    values::{
        color::{FloatColor, PredefinedColor},
        length::{LengthPercentage, LengthPercentageOrAuto},
        percentage::DimensionPercentage,
    },
};
use notify::Watcher;
use std::collections::HashMap;
use std::path::PathBuf;

// TODO: outline のパース

/// 単一のCSSファイルからパースされたスタイルクラスの集合。
#[derive(Clone, Default, Debug)]
pub struct Dss {
    pub(crate) key: String,
    pub(crate) file_path: Option<PathBuf>,
    pub(crate) classes: HashMap<String, ThisStyle>,
    pub(crate) hot_reload_enabled: bool,
}

impl Dss {
    /// 新しい独立スタイルシートの起点を作成します。
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            file_path: None,
            classes: HashMap::new(),
            hot_reload_enabled: false,
        }
    }

    /// 参照元の CSS ファイルパスをアタッチ）。
    pub fn from_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    /// ホットリロードの有効化フラグを設定。
    pub fn hot_reload(mut self, enabled: bool) -> Self {
        self.hot_reload_enabled = enabled;
        self
    }

    /// シート内部の単一のクラス名からスタイルを安全に取得します。
    #[inline]
    pub fn class(&self, class_name: &str) -> ThisStyle {
        self.classes.get(class_name).cloned().unwrap_or_default()
    }
}

/// 複数の独立したスタイルシートを名前空間（キー）ごとに保持するコンテナ。
#[derive(Clone, Default, Debug)]
pub struct DssSet {
    pub(crate) sheets: HashMap<String, Dss>,
}

impl DssSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// 新規構築用のビルダーインスタンスを開始します。
    pub fn builder() -> DynamicStyleSheetSetBuilder {
        DynamicStyleSheetSetBuilder::new()
    }

    /// ユーザー露出用の直感的な 2引数ゲッター。
    /// 指定されたシート（名前空間）またはクラスが存在しない場合は、
    /// 安全にスタイル未設定（Default）を返してクラッシュを防ぎます。
    #[inline]
    pub fn class(&self, sheet_key: &str, class_name: &str) -> ThisStyle {
        self.sheets
            .get(sheet_key)
            .map(|sheet| sheet.class(class_name))
            .unwrap_or_default()
    }

    #[inline]
    pub(crate) fn sheet(&self, key: &str) -> &Dss {
        static EMPTY_SHEET: std::sync::OnceLock<Dss> = std::sync::OnceLock::new();
        self.sheets
            .get(key)
            .unwrap_or_else(|| EMPTY_SHEET.get_or_init(Dss::default))
    }
}

/// 複数シートの一括初期ロードおよび非同期ホットリロード監視をバインドするビルダー。
pub struct DynamicStyleSheetSetBuilder {
    sheets: Vec<Dss>,
}

impl DynamicStyleSheetSetBuilder {
    pub fn new() -> Self {
        Self { sheets: Vec::new() }
    }

    /// スタイルシートを追加します（ビルダー）。
    pub fn add_sheet(mut self, sheet: Dss) -> Self {
        self.sheets.push(sheet);
        self
    }

    /// 全シートのパースと監視を一括構築し、ReadSignal と監視用ガードオブジェクトを返します。
    pub fn build_and_watch(self, cx: &mut Context) -> (ReadSignal<DssSet>, StyleSheetWatchGuard) {
        let mut initial_set = DssSet::new();
        let mut watchers = Vec::new();

        // 1. 全追加シートの同期的な初期ロードとパース
        for mut sheet in self.sheets {
            if let Some(ref path) = sheet.file_path
                && path.exists()
            {
                let css_content = std::fs::read_to_string(path).unwrap_or_default();
                sheet.classes = parse_css_to_stylesheet(&css_content).classes;
            }
            initial_set.sheets.insert(sheet.key.clone(), sheet);
        }

        // 監視登録時にスレッドローカルを叩かないよう、
        // 同期的に構築したばかりの `initial_set` を退避したうえでシグナルを生成。
        let initial_set_for_watch = initial_set.clone();

        // メインスレッド上での更新シグナル（ダブルバッファ）を生成
        let (read_sig, write_sig) = cx.create_signal(initial_set);

        // 2. 監視フラグが有効なシートに対して、個別に notify 監視をキック
        for (key, sheet) in &initial_set_for_watch.sheets {
            if sheet.hot_reload_enabled
                && let Some(ref file_path) = sheet.file_path
            {
                // 物理ファイルが実際に存在する場合のみ、watch を開始します。
                // これにより、カレントディレクトリのズレ等でファイルが見つからなくても起動クラッシュするのを完全に防止します。
                if !file_path.exists() {
                    let cwd = std::env::current_dir().unwrap_or_default();
                    eprintln!(
                        "Warning [michiu_ui]: Hot-reload watch target does not exist.\n\
                                         Target path: {:?}\n\
                                         Current working directory (CWD): {:?}\n\
                                         Please adjust your relative path based on the CWD above.",
                        file_path, cwd
                    );
                    continue;
                }

                // スレッド安全に STA メインスレッドへメッセージを送るための TaskSender をクローン取得
                let task_sender = cx.task_sender();
                let key_clone = key.clone();
                let file_path_clone = file_path.clone(); // 目的ファイルの絶対パス
                let write_sig_clone = write_sig;
                let read_sig_clone = read_sig;
                let file_path_for_match = file_path_clone.clone();

                let watcher = notify::recommended_watcher(
                    move |res: Result<notify::Event, notify::Error>| {
                        if let Ok(event) = res {
                            // A. 届いたイベントの発生パス（複数可）の中に、監視対象の絶対パスが含まれているか検証
                            let has_target_file =
                                event.paths.iter().any(|p| p == &file_path_for_match);

                            // B. 対象ファイルがあり、かつ単なる読込（is_access）以外のすべての書き込み・リネームイベントを許容
                            if has_target_file && !event.kind.is_access() {
                                let path = file_path_clone.clone();
                                let task_sender_clone = task_sender.clone();
                                let key_name = key_clone.clone();
                                let write_sig_inner = write_sig_clone;
                                let read_sig_inner = read_sig_clone;

                                // パースをワーカースレッドへオフロード
                                std::thread::spawn(move || {
                                    if let Ok(css_content) = std::fs::read_to_string(&path) {
                                        let mut new_sheet = parse_css_to_stylesheet(&css_content);
                                        new_sheet.key = key_name.clone();
                                        new_sheet.file_path = Some(path);
                                        new_sheet.hot_reload_enabled = true;

                                        // メインスレッド（STA）上で置換
                                        let _ = task_sender_clone.send(move |cx| {
                                            let mut current_set = read_sig_inner.get();
                                            current_set.sheets.insert(key_name, new_sheet);
                                            write_sig_inner.set(current_set);
                                            cx.mark_layout_dirty(cx.find_root_entity().unwrap());
                                            cx.mark_render_dirty(cx.find_root_entity().unwrap());
                                        });
                                    }
                                });
                            }
                        }
                    },
                )
                .unwrap();

                let mut watcher = watcher;
                // ファイルが属している親ディレクトリを登録。
                let parent_dir = file_path.parent().expect("Failed to get parent directory");
                watcher
                    .watch(parent_dir, notify::RecursiveMode::NonRecursive)
                    .unwrap();
                watchers.push(watcher);
            }
        }

        (
            read_sig,
            StyleSheetWatchGuard {
                _watchers: watchers,
            },
        )
    }
}

impl Default for DynamicStyleSheetSetBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 監視ウォッチャーの生存を保証する RAII ガード。
/// 本オブジェクトがドロップされると、自動的に監視スレッドがアンロードされます。
pub struct StyleSheetWatchGuard {
    _watchers: Vec<notify::RecommendedWatcher>,
}

/// lightningcss の抽象構文木 (AST) を走査して ThisStyle に高精度にデコードします。
pub fn parse_css_to_stylesheet(css_content: &str) -> Dss {
    let mut classes: HashMap<String, ThisStyle> = HashMap::new();

    // lightningcss による字句解析・構文解析
    let stylesheet = StyleSheet::parse(css_content, ParserOptions::default())
        .unwrap_or_else(|_| StyleSheet::parse("", ParserOptions::default()).unwrap());

    for rule in &stylesheet.rules.0 {
        if let CssRule::Style(style_rule) = rule {
            for selector in &style_rule.selectors.0 {
                let mut class_name = None;
                let mut target = crate::StyleTarget::Base;

                // セレクタ内のコンポーネント（.class や疑似クラス :hover 等）を検出
                for component in selector.iter() {
                    match component {
                        Component::Class(ident) => {
                            class_name = Some(ident.0.to_string());
                        }
                        Component::NonTSPseudoClass(pseudo) => {
                            target = match pseudo {
                                PseudoClass::Hover => crate::StyleTarget::Hovered,
                                PseudoClass::Focus => crate::StyleTarget::Focused,
                                PseudoClass::Active => crate::StyleTarget::Pressed,
                                PseudoClass::Disabled => crate::StyleTarget::Disabled,
                                PseudoClass::Checked => crate::StyleTarget::Selected,
                                PseudoClass::FocusVisible => crate::StyleTarget::Focused,
                                PseudoClass::FocusWithin => crate::StyleTarget::FocusedWithin,
                                _ => crate::StyleTarget::Base,
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

    Dss {
        key: String::new(),
        file_path: None,
        classes,
        hot_reload_enabled: false,
    }
}

fn apply_declarations_to_style(
    mut style: ThisStyle,
    declarations: &[Property],
    target: crate::StyleTarget,
) -> ThisStyle {
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
                    let r = crate::CornerRadius::radius(tl, tr, br, bl);
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
                    let m_rect = crate::Rect::new(t, r, b, l);
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
                    LengthPercentageOrAuto::Auto => Some(crate::Val::Auto),
                    LengthPercentageOrAuto::LengthPercentage(lp) => match lp {
                        DimensionPercentage::Dimension(d) => {
                            Some(crate::Val::Px(d.to_px().unwrap_or(0.0)))
                        }
                        DimensionPercentage::Percentage(p) => {
                            Some(crate::Val::Percent(p.0 * 100.0))
                        }
                        _ => None,
                    },
                };
                if let Some(v) = val {
                    style = map_style_prop(style, target, |s| s.basis(v));
                }
            }
            Property::FlexWrap(fw, _) => {
                use lightningcss::properties::flex::FlexWrap as CssFw;
                style = match fw {
                    CssFw::Wrap => map_style_prop(style, target, |s| s.flex_wrap()),
                    CssFw::WrapReverse => map_style_prop(style, target, |s| s.flex_wrap_reverse()),
                    _ => map_style_prop(style, target, |s| s.flex_nowrap()),
                };
            }

            Property::BorderWidth(width) => {
                if let Some(t) = parse_border_side_width(&width.top)
                    && let Some(r) = parse_border_side_width(&width.right)
                    && let Some(b) = parse_border_side_width(&width.bottom)
                    && let Some(l) = parse_border_side_width(&width.left)
                {
                    let b_rect = crate::Rect::new(t, r, b, l);
                    style = map_style_prop(style, target, |s| s.border_solid(b_rect));
                }
            }
            Property::Border(border) => {
                if let Some(w) = parse_border_side_width(&border.width)
                    && let Some(c) = parse_css_color(&border.color)
                {
                    let b_style = parse_line_style(&border.style);
                    let b_rect = crate::Rect::all(w);
                    style = map_style_prop(style, target, |s| {
                        s.border(b_style, b_rect).border_color(c)
                    });
                }
            }
            Property::BorderTop(border) => {
                if let Some(w) = parse_border_side_width(&border.width)
                    && let Some(c) = parse_css_color(&border.color)
                {
                    let b_style = parse_line_style(&border.style);
                    style =
                        map_style_prop(style, target, |s| s.border_top(b_style, w).border_color(c));
                }
            }
            Property::BorderRight(border) => {
                if let Some(w) = parse_border_side_width(&border.width)
                    && let Some(c) = parse_css_color(&border.color)
                {
                    let b_style = parse_line_style(&border.style);
                    style = map_style_prop(style, target, |s| {
                        s.border_right(b_style, w).border_color(c)
                    });
                }
            }
            Property::BorderBottom(border) => {
                if let Some(w) = parse_border_side_width(&border.width)
                    && let Some(c) = parse_css_color(&border.color)
                {
                    let b_style = parse_line_style(&border.style);
                    style = map_style_prop(style, target, |s| {
                        s.border_bottom(b_style, w).border_color(c)
                    });
                }
            }
            Property::BorderLeft(border) => {
                if let Some(w) = parse_border_side_width(&border.width)
                    && let Some(c) = parse_css_color(&border.color)
                {
                    let b_style = parse_line_style(&border.style);
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
                        AbsoluteFontWeight::Weight(w) => Some(*w as u32),
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
                    FontStyle::Italic => Some(2), // 2 = Italic
                    _ => Some(0),                 // 0 = Normal
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
                        let b_shadow = crate::BoxShadow::new()
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
                let mut dcomp_transform = crate::Transform::new();
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

                        let dcomp_transition =
                            crate::Transition::new(property_list, duration, curve);

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
                            // filter: opacity() ➔ そのまま ThisStyle::opacity にバインド
                            Filter::Opacity(val) => {
                                let opacity_f32 = parse_number_or_percentage(val);
                                style = map_style_prop(style, target, |s| s.opacity(opacity_f32));
                            }
                            // filter: brightness(0.85) ➔ 擬似的に ThisStyle::opacity にフォールバック適用
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
                        DisplayInside::Flex(_) => crate::Display::Flex,
                        DisplayInside::Grid => crate::Display::Grid,
                        _ => crate::Display::Block,
                    },
                    Display::Keyword(keyword) => match keyword {
                        DisplayKeyword::None => crate::Display::None,
                        _ => crate::Display::Block,
                    },
                };
                style = map_style_prop(style, target, |s| s.display(val));
            }

            Property::Position(position_value) => {
                let val = match position_value {
                    lightningcss::properties::position::Position::Absolute => {
                        crate::Position::Absolute
                    }
                    _ => crate::Position::Relative,
                };
                style = map_style_prop(style, target, |s| s.position(val));
            }

            Property::FlexDirection(direction_value, _) => {
                use lightningcss::properties::flex::FlexDirection;
                let val = match direction_value {
                    FlexDirection::Column => crate::FlexDirection::Column,
                    FlexDirection::ColumnReverse => crate::FlexDirection::ColumnReverse,
                    FlexDirection::RowReverse => crate::FlexDirection::RowReverse,
                    FlexDirection::Row => crate::FlexDirection::Row,
                };
                style = map_style_prop(style, target, |s| s.flex_direction(val));
            }

            Property::Cursor(c) => {
                use lightningcss::properties::ui::CursorKeyword;
                let cursor = match c.keyword {
                    CursorKeyword::Pointer => crate::CursorIcon::Pointer(None),
                    CursorKeyword::Grab => crate::CursorIcon::Grab(None),
                    CursorKeyword::Grabbing => crate::CursorIcon::Grabbing(None),
                    CursorKeyword::NotAllowed => crate::CursorIcon::NotAllowed(None),
                    CursorKeyword::Text => crate::CursorIcon::Text(None),
                    _ => crate::CursorIcon::Default(None),
                };
                style = map_style_prop(style, target, |s| s.cursor(cursor));
            }

            Property::AlignItems(a, _) => {
                use lightningcss::properties::align::{
                    AlignItems, BaselinePosition, OverflowPosition, SelfPosition,
                };
                let align = match a {
                    AlignItems::BaselinePosition(base) => Some(crate::AlignItems::Baseline),
                    AlignItems::SelfPosition { overflow, value } => match overflow {
                        Some(OverflowPosition::Safe) => match value {
                            SelfPosition::Center => Some(crate::AlignItems::SafeCenter),
                            SelfPosition::Start | SelfPosition::SelfStart => {
                                Some(crate::AlignItems::SafeStart)
                            }
                            SelfPosition::End | SelfPosition::SelfEnd => {
                                Some(crate::AlignItems::SafeEnd)
                            }
                            SelfPosition::FlexStart => Some(crate::AlignItems::SafeFlexStart),
                            SelfPosition::FlexEnd => Some(crate::AlignItems::SafeFlexEnd),
                        },
                        _ => match value {
                            SelfPosition::Center => Some(crate::AlignItems::Center),
                            SelfPosition::Start | SelfPosition::SelfStart => {
                                Some(crate::AlignItems::Start)
                            }
                            SelfPosition::End | SelfPosition::SelfEnd => {
                                Some(crate::AlignItems::End)
                            }
                            SelfPosition::FlexStart => Some(crate::AlignItems::FlexStart),
                            SelfPosition::FlexEnd => Some(crate::AlignItems::FlexEnd),
                        },
                    },
                    _ => Some(crate::AlignItems::Stretch),
                };
                style = map_style_prop(style, target, |s| s.align_items(align));
            }

            Property::AlignSelf(a, _) => {
                use lightningcss::properties::align::{
                    AlignSelf, BaselinePosition, OverflowPosition, SelfPosition,
                };
                let align = match a {
                    AlignSelf::BaselinePosition(base) => Some(crate::AlignSelf::Baseline),
                    AlignSelf::SelfPosition { overflow, value } => match overflow {
                        Some(OverflowPosition::Safe) => match value {
                            SelfPosition::Center => Some(crate::AlignSelf::SafeCenter),
                            SelfPosition::Start | SelfPosition::SelfStart => {
                                Some(crate::AlignSelf::SafeStart)
                            }
                            SelfPosition::End | SelfPosition::SelfEnd => {
                                Some(crate::AlignSelf::SafeEnd)
                            }
                            SelfPosition::FlexStart => Some(crate::AlignSelf::SafeFlexStart),
                            SelfPosition::FlexEnd => Some(crate::AlignSelf::SafeFlexEnd),
                        },
                        _ => match value {
                            SelfPosition::Center => Some(crate::AlignSelf::Center),
                            SelfPosition::Start | SelfPosition::SelfStart => {
                                Some(crate::AlignSelf::Start)
                            }
                            SelfPosition::End | SelfPosition::SelfEnd => {
                                Some(crate::AlignSelf::End)
                            }
                            SelfPosition::FlexStart => Some(crate::AlignSelf::FlexStart),
                            SelfPosition::FlexEnd => Some(crate::AlignSelf::FlexEnd),
                        },
                    },
                    _ => Some(crate::AlignSelf::Stretch),
                };
                style = map_style_prop(style, target, |s| s.align_self(align));
            }

            Property::JustifyContent(j, _) => {
                use lightningcss::properties::align::{
                    ContentDistribution, ContentPosition, JustifyContent, OverflowPosition,
                };
                let jus = match j {
                    JustifyContent::ContentDistribution(c) => match c {
                        ContentDistribution::SpaceBetween => {
                            Some(crate::JustifyContent::SpaceBetween)
                        }
                        ContentDistribution::SpaceAround => {
                            Some(crate::JustifyContent::SpaceAround)
                        }
                        ContentDistribution::SpaceEvenly => {
                            Some(crate::JustifyContent::SpaceEvenly)
                        }
                        ContentDistribution::Stretch => Some(crate::JustifyContent::Stretch),
                    },
                    JustifyContent::ContentPosition { value, overflow } => match overflow {
                        Some(OverflowPosition::Safe) => match value {
                            ContentPosition::Center => Some(crate::JustifyContent::SafeCenter),
                            ContentPosition::Start => Some(crate::JustifyContent::SafeStart),
                            ContentPosition::End => Some(crate::JustifyContent::SafeEnd),
                            ContentPosition::FlexStart => {
                                Some(crate::JustifyContent::SafeFlexStart)
                            }
                            ContentPosition::FlexEnd => Some(crate::JustifyContent::SafeFlexEnd),
                        },
                        _ => match value {
                            ContentPosition::Center => Some(crate::JustifyContent::Center),
                            ContentPosition::Start => Some(crate::JustifyContent::Start),
                            ContentPosition::End => Some(crate::JustifyContent::End),
                            ContentPosition::FlexStart => Some(crate::JustifyContent::FlexStart),
                            ContentPosition::FlexEnd => Some(crate::JustifyContent::FlexEnd),
                        },
                    },
                    JustifyContent::Left { overflow } => match overflow {
                        Some(OverflowPosition::Safe) => Some(crate::JustifyContent::SafeStart),
                        _ => Some(crate::JustifyContent::Start),
                    },
                    JustifyContent::Right { overflow } => match overflow {
                        Some(OverflowPosition::Safe) => Some(crate::JustifyContent::SafeEnd),
                        _ => Some(crate::JustifyContent::End),
                    },
                    JustifyContent::Normal => Some(crate::JustifyContent::Stretch),
                };
                style = map_style_prop(style, target, |s| s.justify_content(jus));
            }
            Property::AlignContent(a, _) => {
                use lightningcss::properties::align::{
                    AlignContent, ContentDistribution, ContentPosition, OverflowPosition,
                };
                let align = match a {
                    AlignContent::Normal => Some(crate::AlignContent::Stretch),
                    AlignContent::BaselinePosition(base) => Some(crate::AlignContent::Stretch),
                    AlignContent::ContentDistribution(c) => match c {
                        ContentDistribution::SpaceBetween => {
                            Some(crate::AlignContent::SpaceBetween)
                        }
                        ContentDistribution::SpaceAround => Some(crate::AlignContent::SpaceAround),
                        ContentDistribution::SpaceEvenly => Some(crate::AlignContent::SpaceEvenly),
                        ContentDistribution::Stretch => Some(crate::AlignContent::Stretch),
                    },
                    AlignContent::ContentPosition { overflow, value } => match overflow {
                        Some(OverflowPosition::Safe) => match value {
                            ContentPosition::Center => Some(crate::AlignContent::SafeCenter),
                            ContentPosition::Start => Some(crate::AlignContent::SafeStart),
                            ContentPosition::End => Some(crate::AlignContent::SafeEnd),
                            ContentPosition::FlexStart => Some(crate::AlignContent::SafeFlexStart),
                            ContentPosition::FlexEnd => Some(crate::AlignContent::SafeFlexEnd),
                        },
                        _ => match value {
                            ContentPosition::Center => Some(crate::AlignContent::Center),
                            ContentPosition::Start => Some(crate::AlignContent::Start),
                            ContentPosition::End => Some(crate::AlignContent::End),
                            ContentPosition::FlexStart => Some(crate::AlignContent::FlexStart),
                            ContentPosition::FlexEnd => Some(crate::AlignContent::FlexEnd),
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
fn parse_css_size(size: &Size) -> Option<crate::Val> {
    match size {
        Size::Auto => Some(crate::Val::Auto),
        Size::LengthPercentage(lp) => match lp {
            LengthPercentage::Dimension(d) => Some(crate::Val::Px(d.to_px().unwrap_or(0.0))),
            LengthPercentage::Percentage(p) => Some(crate::Val::Percent(p.0 * 100.0)),
            _ => None,
        },
        _ => None,
    }
}

/// `MaxSize` を Val に展開 (MaxWidth / MaxHeight 等)
fn parse_css_max_size(max_size: &lightningcss::properties::size::MaxSize) -> Option<crate::Val> {
    use lightningcss::properties::size::MaxSize;
    match max_size {
        MaxSize::None => Some(crate::Val::Auto),
        MaxSize::LengthPercentage(lp) => match lp {
            LengthPercentage::Dimension(d) => Some(crate::Val::Px(d.to_px().unwrap_or(0.0))),
            LengthPercentage::Percentage(p) => Some(crate::Val::Percent(p.0 * 100.0)),
            _ => None,
        },
        _ => None,
    }
}

/// `LengthPercentageOrAuto` を Val に展開 (Top / Margin 等)
fn parse_length_percentage_or_auto(val: &LengthPercentageOrAuto) -> Option<crate::Val> {
    match val {
        LengthPercentageOrAuto::Auto => Some(crate::Val::Auto),
        LengthPercentageOrAuto::LengthPercentage(lp) => match lp {
            LengthPercentage::Dimension(d) => Some(crate::Val::Px(d.to_px().unwrap_or(0.0))),
            LengthPercentage::Percentage(p) => Some(crate::Val::Percent(p.0 * 100.0)),
            _ => None,
        },
    }
}

fn parse_length_percentage_or_auto_to_length(
    val: &LengthPercentageOrAuto,
) -> Option<crate::Length> {
    use lightningcss::values::percentage::DimensionPercentage;
    match val {
        LengthPercentageOrAuto::Auto => None,
        LengthPercentageOrAuto::LengthPercentage(lp) => match lp {
            DimensionPercentage::Dimension(d) => Some(crate::Length::Px(d.to_px().unwrap_or(0.0))),
            DimensionPercentage::Percentage(p) => Some(crate::Length::Percent(p.0 * 100.0)),
            _ => None,
        },
    }
}

/// `GapValue` から Val への変換 (LengthPercentage 解決へ修正)
fn parse_css_gap_value(val: &lightningcss::properties::align::GapValue) -> Option<crate::Val> {
    use lightningcss::properties::align::GapValue;
    match val {
        GapValue::Normal => Some(crate::Val::Auto),
        GapValue::LengthPercentage(lp) => match lp {
            LengthPercentage::Dimension(d) => Some(crate::Val::Px(d.to_px().unwrap_or(0.0))),
            LengthPercentage::Percentage(p) => Some(crate::Val::Percent(p.0 * 100.0)),
            _ => None,
        },
    }
}

/// `BorderSideWidth` から Length への変換
fn parse_border_side_width(
    val: &lightningcss::properties::border::BorderSideWidth,
) -> Option<crate::Length> {
    use lightningcss::properties::border::BorderSideWidth;
    match val {
        BorderSideWidth::Length(len) => Some(crate::Length::Px(len.to_px().unwrap_or(0.0))),
        BorderSideWidth::Thin => Some(crate::Length::Px(1.0)),
        BorderSideWidth::Medium => Some(crate::Length::Px(3.0)),
        BorderSideWidth::Thick => Some(crate::Length::Px(5.0)),
    }
}

// モノモルファイズの削減
#[inline]
fn map_style_prop<F>(style: ThisStyle, target: crate::StyleTarget, f: F) -> ThisStyle
where
    F: FnOnce(ThisStyle) -> ThisStyle,
{
    // Option で包むことで FnMut としてトレイトオブジェクト化
    let mut f = Some(f);

    map_style_prop_impl(style, target, &mut |s| {
        f.take().expect("f called more than once")(s)
    })
}

// map_style_prop_impl は1度しかコンパイルされない
fn map_style_prop_impl(
    style: ThisStyle,
    target: crate::StyleTarget,
    f: &mut dyn FnMut(ThisStyle) -> ThisStyle,
) -> ThisStyle {
    match target {
        crate::StyleTarget::Base => f(style),
        crate::StyleTarget::Hovered => style.hovered(f(ThisStyle::new())),
        crate::StyleTarget::Focused => style.focused(f(ThisStyle::new())),
        crate::StyleTarget::Pressed => style.pressed(f(ThisStyle::new())),
        crate::StyleTarget::Disabled => style.disabled(f(ThisStyle::new())),
        crate::StyleTarget::FocusedWithin => style.focus_within(f(ThisStyle::new())),
        _ => style,
    }
}

fn parse_css_color(color: &lightningcss::values::color::CssColor) -> Option<crate::Color> {
    use lightningcss::values::color::CssColor;
    match color {
        CssColor::CurrentColor => Some(crate::Color::WHITE),
        CssColor::Float(f) => match &**f {
            FloatColor::HSL(h) => Some(crate::Color::hsla(h.h, h.s * 100.0, h.l * 100.0, h.alpha)),
            _ => None,
        },
        CssColor::RGBA(rgba) => Some(crate::Color::rgba_f32(
            rgba.red as f32 / 255.0,
            rgba.green as f32 / 255.0,
            rgba.blue as f32 / 255.0,
            rgba.alpha as f32 / 255.0,
        )),
        _ => None,
    }
}

fn parse_css_length_f32(lp: &lightningcss::values::length::LengthPercentage) -> Option<f32> {
    use lightningcss::values::percentage::DimensionPercentage;
    match lp {
        DimensionPercentage::Dimension(d) => d.to_px(),
        DimensionPercentage::Percentage(p) => Some(p.0 * 100.0),
        _ => None,
    }
}

fn parse_line_style(style: &lightningcss::properties::border::LineStyle) -> crate::BorderStyle {
    use lightningcss::properties::border::LineStyle;
    match style {
        LineStyle::Dotted => crate::BorderStyle::Dotted,
        LineStyle::Dashed => crate::BorderStyle::Dashed,
        LineStyle::Double => crate::BorderStyle::Double,
        _ => crate::BorderStyle::Solid, // solid, inset, groove等も含めてSolidにフォールバック
    }
}

fn parse_number_or_percentage(val: &lightningcss::values::percentage::NumberOrPercentage) -> f32 {
    use lightningcss::values::percentage::NumberOrPercentage;
    match val {
        NumberOrPercentage::Number(n) => *n,
        NumberOrPercentage::Percentage(p) => p.0,
    }
}

/// lightningcss の PropertyId を michiu_ui の PropertyList（ビット対象）にマッピング
fn parse_property_id_to_list(
    prop_id: &lightningcss::properties::PropertyId,
) -> Option<crate::PropertyList> {
    use lightningcss::properties::PropertyId;
    match prop_id {
        PropertyId::BackgroundColor => Some(crate::PropertyList::BackgroundColor),
        PropertyId::BorderColor => Some(crate::PropertyList::BorderColor),
        PropertyId::Opacity => Some(crate::PropertyList::Opacity),
        PropertyId::TransformBox => Some(crate::PropertyList::Transform),
        PropertyId::BorderRadius(_) => Some(crate::PropertyList::CornerRadius),
        PropertyId::Width => Some(crate::PropertyList::Width),
        PropertyId::Height => Some(crate::PropertyList::Height),
        PropertyId::BoxShadow(_) => Some(crate::PropertyList::BoxShadow),
        PropertyId::Filter(_) => Some(crate::PropertyList::Opacity),
        _ => None, // 未対応のプロパティはアニメーション対象外として無視
    }
}

/// CSS の Time（s / ms）を std::time::Duration に安全変換
fn parse_css_time(time: &lightningcss::values::time::Time) -> std::time::Duration {
    use lightningcss::values::time::Time;
    match time {
        Time::Seconds(s) => std::time::Duration::from_secs_f32(*s),
        Time::Milliseconds(ms) => std::time::Duration::from_secs_f32(*ms / 1000.0),
    }
}

/// CSS のタイミング関数（Easing）を AnimationCurve にマッピング
fn parse_css_easing(
    easing: &lightningcss::values::easing::EasingFunction,
) -> crate::AnimationCurve {
    use lightningcss::values::easing::EasingFunction;
    match easing {
        EasingFunction::Linear => crate::AnimationCurve::Linear,
        EasingFunction::EaseIn => crate::AnimationCurve::EaseInQuad,
        EasingFunction::EaseOut => crate::AnimationCurve::EaseOutQuad,
        // ease, ease-in-out, cubic-bezier 等は EaseInOut 基準にフォールバックマッピング
        _ => crate::AnimationCurve::EaseInOutQuad,
    }
}
