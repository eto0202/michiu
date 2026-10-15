#![allow(
    clippy::similar_names,
    clippy::struct_field_names,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::struct_excessive_bools,
    clippy::doc_markdown
)]

//! # `michiu_ui`
//!
//! `michiu_ui` は Windows 向け GUI ライブラリ [`michiu`](https://crates.io/crates/michiu) のコアエンジンです。
//! UIツリーの構築、シグナルによるリアクティブな状態管理、レイアウト計算、および wgpu と DirectComposition によるレンダリング・合成を担当します。
//!
//! > **Note:**
//! > 一般的なGUIアプリケーション開発には、ウィンドウ生成・管理等を統合した最上位クレート [`michiu`](https://crates.io/crates/michiu) の利用を推奨します。
//!
//! ---
//!
//! ## 要約
//!
//! 本クレートのパイプラインは、主に以下の4つの要素で構成されます。
//!
//! 1. **レンダラーの用意**:
//!    [`MichiuRenderer`] は、DirectComposition と wgpu (DirectX 12) を統合したバックエンドです。任意の手段で作成したウィンドウ（`HWND`）を渡して非同期で初期化します。
//! 2. **[`Context`] の初期化**:
//!    UI状態、スタイル定義、入力キュー、レイアウトキャッシュ等を一括管理するコア構造体です。アクセシビリティ機能のバインドもここで行います。
//! 3. **UIの構築**:
//!    [`build_ui`] を使用して宣言的にUIツリーを組み立てます。
//! 4. **イベントのキューイングと描画サイクル**:
//!    [`inject_action`](Context::inject_action) でOSイベントをキューに積み、フレーム描画時にパイプラインを実行してレンダラーへ描画します。
//!
//! ## 基本的な初期化手順
//!
//! ```rust,ignore
//! use michiu_ui::prelude::*;
//!
//! // Context と Renderer を一元管理するためのアプリケーション状態
//! struct AppState {
//!    renderer: MichiuRenderer,
//!    context: Context,
//!    root_id: EntityId,
//! }
//!
//! // 1. 任意の方法でウィンドウを作成し、HWND とスケールファクターを取得
//! let hwnd = create_window();
//! let scale_factor = get_scale_factor();
//!
//! // 2. レンダラーの初期化（MichiuRenderer::new は非同期）
//! let renderer = pollster::block_on(MichiuRenderer::new(
//!     hwnd,
//!     LayoutSize::new(600.0, 500.0),
//!     scale_factor,
//! ))?;
//!
//! // 3. `Context` の初期化
//! // アクセシビリティ機能を有効化する場合は .with_accessibility(hwnd) をチェーンします
//! let mut context = Context::new();
//!
//! // 4. UI の構築
//! let root = build_ui(&mut context, move || {
//!     let (count, set_count) = create_signal(0u32); // Initialize the signal.
//!
//!     flex(
//!         ts().size_full()
//!             .justify_center()
//!             .bg_color(rgb(22, 24, 29))
//!     )
//!     .on_click(move || set_count.set(count.get() + 1))
//!     .label(
//!         move || format!("Count: {}", count.get()),
//!         ts().text_color(rgb(156, 158, 163)),
//!     )
//! });
//!
//! let app_state = AppState {
//!    renderer,
//!    context,
//!    root_id: root.id()
//! };
//!
//! // 5. イベントと描画の処理（後述の「イベント処理から最終描画までのサイクル」を参照）
//!
//! ```
//!
//! ---
//!
//! ## イベント処理から最終描画までのサイクル
//!
//! 本クレートは、入力の蓄積（キューイング） とパイプラインによる一括評価・描画を分離した設計になっています。
//! イベントループ内では以下のような流れで処理を進めます。
//! ```rust,ignore
//! use michiu_ui::prelude::*;
//! use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
//! use windows::Win32::Graphics::Dwm::DwmFlush;
//!
//! fn win32_message(cx: &mut AppState, hwnd: HWND, event: MyWindowEvent) {
//!     match event {
//!         // --- イベント受信時 (例: マウス移動、クリック、キー入力) ---
//!         // 即座にレイアウトや再描画は走らせず、アクションをキューに蓄積します
//!         MyWindowEvent::MouseInput { button, state, modifiers } => {
//!             app.context.inject_action(EventAction::PointerButton { button, state, modifiers });
//!         }
//!         MyWindowEvent::CursorMoved { x, y } => {
//!             app.context.inject_action(EventAction::PointerMove(LayoutPoint::new(x, y)));
//!         }
//!         // --- 描画要求時 (WM_PAINT、リサイズなど) ---
//!         MyWindowEvent::RedrawRequested => {
//!             // 1. イベントキューを消化し、ホバー・クリック等のインタラクション状態を更新
//!             app.context.begin_frame();
//!
//!             // 2. アニメーションおよび自動ドラッグの内部タイマーを1ステップ進行。
//!             app.context.tick_system_frame(&TickType::All);
//!
//!             // 3. レイアウトの再計算
//!             // スクロールバーの出現に伴うコンテンツ領域の変動を正しく解決するため、内部で2パスの計算を行います。
//!             app.context.sync_layout(app.root_id, app.renderer.layout_size());
//!
//!             // 4. レイアウト結果に基づき、外部Visualや背景の DirectComposition ツリーを同期。
//!             app.renderer.update_composition_tree(&mut app.context);
//!
//!             // 5. 必要に応じてアクセシビリティ（AccessKit/UIA）ツリーを更新。
//!             app.context.update_accessibility();
//!
//!             // 6. テキスト整形、インスタンスバッファのGPU転送、および wgpu による描画実行と DirectComposition によるコミット。
//!             // MichiuInspector が有効な場合、このステップの最後にログ等が送信されます。
//!             // Win32API を使用する場合、描画の準備と終了処理が必要です。
//!             let mut ps = PAINTSTRUCT::default();
//!             let _hdc = unsafe { BeginPaint(hwnd, &mut ps) };
//!
//!             app.renderer.draw(&mut app.context);
//!
//!             let _ = unsafe { EndPaint(hwnd, &ps) };
//!
//!             // 7. アニメーションや遷移が継続している場合、次フレームの再描画をスケジューリング。
//!             if app.context.has_active_frame() {
//!                 let _ = unsafe { DwmFlush() };
//!                 let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! ### パイプライン各フェーズの責務
//!
//! | フェーズ | 実行メソッド | 主な役割 |
//! | :--- | :--- | :--- |
//! | **1. 入力評価** | `begin_frame` | キュー内の `EventAction` をツリーへ伝播し、シグナルやハンドラを実行 |
//! | **2. システム更新** | `tick_system_frame` | アニメーション補間値などの時間依存状態を更新 |
//! | **3. レイアウト確定** | `sync_layout` | Flexboxの2パス評価により、スクロール領域を含む全要素の絶対座標を算出 |
//! | **4. ハードウェア合成** | `update_composition_tree` | DirectComposition に外部Visualの配置・変形を更新 |
//! | **5. アクセシビリティ** | `update_accessibility` | スクリーンリーダー等にツリーの更新差分を通知 |
//! | **6. レンダリング** | `draw` | テキスト成形・GPUバッファ更新を行い描画コマンドを実行 |
//! | **7. 次フレーム要求** | `has_active_frame` | アニメーションが完了するまで再描画を継続 |
//!
//! 内部の詳しいフェーズ遷移については、[`pipeline`] のドキュメントを参照してください。
//!
//! ---
//! ## アーキテクチャと設計思想
//!
//! ### 1. なぜ DirectComposition なのか？（ハードウェア合成と外部Visualの統合）
//! 一般的なGUIエンジンは単一のサーフェスにすべてを描画するか、OSの子ウィンドウを重ねます。
//! しかし `michiu_ui` では、DirectComposition のビジュアルツリーを基盤としています。
//!
//! wgpu によるUIレンダリング層の下に、外部コンポーネントが生成したネイティブの `IDCompositionVisual2` を直接配置することができます。
//! これにより、テクスチャのメモリコピーやオーバーヘッドなしにOSネイティブの高速なハードウェア合成を実現しています。
//!
//! ### 2. データ指向設計による状態管理
//! `Context` の内部は、フラットなDOD（Data-Oriented Design）で設計されています。
//!
//! `Element` の実体は肥大なUIオブジェクトではなく、内部にあるSoAのキー（[`EntityId`]）です。そのため、Rustの所有権や借用規則に縛られることなく、要件に合わせて以下の4つのアプローチを自由に選択・混用できます。
//!
//! | アプローチ | メリット | 推奨ユースケース |
//! | :--- | :--- | :--- |
//! | **1. Signal** | 状態と描画の分離 | 複雑なビジネスロジック、グローバル状態 |
//! | **2. Direct** | 低オーバーヘッド | 単純なトグル、使い捨てのローカル状態 |
//! | **3. Event Context** | 自己完結したコンポーネント化 | イベントを起点とした操作 |
//! | **4. Tag Query** | 型安全な疎結合（コンポーネント間通信） | 兄弟要素や親・子要素の直接操作 |
//!
//! #### 1. Signal アプローチ（宣言的・リアクティブ）
//! シグナル値の変更が、そのシグナルを購読しているプロパティ（クロージャ）へ自動的に伝播します。
//! ```rust,ignore
//! # use michiu_ui::prelude::*;
//! let (count, set_count) = create_signal(0u32);
//!
//! div_n().on_click(move || set_count.set(count.get() + 1))
//!        .label(move || format!("Signal: {}", count.get()), ts())
//! ```
//!
//! #### 2. Direct アプローチ（ローカル変数の直接キャプチャ）
//! `Element` をそのまま イベントハンドラ (例: `on_click` ) クロージャ内へ `move` して、命令的に操作します。
//! ```rust,ignore
//! # use michiu_ui::prelude::*;
//! let el = h_flex(ts().bg_color(Color::BLUE));
//!
//! let mut count = 0u32;
//!
//! el.label(move || format!("Direct: {count}"), ts())
//!   .on_click(move || {
//!        count += 1;
//!        el.set_contents(div_n().label(move || format!("Direct: {count}"), ts()))
//!          .style(ts().bg_color(Color::RED));
//!   })
//! ```
//!
//! #### 3. Event Context アプローチ（カレント要素の操作）
//! `_with` サフィックス付きのハンドラの引数 `cx: &mut Context` から今イベントを発火した要素（`cx.current()`）」を取得して操作します。
//! ```rust,ignore
//! # use michiu_ui::prelude::*;
//! let mut count = 0u32;
//!
//! h_flex(
//!     ts().bg_color(Color::BLUE)
//! )
//! .label(move || format!("Event: {count}"), ts())
//! .on_click_with(move |cx| {
//!      count += 1;
//!      cx.current().set_contents(div_n().label(move || format!("Event: {count}"), ts()))
//!        .style(ts().bg_color(Color::RED));
//! })
//! ```
//!
//! #### 4. Tag Query アプローチ
//! Tag機能は、UIツリーの任意の要素に型情報を付与し、
//! 親子関係や階層構造を意識することなく、別の場所から直接要素を逆引き・操作するための仕組みです。
//!
//! 従来の宣言的UIでは、深くネストされた子要素（例: メニュー項目）から外側の要素（例: メニュー全体）を操作する場合、状態を親へ持ち上げ、何階層もコールバック関数をバケツリレーする必要がありました。
//!
//! `michiu_ui` では、DODによるフラットなID管理を活かし、閉じた子要素から一方的に特定の要素をクエリして操作することが可能です。
//! ```rust,ignore
//! # use michiu_ui::prelude::*;
//! let mut count = 0u32;
//!
//! let left = flex(ts()).tag::<a11y::AButton>();
//!
//! let right = flex(ts()).label("Click", ts()).on_click_with(move |cx| {
//!     let el = cx.query_first::<a11y::AButton>();
//!     count += 1;
//!     el.set_contents(div_n().label(move || format!("Tag: {count}"), ts()));
//! });
//!
//! h_flex(ts()).children([left, right])
//! ```
//!
//! #### メリット
//! 1. **コンポーネントの完全な疎結合**:
//!    `right` は自分がどこに配置されているかを知る必要がありません。
//!    「クリックされたら `a11y::AButton` を持っている要素にラベルを追加・置き換える」という単一の責務に集中できます。
//! 2. **アクセシビリティ（a11y）との一体化**:
//!    マーカー型として [`a11y::AButton`] のようなタグ（推論タグ）を使用することで、
//!    Windows の UI Automation (AccessKit経由) へのセマンティクス伝達と、Tagクエリのキー定義を一度に完了できます。
//!    推論タグはあらかじめ用意されたタグの他に、 [`A11yInferenceTag`](crate::a11y::A11yInferenceTag) トレイトを使用することで自作することが出来ます。
//!
//! ---
//!
//! # リアクティビティとコンテキスト提供
//!
//! `michiu_ui` では、シグナルをツリーの上流で `.provide()` することで、引数のバケツリレーなしに、任意の下流コンポーネントへ型ベースで状態を自動提供できます。
//!
//! ## 基本的な使用法
//!
//! ```rust,ignore
//! use michiu_ui::prelude::*;
//!
//! // 1. ルート（または上位ノード）でシグナルを生成して provide
//! pub fn create_root() -> Element {
//!     let (theme, _) = create_signal(Theme::dark());
//!     let (search_text, _) = create_signal(SearchText(String::new()));
//!
//!     v_flex(move || ts().size_full().bg_color(theme.get().background))
//!         .provide(theme)
//!         .provide(search_text)
//!         .children([
//!             header(),
//!             sidebar(),
//!             main_area(),
//!         ])
//! }
//!
//! // 2. 下流の任意のコンポーネントで購読
//! fn some_label() -> Element {
//!     // provide された Theme から primary カラーを購読（自動追従）
//!     label("Hello", ts().text_color(dynamic(|t: &Theme| t.primary)))
//! }
//! ```
//!
//! ---
//!
//! ## Tag と Signal（Provide）の使い分け
//!
//! * **Tag が得意なこと**:
//!   「クリックされた時に、特定のオーバーレイや兄弟要素を直接操作する」ようなイベント駆動の局所的な命令操作。
//! * **Signal + Provide が得意なこと**:
//!   「テーマ」「検索クエリ」「ソート順」など、ツリー全体に暗黙的に共有され、値の変更が全自動で波及すべき状態。
//!
//! ---
//!
//! # 動的スタイリングとパフォーマンスガイド
//!
//! `michiu_ui` では、テーマ変更や状態遷移に応じた動的なスタイル指定として、
//! 「スタイル全体の動的評価（`style_d` / `style`）」 と 「プロパティ単位の評価（`dynamic`）」 の2系統を提供しています。
//!
//! ---
//!
//! ## 1. `style(move || ...)` と `style_d` の等価性
//!
//! 以下の2つの記述は、ライブラリ内部において全く同一の挙動および再評価コストを持ちます。
//!
//! ```rust,ignore
//! // パターン A: 通常の style メソッドに引数なしの動的クロージャを渡す
//! .style(move || {
//!     let t = use_provided::<Theme>().get();
//!     ts().bg_color(t.background)
//! })
//!
//! // パターン B: style_d メソッドを使用する（糖衣構文）
//! .style_d(|t: &Theme| {
//!     ts().bg_color(t.background)
//! })
//! ```
//!
//! * **同一エフェクトカテゴリ**: どちらも `Prop::Dynamic` として処理され、要素ごとに1つだけ割り当てられる `EffectCategory::Style` に登録されます。
//! * **依存関係の自動構築**: クロージャ評価中にシグナル（`Theme`）の `.get()` が呼ばれるため、構築される購読関係やシグナル更新時の発火タイミングは完全に一致します。
//!
//! ---
//!
//! ## 2. スタイルの全体評価とプロパティ単位の評価（`dynamic`）
//!
//! | 比較項目 | スタイル全体 (`style_d`) | プロパティ単位 (`dynamic`) |
//! | :--- | :--- | :--- |
//! | **評価単位** | `ThisStyle` オブジェクト単位 | 各プロパティ単体（色、サイズ等） |
//! | **ツリー遡上回数 (`use_provided`)** | **1回のみ**（クロージャ開始時に一括解決） | **プロパティの個数分発生**（各セッターが個別解決） |
//! | **再評価時の挙動** | 新しい `ThisStyle` を再生成し、SoA全体を一括マージ（`override_with`） | 対象の動的セッターのみを実行し、SoAの該当フィールドだけを直書き |
//! | **メモリとCPU特性** | 一括代入による優れたCPUキャッシュ効率 | 構造体生成を完全にバイパスする最小フットプリント |
//!
//! > `michiu_ui` の内部SoAマージはヒープ確保を伴わない単純なメモリ値（数値等）の連続代入のみで完結します。
//! > そのため、プロパティ数が多い場合は個別にツリーを遡って10回クロージャを呼ぶよりも、1回ツリーを遡ってメモリを一括コピーする方が高速になります。
//!
//! ---
//!
//! ## 3. シーン別の選択基準
//!
//! ### 基準A: 動的な値が少数（1〜数個程度）の場合
//! * **推奨**: **プロパティ単位の `dynamic()`**
//! * **理由**: `ThisStyle` の生成やSoAの再マージを完全にスキップできるためオーバーヘッドが極小になります。
//!
//! ```rust,ignore
//! div(ts().bg_color(dynamic(|t: &Theme| t.background)))
//! ```
//!
//! ### 基準B: 動的な値が多数（境界線、背景、ホバー、テキスト色など）の場合
//! * **推奨**: **スタイル全体の `style_d()`**
//! * **理由**:
//!   - **親ツリー探索（`use_provided`）の削減**: 依存シグナルを親へ遡る処理が1回で完了します（ツリーが深いほど差が大きくなります）。
//!   - **CPUキャッシュ効率の最大化**: 多数のクロージャを順次スタック評価するより、一括メモリ代入の方が高速に動作します。
//!   - **可読性の向上**: 静的なレイアウトと動的なレイアウトが明確に分離されます。
//!
//! ```rust,ignore
//! div(ts().w_full().p_y(4.0))
//!     .style_d(|t: &Theme| {
//!         ts().border_color(t.border)
//!             .bg_color(t.background)
//!             .pressed(ts().border_color(t.primary).bg_color(t.background_hover))
//!     })
//! ```
//!
//! ---
//!
//! ## 4. クイックリファレンス
//!
//! | ケース | 推奨アプローチ |
//! | :--- | :--- |
//! | 静的なスタイルのみ | `style(ts())` |
//! | 条件分岐などでスタイルを変えたい | `style(\|\| ts())` |
//! | 動的なプロパティが数個 | `dynamic(\|t: &T\|)` |
//! | 多数の動的なプロパティ | `style_d(\|t: &T\|)` |
//! | 動的なプロパティと条件分岐の組み合わせ | `style_d` |
//! | 静的スタイルと一部だけ動的なプロパティ | `style` + `dynamic` |
//! | 静的・動的なプロパティがともに多数 | `style(静的)` + `style_d(動的)` |
//!
//! ---
//!
//! ## 詳細ガイド (未実装)
//!
//! 各サブシステムの詳細仕様と内部アーキテクチャについては、以下の各セクションを参照してください。
//!
//! * [**レンダラーの実装**](crate::renderer) - DirectComposition サーフェス管理と wgpu による描画コマンドの生成
//! * [**`Context` の構造**](crate::context) - 全体状態の保持、ストレージ管理、ライフサイクル制御
//! * [**UIの構築とスタイリング**](crate::element) - [`build_ui`] によるウィジェットツリー構築と `ts()`（スタイルビルダー）のプロパティ一覧
//! * [**リアクティビティ**](crate::signal) - [`create_signal`] によるリアクティブプログラミングモデルと依存関係の追跡
//! * [**Tag機能**](crate::tag) - Element への識別タグ付与とツリー探索・クエリ
//! * [**アクセシビリティ**](crate::context::acce_store) - AccessKit との連携およびツリー同期
//! * [**外部要素の埋め込み**](crate::external) - 画像や動画、外部ファイル、`IDCompositionVisual2` の統合
//! * [**デバッグツール**](crate::context::debug_store) - エラーを含むログの収集やライフサイクルの可視化
//!
//! ---
//! ---
//! ---
//!
//! # `michiu_ui`
//!
//! `michiu_ui` is the core engine of the Windows GUI library [`michiu`](https://crates.io/crates/michiu).
//! It handles UI tree construction, reactive state management via signals, layout calculations, and rendering and compositing using wgpu and DirectComposition.
//!
//! > **Note:**
//! > For general GUI application development, we recommend using the top-level crate [`michiu`](https://crates.io/crates/michiu), which integrates window creation and management.
//!
//! ---
//!
//! ## Summary
//!
//! The pipeline in this crate consists primarily of the following four elements.
//!
//! 1. **Preparing the Renderer**:
//!    [`MichiuRenderer`] is a backend that integrates DirectComposition and wgpu (DirectX 12). It is initialized asynchronously by passing a window (`HWND`) created by any means.
//! 2. **Initializing [`Context`]**:
//!    This is a core structure that centrally manages UI state, style definitions, the input queue, the layout cache, and more. Accessibility feature bindings are also performed here.
//! 3. **Building the UI**:
//!    Use [`build_ui`] to declaratively construct the UI tree.
//! 4. **Event Queuing and Rendering Cycle**:
//!    [`inject_action`](Context::inject_action) queues OS events, and during frame rendering, executes the pipeline to render to the renderer.
//!
//! ## Basic Initialization Procedure
//!
//! ```rust,ignore
//! use michiu_ui::prelude::*;
//!
//! // Application State for Centralized Management of Context and Renderer
//! struct AppState {
//!    renderer: MichiuRenderer,
//!    context: Context,
//!    root_id: EntityId,
//! }
//!
//! // 1. Create a window using any method, and retrieve the HWND and scale factor
//! let hwnd = create_window();
//! let scale_factor = get_scale_factor();
//!
//! // 2. Initializing the Renderer (MichiuRenderer::new is asynchronous)
//! let renderer = pollster::block_on(MichiuRenderer::new(
//!     hwnd,
//!     LayoutSize::new(600.0, 500.0),
//!     scale_factor,
//! ))?;
//!
//! // 3. Initializing `Context`
//! // To enable accessibility features, chain `.with_accessibility(hwnd)`
//! let mut context = Context::new();
//!
//! // 4. Building the UI
//! let root = build_ui(&mut context, move || {
//!     let (count, set_count) = create_signal(0u32); // Initialize the signal.
//!
//!     flex(
//!         ts().size_full()
//!             .justify_center()
//!             .bg_color(rgb(22, 24, 29))
//!     )
//!     .on_click(move || set_count.set(count.get() + 1))
//!     .label(
//!         move || format!("Count: {}", count.get()),
//!         ts().text_color(rgb(156, 158, 163)),
//!     )
//! });
//!
//! let app_state = AppState {
//!    renderer,
//!    context,
//!    root_id: root.id()
//! };
//!
//! // 5. Event and Rendering Processing (see “The Cycle from Event Handling to Final Rendering” below)
//!
//! ```
//!
//! ---
//!
//! ## The Cycle from Event Processing to Final Rendering
//!
//! This crate is designed to separate input queuing from batch evaluation and rendering via a pipeline.
//! Within the event loop, processing proceeds as follows:
//! ```rust,ignore
//! use michiu_ui::prelude::*;
//! use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
//! use windows::Win32::Graphics::Dwm::DwmFlush;
//!
//! fn win32_message(cx: &mut AppState, hwnd: HWND, event: MyWindowEvent) {
//!     match event {
//!         // --- When an event is received (e.g., mouse movement, click, keystroke) ---
//!         // Do not run layout or redraw immediately; instead, accumulate actions in a queue.
//!         MyWindowEvent::MouseInput { button, state, modifiers } => {
//!             app.context.inject_action(EventAction::PointerButton { button, state, modifiers });
//!         }
//!         MyWindowEvent::CursorMoved { x, y } => {
//!             app.context.inject_action(EventAction::PointerMove(LayoutPoint::new(x, y)));
//!         }
//!         // --- When a drawing request is received (WM_PAINT, resize, etc.) ---
//!         MyWindowEvent::RedrawRequested => {
//!             // 1. Process the event queue and update interaction states such as hover and click
//!             app.context.begin_frame();
//!
//!             // 2. Advance the internal timer for animations and auto-drag by one step.
//!             app.context.tick_system_frame(&TickType::All);
//!
//!             // 3. Layout Recalculation
//!             // To correctly handle changes in the content area caused by the appearance of scrollbars, two passes of calculation are performed internally.
//!             app.context.sync_layout(app.root_id, app.renderer.layout_size());
//!
//!             // 4. Synchronize the DirectComposition trees for external visuals and backgrounds based on the layout results.
//!             app.renderer.update_composition_tree(&mut app.context);
//!
//!             // 5. Update the accessibility (AccessKit/UIA) tree as needed.
//!             app.context.update_accessibility();
//!
//!             // 6. Text formatting, GPU transfer of the instance buffer, rendering execution using wgpu, and commit via DirectComposition.
//!             // If MichiuInspector is enabled, logs and other information are sent at the end of this step.
//!             // When using the Win32 API, rendering preparation and cleanup are required.
//!             let mut ps = PAINTSTRUCT::default();
//!             let _hdc = unsafe { BeginPaint(hwnd, &mut ps) };
//!
//!             app.renderer.draw(&mut app.context);
//!
//!             let _ = unsafe { EndPaint(hwnd, &ps) };
//!
//!             // 7. If an animation or transition is still in progress, schedule the redrawing of the next frame.
//!             if app.context.has_active_frame() {
//!                 let _ = unsafe { DwmFlush() };
//!                 let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! ### Responsibilities for Each Phase of the Pipeline
//!
//! | Phase | Execution Method | Main Role |
//! | :--- | :--- | :--- |
//! | **1. Input Evaluation** | `begin_frame` | Propagates `EventAction`s from the queue to the tree and executes signals and handlers |
//! | **2. System Update** | `tick_system_frame` | Updates time-dependent states, such as animation interpolation values |
//! | **3. Layout Finalization** | `sync_layout` | Calculates the absolute coordinates of all elements, including the scroll area, via Flexbox’s two-pass evaluation |
//! | **4. Hardware Composition** | `update_composition_tree` | Updates the placement and transformation of external visuals in DirectComposition |
//! | **5. Accessibility** | `update_accessibility` | Notifies screen readers and other assistive technologies of changes to the tree |
//! | **6. Rendering** | `draw` | Performs text formatting and GPU buffer updates, then executes drawing commands |
//! | **7. Next Frame Request** | `has_active_frame` | Continues redrawing until the animation is complete |
//!
//! For details on the internal phase transitions, see the [`pipeline`] documentation.
//!
//! ---
//! ## Architecture and Design Philosophy
//!
//! ### 1. Why DirectComposition? (Hardware Composition and Integration with External Visuals)
//! Typical GUI engines either render everything onto a single surface or overlay child windows provided by the OS.
//! However, `michiu_ui` is based on the DirectComposition visual tree.
//!
//! You can place a native `IDCompositionVisual2` generated by an external component directly beneath the UI rendering layer provided by wgpu.
//! This enables fast, OS-native hardware compositing without texture memory copies or overhead.
//!
//! ### 2. State Management via Data-Oriented Design
//! The internal structure of `Context` is designed using a flat DOD (Data-Oriented Design).
//!
//! An `Element` is not a bloated UI object, but rather an internal SoA key ([`EntityId`]). Therefore, without being constrained by Rust’s ownership or borrowing rules, you can freely choose and mix the following four approaches to suit your requirements.
//!
//! | Approach | Benefits | Recommended Use Cases |
//! | :--- | :--- | :--- |
//! | **1. Signal** | Separation of state and rendering | Complex business logic, global state |
//! | **2. Direct** | Low overhead | Simple toggles, disposable local state |
//! | **3. Event Context** | Self-contained componentization | Event-driven operations |
//! | **4. Tag Query** | Type-safe loose coupling (inter-component communication) | Direct manipulation of sibling, parent, or child elements |
//!
//! #### 1. Signal Approach (Declarative and Reactive)
//! Changes to a signal's value are automatically propagated to the properties (closures) subscribed to that signal.
//! ```rust,ignore
//! # use michiu_ui::prelude::*;
//! let (count, set_count) = create_signal(0u32);
//!
//! div_n().on_click(move || set_count.set(count.get() + 1))
//!        .label(move || format!("Signal: {}", count.get()), ts())
//! ```
//!
//! #### 2. Direct Approach (Direct Capture of Local Variables)
//! `move` the `Element` directly into the event handler (e.g., `on_click`) closure and manipulate it imperatively.
//! ```rust,ignore
//! # use michiu_ui::prelude::*;
//! let el = h_flex(ts().bg_color(Color::BLUE));
//!
//! let mut count = 0u32;
//!
//! el.label(move || format!("Direct: {count}"), ts())
//!   .on_click(move || {
//!        count += 1;
//!        el.set_contents(div_n().label(move || format!("Direct: {count}"), ts()))
//!          .style(ts().bg_color(Color::RED));
//!   })
//! ```
//!
//! #### 3. Event Context Approach (Manipulating the Current Element)
//! Retrieve the element that triggered the current event (`cx.current()`) from the handler argument `cx: &mut Context` (which has the `_with` suffix) and manipulate it.
//! ```rust,ignore
//! # use michiu_ui::prelude::*;
//! let mut count = 0u32;
//!
//! h_flex(
//!     ts().bg_color(Color::BLUE)
//! )
//! .label(move || format!("Event: {count}"), ts())
//! .on_click_with(move |cx| {
//!      count += 1;
//!      cx.current().set_contents(div_n().label(move || format!("Event: {count}"), ts()))
//!        .style(ts().bg_color(Color::RED));
//! })
//! ```
//!
//! #### 4. Tag Query Approach
//! The Tag feature assigns type information to any element in the UI tree,
//! providing a mechanism to look up and manipulate elements directly from another location without having to be aware of parent-child relationships or hierarchical structures.
//!
//! In traditional declarative UIs, when manipulating an outer element (e.g., the entire menu) from a deeply nested child element (e.g., a menu item), it was necessary to propagate the state up to the parent and pass callback functions back down through multiple levels in a “bucket brigade” fashion.
//!
//! With `michiu_ui`, you can leverage DOD's flat ID management to query and manipulate specific elements unidirectionally from closed child elements.
//! ```rust,ignore
//! # use michiu_ui::prelude::*;
//! let mut count = 0u32;
//!
//! let left = flex(ts()).tag::<a11y::AButton>();
//!
//! let right = flex(ts()).label("Click", ts()).on_click_with(move |cx| {
//!     let el = cx.query_first::<a11y::AButton>();
//!     count += 1;
//!     el.set_contents(div_n().label(move || format!("Tag: {count}"), ts()));
//! });
//!
//! h_flex(ts()).children([left, right])
//! ```
//!
//! #### Benefits
//! 1. **Complete decoupling of components**:
//!    `right` does not need to know where it is placed.
//!    It can focus on its single responsibility: “When clicked, add or replace a label on an element that has an `a11y::AButton`.”
//! 2. **Integration with Accessibility (a11y)**:
//!    By using tags (inference tags) such as [`a11y::AButton`] as marker types,
//!    you can simultaneously convey semantics to Windows UI Automation (via AccessKit) and define keys for tag queries.
//!    In addition to predefined tags, you can create your own inference tags by using the [`A11yInferenceTag`](crate::a11y::A11yInferenceTag) trait.
//!
//! ---
//!
//! # Reactivity and Context Provision
//!
//! In `michiu_ui`, by calling `.provide()` on a signal upstream in the tree, you can automatically provide state to any downstream component in a type-based manner, without the need for a bucket relay of arguments.
//!
//! ## Basic Usage
//!
//! ```rust,ignore
//! use michiu_ui::prelude::*;
//!
//! // 1. Generate a signal at the root (or parent node) and provide it
//! pub fn create_root() -> Element {
//!     let (theme, _) = create_signal(Theme::dark());
//!     let (search_text, _) = create_signal(SearchText(String::new()));
//!
//!     v_flex(move || ts().size_full().bg_color(theme.get().background))
//!         .provide(theme)
//!         .provide(search_text)
//!         .children([
//!             header(),
//!             sidebar(),
//!             main_area(),
//!         ])
//! }
//!
//! // 2. Subscribe at any downstream component
//! fn some_label() -> Element {
//!     // Automatically adopt the primary color from the provided theme
//!     label("Hello", ts().text_color(dynamic(|t: &Theme| t.primary)))
//! }
//! ```
//!
//! ---
//!
//! ## Distinguishing Between Tags and Signals (Provide)
//!
//! * **What Tags Excel At**:
//!   Event-driven, localized command operations such as “directly manipulating specific overlays or sibling elements when clicked.”
//! * **What Signals + Provide Excel At**:
//!   States—such as “theme,” “search query,” or “sort order”—that are implicitly shared across the entire tree, where changes to their values should propagate automatically throughout the system.
//!
//! ---
//!
//! # Dynamic Styling and Performance Guide
//!
//! In `michiu_ui`, for dynamic styling based on theme changes and state transitions,
//! we provide two approaches: “dynamic evaluation of the entire style (`style_d` / `style`)” and “evaluation on a per-property basis (`dynamic`)”.
//!
//! ---
//!
//! ## 1. Equivalence of `style(move || ...)` and `style_d`
//!
//! Internally, the following two expressions have exactly the same behavior and reevaluation cost.
//!
//! ```rust,ignore
//! // Pattern A: Passing a dynamic closure with no arguments to the standard `style` method
//! .style(move || {
//!     let t = use_provided::<Theme>().get();
//!     ts().bg_color(t.background)
//! })
//!
//! // Pattern B: Using the `style_d` method (syntax sugar)
//! .style_d(|t: &Theme| {
//!     ts().bg_color(t.background)
//! })
//! ```
//!
//! * **Same Effect Category**: Both are treated as `Prop::Dynamic` and registered with `EffectCategory::Style`, which is assigned only once per element.
//! * **Automatic Dependency Construction**: Since `.get()` on the signal (`Theme`) is called during closure evaluation, the constructed subscription relationships and the firing timing when the signal is updated match perfectly.
//!
//! ---
//!
//! ## 2. Overall Style Evaluation and Property-Level Evaluation (`dynamic`)
//!
//! | Comparison Criteria | Overall Style (`style_d`) | Property-Level (`dynamic`) |
//! | :--- | :--- | :--- |
//! | **Evaluation Unit** | Per `ThisStyle` object | Each individual property (color, size, etc.) |
//! | **Number of Tree Traversals (`use_provided`)** | **Only once** (resolved in bulk when the closure begins) | **Occurs for each property** (each setter is resolved individually) |
//! | **Behavior During Reevaluation** | Regenerates a new `ThisStyle` and performs a bulk merge (`override_with`) of the entire SoA | Executes only the target dynamic setters and directly writes to only the relevant fields of the SoA |
//! | **Memory and CPU Characteristics** | Excellent CPU cache efficiency due to bulk assignment | Minimal footprint by completely bypassing structure generation |
//!
//! > The internal SoA merge in `michiu_ui` is completed solely through sequential assignment of simple memory values (such as numbers), without any heap allocation.
//! > Therefore, when there are many properties, performing a single bulk memory copy by traversing the tree once is faster than calling the closure 10 times by traversing the tree individually.
//!
//! ---
//!
//! ## 3. Selection Criteria by Scene
//!
//! ### Criterion A: When there are only a few dynamic values (1 to a few)
//! * **Recommended**: **`dynamic()` per property**
//! * **Reason**: Overhead is minimized because it allows you to completely skip the generation of `ThisStyle` and the re-merging of SoA.
//!
//! ```rust,ignore
//! div(ts().bg_color(dynamic(|t: &Theme| t.background)))
//! ```
//!
//! ### Criterion B: When there are many dynamic values (border, background, hover, text color, etc.)
//! * **Recommended**: **`style_d()` for the entire style**
//! * **Reason**:
//!   - **Reduction in parent tree traversal (`use_provided`)**: The process of tracing dependency signals back to the parent is completed in a single pass (the deeper the tree, the greater the difference).
//!   - **Maximizing CPU cache efficiency**: A single memory assignment is faster than sequentially evaluating numerous closures on the stack.
//!   - **Improved readability**: Static and dynamic layouts are clearly separated.
//!
//! ```rust,ignore
//! div(ts().w_full().p_y(4.0))
//!     .style_d(|t: &Theme| {
//!         ts().border_color(t.border)
//!             .bg_color(t.background)
//!             .pressed(ts().border_color(t.primary).bg_color(t.background_hover))
//!     })
//! ```
//!
//! ---
//!
//! ## 4. Quick Reference
//!
//! | Case | Recommended Approach |
//! | :--- | :--- |
//! | Static styles only | `style(ts())` |
//! | Want to change styles based on conditionals, etc. | `style(\|\| ts())` |
//! | A few dynamic properties | `dynamic(\|t: &T\|)` |
//! | Many dynamic properties | `style_d(\|t: &T\|)` |
//! | Combination of dynamic properties and conditional branching | `style_d` |
//! | Static styles with only some dynamic properties | `style` + `dynamic` |
//! | Many static and dynamic properties | `style(static)` + `style_d(dynamic)` |
//!
//! ---
//!
//! ## Detailed Guide (Not Yet Implemented)
//!
//! For detailed specifications and internal architecture of each subsystem, refer to the sections below.
//!
//! * [**Renderer Implementation**](crate::renderer) - DirectComposition surface management and rendering command generation via wgpu
//! * [**`Context` Structure**](crate::context) - Global state maintenance, storage management, and lifecycle control
//! * [**UI Construction and Styling**](crate::element) - Widget tree construction via [`build_ui`] and the list of properties for `ts()` (style builder)
//! * [**Reactivity**](crate::signal) - Reactive programming model and dependency tracking via [`create_signal`]
//! * [**Tag Feature**](crate::tag) - Assigning identification tags to elements, and tree traversal and querying
//! * [**Accessibility**](crate::context::acce_store) - Integration with AccessKit and tree synchronization
//! * [**Embedding External Elements**](crate::external) - Integration of images, videos, external files, and `IDCompositionVisual2`
//! * [**Debugging Tools**](crate::context::debug_store) - Collection of logs (including errors) and visualization of the lifecycle

mod bitmap;
pub mod context;
pub mod element;
pub mod external;
mod option;
pub mod renderer;
pub mod signal;
mod style;
pub mod tag;
mod types;
mod utils;

#[doc(inline)]
pub use bitmap::*;
#[doc(inline)]
pub use context::*;
#[doc(inline)]
pub use element::*;
#[doc(inline)]
pub use external::*;
#[doc(inline)]
pub use option::*;
#[doc(inline)]
pub use renderer::*;
#[doc(inline)]
pub use signal::*;
#[doc(inline)]
pub use soa::MichiuSoA;
#[doc(inline)]
pub use style::*;
#[doc(inline)]
pub use tag::*;
#[doc(inline)]
pub use types::*;
#[doc(inline)]
pub use utils::*;

pub mod accessibility {
    pub const NO_A11Y_NODE: crate::Prop<Option<accesskit::Node>> = crate::Prop::None;
    pub use crate::a11y::{self, A11yInferenceTag};
    pub use accesskit::{Action, Node, NodeId, Role};
}

#[doc(hidden)]
pub mod prelude {
    pub use crate::{
        bitmap::PropertyList,
        context::{Context, EntityId, MichiuInspector},
        dnd::{DndDragPayload, DndDropTarget},
        element::{Element, build_ui},
        input::InputContents,
        pipeline::{EventAction, TickType},
        renderer::MichiuRenderer,
        scrollbar::{ScrollbarDisplay, ScrollbarMode, ScrollbarStyle},
        signal::{ReadSignal, SignalId, WriteSignal},
        style::ThisStyle,
        types::{
            AnimationCurve, Backdrop, BorderAlignment, BorderStyle, Color, CursorIcon, Display,
            ElementState, InteractionState, KeyframeAnimation, LayoutPoint, LayoutRect, LayoutSize,
            Modifiers, MouseButton, PlaybackCount, PointerEvents, Transform, Transition,
        },
        utils::{
            auto, block_box, blur, create_signal, div, div_n, dynamic, external_texture, fill,
            flex, flex_d, get_win32_clipboard, grid_box, h_flex, h_flex_d, hex, hidden_box, hsl,
            hsla, input, input_area, input_area_d, input_d, offset, pct, px, rgb, rgba,
            set_win32_clipboard, shadow, spread, text, text_d, ts, use_provided,
            use_provided_setter, v_flex, v_flex_d,
        },
    };
}
