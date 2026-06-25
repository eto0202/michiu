struct GlobalConfig {
    screen_size: vec2<f32>,
    scale: f32,
    padding: f32,
};

@group(0) @binding(0) var<uniform> config: GlobalConfig;

// Rust 側の binding: 1 (atlas.view) と同期
@group(0) @binding(1) var t_atlas: texture_2d<f32>;

// Rust 側の binding: 2 (atlas.sampler) と同期
@group(0) @binding(2) var s_atlas: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct InstanceInput {
    @location(1) rect: vec4<f32>, // x, y, w, h
    @location(2) m0: vec4<f32>, // transform matrix...
    @location(3) m1: vec4<f32>,
    @location(4) m2: vec4<f32>,
    @location(5) m3: vec4<f32>,
    @location(6) color: vec4<f32>, // bg_color
    @location(7) corner_radius: vec4<f32>, // tl, tr, br, bl
    @location(8) border_width: vec4<f32>, // t, r, b, l
    @location(9) border_color: vec4<f32>,
    @location(10) opacity_and_mode: vec4<f32>,
    @location(11) uv_range: vec4<f32>, // uv_min(xy) と uv_max(zw) が入っている
    @location(12) gradient_end_color: vec4<f32>,
    @location(13) gradient_angle_and_origin: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) bg_color: vec4<f32>,
    @location(1) border_color: vec4<f32>,
    @location(2) local_pos: vec2<f32>,

    @location(3) size: vec2<f32>,
    @location(4) corner_radius: vec4<f32>,
    @location(5) border_width: vec4<f32>,
    @location(6) opacity: f32,

    @location(7) uv: vec2<f32>,

    // mode を u32 に変更してフラット補間
    @location(8) @interpolate(flat) mode: u32,
    @location(9) gradient_end_color: vec4<f32>,
    @location(10) gradient_angle: f32,
};

@vertex
fn vs_main(model: VertexInput, instance: InstanceInput) -> VertexOutput {
    let transform = mat4x4<f32>(instance.m0, instance.m1, instance.m2, instance.m3);

    // パックされたデータから実値を取り出す
    let uv_min = instance.uv_range.xy;
    let uv_max = instance.uv_range.zw;

    let gradient_angle = instance.gradient_angle_and_origin.x;
    let transform_origin = instance.gradient_angle_and_origin.yz;

    // 基準点の物理オフセットを計算
    let origin_offset = transform_origin * instance.rect.zw;

    // 頂点座標を「基準点 (transform_origin)」を原点とするローカル空間に変換
    let local_origin_pos = (model.position - transform_origin) * instance.rect.zw;

    // ローカル空間でトランスフォームを適用
    let local_transformed = transform * vec4<f32>(local_origin_pos, 0.0, 1.0);

    // 絶対座標に復元しつつスクリーン座標に配置
    let world_pos = vec4<f32>(instance.rect.xy + origin_offset + local_transformed.xy, 0.0, 1.0);

    // 1x1のモデル座標を矩形サイズに合わせる
    let local_pos = model.position * instance.rect.zw;

    // スクリーン座標 (0..width, 0..height) を NDC (-1..1) に変換
    // Y座標は上がプラスになるよう反転
    let nx = (world_pos.x / config.screen_size.x) * 2.0 - 1.0;
    let ny = 1.0 - (world_pos.y / config.screen_size.y) * 2.0;

    let opacity = instance.opacity_and_mode.x;
    // 安全に u32 へ丸めキャスト
    let mode = u32(instance.opacity_and_mode.y + 0.5);

    var out: VertexOutput;
    out.clip_position = vec4<f32>(nx, ny, 0.0, 1.0);
    out.bg_color = instance.color;
    out.border_color = instance.border_color;
    out.local_pos = local_pos;
    out.size = instance.rect.zw;
    out.corner_radius = instance.corner_radius;
    out.border_width = instance.border_width;
    out.opacity = opacity;
    out.uv = mix(uv_min, uv_max, model.position);
    out.mode = mode;
    out.gradient_end_color = instance.gradient_end_color;
    out.gradient_angle = gradient_angle;
    return out;
}

fn sd_rounded_box(p: vec2<f32>, b: vec2<f32>, r: vec4<f32>) -> f32 {
    // select組み込み関数を用いて分岐なしで角の半径を決定
    // select(false_value, true_value, condition)
    let rad = select(
        select(r.x, r.y, p.x > 0.0), // y <= 0.0 (Top)
        select(r.w, r.z, p.x > 0.0), // y >  0.0 (Bottom)
        p.y > 0.0
    );

    let q = abs(p) - b + rad;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - rad;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 座標系を中心基準に変換 (-size/2 ～ +size/2)
    let half_size = in.size * 0.5;
    let p = in.local_pos - half_size;

    // 外側の境界 (Outer Edge)
    let d_outer = sd_rounded_box(p, half_size, in.corner_radius);

    // アンチエイリアス（1ピクセル幅で滑らかにする）
    let edge_softness = max(fwidth(d_outer), 0.0001);
    let outer_mask = 1.0 - smoothstep(-edge_softness, edge_softness, d_outer);

    if (outer_mask <= 0.0) { discard; }

    var base_color: vec4<f32>;

    // モード判定を switch 文に移行
    switch (in.mode) {
        case 1u: { // Gradient Mode
            // ピクセルのローカル正規化座標 (0..1)
            let uv = in.local_pos / in.size;

            // 角度に応じたグラデーションのブレンド比率(t)を射影計算
            let angle_cos = cos(in.gradient_angle);
            let angle_sin = sin(in.gradient_angle);

            // 回転軸へ投影して 0.0 ～ 1.0 にクランプ
            let t = clamp(uv.x * angle_cos + uv.y * angle_sin, 0.0, 1.0);

            // 2つの色を線形補間
            base_color = mix(in.bg_color, in.gradient_end_color, t);
        }
        default: { // Solid Mode (0u などのその他のデフォルト)
            base_color = in.bg_color;
        }
    }

    var final_color: vec3<f32>;
    var final_alpha: f32;

    if (in.mode == 2u) { // Text Mode
        // 安全のために mipmap level 0 を指定して非一様制御フロー内でも正しくサンプリング
        let text_alpha = textureSampleLevel(t_atlas, s_atlas, in.uv, 0.0).a;

        // テキスト色にマスクを適用して PMA 化
        let mask = text_alpha * outer_mask;
        final_color = in.bg_color.rgb * in.bg_color.a * mask;
        final_alpha = in.bg_color.a * mask;
    } else if (in.mode == 3u) { // 静止 WebView2 サンプリング
        // DCompのWebView2からキャプチャしたRGBAカラーテクスチャをサンプリング
        // 非一様制御フロー内のため、textureSampleLevel(..., 0.0) を使用してサンプリングエラーを防ぎます
        let webview_color = textureSampleLevel(t_atlas, s_atlas, in.uv, 0.0);

        // 透過角丸（outer_mask）を乗算して綺麗に PMA 合成
        final_color = webview_color.rgb * outer_mask;
        final_alpha = webview_color.a * outer_mask;
    } else {
        // 通常の背景 ＆ 枠線（ボーダー）の描画
        let b_width = in.border_width.x; // Topを基準とする

        if (b_width > 0.0) {
            let d_inner = d_outer + b_width;
            let inner_mask = 1.0 - smoothstep(-edge_softness, edge_softness, d_inner);

            // 外側と内側の引き算ではなく、エッジの中心線からの絶対距離（abs）を用いて描画します。
            // これにより、どれだけ枠線が太くても、内側の丸みが角ばるバグが完全に解決され、滑らかな曲線になります。
            let d_border = abs(d_outer + b_width * 0.5) - b_width * 0.5;
            let border_mask = 1.0 - smoothstep(-edge_softness, edge_softness, d_border);

            // 背景（内側）のマスク
            let bg_mask = 1.0 - smoothstep(-edge_softness, edge_softness, d_outer + b_width);

            let c_bg = base_color.rgb * base_color.a;
            let c_border = in.border_color.rgb * in.border_color.a;

            // PMA 状態を維持しながら重ね合わせる
            final_color = c_bg * bg_mask + c_border * border_mask;
            final_alpha = base_color.a * bg_mask + in.border_color.a * border_mask;
        } else {
            // 単色の背景をマスクで PMA 化
            final_color = base_color.rgb * base_color.a * outer_mask;
            final_alpha = base_color.a * outer_mask;
        }
    }

    // グローバル不透明度の適用（すでに final_color / final_alpha は PMA 化されています）
    return vec4<f32>(final_color * in.opacity, final_alpha * in.opacity);
}
