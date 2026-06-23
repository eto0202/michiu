use crate::{AnimationCurve, Color, CornerRadius, PropertyList};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TransitionValue {
    Color(Color),
    Opacity(f32),
    Transform([[f32; 4]; 4]),
    CornerRadius(CornerRadius),
    Width(f32),
    Height(f32),
}

impl TransitionValue {
    /// 進行度 t (0.0 ～ 1.0) に基づいて、自己と目標値を線形補間（Lerp）します
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        match (self, other) {
            (TransitionValue::Color(s), TransitionValue::Color(e)) => {
                TransitionValue::Color(Color {
                    r: s.r + (e.r - s.r) * t,
                    g: s.g + (e.g - s.g) * t,
                    b: s.b + (e.b - s.b) * t,
                    a: s.a + (e.a - s.a) * t,
                })
            }
            (TransitionValue::Opacity(s), TransitionValue::Opacity(e)) => {
                TransitionValue::Opacity(s + (e - s) * t)
            }
            (TransitionValue::Transform(s), TransitionValue::Transform(e)) => {
                let d_start = decompose_2d(s);
                let d_end = decompose_2d(e);

                // 各変形要素の線形補間 (Lerp)
                let tx =
                    d_start.translation[0] + (d_end.translation[0] - d_start.translation[0]) * t;
                let ty =
                    d_start.translation[1] + (d_end.translation[1] - d_start.translation[1]) * t;

                let sx = d_start.scale[0] + (d_end.scale[0] - d_start.scale[0]) * t;
                let sy = d_start.scale[1] + (d_end.scale[1] - d_start.scale[1]) * t;

                // 境界 (-PI ～ PI) での逆逆回転を防ぐ最短軌道での角度差分補間
                let mut diff = d_end.rotation - d_start.rotation;
                const PI: f32 = std::f32::consts::PI;
                const TWO_PI: f32 = PI * 2.0;
                diff = (diff + PI).rem_euclid(TWO_PI) - PI; // 最短角度差にクランプ

                let rotation = d_start.rotation + diff * t;

                let d_mid = Decomposed2D {
                    translation: [tx, ty],
                    scale: [sx, sy],
                    rotation,
                };

                // 正確に再合成して 4x4 行列として返却
                TransitionValue::Transform(recompose_2d(&d_mid))
            }
            (TransitionValue::CornerRadius(s), TransitionValue::CornerRadius(e)) => {
                TransitionValue::CornerRadius(CornerRadius {
                    top_left: s.top_left + (e.top_left - s.top_left) * t,
                    top_right: s.top_right + (e.top_right - s.top_right) * t,
                    bottom_right: s.bottom_right + (e.bottom_right - s.bottom_right) * t,
                    bottom_left: s.bottom_left + (e.bottom_left - s.bottom_left) * t,
                })
            }
            (TransitionValue::Width(s), TransitionValue::Width(e)) => {
                TransitionValue::Width(s + (e - s) * t)
            }
            (TransitionValue::Height(s), TransitionValue::Height(e)) => {
                TransitionValue::Height(s + (e - s) * t)
            }
            _ => *self, // 型不一致の場合は遷移せずに自己を返す
        }
    }
}

/// 現在駆動中のアクティブなトランジション
#[derive(Debug, Clone, Copy)]
pub(crate) struct ActiveTransition {
    pub(crate) property_list: PropertyList,
    pub(crate) start_time: Instant,
    pub(crate) duration: Duration,
    pub(crate) curve: AnimationCurve,
    pub(crate) start_value: TransitionValue,
    pub(crate) end_value: TransitionValue,
}

#[derive(Debug, Clone, Copy)]
struct Decomposed2D {
    translation: [f32; 2],
    scale: [f32; 2],
    rotation: f32, // ラジアン単位のZ軸回転
}

/// 4x4 Column-Major 行列から 2D 平行移動・スケール・回転を分解抽出する
fn decompose_2d(m: &[[f32; 4]; 4]) -> Decomposed2D {
    let tx = m[3][0];
    let ty = m[3][1];

    // X軸の基底ベクトル [m[0][0], m[0][1]] から Xスケールを算出
    let m00 = m[0][0];
    let m01 = m[0][1];
    let sx = (m00 * m00 + m01 * m01).sqrt();

    // Y軸の基底ベクトル [m[1][0], m[1][1]] から Yスケールを算出
    let m10 = m[1][0];
    let m11 = m[1][1];
    let sy = (m10 * m10 + m11 * m11).sqrt();

    // atan2 で回転角度を正確に抽出する (sx が極小の場合は 0.0)
    let rotation = if sx > 1e-6 { m01.atan2(m00) } else { 0.0 };

    Decomposed2D {
        translation: [tx, ty],
        scale: [sx, sy],
        rotation,
    }
}

/// 補間された2Dパラメータから 4x4 アフィン行列へ再ビルドする
fn recompose_2d(d: &Decomposed2D) -> [[f32; 4]; 4] {
    let cos = d.rotation.cos();
    let sin = d.rotation.sin();

    let mut m = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];

    // スケールと回転の合成適用
    m[0][0] = d.scale[0] * cos;
    m[0][1] = d.scale[0] * sin;
    m[1][0] = d.scale[1] * -sin;
    m[1][1] = d.scale[1] * cos;

    // 平行移動
    m[3][0] = d.translation[0];
    m[3][1] = d.translation[1];

    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use std::time::{Duration, Instant};

    // 基本的なカラー / オパシティ補間 (Lerp) の検証
    #[test]
    fn test_transition_lerp_logic() {
        let start_color = Color::rgb(1.0, 0.0, 0.0); // 赤
        let end_color = Color::rgb(0.0, 0.0, 1.0); // 青

        let start_val = TransitionValue::Color(start_color);
        let end_val = TransitionValue::Color(end_color);

        // 進行度 50% での中間色補間を検証
        let mid_val = start_val.lerp(&end_val, 0.5);
        if let TransitionValue::Color(c) = mid_val {
            assert_eq!(c.r, 0.5);
            assert_eq!(c.g, 0.0);
            assert_eq!(c.b, 0.5);
            assert_eq!(c.a, 1.0);
        } else {
            panic!("Expected Color variant");
        }
    }

    // トランジションのライフサイクル（開始 ➔ 中間 ➔ 完了と自動クリーンアップ）
    #[test]
    fn test_transition_lifecycle_and_ticking() {
        let mut cx = Context::new();
        let mut el_handle = None;

        // build_ui は Element（この場合は base）のみを返します
        let _root = build_ui(&mut cx, || {
            let h_style = ThisStyle::new()
                .bg_color(Color::rgb(0.0, 0.0, 1.0)) // ホバー時は青
                .opacity(0.5);

            let base = div(ts()
                .bg_color(Color::rgb(1.0, 0.0, 0.0)) // 初期は赤
                .opacity(1.0)
                // 背景色は 1秒 (1000ms) で EaseInOutQuad トランジション
                .transition(Transition::new(
                    prop_bg_color(),
                    Duration::from_millis(1000),
                    ease_in_out_quad(),
                ))
                .hovered(h_style));

            el_handle = Some(base); // 外部変数へ退避
            base
        });

        let el = el_handle.unwrap();

        // 座標同期を実行（これでスタイルが解決され、初期値「赤」が登録されます）
        cx.sync_layout_and_render_list(el.id, LayoutSize::new(100.0, 100.0));
        assert_eq!(
            cx.visual_properties[el.id].bg_color,
            Some(Color::rgb(1.0, 0.0, 0.0))
        );

        // 1. ホバー状態をオンにする
        {
            let _guard = bind_context(&cx);
            cx.set_hovered(el.id, true);
        }

        // トランジションがアクティブに起動したか検証
        assert!(cx.has_active_animations());
        // 起きた瞬間は、まだ赤（スナップしていないこと）を検証
        assert_eq!(
            cx.visual_properties[el.id].bg_color,
            Some(Color::rgb(1.0, 0.0, 0.0))
        );

        // 2. 時間を擬似的に進める (500ms 経過状態を作る)
        if let Some(list) = cx.active_transitions.get_mut(el.id) {
            for t in list.iter_mut() {
                t.start_time = Instant::now() - Duration::from_millis(500);
            }
        }

        // 1 Tick 進める
        cx.tick_transitions();

        // 50% 時点の EaseInOutQuad 補間色を確認 (赤 0.5、青 0.5 になっているか)
        let current_color = cx.visual_properties[el.id].bg_color.unwrap();
        // 浮動小数点の直接比較 (assert_eq!) を避け、実時間の経過誤差を許容する
        assert!(
            (current_color.r - 0.5).abs() < 0.01,
            "Expected r to be near 0.5, got {}",
            current_color.r
        );
        assert!(
            (current_color.b - 0.5).abs() < 0.01,
            "Expected b to be near 0.5, got {}",
            current_color.b
        );

        // 3. さらに時間を進めて完了させる (1200ms 経過状態を作る)
        if let Some(list) = cx.active_transitions.get_mut(el.id) {
            for t in list.iter_mut() {
                t.start_time = Instant::now() - Duration::from_millis(1200);
            }
        }

        // 1 Tick 進める（これで完了するはず）
        cx.tick_transitions();

        // 目標値（青）に完全に達していること、およびアニメーションが終了してクリーンアップされたことを検証
        assert_eq!(
            cx.visual_properties[el.id].bg_color,
            Some(Color::rgb(0.0, 0.0, 1.0))
        );
        assert!(!cx.has_active_animations()); // 完了したのでリストは空のはず

        cx.despawn(el);
    }

    // アニメーション途中での割り込み（逆再生 / 中断）の滑らかな補間検証
    #[test]
    fn test_transition_interruption_lerp() {
        let mut cx = Context::new();
        let mut el_handle = None;

        let _root = build_ui(&mut cx, || {
            let h_style = ts().bg_color(Color::rgb(0.0, 0.0, 1.0)); // ホバー時は青

            let base = div(ts()
                .bg_color(Color::rgb(1.0, 0.0, 0.0)) // 初期は赤
                .transition(Transition::new(
                    prop_bg_color(),
                    Duration::from_millis(1000),
                    linear(), // 線形変化
                ))
                .hovered(h_style));

            el_handle = Some(base); // 外部変数へ退避
            base
        });

        let el = el_handle.unwrap();

        cx.sync_layout_and_render_list(el.id, LayoutSize::new(100.0, 100.0));

        // 1. ホバー開始
        {
            let _guard = bind_context(&cx);
            cx.set_hovered(el.id, true);
        }

        // 300ms (30%) 経過させる
        if let Some(list) = cx.active_transitions.get_mut(el.id) {
            for t in list.iter_mut() {
                t.start_time = Instant::now() - Duration::from_millis(300);
            }
        }
        cx.tick_transitions();

        // 30% 変化した中間色を検証 (赤: 0.7, 青: 0.3)
        let mid_color = cx.visual_properties[el.id].bg_color.unwrap();
        assert!((mid_color.r - 0.7).abs() < 0.01);
        assert!((mid_color.b - 0.3).abs() < 0.01);

        // 2. 完了前にホバー解除（割り込み）
        {
            let _guard = bind_context(&cx);
            cx.set_hovered(el.id, false); // 元の「赤」へ戻す
        }

        // ★ 割り込みによって、新しい開始点が「中間地点のカラー (0.7, 0.0, 0.3)」に設定されているか検証
        if let Some(list) = cx.active_transitions.get(el.id) {
            let t = &list[0];
            if let TransitionValue::Color(c_start) = t.start_value {
                assert!((c_start.r - 0.7).abs() < 0.01);
                assert!((c_start.b - 0.3).abs() < 0.01);
                assert_eq!(
                    t.end_value,
                    TransitionValue::Color(Color::rgb(1.0, 0.0, 0.0))
                ); // 目標値は赤
            } else {
                panic!("Expected color start value");
            }
        }

        cx.despawn(el);
    }
}
