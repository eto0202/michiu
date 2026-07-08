#![allow(dead_code)]
use michiu_ui::{BoxShadow, prelude::*};
use std::time::Duration;

pub struct Theme {}

impl Theme {
    /// プライマリ基準色 (HSL: 318, 56%, 59%)
    pub const PRIMARY: Color = Color::rgb_f32(0.8196, 0.3604, 0.6818);
    /// プライマリ・ホバー (HSL: 318, 56%, 52%)
    pub const PRIMARY_HOVER: Color = Color::rgb_f32(0.7944, 0.2456, 0.6297);
    /// プライマリ・アクティブ / プレス (HSL: 318, 56%, 45%)
    pub const PRIMARY_ACTIVE: Color = Color::rgb_f32(0.7020, 0.1700, 0.5444);
    /// プライマリ・ライトコンテナ / 選択背景 (HSL: 318, 56%, 95%)
    pub const PRIMARY_CONTAINER: Color = Color::rgb_f32(0.9780, 0.8800, 0.9472);

    /// セカンダリ基準色 - バイオレットブルー (HSL: 255, 40%, 58%)
    pub const SECONDARY: Color = Color::rgb_f32(0.4960, 0.4120, 0.7480);
    /// セカンダリ・ホバー (HSL: 255, 40%, 51%)
    pub const SECONDARY_HOVER: Color = Color::rgb_f32(0.4020, 0.3100, 0.6900);
    /// セカンダリ・アクティブ / プレス (HSL: 255, 40%, 44%)
    pub const SECONDARY_ACTIVE: Color = Color::rgb_f32(0.3200, 0.2200, 0.5800);
    /// セカンダリ・ライトコンテナ (HSL: 255, 40%, 95%)
    pub const SECONDARY_CONTAINER: Color = Color::rgb_f32(0.9350, 0.9200, 0.9700);

    /// アプリ全体の背景色 (ライト) (HSL: 218, 20%, 98%)
    pub const LIGHT_BG: Color = Color::rgb_f32(0.9760, 0.9790, 0.9840);
    /// カード、ポップアップなどの表面色 (ライト) (HSL: 0, 0%, 100%)
    pub const LIGHT_SURFACE: Color = Color::WHITE;
    /// コンポーネント枠線色 (ライト) (HSL: 225, 10%, 88%)
    pub const LIGHT_BORDER: Color = Color::rgb_f32(0.8680, 0.8740, 0.8920);
    /// メインテキスト (ライト) (HSL: 224, 15%, 15%)
    pub const LIGHT_TEXT: Color = Color::rgb_f32(0.1280, 0.1400, 0.1720);
    /// サブテキスト / プレースホルダー (ライト) (HSL: 228, 10%, 50%)
    pub const LIGHT_TEXT_MUTED: Color = Color::rgb_f32(0.4500, 0.4700, 0.5500);

    /// アプリ全体の背景色 (ダーク) (HSL: 224, 15%, 10%)
    pub const DARK_BG: Color = Color::rgb_f32(0.0850, 0.0930, 0.1150);
    /// カード、ポップアップなどの表面色 (ダーク) (HSL: 220, 12%, 15%)
    pub const DARK_SURFACE: Color = Color::rgb_f32(0.1320, 0.1440, 0.1680);
    /// コンポーネント枠線色 (ダーク) (HSL: 220, 12%, 25%)
    pub const DARK_BORDER: Color = Color::rgb_f32(0.2200, 0.2400, 0.2800);
    /// メインテキスト (ダーク) (HSL: 225, 20%, 95%)
    pub const DARK_TEXT: Color = Color::rgb_f32(0.9400, 0.9450, 0.9600);
    /// サブテキスト / プレースホルダー (ダーク) (HSL: 225, 9%, 65%)
    pub const DARK_TEXT_MUTED: Color = Color::rgb_f32(0.6200, 0.6350, 0.6800);

    /// 危険 / エラー（プライマリのピンクに調和するクリムゾンレッド） (HSL: 351, 70%, 54%)
    pub const DANGER: Color = Color::rgb_f32(0.8620, 0.2180, 0.3100);
    /// 成功（彩度を抑えたエメラルドグリーン） (HSL: 144, 50%, 45%)
    pub const SUCCESS: Color = Color::rgb_f32(0.2250, 0.6750, 0.4050);
    /// 警告（温かみのあるアンバーオレンジ） (HSL: 35, 75%, 50%)
    pub const WARNING: Color = Color::rgb_f32(0.8750, 0.5625, 0.1250);

    // タイポグラフィ
    pub const FONT_MAIN: &str = "Segoe UI";
    pub const FONT_SIZE_SM: f32 = 12.0;
    pub const FONT_SIZE_MD: f32 = 14.0;
    pub const FONT_SIZE_LG: f32 = 18.0;

    // 共通スタイル
    /// 基本的なボタンのインタラクション・サイズ・フォントの土台
    pub fn button_base() -> ThisStyle {
        ts().p((8.0, 16.0))
            .r(4.0)
            .font_family(Theme::FONT_MAIN)
            .font_size(Theme::FONT_SIZE_MD)
            .pressed(ts().transform_scale(0.97, 0.97))
            .trans_transform(Duration::from_millis(80), AnimationCurve::EaseInOutQuad)
            .trans_bg_color(Duration::from_millis(150), AnimationCurve::EaseOutQuad)
    }

    /// プライマリボタン (Pink)
    pub fn primary_button() -> ThisStyle {
        Theme::button_base()
            .bg_color(Theme::PRIMARY)
            .text_color(Color::WHITE)
            .hovered(ts().bg_color(Theme::PRIMARY_HOVER))
            .pressed(
                ts().bg_color(Theme::PRIMARY_ACTIVE)
                    .transform_scale(0.97, 0.97),
            )
    }

    /// セカンダリボタン (Violet)
    pub fn secondary_button() -> ThisStyle {
        Theme::button_base()
            .bg_color(Theme::SECONDARY)
            .text_color(Color::WHITE)
            .hovered(ts().bg_color(Theme::SECONDARY_HOVER))
            .pressed(
                ts().bg_color(Theme::SECONDARY_ACTIVE)
                    .transform_scale(0.97, 0.97),
            )
    }

    /// アウトラインボタン (透明背景 + 枠線)
    pub fn outline_button() -> ThisStyle {
        Theme::button_base()
            .bg_color(Color::TRANSPARENT)
            .border_solid(1.0)
            .border_color(Theme::LIGHT_BORDER)
            .text_color(Theme::LIGHT_TEXT)
            .hovered(
                ts().bg_color(Theme::PRIMARY_CONTAINER)
                    .border_color(Theme::PRIMARY),
            )
            .pressed(
                ts().bg_color(Theme::SECONDARY_CONTAINER)
                    .transform_scale(0.97, 0.97),
            )
            .trans_border_color(Duration::from_millis(150), AnimationCurve::EaseOutQuad)
    }

    /// ゴーストボタン (背景なし・枠線なし、ホバー時のみ強調)
    pub fn ghost_button() -> ThisStyle {
        Theme::button_base()
            .bg_color(Color::TRANSPARENT)
            .text_color(Theme::LIGHT_TEXT_MUTED)
            .hovered(
                ts().bg_color(Theme::PRIMARY_CONTAINER)
                    .text_color(Theme::PRIMARY),
            )
            .pressed(ts().transform_scale(0.97, 0.97))
    }

    /// テキスト入力フィールド（Input）の基本スタイル土台
    pub fn input_base() -> ThisStyle {
        ts().p((8.0, 12.0))
            .r(4.0)
            .border_solid(1.0)
            .border_color(Theme::LIGHT_BORDER)
            .bg_color(Theme::LIGHT_SURFACE)
            .text_color(Theme::LIGHT_TEXT)
            .font_family(Theme::FONT_MAIN)
            .font_size(Theme::FONT_SIZE_MD)
            .select_text()
            .select_bg_color(Color::rgba_f32(0.8196, 0.3604, 0.6818, 0.35)) // プライマリの35%不透明度を選択背景に
            // フォーカスされた際の枠線色アニメーション
            .focused(
                ts().border_color(Theme::PRIMARY)
                    .bg_color(Theme::LIGHT_SURFACE),
            )
            .trans_border_color(Duration::from_millis(150), AnimationCurve::EaseOutQuad)
            .trans_bg_color(Duration::from_millis(150), AnimationCurve::EaseOutQuad)
    }

    /// ダークテーマ用入力フィールド
    pub fn input_dark() -> ThisStyle {
        Theme::input_base()
            .bg_color(Theme::DARK_SURFACE)
            .border_color(Theme::DARK_BORDER)
            .text_color(Theme::DARK_TEXT)
            .focused(
                ts().border_color(Theme::PRIMARY)
                    .bg_color(Theme::DARK_SURFACE),
            )
    }

    /// コンポーネント格納用カードコンテナ (Light)
    pub fn card_light() -> ThisStyle {
        ts().p(16.0)
            .r(8.0)
            .bg_color(Theme::LIGHT_SURFACE)
            .border_solid(1.0)
            .border_color(Theme::LIGHT_BORDER)
            .box_shadow(BoxShadow::md()) // types.rs にて定義済みの標準中程度ソフトシャドウ
    }

    /// コンポーネント格納用カードコンテナ (Dark)
    pub fn card_dark() -> ThisStyle {
        ts().p(16.0)
            .r(8.0)
            .bg_color(Theme::DARK_SURFACE)
            .border_solid(1.0)
            .border_color(Theme::DARK_BORDER)
            .box_shadow(BoxShadow::lg().color(Color::rgba_f32(0.0, 0.0, 0.0, 0.3))) // より深い黒陰影
    }

    /// ステータス表示や通知カウント用の軽量バッジスタイル
    pub fn badge_base() -> ThisStyle {
        ts().p((2.0, 8.0))
            .rounded_full()
            .font_family(Theme::FONT_MAIN)
            .font_size(Theme::FONT_SIZE_SM)
            .font_weight(600) // セミボールド
    }

    /// プライマリバッジ
    pub fn primary_badge() -> ThisStyle {
        Theme::badge_base()
            .bg_color(Theme::PRIMARY_CONTAINER)
            .text_color(Theme::PRIMARY)
    }

    /// セカンダリバッジ
    pub fn secondary_badge() -> ThisStyle {
        Theme::badge_base()
            .bg_color(Theme::SECONDARY_CONTAINER)
            .text_color(Theme::SECONDARY)
    }
}
