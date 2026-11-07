# Accessibility Component Catalog (`a11y_catalog`)

> **Languages**: [English](#english) | [日本語](#原文)

# English

This crate provides a catalog of accessible UI components, inference tags, and themes compliant with WCAG (Web Content Accessibility Guidelines) and WAI-ARIA specifications.

Instead of enforcing a heavy library-level dependency, it is designed around the philosophy of **"bringing only the core traits and the exact files you need into your own codebase."**

---

## Running the Catalog App

You can directly preview and verify component appearance, keyboard interactions, and screen reader behavior (e.g., Windows Narrator).

```bash
# Run from the workspace root
cargo run -p a11y_catalog
```

---

## Directory Structure

Components are managed as single files or folder modules depending on their complexity, all unified under lightweight core traits.

```text
a11y_catalog/
├── Cargo.toml
├── README.md
└── src/
    ├── main.rs                  # Application entry point
    ├── app.rs                   # Application root setup
    ├── logger.rs                # Error logging
    ├── catalog.rs               # Module declarations
    │
    ├── app/                     # Internal catalog app UI
    │   ├── header.rs
    │   ├── main_area.rs
    │   ├── sidebar.rs
    │   └── theme.rs
    │
    ├── assets/                  # Static assets such as cursor icons and images
    │
    ├── catalog/                 # Exportable component, theme, tag...etc.
    │   ├── core.rs              # [Core Trait] MichiuWidget & MichiuTheme & Oklch
    │   ├── a11y_theme.rs        # Theme module declarations
    │   ├── components.rs        # Component module declarations & registry
    │   ├── inference_tag.rs     # Module declarations
    │   ├── utils.rs             # Module declarations
    │   │
    │   ├── a11y_theme/          # Accessible themes meeting contrast ratios
    │   │   └── simple_theme.rs # Default 12-scale WCAG AA palette
    │   │
    │   ├── inference_tag/       # Reusable inference tags (A11yInferenceTag implementations)
    │   │
    │   ├── utils/               # Internal helpers and utilities
    │   │
    │   ├── components/          # Exportable component collection
    │   │   ├── simple_button.rs        # [Single File] Self-contained components
    │   │   ├── tabs/            # [Folder Module] Complex multi-file components
    │   │   │   ├── mod.rs
    │   │   │   ├── item.rs
    │   │   │   └── panel.rs
```

---

## How to Use (3-Step Integration)

Integrating components into your project is designed to be straightforward and modular:

```text
Step 1: Copy core.rs
        └── Brings MichiuWidget & MichiuTheme traits & Oklch
Step 2: Prepare Theme
        └── Copy from a11y_theme/ OR implement MichiuTheme on your own theme struct
Step 3: Copy Components
        └── Copy from components/ (by file, folder, or struct block)
```

### Step 1: Copy `core.rs` into Your Project

First, copy `src/catalog/core.rs` into your project. This file defines the core traits (`MichiuWidget`, `MichiuTheme`, `Oklch`) and has zero heavy external dependencies.

### Step 2: Prepare Your Theme

Prepare a theme conforming to `MichiuTheme`:

- **Option A**: Copy an existing WCAG AA theme from `catalog/a11y_theme/` (e.g., `SimpleBlueTheme`).
- **Option B**: Implement `MichiuTheme` on your existing application theme struct.

### Step 3: Copy Components from `components/`

Copy the components you need:

- **Single file**: Copy files like `components/button.rs` into your project.
- **Folder module**: Copy directories like `components/tabs/` into your project.
- **Snippet**: Copy only the target struct and its `MichiuWidget` impl directly into your code.

---

## Customizing Styles & Events

When you need to adjust styles, dimensions, or attach click/pointer events, simply chain modifiers **after calling `.build(&theme)`**.

Any methods chained after `build()` will **take precedence and overwrite defaults (Last-Write-Wins)**.

```rust
// Basic rendering with defaults
let button = SimpleButton::new("Save").build(&theme);

// Customizing style & events (overwriting after build)
let customized_button = SimpleButton::new("Delete")
    .variant(ButtonVariant::Outline)
    .build(&theme)
    // Overwrite styles after build
    .modify(ii().bg_color(Color::WHITE))
    // Attach event handlers
    .on_click(|| println!("Deleted!"));
```

---

## Design Principles

Follow these strict guidelines when authoring or contributing components and themes:

### 1. Self-Contained within File or Folder

- Each file or directory under `components/` **must be completely self-contained** (except for depending on `core.rs` traits).
- Components **must never depend on other catalog components**.
- Internal helper logic, modifiers, or sub-widgets should remain private or `pub(crate)` within that file/folder. Code duplication is intentionally encouraged over cross-component coupling.

### 2. Single File vs. Folder Module

- **Single File (`name.rs`)**: Standard widgets should live within a single `.rs` file.
- **Folder Module (`name/`)**: Complex components with multiple internal sub-elements or state machines (e.g., tabs, comboboxes, modals) must be grouped in a dedicated folder. **All internal dependencies must strictly remain inside that folder.**

### 3. Separation and Documentation of Event Loop Handlers

- If a component requires processing outside its own render tree (e.g., `Esc` key dismissal, outside-click detection, focus trap), **provide dedicated event handler functions (e.g., `pub fn handle_event(...)`) inside that component module**.
- Explicitly detail required event loop bindings in the rustdoc documentation.

### 4. Explicit Dependency Scope in Documentation

Document all widgets with clear preconditions using the following template:

```rust
/// Accessible button component compliant with WAI-ARIA Button Pattern.
///
/// # Scope
/// - **Stand-alone**: Can be used by copying only this struct and its impl blocks,
///   provided `core.rs` (`MichiuWidget`, `MichiuTheme`) is present.
///
/// # Required Assets
/// - None
///
/// # Event Loop Requirements
/// - None (Self-contained within widget tree)
///
/// # Feature Details
/// - **Keyboard**: `Space` / `Enter` triggers activation. `Tab` / `Shift+Tab` moves focus.
/// - **Screen Reader**: `Role::Button` (inferred via `tag::<a11y::AButton>()`).
/// - **Visual / WCAG**:
///   - Target Size: >= 24x24px (WCAG 2.5.8 Level AA compliant).
///   - Focus ring: >= 3.0:1 contrast ratio against background (WCAG 1.4.11 / 2.4.11).
/// - **Customization**:
///   - Chain modifier methods directly after `.build(&theme)` to override styles or wire `on_click`.
pub struct SimpleButton { ... }
```

---

## Accessibility & Design Specifications

- **WCAG / WAI-ARIA Compliance**:
  - Keyboard interactions (Tab, Shift+Tab, Arrow keys, Home/End, Enter, Space, Esc, etc.) must operate according to APG specifications.
  - Focus order, AccessKit node `Role`, and states (Selected, Checked, Disabled, etc.) must be accurately synchronized.
- **Styling Flexibility**:
  - Visual styling can be freely customized **as long as it meets WCAG standards (contrast ratios, focus ring visibility, target sizes)**. We encourage clean, unopinionated defaults that are easy to adapt to any brand.

---

## Rules for Listing in the Catalog App (`src/app/`)

When introducing a new component, register it in the catalog app (`main_area.rs`, `sidebar.rs`) to enable real-time verification.

- **UI Components (`catalog/components/`)**:
  - Provide a preview implementation where clicks, keyboard navigation, and screen reader announcements (Windows Narrator / NVDA) can be tested.
- **Themes (`catalog/a11y_theme/`)**:
  - Provide contrast comparison previews validating foreground, background, and focus indicators.
- **Inference Tags (`catalog/inference_tag/`)**:
  - Since inference tags are purely functional, **direct UI previews are not required**.

---

## License

All source code and assets contained in this catalog are dual-licensed under the **MIT License** or **Apache License 2.0**, consistent with the main repository.

### Attribution Waiver

When copying, modifying, or integrating code and assets from this catalog (including `src/components/`, internal helpers, and assets) into your own projects, **no attribution, copyright notice, or inclusion of the license terms is required**, for both commercial and non-commercial use. Feel free to use and adapt them directly as part of your own codebase.

---

## Recommended Tools & Resources

- [Accessible Palette](https://accessiblepalette.com/)
- [Radix Colors](https://www.radix-ui.com/colors)
- [oklch.com](https://oklch.com/)
- [WAI-ARIA Authoring Practices Guide (APG)](https://www.w3.org/WAI/ARIA/apg/patterns/)
- [Web Content Accessibility Guidelines (WCAG) 2.2](https://www.w3.org/TR/WCAG22/)
- [Accessibility Insights for Windows](https://accessibilityinsights.io/docs/windows/overview/)

---

# 原文

このクレートは、WCAG（Web Content Accessibility Guidelines）および WAI-ARIA 仕様に準拠した、アクセシブルな UI コンポーネント・推論タグ・テーマのカタログ集です。

クレート全体の重い依存を強制せず、**「共通トレイトと必要なファイルだけを自分のプロジェクトに持ち帰る」** 思想で設計されています。

---

## カタログアプリの起動

収録されているコンポーネントの見た目、キーボード操作、スクリーンリーダー（Windows ナレーター等）の挙動を直接プレビュー・検証できます。

```bash
# ワークスペースルートから実行
cargo run -p a11y_catalog
```

---

## ディレクトリ構成

コンポーネントは規模に応じて単一ファイルまたはフォルダ（モジュール）で管理され、すべて軽量なコアトレイト（`core.rs`）を基準に設計されています。

```text
a11y_catalog/
├── Cargo.toml
├── README.md
└── src/
    ├── main.rs                  # アプリエントリーポイント
    ├── app.rs                   # アプリケーションのルート構築
    ├── logger.rs                # エラーログ出力
    ├── catalog.rs               # catalog 配下のモジュール宣言
    │
    ├── app/                     # カタログアプリ自身の UI 実装
    │   ├── header.rs
    │   ├── main_area.rs
    │   ├── sidebar.rs
    │   └── theme.rs
    │
    ├── assets/                  # カーソルアイコンや画像などの静的リソース
    │
    ├── catalog/                 # ユーザーが持ち帰るコンポーネント・テーマ構造体など
    │   ├── core.rs              # [必須コア] MichiuWidget & MichiuTheme トレイト & Oklch 定義
    │   ├── a11y_theme.rs        # a11y_theme 配下のモジュール宣言
    │   ├── components.rs        # components 配下のモジュール宣言・カタログ一覧
    │   ├── inference_tag.rs     # inference_tag 配下のモジュール宣言
    │   ├── utils.rs             # utils 配下のモジュール宣言
    │   │
    │   ├── a11y_theme/          # コントラスト比等を満たしたアクセシブルなテーマ定義
    │   │   └── simple_theme.rs # 12段階スケールに基づく WCAG AA 準拠テーマ
    │   │
    │   ├── inference_tag/       # 再利用可能な推論タグ (A11yInferenceTag 実装)
    │   │
    │   ├── utils/               # その他ヘルパー・ユーティリティ
    │   │
    │   ├── components/          # ユーザーが持ち帰るコンポーネント集
    │   │   ├── simple_button.rs        # [単一ファイル] 小規模・自己完結コンポーネント
    │   │   ├── tabs/            # [フォルダモジュール] 肥大化・複数ファイル分割コンポーネント
    │   │   │   ├── mod.rs
    │   │   │   ├── item.rs
    │   │   │   └── panel.rs
```

---

## 使い方（持ち帰り 3 ステップ）

コンポーネントの導入は以下の 3 ステップで直感的に行えます。

```text
ステップ 1: core.rs を持ち帰る
            └── MichiuWidget, MichiuTheme トレイト, Oklch を配置
ステップ 2: テーマを用意する
            └── a11y_theme/ からコピペ、または自前テーマに MichiuTheme を実装
ステップ 3: コンポーネントを持ち帰る
            └── components/ からファイル単位、フォルダ単位、または構造体をコピペ
```

### ステップ 1: `core.rs` を配置する

最初に `src/catalog/core.rs` を自分のプロジェクトへコピーします。コンポーネントとテーマの境界となる最小限のトレイト（`MichiuWidget`, `MichiuTheme`, `Oklch`）が定義されています。

### ステップ 2: テーマを用意する

`MichiuTheme` を満たすテーマを準備します：

- **方法 A**: `catalog/a11y_theme/`（例: `SimpleBlueTheme`）からそのままファイルをコピーして使う。
- **方法 B**: 自前アプリの既存テーマ構造体に `MichiuTheme` トレイトを実装する。

### ステップ 3: `components/` からコンポーネントを配置する

使いたい UI コンポーネントを持ち帰ります：

- **単一ファイルの場合**: `components/button.rs` などを自分のプロジェクトに配置します。
- **フォルダモジュールの場合**: `components/tabs/` などのフォルダを丸ごと配置します。
- **スニペット利用の場合**: 必要な構造体とその `impl MichiuWidget` ブロックだけをコピペして取り込みます。

---

## スタイル・イベントのカスタマイズ

見た目やパディング、クリックイベント（`on_click`）などを変更したい場合は、**`.build(&theme)` の後にメソッドチェーンを繋げます**。

`build(&theme)` の後に繋げたメソッドが **後勝ちでデフォルトのスタイルやハンドラを上書き** します。

```rust
// デフォルトのまま描画する場合
let button = SimpleButton::new("保存").build(&theme);

// スタイルやイベントをカスタマイズする場合（build の後に続けて上書き）
let custom_button = SimpleButton::new("削除")
    .variant(ButtonVariant::Outline)
    .build(&theme)
    // build 後のメソッドチェーンでスタイルを上書き（後勝ち）
    .modify(ii().bg_color(Color::WHITE))
    // イベントハンドラを接続
    .on_click(|| println!("Deleted!"));
```

---

## 設計原則

新しいコンポーネントやテーマを追加する際は、以下の原則を厳守してください。

### 1. ファイルまたはフォルダ単位で完結（自己完結）

- `components/` 以下の各ファイル・フォルダは、`core.rs` を除き**他のカタログ内コンポーネントに一切依存してはいけません**。
- コンポーネントの内部構造体やヘルパー関数は、そのファイル内またはフォルダ内に閉じ込めてください（過度な共通化より重複コードを推奨）。

### 2. 1ファイル / フォルダモジュールの基準

- **単一ファイル（`name.rs`）**: 通常のコンポーネントは 1 ファイルにまとめます。
- **フォルダモジュール（`name/`）**: 構造体や状態遷移が多くコードが肥大化する場合（タブ、コンボボックス、モーダル等）はフォルダを作成して内部を分割してください。**その場合もフォルダ内だけで依存関係を完結させてください。**

### 3. イベントループ・外部ハンドラの分離と明記

- モーダル外クリック検知、`Esc` キー監視、フォーカストラップなど、コンポーネントツリー外のイベント処理が必要な場合は、**専用ハンドラ関数（例: `pub fn handle_event(...)`）をそのモジュール内に同梱** してください。
- 呼び出し手順は rustdoc に必ず明記してください。

### 4. ドキュメントへの依存スコープ明記

提供するコンポーネント構造体には、以下のように rustdoc を記載してください。

```rust
/// Accessible button component compliant with WAI-ARIA Button Pattern.
///
/// # Scope
/// - **Stand-alone**: `core.rs`（`MichiuWidget`, `MichiuTheme`）があれば、
///   この構造体と impl ブロックをコピペするだけで単体動作します。
///
/// # Required Assets
/// - None
///
/// # Event Loop Requirements
/// - None（コンポーネント内部で完結）
///
/// # Feature Details
/// - **Keyboard**: `Space` / `Enter` で発火。`Tab` / `Shift+Tab` でフォーカス移動。
/// - **Screen Reader**: `Role::Button`（`tag::<a11y::AButton>()` により推論）。
/// - **Visual / WCAG**:
///   - ターゲットサイズ: 24x24px 以上を保証（WCAG 2.5.8 レベル AA 適合）。
///   - フォーカスリング: 背景に対して 3.0:1 以上のコントラスト比を確保（WCAG 1.4.11 / 2.4.11）。
/// - **Customization**:
///   - `.build(&theme)` の後にメソッドチェーンを繋げることで、後勝ちでスタイルや `on_click` を上書き可能です。
pub struct SimpleButton { ... }
```

---

## アクセシビリティ・デザイン仕様

- **WCAG / WAI-ARIA 準拠**:
  - キーボード操作（Tab, Shift+Tab, 矢印キー移動, Home/End, Enter, Space, Esc 等）が仕様に沿って動作すること。
  - フォーカス順序、AccessKit ノードの `Role`、状態（Selected, Checked, Disabled など）が適切に反映されていること。
- **デザインの自由度**:
  - 見た目は **WCAG の基準（コントラスト比、フォーカス枠の視認性、ターゲットサイズなど）を満たしていれば自由** です。プロダクトに合わせて改造しやすい、プレーンかつ洗練されたスタイルを推奨します。

---

## カタログアプリ (`src/app/`) への掲載ルール

コンポーネントを追加した際は、利用者が動作を確認できるようアプリ内（`main_area.rs`, `sidebar.rs` 等）に登録してください。

- **UI コンポーネント (`catalog/components/`)**:
  - `MichiuWidget` を実装し、クリック・キーボード遷移・ナレーター読み上げがリアルタイムでテストできるプレビューを実装してください。
- **テーマ (`catalog/a11y_theme/`)**:
  - `MichiuTheme` を実装し、背景色と文字色、フォーカスリングの対比など、簡単な色比較（コントラスト確認）ができる表示を用意してください。
- **推論タグ (`catalog/inference_tag/`)**:
  - タグ単体での UI 表示は難しいため、カタログアプリ内での無理なプレビュー実装は不要です。

---

## ライセンス

本カタログに含まれるすべてのソースコードおよびアセットは、本体リポジトリと同様に **MIT License** または **Apache License 2.0** のデュアルライセンスのもとで公開されています。

### 著作権表示（クレジット）に関する特記事項

当カタログ内のコードをご自身のプロジェクトにコピー＆ペースト、改変、または組み込んで利用する場合、**商用・非商用を問わず、著作権表示やライセンス条項の記載（クレジット表示義務）は不要**です。ご自身のコードベースの一部として自由にご利用いただけます。

---

## 支援ツール・リソース

- [Accessible Palette](https://accessiblepalette.com/)
- [Radix Colors](https://www.radix-ui.com/colors)
- [oklch.com](https://oklch.com/)
- [WAI-ARIA Authoring Practices Guide (APG)](https://www.w3.org/WAI/ARIA/apg/patterns/)
- [Web Content Accessibility Guidelines (WCAG) 2.2](https://www.w3.org/TR/WCAG22/)
- [Accessibility Insights for Windows](https://accessibilityinsights.io/docs/windows/overview/)

---

Created: [2026/10/30]

Last Updated: [2026/11/08]
