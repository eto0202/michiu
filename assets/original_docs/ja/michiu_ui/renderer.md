# レンダリングアーキテクチャと合成パイプライン

[`MichiuRenderer`] は、 wgpu (DirectX 12) と Windows のハードウェア合成エンジン DirectComposition を統合したハイブリッドレンダラーです。

通常のUI描画にとどまらず、動画プレイヤーや画像、外部プロセスが生成したネイティブの `IDCompositionVisual2`（[`ExternalVisual`](crate::ExternalVisual)）を、ゼロコピーでUIとシームレスに重ね合わせる独自の合成パイプラインを備えています。

---

## なぜこの構成なのか？（2レイヤー ＋ パンチアウト合成）

`ExternalVisual` とUIを重ね合わせる際、DirectComposition には以下の技術的制約が存在します。

1.  **3レイヤー（サンドイッチ構造）の同期ズレ**:
    `[背景 (wgpu描画)]` → `[ExternalVisual]` → `[UI (wgpu描画)]` の3つのVisualを個別に管理すると、ウィンドウリサイズ時などに各サーフェスのコミットタイミングがズレ、目に見える同期ズレが発生します。
2.  **デスクトップ透過問題**:
    レイヤー数を減らすために背景を無くしサーフェスをアルファ透過させると、ウィンドウの背景を通り越して OSのデスクトップ（壁紙や他アプリ）まで透けてしまうという DirectComposition 特有の挙動が存在します。

この2つの課題を克服するため、`michiu_ui` では「2レイヤー構成 ＋ パンチアウト ＋ 減算ブレンド」 というアプローチを採用しています。

### レイヤー断面図

```text
[ 視点（ユーザー） ]
       │
       ▼
┌─────────────────────────────────────────────────────────┐
│ 前面レイヤー (wgpu / DirectX 12)                          │
│  - UI要素の描画                                           │
│  - ExternalVisual 領域をパンチアウト                       │
│  - 減算ブレンドによる前面からの背景色合成                      │
└─────────────────────────┬───────────────────────────────┘
                          │ (穴から覗く)
                          ▼
┌─────────────────────────────────────────────────────────┐
│ 背面レイヤー (DirectComposition)                          │
│  - ExternalVisual (IDCompositionVisual2)                │
│  - 不透明な背景色（デスクトップの透過を防止）                   │
└─────────────────────────────────────────────────────────┘
```

### SDFマスクによる前面パンチアウト

`ExternalVisual`をUI背面に配置する場合、前面の wgpu レイヤーから該当する矩形領域を物理的にくり抜く必要があります。

`michiu_ui` では、ステンシルバッファや別レンダーパスの追加によるオーバーヘッドを避け、
SDFマスクと特殊なブレンドステートの乗算によって、角丸を含む任意の形状で高速かつアンチエイリアス付きの穴あけを行っています。

```rust
// パンチアウト実行時のブレンドステート
blend: Some(wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        // アルファが 0.0 に消去される領域ではカラー値も同時に 0.0 になるよう、
        // (1.0 - SrcAlpha) を乗算して元の背景色を削り取る（減衰消去）
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, // アルファを SDF マスク分だけ消去
        operation: wgpu::BlendOperation::Add,
    },
}),
```

#### 数学的メカニズム

このブレンドステートでは、ピクセルの計算結果は常に以下の数式に従います。

`Result = Dst * (1.0 - Src.Alpha)`

- **くり抜く領域 (`Src.Alpha = 1.0`)**:
  `Dst * 0.0 = 0.0` となり、前面のカラーとアルファが完全にクリアされ、背面の `ExternalVisual` が露出します。
- **くり抜かない領域 (`Src.Alpha = 0.0`)**:
  `Dst * 1.0 = Dst` となり、既存のUI描画がそのまま維持されます。
- **角丸などのエッジ境界 (`0.0 < Src.Alpha < 1.0`)**:
  SDFから出力されるアンチエイリアス値に応じてカラーとアルファが滑らかにフェードアウトし、ジャギーのない美しい境界線でハードウェア合成されます。

---

# `WgpuRenderer`

wgpu (DirectX 12) をバックエンドとする、`michiu_ui` のコアUIレンダラーです。
共通の 1x1 矩形メッシュを用いたインスタンシング描画と、通常描画/パンチアウトのバッチパイプラインによって、最小限のドローコールでレンダリングを実現します。

---

## レンダリングパイプラインの全体フロー ([`WgpuRenderer::render`])

1フレームのレンダリングは、大まかに以下の7つのステップで一直線に実行されます。

```text
[ Context ]
     │ 1. collect_render_data
     ▼
[ RenderData ] ─── 2. build_quad_instance_for_entity ───► [ instance_staging ]
                                                                │ 3. queue.write_buffer
┌───────────────────────────────────────────────────────────────┘
▼
[ VRAM Instance Buffer ]
     │
     │ 4. begin_render_pass (Bind 1x1 Quad & Config Uniform)
     │
     │ 5. バッチループ (for batch in render_data.batches)
     │     ├─ パイプライン選択 (Normal: PMAブレンド / Punchout: 穴あけブレンド)
     │     ├─ テクスチャバインド (Atlas / ExternalTexture)
     │     ├─ set_scissor_rect (クリッピング領域)
     │     └─ draw_indexed (インスタンス一括描画)
     │
     ▼
6. queue.submit & queue.present
     │
     ▼
7. render_data を self へ返却（メモリ再利用）
```

---

### パイプラインの各ステップ詳細

#### Step 1: 描画データの収集 (`collect_render_data`)

`collect_render_data` を介して、`Context` の SoA から現在のフレームで描画すべきエンティティ情報を吸い出します。
`RendererView` を通じてテクスチャアトラスやテキストキャッシュを更新しつつ、前面・背面に分類されたバッチ情報を生成します。

#### Step 2: インスタンスバッファの構築

`michiu_ui` は頂点バッファを要素ごとに生成しません。
全てのUI要素（矩形、角丸、境界線等）は、共通の 1x1 矩形頂点に対するインスタンスデータ（[`QuadInstance`]）として表現されます。CPU側ステージングバッファ（`instance_staging`）にエンティティごとの座標、サイズ、SDFパラメータ、カラー等をパックします。

#### Step 3: VRAM への一括転送

構築したインスタンス配列を `queue.write_buffer` で GPU 側の `instance_buffer` へ一括転送します。

#### Step 4〜5: レンダーパスとバッチ描画ループ

コマンドエンコーダーを作成し、共通の頂点・インデックスバッファとスクリーン投影用の Uniform（`config_bind_group`）をセットした後、バッチ単位でドローコールを発行します。

- **パイプラインの動的切り替え**:
  - `BatchType::Normal`: 通常のUI要素。事前乗算アルファ（PMA: Pre-multiplied Alpha）による上書き合成。
  - `BatchType::Punchout`: ExternalVisual 露出用の穴あけ矩形。前述の `Zero` + `OneMinusSrcAlpha` ブレンドにより、サーフェスのカラーとアルファを消去。
  - ※ 直前のバッチと異なる場合のみ `set_pipeline` を発行し、GPUステート変更の負荷を抑制します。
- **テクスチャの動的バインディング**:
  通常のアトラスに加え、非アクティブ時に静止画化された `ExternalTexture`（`external_bind_groups`）をバッチに応じて自動で切り替えます。
- **シザー矩形によるクリッピング**:
  `overflow: hidden` やスクロール領域のクリッピングを、物理ピクセル単位の `set_scissor_rect` でハードウェアレベルで制限します。
- **描画発行**:
  `draw_indexed` により、バッチ内のクアッド群をインスタンシングで一度に描画します。

#### Step 6〜7: コミットとバッファの返却

コマンドを GPU キューへ `submit` し、スワップチェーンを `present` します。
最後に一時退避していた `render_data` を `self.render_data` へ戻すことで、次フレームでもベクタの内部キャパシティを維持し、ヒープ確保の発生を抑えます。

---

## 主要フィールドの役割

| フィールド                           | 型                     | 役割                                                              |
| :----------------------------------- | :--------------------- | :---------------------------------------------------------------- |
| **`surface` / `device` / `queue`**   | wgpu コア型            | DirectX 12 デバイス、実行キュー、出力サーフェス                   |
| **`pipeline`**                       | `wgpu::RenderPipeline` | 通常描画用（Pre-multiplied Alpha ブレンド）                       |
| **`punchout_pipeline`**              | `wgpu::RenderPipeline` | ExternalVisual 用のくり抜き描画（減衰消去ブレンド）               |
| **`vertex_buffer` / `index_buffer`** | `wgpu::Buffer`         | 全UI要素で共有される 1x1 の単一クアッドメッシュ                   |
| **`instance_buffer`**                | `wgpu::Buffer`         | 各クアッドの座標、サイズ、SDF装飾情報が詰まったGPUバッファ        |
| **`atlas`**                          | `TextureAtlas`         | フォントグリフをパッキングした動的テクスチャアトラス              |
| **`text_cache`**                     | `FxHashMap`            | 整形済みテキストのレイアウトおよびグリフ配置キャッシュ            |
| **`external_bind_groups`**           | `FxHashMap`            | ExtrnalVisual の静止画キャプチャなどの ExternalTexture キャッシュ |

---

↑ここまでが前座

↓ここからが真打

---

# `MichiuRenderer`

Windows のハードウェア合成エンジン DirectComposition と wgpu (DirectX 12) を統合した最上位のレンダラーです。

UIコンポーネントを描画する前面レイヤー（[`WgpuRenderer`]）と、
[`ExternalVisual`](crate::ExternalVisual) をゼロコピーで合成する背面レイヤー（DirectComposition ビジュアルツリー）を統括します。

---

## ビジュアルツリーのアーキテクチャ

`MichiuRenderer` は、単一のウィンドウ（`HWND`）に対して以下のようなDirectCompositionツリーを構築・維持します。

```text
IDCompositionTarget (ウィンドウへのバインド)
  └─ root_visual
       ├─ cont_external_visual (背面: ExternalVisual 群)
       └─ wgpu_visual     (前面: wgpu によるUI描画 + パンチアウト)
```

## レンダラーの実行パイプライン (`update_composition_tree` → `draw`)

`MichiuRenderer` のフレーム更新は、
「DirectComposition ツリーの同期（状態確定と穴あけ指示）」と「wgpu による描画とコミット」の2つのフェーズで完結します。

### フェーズ 1: `update_composition_tree`

レイアウト計算（`sync_layout`）の直後に呼び出され、`ExternalVisual` の状態解決とDCompツリーの更新を行います。

1.  **バックドロップの同期**:
    ウィンドウに設定された Mica や Acrylic などの OS マテリアル状態を更新します。

2.  **遅延解放キューの消化**:
    前フレームまでに蓄積された `pending_dcomp_releases` を進め、wgpu 側のピクセル定着が完了した Visual を安全にツリーから切り離します（`RemoveVisual`）。

3.  **安定性の事前評価**:
    各 `ExternalVisual` に対し、リサイズ中か、クールダウン中か、あるいはアニメーション中かといった安定状態を判定します。この結果は後続の更新フックおよび静止画化の判定フラグとして利用されます。

4.  **外部Visualの状態更新フック ([`ExternalVisual::update`](crate::ExternalVisual::update))**:
    前段で算出した安定性フラグやデバイス参照を [`VisualUpdateContext`](crate::VisualUpdateContext) に詰め、外部Visualのライフサイクルメソッドを呼び出します。

    ```rust
    pub struct VisualUpdateContext<'a> {
        pub hwnd: HWND,
        pub rect: LayoutRect,
        pub scale_factor: f32,
        pub is_interactive: bool,
        pub is_stable: bool,
        pub device: &'a wgpu::Device,
        pub queue: &'a wgpu::Queue,
        pub wic_factory: &'a IWICImagingFactory,
    }
    ```

    `ExternalVisual` はこのコンテキストを通じて、「安定しているときだけ静止画キャプチャを更新する」「WICで画像をデコードする」といった自律的な判断・描画処理を行います。

5.  **レイアウトとクリッピングの同期 (`sync_visual_layout_and_clip`)**:
    最新の絶対座標、物理ピクセルサイズ等に伴うクリッピング矩形を DirectComposition の Visual へ適用します。

6.  **静止画キャプチャと実体の分岐**:
    更新結果に基づき、描画アプローチを分岐します。この分岐は [`static_texture`](crate::ExternalVisual::static_texture) の返り値 `Option<Arc<dyn ExternalTexture>>` によって決定されます。
    - **静止画キャプチャが存在する場合 (Some)**:
      DComp ツリーから実体 Visual をアンマウント（または削除）し、wgpu 側の通常テクスチャとして軽量描画させます。
    - **実体 Visual として表示する場合 (None)**:
      DComp ツリーへマウントし、後続の wgpu 描画パスに対して「この領域をパンチアウトする (`cx.renders.rnd_active_external_visual.insert(id)`)」指示を発行します。

7.  **クリーンアップ**:
    UIツリーからデスポーン・非アクティブな `ExternalVisual` を解放(`RemoveVisual`) します。

---

### フェーズ 2: `draw`

すべての準備が整った後、実際のピクセル出力とログ送信を行います。

```rust
pub(crate) fn draw(&mut self, cx: &mut Context) {
    // 1. 前面 UI の描画（wgpu でピクセル化して折り返し！）
    self.wgpu_renderer.render(cx, self.scale_factor);

    // 2. DirectComposition の変更を一括コミット
    unsafe {
        self.desktop_device
            .Commit()
            .unwrap_or_trace(None, &mut cx.debug);
    };

    // 3. 描画完了に伴い、Context 側のダーティフラグをクリア
    cx.clear_dirty();

    // 4. 1フレームの終点：ログを送信
    flush_trace!(cx, self);
}
```

1.  **`wgpu_renderer.render`**:
    パンチアウト指示に従ってくり抜きを行い、インスタンス描画でUIをサーフェスに焼き付けます。
2.  **`Commit()`**:
    DirectComposition を通じて、前面の wgpu サーフェスと背面の `ExternalVisual` をOSへコミットします。
3.  **`cx.clear_dirty()`**:
    描画がGPUに渡ったことで、このフレームで生じたUIやレイアウトの変更がすべて確定・反映されたため、Context内のダーティフラグをクリアします。
4.  **`flush_trace!`**:
    フレームのメトリクスやトレースログを外部（[`MichiuInspector`](crate::MichiuInspector)）へ一括フラッシュします。

---

## 主要フィールド

| フィールド             | 型                   | 責務 / 役割                                                                                                                                                                                                                                                                                                                                                                                                                        |
| :--------------------- | :------------------- | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`hwnd`**             | `HWND`               | 描画対象の Win32 ウィンドウハンドル                                                                                                                                                                                                                                                                                                                                                                                                |
| **`sync`**             | `WindowSynchronizer` | ウィンドウ状態の同期管理（`window_size`、`scale_factor`、リサイズ時のチラつきを抑える `resize_cooldown_frames`）                                                                                                                                                                                                                                                                                                                   |
| **`platform`**         | `PlatformContext`    | 不変のグラフィックスランタイムおよび WinRT 基盤：<br>• **`d3d11` / `dcomp`**: コアとなる D3D11 および DirectComposition デバイス<br>• **`compositor` / `controller`**: WinRT Compositor と DispatcherQueue のライフサイクル管理<br>• **`wic`**: 画像・テクスチャ読み込み用の WIC イメージングファクトリ                                                                                                                            |
| **`workspace`**        | `DCompWorkspace`     | ビジュアルツリーとリソースのライフサイクル管理：<br>• **`target` / `root_visual`**: ルートコンポジションターゲットと親 Visual<br>• **`wgpu_visual`**: wgpu のスワップチェーンサーフェスをホストする Visual<br>• **`promoted_visuals`**: コンポジションツリーにマウントされたアクティブな外部 Visual 群<br>• **`pending_releases`**: ツリー更新時の画面のチラつきを防ぐための遅延破棄キュー |
| **`wgpu_renderer`**    | [`WgpuRenderer`]     | インスタンス描画による UI レンダリングおよびパンチアウト処理を担うサブシステム                                                                                                                                                                                                                                                                                                                                     |
| **`current_backdrop`** | `Backdrop`           | 現在ウィンドウに適用中のシステムマテリアル設定                                                                                                                                                                                                                                                                                                                                                               |

---

## 低レイヤ操作と脱出ハッチ（[`RawWgpuRenderer<'a>`] と [`RawMichiuRenderer<'a>`]）

`WgpuRenderer` および `RawMichiuRenderer` のフィールドは通常 `pub(crate)` で保護されていますが、
内部のフィールドへ直接アクセスするための脱出ハッチとして、
[`raw_wgpu_renderer_mut`](WgpuRenderer::raw_wgpu_renderer_mut) と [`raw_michiu_renderer_mut`](MichiuRenderer::raw_michiu_renderer_mut) を提供しています。

### 内部設計：`RawWgpuRenderer` と `RawMichiuRenderer` による分割借用

[`MichiuRenderer`] は、`Context` と同様の分割借用パターンを採用しています。[`RawWgpuRenderer`] は [`MichiuRenderer::raw_wgpu_renderer_mut`](MichiuRenderer::raw_wgpu_renderer_mut) によってのみ取得出来ます。

```rust
// クレート内部で各フィールドを個別の可変参照として展開する脱出ハッチ
let mut raw_michiu = renderer.raw_michiu_renderer_mut();
let mut raw_wgpu = raw_michiu.raw_wgpu_renderer_mut();

// device, queue, instance_buffer などを同時に別々の関数へ安全に受け渡し可能
upload_resources(raw_wgpu.device, raw_wgpu.queue, raw_wgpu.instance_buffer);
update_atlas(raw_wgpu.atlas, raw_wgpu.temp_uv_map);
```

一方で、外部のユーザーコードに対しては [`d3d11_device(&self)`](MichiuRenderer::d3d11_device) や [`surface(&self)`](WgpuRenderer::surface) などのイミュータブルなゲッターメソッドを通じて、安全な参照のみを公開しています。
