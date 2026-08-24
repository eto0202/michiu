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

struct InstanceData {
    rect: vec4<f32>,
    transform_0: vec4<f32>,
    transform_1: vec4<f32>,
    transform_2: vec4<f32>,
    color: vec4<f32>,
    corner_radius: vec4<f32>,
    border_width: vec4<f32>,
    border_color: vec4<f32>,
    opacity_mode_sizing: vec4<f32>,
    uv_range: vec4<f32>,
    gradient_end_color: vec4<f32>,
    gradient_angle_and_origin: vec4<f32>,
    shadow_color: vec4<f32>,
    shadow_params: vec4<f32>,
    border_lengths: vec4<f32>,
    outline_width: vec4<f32>,
    outline_color: vec4<f32>,
    outline_lengths: vec4<f32>,
    outline_offset_and_flags: vec4<f32>,
};

@group(0) @binding(3) var<storage, read> instances: array<InstanceData>;

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) instance_idx: u32,
};

@vertex
fn vs_main(vertex: VertexInput, @builtin(instance_index) instance_idx: u32) -> VertexOutput {
    var out: VertexOutput;

    let instance = instances[instance_idx];

    let width = instance.rect.z;
    let height = instance.rect.w;

    // 影（BoxShadow）による頂点描画境界の自動拡張
    var margin = 0.0;
    if (instance.shadow_color.a > 0.0) {
        let shadow_offset = instance.shadow_params.xy;
        let shadow_blur = instance.shadow_params.z;
        let shadow_spread = instance.shadow_params.w;

        // 影が完全に消え去るのに必要なマージンを物理ピクセル単位で算出
        margin = max(0.0, shadow_spread) + shadow_blur * 3.0 + max(abs(shadow_offset.x), abs(shadow_offset.y));
    }

    let o_width = instance.outline_width;
    let o_offset = instance.outline_offset_and_flags.x;
    if (instance.outline_color.a > 0.0 && (o_width.x + o_width.y + o_width.z + o_width.w) > 0.0) {
        // オフセット + 最大アウトライン太さを物理ピクセルでクランプ
        let max_o_width = max(max(o_width.x, o_width.y), max(o_width.z, o_width.w));
        let outline_margin = max_o_width + max(0.0, o_offset);
        margin = max(margin, outline_margin + 1.5); // 1.5pxはアンチエイリアスの余白
    }

    // 1x1 の頂点（0.0 ～ 1.0）を、[-margin, size + margin] の物理ピクセル座標へ引き伸ばす
    let size_vec = vec2<f32>(width, height);
    let local_pixel = mix(vec2<f32>(-margin), size_vec + vec2<f32>(margin), vertex.position);
    out.local_pos = local_pixel;

    // トランスフォーム行列の再構築 (Column-Major)
    let transform = mat4x4<f32>(
        instance.transform_0,
        instance.transform_1,
        vec4<f32>(0.0, 0.0, 1.0, 0.0), // Z軸復元
        instance.transform_2           // 平行移動部
    );

    // トランスフォーム中心 (Transform Origin) の考慮
    let origin_pixel = instance.gradient_angle_and_origin.yz * size_vec;
    // 拡張されたピクセル座標にトランスフォームを適用
    let pos_centered = local_pixel - origin_pixel;
    let pos_transformed = (transform * vec4<f32>(pos_centered, 0.0, 1.0)).xy + origin_pixel;

    // 親ウィンドウ上の絶対物理座標
    var abs_phys_pos = instance.rect.xy * config.scale + pos_transformed * config.scale;

    // ウィンドウ空間の NDC（-1.0 ～ 1.0）への投影変換 (DComp は Y軸下向き正)
    let ndc_x = (abs_phys_pos.x / config.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (abs_phys_pos.y / config.screen_size.y) * 2.0;

    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);

    // 本来の要素サイズ（size_vec）に対するローカル物理位置の比率を正確に算出
    // (マージンは負数や1.0を超える値へと正確にスケーリング)
    let local_ratio = local_pixel / size_vec;

    // UV の補間 (通常の画像 / テキスト兼用)
    out.uv = mix(instance.uv_range.xy, instance.uv_range.zw, local_ratio);

    out.instance_idx = instance_idx; // インデックスのみをフラグメントに退避
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
    // フラグメント側でインデックスを用いて直接ストレージからインスタンスデータをルックアップ
    let instance = instances[in.instance_idx];

    let opacity = instance.opacity_mode_sizing.x;
    let mode = instance.opacity_mode_sizing.y;
    let box_sizing = instance.opacity_mode_sizing.z;

    let size_vec = instance.rect.zw;
    let b = size_vec * 0.5; // ハーフサイズ
    let local_center = in.local_pos - b; // 中心原点の座標

    // 入力されるすべての sRGB カラーを Linear 空間へ一斉デガンマ
    let color_linear = srgb_to_linear(instance.color);
    let border_color_linear = srgb_to_linear(instance.border_color);
    let gradient_end_linear = srgb_to_linear(instance.gradient_end_color);
    let shadow_color_linear = srgb_to_linear(instance.shadow_color);

    let min_edge = min(size_vec.x, size_vec.y);
    let max_radius = min_edge * 0.5;
    let clamped_radius = min(instance.corner_radius, vec4<f32>(max_radius));

    // 本体および丸角の描画計算
    let dist_to_box = sd_rounded_box(local_center, b, clamped_radius);
    // 1ピクセル幅のアンチエイリアシング
    var box_alpha = 1.0 - smoothstep(-0.5, 0.5, dist_to_box);

    if (mode < -0.5) {
        // 装飾・キャレットモード：SDF の 1px 減衰ボケをバイパスし、
        // 描画矩形内にピクセルがある場合はクッキリとした不透明（1.0）にする。
        box_alpha = 1.0;
    }

    // ソフトシャドウ（BoxShadow）の描画計算
    var shadow_out = vec4<f32>(0.0);
    if (shadow_color_linear.a > 0.0) {
        let shadow_offset = instance.shadow_params.xy;
        let shadow_blur = instance.shadow_params.z;
        let shadow_spread = instance.shadow_params.w;

        // 影の位置をずらし、spread 分だけ大きさを拡張
        let shadow_pos = local_center - shadow_offset;
        let shadow_b = b + vec2<f32>(shadow_spread);
        let shadow_radius = instance.corner_radius + vec4<f32>(shadow_spread);

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
    let has_border = border_color_linear.a > 0.0 && (instance.border_width.x + instance.border_width.y + instance.border_width.z + instance.border_width.w) > 0.0;

    if (has_border) {
        let b_width = instance.border_width;

        let border_center_shift = vec2<f32>(
            (b_width.w - b_width.y) * 0.5,
            (b_width.x - b_width.z) * 0.5
        );
        let inner_b = b - vec2<f32>(
            (b_width.w + b_width.y) * 0.5,
            (b_width.x + b_width.z) * 0.5
        );
        let inner_pos = local_center - border_center_shift;

        let inner_radius = max(instance.corner_radius - vec4<f32>(b_width.x, b_width.y, b_width.z, b_width.w), vec4<f32>(0.0));

        let dist_to_inner = sd_rounded_box(inner_pos, inner_b, inner_radius);

        // 各ピクセルの属する辺の判定
        // 太さが0.0の辺を距離判定から除外
        let dist_to_top = select(10000.0, local_center.y - (-b.y), b_width.x > 0.0);
        let dist_to_right = select(10000.0, b.x - local_center.x, b_width.y > 0.0);
        let dist_to_bottom = select(10000.0, b.y - local_center.y, b_width.z > 0.0);
        let dist_to_left = select(10000.0, local_center.x - (-b.x), b_width.w > 0.0);

        let min_dist = min(min(dist_to_top, dist_to_bottom), min(dist_to_left, dist_to_right));

        var edge_idx = 0u; // 0: top, 1: right, 2: bottom, 3: left
        if (min_dist == dist_to_right) { edge_idx = 1u; }
        else if (min_dist == dist_to_bottom) { edge_idx = 2u; }
        else if (min_dist == dist_to_left) { edge_idx = 3u; }

        // ビットフラグの解凍 (デコード)
        let flags = u32(instance.opacity_mode_sizing.w);
        let style = (flags >> (edge_idx * 4u)) & 3u;      // 0: Solid, 1: Dotted, 2: Dashed, 3: Double
        let alignment = (flags >> (edge_idx * 4u + 2u)) & 3u; // 0: Start, 1: End, 2: Center

        // アライメント基準点に基づく長さトリミングの計算
        let lengths = instance.border_lengths;
        var len_limit = 1.0;
        if (edge_idx == 0u) { len_limit = lengths.x; }
        else if (edge_idx == 1u) { len_limit = lengths.y; }
        else if (edge_idx == 2u) { len_limit = lengths.z; }
        else { len_limit = lengths.w; }

        // 各辺に沿った横軸/縦軸の進捗比率 t (0.0 -> 1.0)
        var t = 0.0;
        var pos_edge = 0.0; // 物理ピクセル位置（点線等で使用）
        if (edge_idx == 0u || edge_idx == 2u) {
            t = (local_center.x + b.x) / (b.x * 2.0);
            pos_edge = local_center.x + b.x;
        } else {
            t = (local_center.y + b.y) / (b.y * 2.0);
            pos_edge = local_center.y + b.y;
        }

        let edge_fade = 0.5 / max(size_vec.x, size_vec.y);
        var length_alpha = 1.0;

        // 長さ制限が 1.0 (ほぼ100%) の場合はバイパス
        if (len_limit < 0.999) {
            if (alignment == 0u) {
                // Start: 基準点が左端/上端 (tが上限長さを超えたらカット)
                length_alpha = 1.0 - smoothstep(len_limit - edge_fade, len_limit + edge_fade, t);
            } else if (alignment == 1u) {
                // End: 基準点が右端/下端 (tが下限に満たなければカット)
                let lower_bound = 1.0 - len_limit;
                length_alpha = smoothstep(lower_bound - edge_fade, lower_bound + edge_fade, t);
            } else {
                // Center: 基準点が中央 (中心 0.5 から対称に広げる)
                let half_len = len_limit * 0.5;
                let dist_from_center = abs(t - 0.5);
                length_alpha = 1.0 - smoothstep(half_len - edge_fade, half_len + edge_fade, dist_from_center);
            }
        }

        // スタイル別の模様パターン（点線、破線、二重線）の生成
        let w = b_width[edge_idx]; // この辺の太さ
        var style_alpha = 1.0;

        // 枠線の中心からの相対的な厚み方向割合 (外側 0.0 -> 内側 1.0)
        let thick_t = clamp(-dist_to_box / (-dist_to_box + dist_to_inner), 0.0, 1.0);

        if (style == 1u) {
            // Dotted (丸点の連続)
            let period = w * 2.2;
            let cycle_t = fract(pos_edge / period) * period - (period * 0.5);
            let radial_dist = length(vec2<f32>(cycle_t, (thick_t - 0.5) * w));
            style_alpha = 1.0 - smoothstep(w * 0.4 - 0.5, w * 0.4 + 0.5, radial_dist);
        } else if (style == 2u) {
            // Dashed (破線の連続)
            let period = w * 5.0; // 3w長さ、2w隙間
            let cycle_t = fract(pos_edge / period) * period;
            style_alpha = 1.0 - smoothstep(w * 3.0 - 0.5, w * 3.0 + 0.5, cycle_t);
        } else if (style == 3u) {
            // Double (二重枠線：外側1/3、隙間1/3、内側1/3)
            // 外側（0.0 ~ 0.33）および 内側（0.67 ~ 1.0）の時のみアルファ 1.0
            let is_double_void = smoothstep(0.30, 0.33, thick_t) * (1.0 - smoothstep(0.67, 0.70, thick_t));
            style_alpha = 1.0 - is_double_void;
        }

        // 属する辺のローカルな太さ w に基づいて内側への距離を制限。
        // w が 0.0 の辺では dist_to_inner_clamped が dist_to_box と一致し、smoothstep は 0.0 を出力。
        let dist_to_inner_clamped = min(dist_to_inner, dist_to_box + w);

        // すべてのアンチエイリアスマスクを合成
        border_alpha = box_alpha * smoothstep(-0.5, 0.5, dist_to_inner_clamped) * length_alpha * style_alpha;

        // 辺の太さ w が 0.0 のとき、枠線アルファを完全に 0.0 に潰し残留ノイズを一掃
        border_alpha = border_alpha * clamp(w, 0.0, 1.0);
    }

    var outline_alpha = 0.0;
    let outline_color_linear = srgb_to_linear(instance.outline_color);
    let o_offset = instance.outline_offset_and_flags.x;
    let o_width = instance.outline_width;
    let has_outline = outline_color_linear.a > 0.0 && (o_width.x + o_width.y + o_width.z + o_width.w) > 0.0;

    if (has_outline) {
        // ピクセルが属している辺を判定 (o_edge_idx)
        // 太さが0.0の辺を距離判定から除外
        // 修正: 判定時は太さに関わらず純粋な物理距離で最も近い辺を特定
        let dist_to_top_raw = local_center.y - (-b.y);
        let dist_to_right_raw = b.x - local_center.x;
        let dist_to_bottom_raw = b.y - local_center.y;
        let dist_to_left_raw = local_center.x - (-b.x);

        let min_dist_raw = min(min(dist_to_top_raw, dist_to_bottom_raw), min(dist_to_left_raw, dist_to_right_raw));

        var o_edge_idx = 0u; // 0: top, 1: right, 2: bottom, 3: left

        if (min_dist_raw == dist_to_right_raw) { o_edge_idx = 1u; }
        else if (min_dist_raw == dist_to_bottom_raw) { o_edge_idx = 2u; }
        else if (min_dist_raw == dist_to_left_raw) { o_edge_idx = 3u; }

        let o_w = o_width[o_edge_idx]; // この辺の個別のアウトライン太さ

        // アウトラインの内側半径と外側半径
        let r_inner = clamped_radius + vec4<f32>(o_offset);
        let r_outer = clamped_radius + vec4<f32>(o_offset + o_w);

        let dist_to_inner_o = sd_rounded_box(local_center, b + vec2<f32>(o_offset), r_inner);
        let dist_to_outer_o = sd_rounded_box(local_center, b + vec2<f32>(o_offset + o_w), r_outer);

        // ベースのアウトライン太さマスク
        var base_o_alpha = smoothstep(-0.5, 0.5, dist_to_inner_o) * (1.0 - smoothstep(-0.5, 0.5, dist_to_outer_o));

        // ビットフラグ（outline_flags）をデコード
        let o_flags = u32(instance.outline_offset_and_flags.y);
        let style_o = (o_flags >> (o_edge_idx * 4u)) & 3u;      // 0: Solid, 1: Dotted, 2: Dashed, 3: Double
        let o_alignment = (o_flags >> (o_edge_idx * 4u + 2u)) & 3u; // 0: Start, 1: End, 2: Center

        // アライメント基準点に基づく長さトリミング（o_lengths）の計算
        let o_lengths = instance.outline_lengths;
        var o_len_limit = 1.0;
        if (o_edge_idx == 0u) { o_len_limit = o_lengths.x; }
        else if (o_edge_idx == 1u) { o_len_limit = o_lengths.y; }
        else if (o_edge_idx == 2u) { o_len_limit = o_lengths.z; }
        else { o_len_limit = o_lengths.w; }

        // 辺沿いの進捗比率 t
        var t = 0.0;
        var o_pos_edge = 0.0;
        if (o_edge_idx == 0u || o_edge_idx == 2u) {
            t = (local_center.x + b.x) / (b.x * 2.0);
            o_pos_edge = local_center.x + b.x;
        } else {
            t = (local_center.y + b.y) / (b.y * 2.0);
            o_pos_edge = local_center.y + b.y;
        }

        let o_edge_fade = 0.5 / max(size_vec.x, size_vec.y);
        var o_length_alpha = 1.0;

        // 長さ制限が 1.0 (ほぼ100%) の場合は、外側への飛び出しによるカットをバイパスする
        if (o_len_limit < 0.999) {
            if (o_alignment == 0u) {
                o_length_alpha = 1.0 - smoothstep(o_len_limit - o_edge_fade, o_len_limit + o_edge_fade, t);
            } else if (o_alignment == 1u) {
                let lower_bound = 1.0 - o_len_limit;
                o_length_alpha = smoothstep(lower_bound - o_edge_fade, lower_bound + o_edge_fade, t);
            } else {
                let half_len = o_len_limit * 0.5;
                let dist_from_center = abs(t - 0.5);
                o_length_alpha = 1.0 - smoothstep(half_len - o_edge_fade, half_len + o_edge_fade, dist_from_center);
            }
        }

        // スタイル別の模様パターン（点線、破線、二重線）の適用
        var o_style_alpha = 1.0;

        // アウトラインの中心からの相対的な厚み方向の進捗割合 (0.0 -> 1.0)
        let o_thick_t = clamp(dist_to_inner_o / (dist_to_inner_o - dist_to_outer_o), 0.0, 1.0);

        if (style_o == 1u) {
            // Dotted (丸点)
            let period = o_w * 2.2;
            let cycle_t = fract(o_pos_edge / period) * period - (period * 0.5);
            let radial_dist = length(vec2<f32>(cycle_t, (o_thick_t - 0.5) * o_w));
            o_style_alpha = 1.0 - smoothstep(o_w * 0.4 - 0.5, o_w * 0.4 + 0.5, radial_dist);
        } else if (style_o == 2u) {
            // Dashed (破線)
            let period = o_w * 5.0;
            let cycle_t = fract(o_pos_edge / period) * period;
            o_style_alpha = 1.0 - smoothstep(o_w * 3.0 - 0.5, o_w * 3.0 + 0.5, cycle_t);
        } else if (style_o == 3u) {
            // Double (二重アウトライン)
            let is_double_void = smoothstep(0.30, 0.33, o_thick_t) * (1.0 - smoothstep(0.67, 0.70, o_thick_t));
            o_style_alpha = 1.0 - is_double_void;
        }

        // すべてのアンチエイリアスマスクを結合
        outline_alpha = base_o_alpha * o_length_alpha * o_style_alpha;

        // 選択された辺のアウトライン幅が 0.0 ならアルファを完全に消去
        outline_alpha = outline_alpha * clamp(o_w, 0.0, 1.0);
    }

    // 背景色・グラデーション・サンプリングの取得
    var element_color = vec4<f32>(0.0);

    if (mode == 2.0) {
        // テキスト描画（R8Unormアトラスフォントの Alpha サンプリング、PMA着色）
        let tex_color = textureSample(t_texture, s_sampler, in.uv);
        let alpha = tex_color.r * opacity;
        // let raw_alpha = pow(tex_color.r, 0.75); 
        // let alpha = raw_alpha * opacity;
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
            let angle = instance.gradient_angle_and_origin.x;
            let dir = vec2<f32>(cos(angle), sin(angle));
            let proj = dot(local_center, dir) / (length(size_vec) * 0.5);
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

    // 本体（背景 + 枠線）の物理アルファ付きカラーを算出
    var base_with_outline = element_color * box_alpha;

    // 本体の外枠領域（1.0 - base_with_outline.a）に対して、アウトラインを重ねて合成
    if (has_outline) {
        let o_pma = vec4<f32>(outline_color_linear.rgb * outline_color_linear.a * opacity, outline_color_linear.a * opacity);
        base_with_outline = base_with_outline + o_pma * outline_alpha * (1.0 - base_with_outline.a);
    }

    // 最下層に位置する影（shadow_out）の上に重ねて Premultiplied Alpha 合成を完成
    final_color = base_with_outline + shadow_out * (1.0 - base_with_outline.a);

    return final_color;
}
