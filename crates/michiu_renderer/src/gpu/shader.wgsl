struct GlobalConfig {
    screen_size: vec2<f32>,
    scale: f32,
    padding: f32,
};

@group(0) @binding(0) var<uniform> config: GlobalConfig;

@group(0) @binding(1) var t_mask: texture_2d<f32>;      // 通常文字 (R8Unorm) / 外部テクスチャ
@group(0) @binding(2) var t_color: texture_2d<f32>;     // 絵文字 (Rgba8UnormSrgb)
@group(0) @binding(3) var t_user_mask: texture_2d<f32>; // ユーザーアイコン
@group(0) @binding(4) var s_sampler: sampler;

struct LinearGradientInner {
    colors: array<vec4<f32>, 4>, // 64B (align 16, stride 16)
    stops: vec4<f32>,            // 16B
    angle: f32,                  // 4B
    stop_count: u32,             // 4B
    origin: vec2<f32>,           // 8B
};

struct InstanceData {
    rect: vec4<f32>,
    transform_0: vec4<f32>,
    transform_1: vec4<f32>,
    transform_2: vec4<f32>,
    color: vec4<f32>,
    corner_radius: vec4<f32>,
    border_width: vec4<f32>,
    border_color: vec4<f32>,
    opacity_mode_sizing_border: vec4<f32>,
    uv_range: vec4<f32>,
    gradient: LinearGradientInner,
    transform_origin: vec2<f32>,
    is_color_atlas_pad: vec2<f32>,
    shadow_color: vec4<f32>,
    shadow_params: vec4<f32>,
    border_lengths: vec4<f32>,
    outline_width: vec4<f32>,
    outline_color: vec4<f32>,
    outline_lengths: vec4<f32>,
    outline_offset_and_flags: vec4<f32>,
    alpha_mode_y_flip_srgb_gamma: vec4<f32>,
};

@group(0) @binding(5) var<storage, read> instances: array<InstanceData>;

struct VertexInput {
    @location(0)
    position: vec2<f32>,
};

struct VertexOutput {
    @builtin(position)
    clip_position: vec4<f32>,
    @location(0)
    local_pos: vec2<f32>,
    @location(1)
    uv: vec2<f32>,
    @location(2) @interpolate(flat)
    instance_idx: u32,
};

// opacity_mode_sizing_border.y
// 3.0 忘れ
const MODE_DECORATION_THRESHOLD: f32 = -0.5; // < -0.5: 装飾・キャレット
const MODE_LINEAR_GRADIENT: f32 = 1.0;
const MODE_ATLAS: f32 = 2.0;
const MODE_EXTERNAL_TEXTURE: f32 = 4.0;
const MODE_FOUR_CORNER_GRADIENT: f32 = 5.0;

// is_color_atlas_pad.x
const ATLAS_COLOR_EMOJI: f32 = 0.5; // < 0.5 (0.0)
const ATLAS_TEXT_MASK: f32 = 1.5; // < 1.5 (1.0) (>= 1.5 はユーザーアイコン(2.0))

// Edge indices
const EDGE_TOP: u32 = 0u;
const EDGE_RIGHT: u32 = 1u;
const EDGE_BOTTOM: u32 = 2u;
const EDGE_LEFT: u32 = 3u;

// Stroke styles
const STROKE_SOLID: u32 = 0u;
const STROKE_DOTTED: u32 = 1u;
const STROKE_DASHED: u32 = 2u;
const STROKE_DOUBLE: u32 = 3u;

// Stroke alignments
const ALIGN_START: u32 = 0u;
const ALIGN_END: u32 = 1u;
const ALIGN_CENTER: u32 = 2u;

const INVALID_EDGE_DIST: f32 = 10000.0;

fn sd_rounded_box(p: vec2<f32>, b: vec2<f32>, r: vec4<f32>) -> f32 {
    // 象限に応じて角丸半径 [tl, tr, br, bl] を選択
    var rad = r.x; // top_left (x < 0, y < 0)
    if p.x >= 0.0 && p.y < 0.0 { rad = r.y; } // top_right
    if p.x >= 0.0 && p.y >= 0.0 { rad = r.z; } // bottom_right
    if p.x < 0.0 && p.y >= 0.0 { rad = r.w; } // bottom_left

    let q = abs(p) - b + vec2<f32>(rad);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - rad;
}

fn srgb_to_linear_scalar(c: f32) -> f32 {
    let clamped = max(c, 0.0);
    return select(
        pow((clamped + 0.055) / 1.055, 2.4),
        clamped / 12.92,
        clamped <= 0.04045
    );
}

fn srgb_to_linear(srgb: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(
        srgb_to_linear_scalar(srgb.r),
        srgb_to_linear_scalar(srgb.g),
        srgb_to_linear_scalar(srgb.b),
        srgb.a
    );
}

fn to_pma(linear_color: vec4<f32>, opacity: f32) -> vec4<f32> {
    let a = linear_color.a * opacity;
    return vec4<f32>(linear_color.rgb * a, a);
}

fn compute_vertex_margin(instance: InstanceData) -> f32 {
    var margin = 0.0;

    // BoxShadowによる頂点拡張
    if instance.shadow_color.a > 0.0 {
        let offset = instance.shadow_params.xy;
        let blur = instance.shadow_params.z;
        let spread = instance.shadow_params.w;
        margin = max(0.0, spread) + blur * 3.0 + max(abs(offset.x), abs(offset.y));
    }

    // アウトラインによる頂点拡張
    let o_width = instance.outline_width;
    let o_offset = instance.outline_offset_and_flags.x;
    let total_o_width = o_width.x + o_width.y + o_width.z + o_width.w;
    if instance.outline_color.a > 0.0 && total_o_width > 0.0 {
        let max_o_width = max(max(o_width.x, o_width.y), max(o_width.z, o_width.w));
        let outline_margin = max_o_width + max(0.0, o_offset);
        margin = max(margin, outline_margin + 1.5); // 1.5px はアンチエイリアス用余白
    }

    return margin;
}

fn build_transform_matrix(instance: InstanceData) -> mat4x4<f32> {
    return mat4x4<f32>(
        instance.transform_0,
        instance.transform_1,
        vec4<f32>(0.0, 0.0, 1.0, 0.0), // Z軸復元
        instance.transform_2// 平行移動部
    );
}

fn compute_uv(instance: InstanceData, local_pixel: vec2<f32>, size: vec2<f32>) -> vec2<f32> {
    let local_ratio = local_pixel / size;
    var uv = mix(instance.uv_range.xy, instance.uv_range.zw, local_ratio);

    let mode = instance.opacity_mode_sizing_border.y;
    let y_flip = instance.alpha_mode_y_flip_srgb_gamma.y;
    if mode == MODE_EXTERNAL_TEXTURE && y_flip < 0.0 {
        uv.y = 1.0 - uv.y;
    }

    return uv;
}

@vertex
fn vs_main(vertex: VertexInput, @builtin(instance_index) instance_idx: u32) -> VertexOutput {
    let instance = instances[instance_idx];
    let size = instance.rect.zw;
    let margin = compute_vertex_margin(instance);

    // [0, 1] の頂点を [-margin, size + margin] の物理ローカル座標へ拡張
    let local_pixel = mix(vec2<f32>(-margin), size + vec2<f32>(margin), vertex.position);

    // Transform Origin を基準に行列変換
    let origin_pixel = instance.transform_origin * size;
    let transform = build_transform_matrix(instance);
    let pos_centered = local_pixel - origin_pixel;
    let pos_transformed = (transform * vec4<f32>(pos_centered, 0.0, 1.0)).xy + origin_pixel;

    // 親ウィンドウ上の物理座標 -> DComp 用 NDC（Y軸下向き正）へ投影
    let abs_phys_pos = (instance.rect.xy + pos_transformed) * config.scale;
    let ndc_x = (abs_phys_pos.x / config.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (abs_phys_pos.y / config.screen_size.y) * 2.0;

    var out: VertexOutput;
    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.local_pos = local_pixel;
    out.uv = compute_uv(instance, local_pixel, size);
    out.instance_idx = instance_idx;
    return out;
}

fn select_closest_edge(
    local_center: vec2<f32>,
    half_size: vec2<f32>,
    widths: vec4<f32>,
    skip_zero_width: bool,
) -> u32 {
    let raw_top = local_center.y + half_size.y;
    let raw_right = half_size.x - local_center.x;
    let raw_bottom = half_size.y - local_center.y;
    let raw_left = local_center.x + half_size.x;

    let d_top = select(INVALID_EDGE_DIST, raw_top, !skip_zero_width || widths.x > 0.0);
    let d_right = select(INVALID_EDGE_DIST, raw_right, !skip_zero_width || widths.y > 0.0);
    let d_bottom = select(INVALID_EDGE_DIST, raw_bottom, !skip_zero_width || widths.z > 0.0);
    let d_left = select(INVALID_EDGE_DIST, raw_left, !skip_zero_width || widths.w > 0.0);

    let min_dist = min(min(d_top, d_bottom), min(d_left, d_right));

    var edge_idx = EDGE_TOP;
    if min_dist == d_right { edge_idx = EDGE_RIGHT; }
    else if min_dist == d_bottom { edge_idx = EDGE_BOTTOM; }
    else if min_dist == d_left { edge_idx = EDGE_LEFT; }

    return edge_idx;
}

fn compute_length_alpha(t: f32, len_limit: f32, alignment: u32, edge_fade: f32) -> f32 {
    if len_limit >= 0.999 {
        return 1.0;
    }

    switch alignment {
        case ALIGN_START: {
            return 1.0 - smoothstep(len_limit - edge_fade, len_limit + edge_fade, t);
        }
        case ALIGN_END: {
            let lower_bound = 1.0 - len_limit;
            return smoothstep(lower_bound - edge_fade, lower_bound + edge_fade, t);
        }
        default: { // ALIGN_CENTER
            let half_len = len_limit * 0.5;
            let dist_from_center = abs(t - 0.5);
            return 1.0 - smoothstep(half_len - edge_fade, half_len + edge_fade, dist_from_center);
        }
    }
}

fn compute_style_alpha(style: u32, w: f32, pos_edge: f32, thick_t: f32) -> f32 {
    switch style {
        case STROKE_DOTTED: {
            let period = w * 2.2;
            let cycle_t = fract(pos_edge / period) * period - (period * 0.5);
            let radial_dist = length(vec2<f32>(cycle_t, (thick_t - 0.5) * w));
            return 1.0 - smoothstep(w * 0.4 - 0.5, w * 0.4 + 0.5, radial_dist);
        }
        case STROKE_DASHED: {
            let period = w * 5.0; // 3w 長さ、2w 隙間
            let cycle_t = fract(pos_edge / period) * period;
            return 1.0 - smoothstep(w * 3.0 - 0.5, w * 3.0 + 0.5, cycle_t);
        }
        case STROKE_DOUBLE: {
            // 外側 (0.0 ~ 0.33) と 内側 (0.67 ~ 1.0) のみ残す
            let is_double_void = smoothstep(0.30, 0.33, thick_t) * (1.0 - smoothstep(0.67, 0.70, thick_t));
            return 1.0 - is_double_void;
        }
        default: { // STROKE_SOLID
            return 1.0;
        }
    }
}

fn compute_stroke_pattern_alpha(
    local_center: vec2<f32>,
    half_size: vec2<f32>,
    size: vec2<f32>,
    edge_idx: u32,
    w: f32,
    flags: u32,
    len_limit: f32,
    thick_t: f32,
) -> f32 {
    let style = (flags >> (edge_idx * 4u)) & 3u;
    let alignment = (flags >> (edge_idx * 4u + 2u)) & 3u;

    var pos_edge = 0.0;
    var t = 0.0;
    if edge_idx == EDGE_TOP || edge_idx == EDGE_BOTTOM {
        pos_edge = local_center.x + half_size.x;
        t = pos_edge / (half_size.x * 2.0);
    } else {
        pos_edge = local_center.y + half_size.y;
        t = pos_edge / (half_size.y * 2.0);
    }

    let edge_fade = 0.5 / max(size.x, size.y);
    let length_alpha = compute_length_alpha(t, len_limit, alignment, edge_fade);
    let style_alpha = compute_style_alpha(style, w, pos_edge, thick_t);

    return length_alpha * style_alpha;
}

fn compute_border_alpha(
    instance: InstanceData,
    local_center: vec2<f32>,
    half_size: vec2<f32>,
    size: vec2<f32>,
    dist_to_box: f32,
    box_alpha: f32,
) -> f32 {
    let b_width = instance.border_width;
    let total_width = b_width.x + b_width.y + b_width.z + b_width.w;
    if instance.border_color.a <= 0.0 || total_width <= 0.0 {
        return 0.0;
    }

    // Taffy のレイアウトに合わせ枠線は常に外枠の内側に配置
    let border_center_shift = vec2<f32>(
        (b_width.w - b_width.y) * 0.5,
        (b_width.x - b_width.z) * 0.5
    );
    let inner_b = half_size - vec2<f32>(
        (b_width.w + b_width.y) * 0.5,
        (b_width.x + b_width.z) * 0.5
    );

    let max_radius = min(size.x, size.y) * 0.5;
    let clamped_radius = min(instance.corner_radius, vec4<f32>(max_radius));

    let inner_pos = local_center - border_center_shift;
    let inner_radius = max(clamped_radius - b_width, vec4<f32>(0.0));
    let dist_to_inner = sd_rounded_box(inner_pos, inner_b, inner_radius);

    let edge_idx = select_closest_edge(local_center, half_size, b_width, true);
    let w = b_width[edge_idx];
    let thick_t = clamp(-dist_to_box / (-dist_to_box + dist_to_inner), 0.0, 1.0);

    let pattern_alpha = compute_stroke_pattern_alpha(
        local_center,
        half_size,
        size,
        edge_idx,
        w,
        u32(instance.opacity_mode_sizing_border.w),
        instance.border_lengths[edge_idx],
        thick_t
    );

    let dist_to_inner_clamped = min(dist_to_inner, dist_to_box + w);
    let raw_alpha = box_alpha * smoothstep(-0.5, 0.5, dist_to_inner_clamped) * pattern_alpha;

    // 太さ 0.0 の辺で残留ノイズを完全に消去
    return raw_alpha * clamp(w, 0.0, 1.0);
}

fn compute_outline_alpha(
    instance: InstanceData,
    local_center: vec2<f32>,
    half_size: vec2<f32>,
    size: vec2<f32>,
    clamped_radius: vec4<f32>,
) -> f32 {
    let o_width = instance.outline_width;
    let total_width = o_width.x + o_width.y + o_width.z + o_width.w;
    if instance.outline_color.a <= 0.0 || total_width <= 0.0 {
        return 0.0;
    }

    // アウトラインは太さに関わらず純粋な物理距離で最も近い辺を特定
    let edge_idx = select_closest_edge(local_center, half_size, o_width, false);
    let o_w = o_width[edge_idx];
    let o_offset = instance.outline_offset_and_flags.x;

    let r_inner = clamped_radius + vec4<f32>(o_offset);
    let r_outer = clamped_radius + vec4<f32>(o_offset + o_w);

    let dist_to_inner = sd_rounded_box(local_center, half_size + vec2<f32>(o_offset), r_inner);
    let dist_to_outer = sd_rounded_box(local_center, half_size + vec2<f32>(o_offset + o_w), r_outer);

    let base_alpha = smoothstep(-0.5, 0.5, dist_to_inner) * (1.0 - smoothstep(-0.5, 0.5, dist_to_outer));
    let thick_t = clamp(dist_to_inner / (dist_to_inner - dist_to_outer), 0.0, 1.0);

    let pattern_alpha = compute_stroke_pattern_alpha(
        local_center,
        half_size,
        size,
        edge_idx,
        o_w,
        u32(instance.outline_offset_and_flags.y),
        instance.outline_lengths[edge_idx],
        thick_t
    );

    return base_alpha * pattern_alpha * clamp(o_w, 0.0, 1.0);
}

fn sample_linear_gradient(grad: LinearGradientInner, local_pos: vec2<f32>, size: vec2<f32>) -> vec4<f32> {
    // CSS 角度: 0deg = 上 (0, -1), 90deg = 右 (1, 0)
    let dir = vec2<f32>(sin(grad.angle), -cos(grad.angle));
    let grad_length = abs(size.x * dir.x) + abs(size.y * dir.y);
    let proj = dot(local_pos, dir);
    let t = clamp((proj / grad_length) + 0.5, 0.0, 1.0);

    if t <= grad.stops[0] || grad.stop_count <= 1u {
        return srgb_to_linear(grad.colors[0]);
    }

    for (var i = 0u; i < grad.stop_count - 1u; i = i + 1u) {
        let s0 = grad.stops[i];
        let s1 = grad.stops[i + 1u];
        if t >= s0 && t <= s1 {
            let factor = select(0.0, (t - s0) / (s1 - s0), s1 > s0);
            let c0 = srgb_to_linear(grad.colors[i]);
            let c1 = srgb_to_linear(grad.colors[i + 1u]);
            return mix(c0, c1, factor);
        }
    }

    return srgb_to_linear(grad.colors[grad.stop_count - 1u]);
}

fn sample_four_corner_gradient(grad: LinearGradientInner, local_pos: vec2<f32>, size: vec2<f32>) -> vec4<f32> {
    let uv = clamp(local_pos / size, vec2<f32>(0.0), vec2<f32>(1.0));

    // origin の位置を中間点 0.5 にするよう UV を区分線形変換 (ゼロ除算防止)
    let ox = clamp(grad.origin.x, 0.001, 0.999);
    let oy = clamp(grad.origin.y, 0.001, 0.999);

    let u = select(0.5 + 0.5 * ((uv.x - ox) / (1.0 - ox)), 0.5 * (uv.x / ox), uv.x < ox);
    let v = select(0.5 + 0.5 * ((uv.y - oy) / (1.0 - oy)), 0.5 * (uv.y / oy), uv.y < oy);

    let top = mix(srgb_to_linear(grad.colors[0]), srgb_to_linear(grad.colors[1]), u);
    let bottom = mix(srgb_to_linear(grad.colors[2]), srgb_to_linear(grad.colors[3]), u);

    return mix(top, bottom, v);
}

fn compute_entity_color(
    instance: InstanceData,
    uv: vec2<f32>,
    local_pos: vec2<f32>,
    size: vec2<f32>,
    color_linear: vec4<f32>,
    opacity: f32,
) -> vec4<f32> {
    let mode = instance.opacity_mode_sizing_border.y;

    if mode == MODE_ATLAS {
        let atlas_type = instance.is_color_atlas_pad.x;
        if atlas_type < ATLAS_COLOR_EMOJI {
            let tex_color = textureSample(t_color, s_sampler, uv);
            let alpha = tex_color.a * opacity;
            return vec4<f32>(tex_color.rgb * alpha, alpha);
        } else if atlas_type < ATLAS_TEXT_MASK {
            let alpha = textureSample(t_mask, s_sampler, uv).r * opacity;
            return vec4<f32>(color_linear.rgb * alpha, alpha);
        } else {
            let alpha = textureSample(t_user_mask, s_sampler, uv).r * opacity;
            return vec4<f32>(color_linear.rgb * alpha, alpha);
        }
    }

    if mode == MODE_EXTERNAL_TEXTURE {
        var tex_color = textureSample(t_mask, s_sampler, uv);
        let alpha_mode = instance.alpha_mode_y_flip_srgb_gamma.x; // 0.0: Straight, 1.0: Premultiplied
        let srgb_format = instance.alpha_mode_y_flip_srgb_gamma.z; // 0.0: 手動デガンマ, 1.0: ハード自動変換済み
        let opacity_gamma = instance.alpha_mode_y_flip_srgb_gamma.w;

        if srgb_format < 0.5 {
            tex_color = srgb_to_linear(tex_color);
        }

        let linear_opacity = pow(opacity, opacity_gamma);
        if alpha_mode < 0.5 {
            return to_pma(tex_color, linear_opacity);
        } else {
            return tex_color * linear_opacity;
        }
    }

    // 通常（Solid / グラデーション）
    var base_color = color_linear;
    if mode == MODE_LINEAR_GRADIENT {
        let grad_center = local_pos - (instance.gradient.origin * size);
        base_color = sample_linear_gradient(instance.gradient, grad_center, size);
    } else if mode == MODE_FOUR_CORNER_GRADIENT {
        base_color = sample_four_corner_gradient(instance.gradient, local_pos, size);
    }

    return to_pma(base_color, opacity);
}

fn compute_shadow(
    instance: InstanceData,
    shadow_color_linear: vec4<f32>,
    local_center: vec2<f32>,
    half_size: vec2<f32>,
    dist_to_box: f32,
    opacity: f32,
) -> vec4<f32> {
    if shadow_color_linear.a <= 0.0 {
        return vec4<f32>(0.0);
    }

    let offset = instance.shadow_params.xy;
    let blur = instance.shadow_params.z;
    let spread = instance.shadow_params.w;

    let size = instance.rect.zw;
    let max_radius = min(size.x, size.y) * 0.5;
    let clamped_radius = min(instance.corner_radius, vec4<f32>(max_radius));

    let shadow_pos = local_center - offset;
    let shadow_b = half_size + vec2<f32>(spread);
    let shadow_radius = clamped_radius + vec4<f32>(spread);
    let dist_to_shadow = sd_rounded_box(shadow_pos, shadow_b, shadow_radius);

    var shadow_alpha = 0.0;
    if blur > 0.0 {
        shadow_alpha = 1.0 - smoothstep(-blur, blur, dist_to_shadow);
    } else {
        shadow_alpha = select(0.0, 1.0, dist_to_shadow <= 0.0);
    }

    // 本体境界（-1.0px ～ 0.0px）で影をフェードアウト
    let shadow_gate = smoothstep(-1.0, 0.0, dist_to_box);
    return to_pma(shadow_color_linear, shadow_alpha * shadow_gate * opacity);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let instance = instances[in.instance_idx];

    let opacity = instance.opacity_mode_sizing_border.x;
    let mode = instance.opacity_mode_sizing_border.y;

    let size = instance.rect.zw;
    let half_size = size * 0.5;
    let local_center = in.local_pos - half_size;

    // カラーの一斉デガンマ
    let color_linear = srgb_to_linear(instance.color);
    let border_color_linear = srgb_to_linear(instance.border_color);
    let outline_color_linear = srgb_to_linear(instance.outline_color);
    let shadow_color_linear = srgb_to_linear(instance.shadow_color);

    // 本体と丸角の SDF
    let max_radius = min(size.x, size.y) * 0.5;
    let clamped_radius = min(instance.corner_radius, vec4<f32>(max_radius));
    let dist_to_box = sd_rounded_box(local_center, half_size, clamped_radius);

    var box_alpha = 1.0 - smoothstep(-0.5, 0.5, dist_to_box);
    if mode < MODE_DECORATION_THRESHOLD {
        // 装飾・キャレットモードは AA 減衰をバイパスしてクッキリ描画
        box_alpha = 1.0;
    }

    // 各要素の計算
    let shadow_out = compute_shadow(instance, shadow_color_linear, local_center, half_size, dist_to_box, opacity);
    let border_alpha = compute_border_alpha(instance, local_center, half_size, size, dist_to_box, box_alpha);
    let outline_alpha = compute_outline_alpha(instance, local_center, half_size, size, clamped_radius);
    var entity_color = compute_entity_color(instance, in.uv, in.local_pos, size, color_linear, opacity);

    // 枠線を本体にブレンド
    if border_alpha > 0.0 {
        let b_pma = to_pma(border_color_linear, opacity);
        entity_color = mix(entity_color, b_pma, border_alpha);
    }

    // 本体をボックス形状でクリップし、外側にアウトラインを重ねる
    var base_with_outline = entity_color * box_alpha;
    if outline_alpha > 0.0 {
        let o_pma = to_pma(outline_color_linear, opacity);
        base_with_outline = base_with_outline + o_pma * outline_alpha * (1.0 - base_with_outline.a);
    }

    // 最下層の影の上に Premultiplied Alpha 合成
    return base_with_outline + shadow_out * (1.0 - base_with_outline.a);
}
