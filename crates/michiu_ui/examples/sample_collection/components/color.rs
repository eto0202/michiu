use crate::app::theme::Theme;
use michiu_ui::prelude::*;
use std::{
    cell::{Cell, RefCell},
    ops::{Deref, DerefMut},
    rc::Rc,
};

pub fn container() -> Entity {
    let store = ColorStore::new(Color::BLACK);

    v_flex(ii().gap(16.0).p(16.0))
        .provide(store.color.0)
        .children({
            [
                top_container(&store),
                section(rgb_item(&store), rgba_item(&store)),
                section(hex_item(&store), hex_with_alpha_item(&store)),
                section(hsl_item(&store), hsla_item(&store)),
                section(oklab_item(&store), oklaba_item(&store)),
                section(oklch_item(&store), oklcha_item(&store)),
            ]
        })
}

fn top_container(store: &ColorStore) -> Entity {
    let item = |color: Color| {
        flex(
            ii().size(35.0)
                .r(3.0)
                .bg_color(color)
                .cursor_pointer()
                .hovered(
                    ii().outline_solid(2.0)
                        .outline_color(dynamic(|t: &Theme| t.text)),
                ),
        )
        .on_click_with({
            let store = store.clone();
            move |_| {
                store.set_color(color);
            }
        })
    };

    let row_1 = || {
        h_flex(ii().gap(10.0)).children([
            item(Color::GRAY),
            item(Color::LIGHT_GRAY),
            item(Color::WHITE),
            item(Color::YELLOW),
            item(Color::ORANGE),
            item(Color::RED),
            item(Color::MAGENTA),
            item(Color::PURPLE),
            item(Color::BLUE),
            item(Color::CYAN),
            item(Color::GREEN),
        ])
    };
    let row_2 = || {
        h_flex(ii().gap(10.0)).children([
            item(rgba(0, 0, 0, 0.0)),
            item(rgba(0, 0, 0, 0.1)),
            item(rgba(0, 0, 0, 0.2)),
            item(rgba(0, 0, 0, 0.3)),
            item(rgba(0, 0, 0, 0.4)),
            item(rgba(0, 0, 0, 0.5)),
            item(rgba(0, 0, 0, 0.6)),
            item(rgba(0, 0, 0, 0.7)),
            item(rgba(0, 0, 0, 0.8)),
            item(rgba(0, 0, 0, 0.9)),
            item(rgba(0, 0, 0, 1.0)),
        ])
    };

    let title = text("Select Default Color")
        .modify(ii().text_color(dynamic(|t: &Theme| t.text)).font_size(17.0));

    v_flex(ii().gap(10.0)).children([title, row_1(), row_2()])
}

fn section(left: Entity, right: Entity) -> Entity {
    h_flex(ii().gap(50.0)).children([left, right])
}

fn base_item<F, I>(t: &'static str, color_f: F, entities: I) -> Entity
where
    F: Fn(&CurrentColor) -> Color + Send + Sync + 'static,
    I: IntoIterator<Item = Entity>,
{
    let title = text(t).modify(ii().text_color(dynamic(|t: &Theme| t.text)));

    v_flex(ii().gap(8.0)).children([
        title,
        h_flex(ii().p(4.0).gap(8.0)).children([
            flex(
                ii().r(3.0)
                    .size(60.0)
                    .bg_color(dynamic(move |c: &CurrentColor| color_f(c))),
            ),
            h_flex(ii().gap(6.0).justify_start().items_start()).children(entities),
        ]),
    ])
}

fn common_text(t: &'static str) -> Entity {
    text(t).modify(ii().p_t(10.0).text_color(dynamic(|t: &Theme| t.text)))
}

fn rgb_item(store: &ColorStore) -> Entity {
    base_item(
        "RGB",
        |c| {
            let (r, g, b, _) = c.to_rgba8();
            rgb(r, g, b)
        },
        [
            common_text("R"),
            synced_num_input(
                store,
                |c| c.to_rgba8().0,
                |c, r| {
                    let (_, g, b, a) = c.to_rgba8();
                    rgba(r, g, b, a)
                },
                60.0,
                0u8,
                255u8,
            ),
            common_text("G"),
            synced_num_input(
                store,
                |c| c.to_rgba8().1,
                |c, g| {
                    let (r, _, b, a) = c.to_rgba8();
                    rgba(r, g, b, a)
                },
                60.0,
                0u8,
                255u8,
            ),
            common_text("B"),
            synced_num_input(
                store,
                |c| c.to_rgba8().2,
                |c, b| {
                    let (r, g, _, a) = c.to_rgba8();
                    rgba(r, g, b, a)
                },
                60.0,
                0u8,
                255u8,
            ),
        ],
    )
}

fn rgba_item(store: &ColorStore) -> Entity {
    base_item(
        "RGBA",
        |c| {
            let (r, g, b, a) = c.to_rgba8();
            rgba(r, g, b, a)
        },
        [
            common_text("R"),
            synced_num_input(
                store,
                |c| c.to_rgba8().0,
                |c, r| {
                    let (_, g, b, a) = c.to_rgba8();
                    rgba(r, g, b, a)
                },
                60.0,
                0u8,
                255u8,
            ),
            common_text("G"),
            synced_num_input(
                store,
                |c| c.to_rgba8().1,
                |c, g| {
                    let (r, _, b, a) = c.to_rgba8();
                    rgba(r, g, b, a)
                },
                60.0,
                0u8,
                255u8,
            ),
            common_text("B"),
            synced_num_input(
                store,
                |c| c.to_rgba8().2,
                |c, b| {
                    let (r, g, _, a) = c.to_rgba8();
                    rgba(r, g, b, a)
                },
                60.0,
                0u8,
                255u8,
            ),
            common_text("A"),
            synced_num_input(
                store,
                |c| c.to_rgba8().3,
                |c, a| {
                    let (r, g, b, _) = c.to_rgba8();
                    rgba(r, g, b, a)
                },
                60.0,
                0.0f32,
                1.0f32,
            ),
        ],
    )
}

fn hex_item(store: &ColorStore) -> Entity {
    base_item(
        "HEX",
        |c| hex(c.to_hex()),
        [
            common_text("0x"),
            synced_hex_input(
                store,
                |c| {
                    // to_hex() の戻り値に合わせてフォーマット
                    format!("{:06X}", c.to_hex())
                },
                |c, hex_str| {
                    // 16進数文字列を u32 にパースして Color を生成
                    u32::from_str_radix(hex_str, 16)
                        .ok()
                        .map(|val| hex(val).with_alpha(c.a)) // 元のアルファ値を維持
                },
                6,
            ),
        ],
    )
    .modify(ii().p_r(115.0))
}

fn hex_with_alpha_item(store: &ColorStore) -> Entity {
    base_item(
        "HEX with alpha",
        |c| hex(c.to_hex()).with_alpha(c.a),
        [
            common_text("0x"),
            synced_hex_input(
                store,
                |c| {
                    // RGB 6桁 + Alpha 2桁 (00〜FF)
                    let (r, g, b, _) = c.to_rgba8();
                    let alpha_u8 = (c.a * 255.0).round() as u8;
                    format!("{:02X}{:02X}{:02X}{:02X}", r, g, b, alpha_u8)
                },
                |_c, hex_str| {
                    // 8桁 (RRGGBBAA) をパース
                    let r = u8::from_str_radix(&hex_str[0..2], 16).ok()?;
                    let g = u8::from_str_radix(&hex_str[2..4], 16).ok()?;
                    let b = u8::from_str_radix(&hex_str[4..6], 16).ok()?;
                    let a_u8 = u8::from_str_radix(&hex_str[6..8], 16).ok()?;

                    let a = a_u8 as f32 / 255.0;
                    Some(rgba(r, g, b, a))
                },
                8,
            ),
        ],
    )
}

fn hsl_item(store: &ColorStore) -> Entity {
    base_item(
        "HSL",
        |c| {
            let (h, s, l) = c.to_hsl();
            hsl(h, s, l)
        },
        [
            common_text("H"),
            synced_num_input(
                store,
                |c| c.to_hsl().0,
                |c, h| {
                    let (_, s, l) = c.to_hsl();
                    hsl(h, s, l)
                },
                60.0,
                0.0f32,
                360.0f32,
            ),
            common_text("S"),
            synced_num_input(
                store,
                |c| c.to_hsl().1,
                |c, s| {
                    let (h, _, l) = c.to_hsl();
                    hsl(h, s, l)
                },
                60.0,
                0.0f32,
                100.0f32,
            ),
            common_text("L"),
            synced_num_input(
                store,
                |c| c.to_hsl().2,
                |c, l| {
                    let (h, s, _) = c.to_hsl();
                    hsl(h, s, l)
                },
                60.0,
                0.0f32,
                100.0f32,
            ),
        ],
    )
}

fn hsla_item(store: &ColorStore) -> Entity {
    base_item(
        "HSL",
        |c| {
            let (h, s, l, a) = c.to_hsla();
            hsla(h, s, l, a)
        },
        [
            common_text("H"),
            synced_num_input(
                store,
                |c| c.to_hsla().0,
                |c, h| {
                    let (_, s, l, a) = c.to_hsla();
                    hsla(h, s, l, a)
                },
                60.0,
                0.0f32,
                360.0f32,
            ),
            common_text("S"),
            synced_num_input(
                store,
                |c| c.to_hsla().1,
                |c, s| {
                    let (h, _, l, a) = c.to_hsla();
                    hsla(h, s, l, a)
                },
                60.0,
                0.0f32,
                100.0f32,
            ),
            common_text("L"),
            synced_num_input(
                store,
                |c| c.to_hsla().2,
                |c, l| {
                    let (h, s, _, a) = c.to_hsla();
                    hsla(h, s, l, a)
                },
                60.0,
                0.0f32,
                100.0f32,
            ),
            common_text("A"),
            synced_num_input(
                store,
                |c| c.to_hsla().3,
                |c, a| {
                    let (h, s, l, _) = c.to_hsla();
                    hsla(h, s, l, a)
                },
                60.0,
                0.0f32,
                1.0f32,
            ),
        ],
    )
}

fn oklab_item(store: &ColorStore) -> Entity {
    base_item(
        "OKLAB",
        |c| {
            let (l, a, b) = c.to_oklab();
            oklab(l, a, b)
        },
        [
            common_text("L"),
            synced_num_input(
                store,
                |c| c.to_oklab().0,
                |c, l| {
                    let (_, a, b) = c.to_oklab();
                    oklab(l, a, b)
                },
                60.0,
                0.0f32,
                1.0f32,
            ),
            common_text("A"),
            synced_num_input(
                store,
                |c| c.to_oklab().1,
                |c, a| {
                    let (l, _, b) = c.to_oklab();
                    oklab(l, a, b)
                },
                60.0,
                -0.4f32,
                0.4f32,
            ),
            common_text("B"),
            synced_num_input(
                store,
                |c| c.to_oklab().2,
                |c, b| {
                    let (l, a, _) = c.to_oklab();
                    oklab(l, a, b)
                },
                60.0,
                -0.4f32,
                0.4f32,
            ),
        ],
    )
}

fn oklaba_item(store: &ColorStore) -> Entity {
    base_item(
        "OKLABA",
        |c| {
            let (l, a, b, alpha) = c.to_oklaba();
            oklaba(l, a, b, alpha)
        },
        [
            common_text("L"),
            synced_num_input(
                store,
                |c| c.to_oklaba().0,
                |c, l| {
                    let (_, a, b, alpha) = c.to_oklaba();
                    oklaba(l, a, b, alpha)
                },
                60.0,
                0.0f32,
                1.0f32,
            ),
            common_text("A"),
            synced_num_input(
                store,
                |c| c.to_oklaba().1,
                |c, a| {
                    let (l, _, b, alpha) = c.to_oklaba();
                    oklaba(l, a, b, alpha)
                },
                60.0,
                -0.4f32,
                0.4f32,
            ),
            common_text("B"),
            synced_num_input(
                store,
                |c| c.to_oklaba().2,
                |c, b| {
                    let (l, a, _, alpha) = c.to_oklaba();
                    oklaba(l, a, b, alpha)
                },
                60.0,
                -0.4f32,
                0.4f32,
            ),
            common_text("A"),
            synced_num_input(
                store,
                |c| c.to_oklaba().3,
                |c, alpha| {
                    let (l, a, b, _) = c.to_oklaba();
                    oklaba(l, a, b, alpha)
                },
                60.0,
                0.0f32,
                1.0f32,
            ),
        ],
    )
}

fn oklch_item(store: &ColorStore) -> Entity {
    base_item(
        "OKLCH",
        |c| {
            let (l, c, h) = c.to_oklch();
            oklch(l, c, h)
        },
        [
            common_text("L"),
            synced_num_input(
                store,
                |c| c.to_oklch().0,
                |c, l| {
                    let (_, c, h) = c.to_oklch();
                    oklch(l, c, h)
                },
                60.0,
                0.0f32,
                1.0f32,
            ),
            common_text("C"),
            synced_num_input(
                store,
                |c| c.to_oklch().1,
                |c, ch| {
                    let (l, _, h) = c.to_oklch();
                    oklch(l, ch, h)
                },
                60.0,
                0.0f32,
                0.4f32,
            ),
            common_text("H"),
            synced_num_input(
                store,
                |c| c.to_oklch().2,
                |c, h| {
                    let (l, c, _) = c.to_oklch();
                    oklch(l, c, h)
                },
                60.0,
                0.0f32,
                360.0f32,
            ),
        ],
    )
}

fn oklcha_item(store: &ColorStore) -> Entity {
    base_item(
        "OKLCHA",
        |c| {
            let (l, c, h, alpha) = c.to_oklcha();
            oklcha(l, c, h, alpha)
        },
        [
            common_text("L"),
            synced_num_input(
                store,
                |c| c.to_oklcha().0,
                |c, l| {
                    let (_, c, h, alpha) = c.to_oklcha();
                    oklcha(l, c, h, alpha)
                },
                60.0,
                0.0f32,
                1.0f32,
            ),
            common_text("C"),
            synced_num_input(
                store,
                |c| c.to_oklcha().1,
                |c, ch| {
                    let (l, _, h, alpha) = c.to_oklcha();
                    oklcha(l, ch, h, alpha)
                },
                60.0,
                0.0f32,
                0.4f32,
            ),
            common_text("H"),
            synced_num_input(
                store,
                |c| c.to_oklcha().2,
                |c, h| {
                    let (l, c, _, alpha) = c.to_oklcha();
                    oklcha(l, c, h, alpha)
                },
                60.0,
                0.0f32,
                360.0f32,
            ),
            common_text("A"),
            synced_num_input(
                store,
                |c| c.to_oklcha().3,
                |c, alpha| {
                    let (l, c, h, _) = c.to_oklcha();
                    oklcha(l, c, h, alpha)
                },
                60.0,
                0.0f32,
                1.0f32,
            ),
        ],
    )
}

/// Resulting state after parsing the input
///
/// 入力パースの結果状態
enum ParseResult<T> {
    /// Empty characters or characters entered mid-input (such as “-” or “.”) → Do nothing; hold
    ///
    /// 空文字や入力途中（"-" や "."）→ 何もせず保留
    Pending,
    /// Values within the range → Update only the color (do not change the text)
    ///
    /// 範囲内の正常値 → 色だけ更新（テキストは触らない）
    Valid(T),
    /// Out of range (e.g., 999 or negative numbers) → Force the text in the input field to match the clamping value and update the color
    ///
    /// 範囲外（999 や負数など） → 入力欄の文字もクランプ値に強制補正して色を更新
    Clamped(T),
}

trait ClampInput: Copy + std::fmt::Display {
    /// Whether to allow negative signs (-)
    ///
    /// 負の符号 (-) を許可するかどうか
    fn allows_negative(min: Self) -> bool;

    /// Parse the input string and determine its state
    ///
    /// 入力文字列をパースし、状態を判定する
    fn parse_input(s: &str, min: Self, max: Self) -> ParseResult<Self>;

    fn format_val(self) -> String {
        self.to_string()
    }
}

impl ClampInput for u8 {
    fn allows_negative(_min: Self) -> bool {
        // u8 (RGB, etc.) must never be negative
        // u8 (RGB など) は常にマイナス禁止
        false
    }

    fn parse_input(s: &str, min: Self, max: Self) -> ParseResult<Self> {
        let s = s.trim();
        if s.is_empty() {
            return ParseResult::Pending;
        }

        match s.parse::<i64>() {
            Ok(v) => {
                if v < min as i64 {
                    ParseResult::Clamped(min)
                } else if v > max as i64 {
                    ParseResult::Clamped(max)
                } else {
                    ParseResult::Valid(v as u8)
                }
            }
            Err(e) => match e.kind() {
                // 桁数が多すぎる場合は最大値に補正
                // If there are too many digits, round to the maximum value
                std::num::IntErrorKind::PosOverflow => ParseResult::Clamped(max),
                std::num::IntErrorKind::NegOverflow => ParseResult::Clamped(min),
                _ => ParseResult::Pending,
            },
        }
    }
}

impl ClampInput for f32 {
    fn allows_negative(min: Self) -> bool {
        // // As with OKLAB, negative values are allowed only when min is negative
        // OKLAB のように min が負の場合のみマイナス許可
        min < 0.0
    }

    fn parse_input(s: &str, min: Self, max: Self) -> ParseResult<Self> {
        let s = s.trim();
        // Empty characters and individual symbols entered mid-input are held
        // 空文字や入力途中の記号単体は保留
        if s.is_empty() || s == "-" || s == "." || s == "-." {
            return ParseResult::Pending;
        }

        match s.parse::<f64>() {
            Ok(v) => {
                let rounded = ((v * 10.0).round() / 10.0) as f32;
                if rounded < min {
                    ParseResult::Clamped(min)
                } else if rounded > max {
                    ParseResult::Clamped(max)
                } else {
                    ParseResult::Valid(rounded)
                }
            }
            Err(_) => ParseResult::Pending,
        }
    }

    fn format_val(self) -> String {
        format!("{:.1}", self)
    }
}

#[derive(Clone)]
#[expect(clippy::type_complexity)]
struct ColorStore {
    color: (ReadSignal<CurrentColor>, WriteSignal<CurrentColor>),
    /// A list of listeners that receive a color and update the strings in all fields
    ///
    /// 色を受け取って全フィールドの文字列を更新するリスナーのリスト
    listeners: Rc<RefCell<Vec<Box<dyn Fn(&CurrentColor)>>>>,
}

#[derive(Clone)]
struct CurrentColor {
    color: Color,
}

impl Deref for CurrentColor {
    type Target = Color;
    fn deref(&self) -> &Self::Target {
        &self.color
    }
}
impl DerefMut for CurrentColor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.color
    }
}

impl ColorStore {
    fn new(initial: Color) -> Self {
        Self {
            color: create_signal(CurrentColor { color: initial }),
            listeners: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Called when a palette is clicked or when input is confirmed
    ///
    /// パレットクリックや、入力確定時に呼ぶ
    fn set_color(&self, new_color: Color) {
        self.color.1.set(CurrentColor { color: new_color });
        // Update all registered fields with the new color
        // 登録されている全フィールドを新しい色で更新
        for listener in self.listeners.borrow().iter() {
            listener(&CurrentColor { color: new_color });
        }
    }

    /// /// Register update functions for each input
    ///
    /// 各 Input の更新関数を登録する
    fn register<F>(&self, update_fn: F)
    where
        F: Fn(&CurrentColor) + 'static,
    {
        self.listeners.borrow_mut().push(Box::new(update_fn));
    }
}

/// A helper for creating a numeric input field with synchronization functionality
///
/// 同期機能つきの数値Inputを作るヘルパー
fn synced_num_input<T, G, S>(
    store: &ColorStore,
    getter: G,
    setter: S,
    width: f32,
    min: T,
    max: T,
) -> Entity
where
    T: ClampInput + 'static,
    G: Fn(&Color) -> T + Copy + 'static,
    S: Fn(&Color, T) -> Color + Copy + 'static,
{
    let text = create_signal(String::new());
    let (text_r, text_w) = text;

    // A flag indicating whether the user is currently typing
    // 自分が入力中かどうかを示すフラグ
    let is_editing = Rc::new(Cell::new(false));

    // Synchronization When the Color Changes
    // 色が変わったときの同期処理
    {
        let is_editing = is_editing.clone();
        store.register(move |color| {
            // When `set_color` is triggered by something you entered, do not overwrite your own text.
            // 自分が入力した結果で `set_color` された時は自分の文字を上書きしない
            if is_editing.get() {
                return;
            }
            let val = getter(color);
            text_w.set(val.format_val());
        });
    }

    // When I am entered, calculate a new color and notify Store
    // 自分が入力されたときは、新しい色を計算して Store に通知
    let store_clone = store.clone();
    input(move || {
        InputContents::new((text_r, text_w))
            .is_ime(false)
            .numeric_only(true)
    })
    .modify(
        ii().size((width, 35.0))
            .r(3.0)
            .cursor_text()
            .select_text()
            .p(2.0)
            .items_center()
            .text_center()
            .text_color(dynamic(|t: &Theme| t.text))
            .bg_color(dynamic(|t: &Theme| t.surface)),
    )
    .on_char_input(move |_| {
        let raw = text_r.get();

        // If a ‘-’ is entered in a field where negative numbers are not allowed, remove it immediately.
        // 負数が禁止されている項目で '-' が打たれたら即座に除去する
        let input_text = if !T::allows_negative(min) && raw.contains('-') {
            let cleaned: String = raw.chars().filter(|&c| c != '-').collect();
            text_w.set(cleaned.clone());
            cleaned
        } else {
            raw
        };

        match T::parse_input(&input_text, min, max) {
            ParseResult::Pending => {
                // Do nothing when all characters are cleared or while typing characters such as “-” or “.”
                // 全消し時や、"-" "." などの入力途中は何もしない
            }
            ParseResult::Valid(val) => {
                // Only the color is updated; the text field remains unchanged.
                // 色だけ更新しテキスト欄はそのまま
                let current_color = store_clone.color.0.get();
                let new_color = setter(&current_color, val);

                is_editing.set(true);
                store_clone.set_color(new_color);
                is_editing.set(false);
            }
            ParseResult::Clamped(val) => {
                // Adjust the text in the input field to the maximum/minimum values and update its color
                // 入力欄のテキスト自体も最大値/最小値に補正し色を更新
                text_w.set(val.format_val());

                let current_color = store_clone.color.0.get();
                let new_color = setter(&current_color, val);

                is_editing.set(true);
                store_clone.set_color(new_color);
                is_editing.set(false);
            }
        }
    })
}

/// HEX Input helper with synchronization functionality
///
/// 同期機能つきの HEX Input ヘルパー
fn synced_hex_input<G, S>(store: &ColorStore, getter: G, setter: S, max_len: usize) -> Entity
where
    G: Fn(&Color) -> String + Copy + 'static,
    S: Fn(&Color, &str) -> Option<Color> + Copy + 'static,
{
    let text = create_signal(String::new());
    let (text_r, text_w) = text;

    // When the color changes, set the latest HEX string
    // 色が変わったとき、最新の HEX 文字列をセット
    store.register(move |color| {
        let val = getter(color);
        text_w.set(val);
    });

    let store_clone = store.clone();
    input(move || {
        InputContents::new((text_r, text_w))
            .is_ime(false)
            .max_length(max_len)
            // Set to false to accept A-F
            // A-F を受け付けるため false
            .numeric_only(false)
    })
    .modify(
        ii().size((100.0, 35.0))
            .r(3.0)
            .cursor_text()
            .select_text()
            .p(2.0)
            .items_center()
            .text_center()
            .text_color(dynamic(|t: &Theme| t.text))
            .bg_color(dynamic(|t: &Theme| t.surface)),
    )
    .on_char_input(move |_| {
        let input_str = text_r.get();

        // Remove characters other than 0-9, a-f, and A-F,
        // convert them to uppercase, and truncate to the maximum character count
        // 0-9, a-f, A-F 以外の文字を取り除き、大文字にして最大文字数で切り詰める
        let filtered: String = input_str
            .chars()
            .filter(|c| c.is_ascii_hexdigit())
            .take(max_len)
            .flat_map(|c| c.to_uppercase())
            .collect();

        // Update the input field to display the formatted text
        // 入力フィールドの表示を整形後の文字に更新
        text_w.set(filtered.clone());

        // Update ColorStore only when the required number of digits (6 or 8) is met.
        // 規定の桁数（6桁または8桁）が揃った時だけ、ColorStore を更新する
        if filtered.len() == max_len {
            let current_color = store_clone.color.0.get();
            if let Some(new_color) = setter(&current_color, &filtered) {
                store_clone.set_color(new_color);
            }
        }
    })
}
