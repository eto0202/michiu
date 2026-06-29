struct GlobalConfig {
    screen_size: vec2<f32>,
    scale: f32,
    padding: f32,
};

@group(0) @binding(0) var<uniform> config: GlobalConfig;

// Rust 側の binding: 1 (atlas.view) と同期
@group(0) @binding(1) var t_texture: texture_2d<f32>;

// Rust 側の binding: 2 (atlas.sampler) と同期
@group(0) @binding(2) var s_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct InstanceInput {
    @location(1) rect: vec4<f32>, // x, y, w, h
    @location(2) transform_0: vec4<f32>,         // 4x4 行列の列0
    @location(3) transform_1: vec4<f32>,         // 4x4 行列の列1
    @location(4) transform_2: vec4<f32>,         // 4x4 行列の列2
    @location(5) transform_3: vec4<f32>,         // 4x4 行列の列3
    @location(6) color: vec4<f32>, // bg_color
    @location(7) corner_radius: vec4<f32>, // tl, tr, br, bl
    @location(8) border_width: vec4<f32>, // t, r, b, l
    @location(9) border_color: vec4<f32>,
    @location(10) opacity_mode_sizing: vec4<f32>,
    @location(11) uv_range: vec4<f32>, // uv_min(xy) と uv_max(zw) が入っている
    @location(12) gradient_end_color: vec4<f32>,
    @location(13) gradient_angle_and_origin: vec4<f32>,
    @location(14) shadow_color: vec4<f32>,
    @location(15) shadow_params: vec4<f32>, // offset_x, offset_y, blur, spread
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,           // 要素ローカルの物理ピクセル座標
    @location(1) size: vec2<f32>,                // 要素の物理サイズ (width, height)
    @location(2) uv: vec2<f32>,

    // フラグメントへ引き渡すインスタンス属性
    @location(3) color: vec4<f32>,
    @location(4) corner_radius: vec4<f32>,
    @location(5) border_width: vec4<f32>,
    @location(6) border_color: vec4<f32>,
    @location(7) opacity_mode_sizing: vec4<f32>,
    @location(8) uv_range: vec4<f32>,
    @location(9) gradient_end_color: vec4<f32>,
    @location(10) gradient_angle_and_origin: vec4<f32>,
    @location(11) shadow_color: vec4<f32>,
    @location(12) shadow_params: vec4<f32>,
};

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    let width = instance.rect.z;
    let height = instance.rect.w;
    out.size = vec2<f32>(width, height);

    // 影（BoxShadow）による頂点描画境界の自動拡張
    var margin = 0.0;
    if (instance.shadow_color.a > 0.0) {
        let shadow_offset = instance.shadow_params.xy;
        let shadow_blur = instance.shadow_params.z;
        let shadow_spread = instance.shadow_params.w;

        // 影が完全に消え去るのに必要なマージンを物理ピクセル単位で算出
        margin = max(0.0, shadow_spread) + shadow_blur * 3.0 + max(abs(shadow_offset.x), abs(shadow_offset.y));
    }

    // 1x1 の頂点（0.0 ～ 1.0）を、[-margin, size + margin] の物理ピクセル座標へ引き伸ばす
    let size_vec = vec2<f32>(width, height);
    let local_pixel = mix(vec2<f32>(-margin), size_vec + vec2<f32>(margin), vertex.position);
    out.local_pos = local_pixel;

    // トランスフォーム行列の再構築 (Column-Major)
    let transform = mat4x4<f32>(
        instance.transform_0,
        instance.transform_1,
        instance.transform_2,
        instance.transform_3
    );

    // トランスフォーム中心 (Transform Origin) の考慮
    let origin_pixel = instance.gradient_angle_and_origin.yz * out.size;
    // 拡張されたピクセル座標にトランスフォームを適用
    let pos_centered = local_pixel - origin_pixel;
    let pos_transformed = (transform * vec4<f32>(pos_centered, 0.0, 1.0)).xy + origin_pixel;

    // 親ウィンドウ上の絶対物理座標
    let abs_phys_pos = instance.rect.xy * config.scale + pos_transformed * config.scale;

    // ウィンドウ空間の NDC（-1.0 ～ 1.0）への投影変換 (DComp は Y軸下向き正)
    let ndc_x = (abs_phys_pos.x / config.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (abs_phys_pos.y / config.screen_size.y) * 2.0;

    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);

    // 本来の要素サイズ（size_vec）に対するローカル物理位置の比率を正確に算出
    // (マージンは負数や1.0を超える値へと正確にスケーリング)
    let local_ratio = local_pixel / size_vec;

    // UV の補間 (通常の画像 / テキスト兼用)
    out.uv = mix(instance.uv_range.xy, instance.uv_range.zw, local_ratio);

    // インスタンス変数のフォワード
    out.color = instance.color;
    out.corner_radius = instance.corner_radius;
    out.border_width = instance.border_width;
    out.border_color = instance.border_color;
    out.opacity_mode_sizing = instance.opacity_mode_sizing;
    out.uv_range = instance.uv_range;
    out.gradient_end_color = instance.gradient_end_color;
    out.gradient_angle_and_origin = instance.gradient_angle_and_origin;
    out.shadow_color = instance.shadow_color;
    out.shadow_params = instance.shadow_params;
    return out;
}

fn sd_rounded_box(p: vec2<f32>, b: vec2<f32>, r: vec4<f32>) -> f32 {
    // 象限（x, y の符号）に応じて角丸半径 [tl, tr, br, bl] を切り替える
    var rad = r.x; // default: top_left (x<0, y<0)
    if (p.x >= 0.0 && p.y < 0.0) { rad = r.y; } // top_right
    if (p.x >= 0.0 && p.y >= 0.0) { rad = r.z; } // bottom_right
    if (p.x < 0.0 && p.y >= 0.0) { rad = r.w; } // bottom_left

    let q = abs(p) - b + vec2<f32>(rad);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - rad;
}

// 各チャンネルごとに厳密なデガンマを計算するヘルパー
fn srgb_to_linear_scalar(c: f32) -> f32 {
    let clamped = max(c, 0.0);
    // select(false_value, true_value, condition_bool)
    return select(
        pow((clamped + 0.055) / 1.055, 2.4),
        clamped / 12.92,
        clamped <= 0.04045
    );
}

// 厳密な sRGB から Linear へのカラー変換（デガンマ）
fn srgb_to_linear(srgb: vec4<f32>) -> vec4<f32> {
    let r = srgb_to_linear_scalar(srgb.r);
    let g = srgb_to_linear_scalar(srgb.g);
    let b = srgb_to_linear_scalar(srgb.b);
    
    return vec4<f32>(r, g, b, srgb.a);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let opacity = in.opacity_mode_sizing.x;
    let mode = in.opacity_mode_sizing.y;
    let box_sizing = in.opacity_mode_sizing.z;

    let b = in.size * 0.5; // ハーフサイズ
    let local_center = in.local_pos - b; // 中心原点の座標

    // 入力されるすべての sRGB カラーを Linear 空間へ一斉デガンマ
    let color_linear = srgb_to_linear(in.color);
    let border_color_linear = srgb_to_linear(in.border_color);
    let gradient_end_linear = srgb_to_linear(in.gradient_end_color);
    let shadow_color_linear = srgb_to_linear(in.shadow_color);

    // 本体および丸角の描画計算
    let dist_to_box = sd_rounded_box(local_center, b, in.corner_radius);
    // 1ピクセル幅のアンチエイリアシング
    let box_alpha = 1.0 - smoothstep(-0.5, 0.5, dist_to_box);

    // ソフトシャドウ（BoxShadow）の描画計算
    var shadow_out = vec4<f32>(0.0);
    if (shadow_color_linear.a > 0.0) {
        let shadow_offset = in.shadow_params.xy;
        let shadow_blur = in.shadow_params.z;
        let shadow_spread = in.shadow_params.w;

        // 影の位置をずらし、spread 分だけ大きさを拡張
        let shadow_pos = local_center - shadow_offset;
        let shadow_b = b + vec2<f32>(shadow_spread);
        let shadow_radius = in.corner_radius + vec4<f32>(shadow_spread);

        let dist_to_shadow = sd_rounded_box(shadow_pos, shadow_b, shadow_radius);

        var shadow_alpha = 0.0;
        if (shadow_blur > 0.0) {
            // ぼかし (blur) のあるソフトシャドウを smoothstep でシミュレート
            shadow_alpha = 1.0 - smoothstep(-shadow_blur, shadow_blur, dist_to_shadow);
        } else {
            shadow_alpha = select(0.0, 1.0, dist_to_shadow <= 0.0);
        }

        // 本体の境界（-1.0ピクセル内側から 0.0ピクセル線上）で影をシャットアウト
        let shadow_gate = smoothstep(-1.0, 0.0, dist_to_box);
        shadow_alpha = shadow_alpha * shadow_gate;

        // 乗算済みアルファ対応の影の色
        let s_a = shadow_color_linear.a * shadow_alpha * opacity;
        shadow_out = vec4<f32>(shadow_color_linear.rgb * s_a, s_a);
    }

    // 枠線（Border）の正確な内側描画計算
    // Taffy のレイアウトに100%適合させるため、枠線は常に外枠（b）の内側に
    var border_alpha = 0.0;
    let has_border = border_color_linear.a > 0.0 && (in.border_width.x + in.border_width.y + in.border_width.z + in.border_width.w) > 0.0;

    if (has_border) {
        // 各辺ごとの border_width [top, right, bottom, left] を反映
        let b_width = in.border_width;

        // 左右・上下の非対称性をシフトさせて中心をオフセット
        let border_center_shift = vec2<f32>(
            (b_width.w - b_width.y) * 0.5, // (left - right) / 2
            (b_width.x - b_width.z) * 0.5  // (top - bottom) / 2
        );
        let inner_b = b - vec2<f32>(
            (b_width.w + b_width.y) * 0.5,
            (b_width.x + b_width.z) * 0.5
        );
        let inner_pos = local_center - border_center_shift;

        // 内側丸角の縮小
        let inner_radius = max(in.corner_radius - vec4<f32>(
            b_width.x, b_width.y, b_width.z, b_width.w
        ), vec4<f32>(0.0));

        let dist_to_inner = sd_rounded_box(inner_pos, inner_b, inner_radius);

        // 外枠の内側かつ内枠の外側が border 領域
        border_alpha = box_alpha * smoothstep(-0.5, 0.5, dist_to_inner);
    }

    // 背景色・グラデーション・サンプリングの取得
    var element_color = vec4<f32>(0.0);

    if (mode == 2.0) {
        // テキスト描画（Bgra8アトラスフォントの Alpha サンプリング、PMA着色）
        let tex_color = textureSample(t_texture, s_sampler, in.uv);
        let alpha = tex_color.a * opacity;
        element_color = vec4<f32>(color_linear.rgb * alpha, alpha);

    } else if (mode == 3.0) {
        // 静止画 WebView2 キャッシュ（Bgra8サンプリング、PMA適用）
        let tex_color = textureSample(t_texture, s_sampler, in.uv);
        // 元テクスチャが sRGB/PMA のため、単純に不透明度を乗算
        element_color = tex_color * opacity;

    } else {
        // 通常（Solid / グラデーション描画、PMA）
        var base_color = color_linear;
        if (mode == 1.0) {
            // 2色グラデーション
            let angle = in.gradient_angle_and_origin.x;
            let dir = vec2<f32>(cos(angle), sin(angle));
            let proj = dot(local_center, dir) / (length(in.size) * 0.5);
            let t = clamp(proj * 0.5 + 0.5, 0.0, 1.0);
            base_color = mix(color_linear, gradient_end_linear, t);
        }
        element_color = vec4<f32>(base_color.rgb * base_color.a * opacity, base_color.a * opacity);
    }

    // 枠線（Border）と本体背景、および影（BoxShadow）の PMA 合成
    var final_color = vec4<f32>(0.0);

    if (has_border) {
        let b_pma = vec4<f32>(border_color_linear.rgb * border_color_linear.a * opacity, border_color_linear.a * opacity);
        // 枠線のピクセル割合に応じて、本体の背景を奥側に押しやる
        element_color = mix(element_color, b_pma, border_alpha);
    }

    // 影の上に要素本体を Premultiplied Alpha で合成
    final_color = element_color * box_alpha + shadow_out * (1.0 - element_color.a * box_alpha);

    return final_color;
}
