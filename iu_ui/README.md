# Accessibility Component Catalog (`iu_ui`)

> **Languages**: [English](#english) | [日本語](#原文)

# English

This crate provides a catalog of accessible UI components, inference tags, and themes compliant with WCAG (Web Content Accessibility Guidelines) and WAI-ARIA specifications.

Instead of enforcing a heavy library-level dependency, it is designed around the philosophy of **"bringing only the core traits and the exact files you need into your own codebase."**

---

## Quick Start via CLI (`cargo-iu_ui`)

The recommended and fastest way to install components into your project is using our dedicated CLI tool.

### 1. Installation

```bash
cargo install cargo-iu_ui
```

### 2. Basic Commands

| Command                         | Description                                                                                 |
| :------------------------------ | :------------------------------------------------------------------------------------------ |
| `cargo iu_ui init`              | Sets up directory structure, downloads `core.rs`, `iu_ui.toml`, and creates `setting.toml`. |
| `cargo iu_ui list`              | Lists all available remote catalog components, themes, and tags with install status.        |
| `cargo iu_ui add <comps...>`    | Downloads and installs one or more components (e.g., `simple_button`, `simple_modal`).      |
| `cargo iu_ui add --all`         | Installs all components from the catalog at once.                                           |
| `cargo iu_ui theme <themes...>` | Downloads and configures accessible color themes (e.g., `simple_theme`).                    |
| `cargo iu_ui tag <tags...>`     | Downloads accessibility inference tags (e.g., `simple_tag`).                                |
| `cargo iu_ui utils <utils...>`  | Downloads shared utility and layout helpers.                                                |
| `cargo iu_ui check`             | Checks installed components and assets against remote for available updates.                |

### 3. Usage Example

```bash
# Initialize iu_ui in your project
cargo iu_ui init

# Check available components
cargo iu_ui list

# Add components (Assets like SVG icons are resolved automatically!)
cargo iu_ui add simple_button simple_input

# Check for updates
cargo iu_ui check
```

Then simply add `mod iu_ui;` to your `src/main.rs` or `src/lib.rs`:

```rust
mod iu_ui;

use iu_ui::prelude::*; // Brings in MichiuWidget, MichiuTheme, and installed components
```

---

## Manual Integration (3-Step Integration)

You can also integrate components manually without using the CLI:

```text
Step 1: Copy core.rs
        └── Brings MichiuWidget & MichiuTheme traits & Oklch
Step 2: Prepare Theme
        └── Copy from theme/ OR implement MichiuTheme on your own theme struct
Step 3: Copy Components
        └── Copy from components/ (by file/folder or struct block)
```

---

## Running the Catalog Preview App

You can directly preview and verify component appearance, keyboard interactions, and screen reader behavior (e.g., Windows Narrator).

```bash
# Run catalog preview app from workspace root
cargo run -p iu_ui --example catalog
```

---

## Directory Structure

```text
iu_ui/
├── Cargo.toml
├── README.md
│
├── examples/
│   └── catalog/             # Internal catalog preview app
│       ├── main.rs              # Application entry point
│       ├── app.rs               # Application root setup
│       ├── components.rs        # Component preview registry
│       ├── logger.rs            # Error logging
│       └── app/                 # Catalog app UI (header, sidebar, etc.)
│
└── src/                 # Exportable library & catalog
    ├── lib.rs               # Library entry point & module re-exports
    ├── core.rs              # [Core Trait] MichiuWidget & MichiuTheme & Oklch
    │
    ├── assets/              # Static assets such as cursor icons and SVGs
    │
    ├── theme/               # Accessible themes meeting contrast ratios
    │   ├── mod.rs
    │   └── simple_theme.rs      # Default 12-scale WCAG AA palette
    │
    ├── components/          # Exportable component collection
    │   ├── mod.rs
    │   ├── simple_button.rs     # [Single File] Self-contained components
    │   ├── tabs/                # [Folder Module] Complex multi-file components
    │   │   ├── mod.rs
    │   │   ├── item.rs
    │   │   └── panel.rs
    │   └── ...                  # (accordion, checkbox, modal, slider, etc.)
    │
    ├── tag/                 # Reusable inference tags (A11yInferenceTag implementations)
    │   ├── mod.rs
    │   └── simple_tag.rs
    │
    └── utils/               # Shared internal helpers
        ├── mod.rs
        └── helper.rs
```

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

## Guidelines for Authors & Contributors (Developer Guide)

When authoring new components or maintaining the catalog, strictly adhere to the following conventions:

### 1. Internal Import Convention (`use super::core::*`)

To ensure that components work seamlessly both inside the catalog repository and within end-users' projects without path errors:

- **Always import core traits via `super::core`**:
  ```rust
  use super::core::{MichiuTheme, MichiuWidget, Oklch};
  ```
- Each category's `mod.rs` (e.g., `src/components/mod.rs`, `src/theme/mod.rs`) re-exports core privately via:
  ```rust
  pub(crate) use super::core;
  ```
  _(Never use `use crate::core::*;` directly inside component files, as `crate::` resolves to the user's application root when exported)._

### 2. Updating `registry.json`

The catalog index (`registry.json`) is generated automatically from Git commit histories:

- **Do not update `registry.json` on every tiny edit.**
- Run the registry builder **only after a batch of component changes or additions has settled**:
  ```bash
  cargo run -p registry_builder
  ```

### 3. Self-Contained within File or Folder

- Each file or directory under `components/` **must be completely self-contained** (except for depending on `core.rs` traits).
- Components **must never depend on other catalog components**. Code duplication is intentionally encouraged over cross-component coupling.

### 4. Asset Reference Guidelines

When referencing assets from within a component, embed the binaries using a relative path from the `components/` directory. The CLI will automatically detect these assets and download them:

```rust
// In src/iu_ui/components/tab/mod.rs
const OPEN_ICON: &[u8] = include_bytes!("../../asset/open.svg");

// In src/iu_ui/components/simple_button.rs
const CLOSE_ICON: &[u8] = include_bytes!("../asset/close.svg");
```

### 5. Explicit Dependency Scope in Documentation

Document all widgets with clear preconditions using the following template:

```rust
use super::core::{MichiuTheme, MichiuWidget, Oklch};

/// Accessible button component compliant with WAI-ARIA Button Pattern.
///
/// # Scope
/// - **Stand-alone**: Can be used by copying only this struct and its impl blocks,
///   provided `core.rs` (`MichiuWidget`, `MichiuTheme`) is present.
///
/// # Required Assets
/// - None (or `close.svg`)
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

## Recommended Tools & Resources

- [Accessible Palette](https://accessiblepalette.com/)
- [Radix Colors](https://www.radix-ui.com/colors)
- [oklch.com](https://oklch.com/)

---

- [WAI-ARIA Authoring Practices Guide (APG)](https://www.w3.org/WAI/ARIA/apg/patterns/)
- [Web Content Accessibility Guidelines (WCAG) 2.2](https://www.w3.org/TR/WCAG22/)
- [WCAG2ICT](https://www.w3.org/WAI/standards-guidelines/wcag/non-web-ict/)
- [EN 301 549 V4.1.1](https://www.etsi.org/deliver/etsi_en/301500_301599/301549/04.01.01_60/en_301549v040101p.pdf)
- [Section 508](https://www.section508.gov/)

---

- [Accessibility Insights for Windows](https://accessibilityinsights.io/docs/windows/overview/)

---

## License & Attribution Waiver

All source code and assets contained in this catalog are dual-licensed under the **MIT License** or **Apache License 2.0**.

### Attribution Waiver

When copying, modifying, or integrating code and assets from this catalog into your own projects, **no attribution, copyright notice, or inclusion of the license terms is required**, for both commercial and non-commercial use.

---

# 原文

このクレートは、WCAG（Web Content Accessibility Guidelines）および WAI-ARIA 仕様に準拠した、アクセシブルな UI コンポーネント・推論タグ・テーマのカタログ集です。

クレート全体の重い依存を強制せず、**「共通トレイトと必要なファイルだけを自分のプロジェクトに持ち帰る」** 思想で設計されています。

---

## CLI でのクイックスタート (`cargo-iu_ui`)

専用の CLI ツールを使うことで、必要なコンポーネントやアセットを自分のプロジェクトへ安全かつ即座に導入できます。

### 1. ツールのインストール

```bash
cargo install cargo-iu_ui
```

### 2. 基本コマンド一覧

| コマンド                        | 説明                                         | 備考                             |
| :------------------------------ | :------------------------------------------- | :------------------------------- |
| `cargo iu_ui init`              | フォルダ構成・`core.rs`・`iu_ui.toml` を配置 | プロジェクト初期化               |
| `cargo iu_ui list`              | リモートカタログの一覧と現在の導入状況を表示 | ターミナル上で閲覧可能           |
| `cargo iu_ui add <comps...>`    | 1つまたは複数のコンポーネントを追加          | 競合時: 上書き/リネーム/スキップ |
| `cargo iu_ui add --all`         | カタログ内の全コンポーネントを一括追加       |                                  |
| `cargo iu_ui theme <themes...>` | アクセシブルなカラーテーマを追加             | `src/iu_ui/theme/` へ            |
| `cargo iu_ui tag <tags...>`     | 推論タグを追加                               | `src/iu_ui/tag/` へ              |
| `cargo iu_ui utils <utils...>`  | 共通ユーティリティ関数を追加                 | `src/iu_ui/utils/` へ            |
| `cargo iu_ui check`             | インストール済みコンポーネントの更新確認     | `setting.toml` と最新版を照合    |

### 3. 使い方の流れ

```bash
# プロジェクト直下で初期化（フォルダと core.rs を自動配置）
cargo iu_ui init

# 利用可能なコンポーネント一覧を確認
cargo iu_ui list

# ボタンと入力を追加（依存する SVG アイコン等も自動でダウンロードされます）
cargo iu_ui add simple_button simple_input

# 本家に更新があるか確認
cargo iu_ui check
```

あとはご自身の `src/main.rs` または `src/lib.rs` に `mod iu_ui;` を追加するだけで使えます：

```rust
mod iu_ui;

use iu_ui::prelude::*; // MichiuWidget, MichiuTheme や追加したコンポーネントが一括インポートされます
```

---

## 手動での使い方（持ち帰り 3 ステップ）

CLI を使わず、手動でコピペして持ち帰ることも可能です：

```text
ステップ 1: core.rs を持ち帰る
            └── MichiuWidget, MichiuTheme トレイト, Oklch を配置
ステップ 2: テーマを用意する
            └── theme/ からコピペ、または自前テーマに MichiuTheme を実装
ステップ 3: コンポーネントを持ち帰る
            └── components/ からファイル/フォルダ単位、または構造体をコピペ
```

---

## カタログアプリの起動

収録されているコンポーネントの見た目、キーボード操作、スクリーンリーダー（Windows ナレーター等）の挙動を直接プレビュー・検証できます。

```bash
# ワークスペースルートから実行
cargo run -p iu_ui --example catalog
```

---

## ディレクトリ構成

```text
iu_ui/
├── Cargo.toml
├── README.md
│
├── examples/
│   └── catalog/             # カタログ検証用プレビューアプリ
│       ├── main.rs              # アプリエントリーポイント
│       ├── app.rs               # アプリケーションのルート構築
│       ├── components.rs        # プレビュー表示用カタログ一覧
│       ├── logger.rs            # エラーログ出力
│       └── app/                 # カタログアプリ自身の UI 実装（header, sidebar 等）
│
└── src/                 # 配布用ライブラリ・コンポーネント本体
    ├── lib.rs               # ライブラリエントリーポイント & モジュール公開
    ├── core.rs              # [必須コア] MichiuWidget & MichiuTheme トレイト & Oklch 定義
    │
    ├── assets/              # カーソルアイコンや SVG などの静的リソース
    │
    ├── theme/               # コントラスト比等を満たしたアクセシブルなテーマ定義
    │   ├── mod.rs
    │   └── simple_theme.rs      # 12段階スケールに基づく WCAG AA 準拠テーマ
    │
    ├── components/          # ユーザーが持ち帰るコンポーネント集
    │   ├── mod.rs
    │   ├── simple_button.rs     # [単一ファイル] 自己完結コンポーネント
    │   ├── tabs/                # [フォルダモジュール] 肥大化・複数ファイル分割コンポーネント
    │   │   ├── mod.rs
    │   │   ├── item.rs
    │   │   └── panel.rs
    │   └── ...                  # (accordion, checkbox, modal, slider 等)
    │
    ├── tag/                 # 再利用可能な推論タグ (A11yInferenceTag 実装)
    │   ├── mod.rs
    │   └── simple_tag.rs
    │
    └── utils/               # その他内部ヘルパー・ユーティリティ
        ├── mod.rs
        └── helper.rs
```

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

## 開発者・メンテナー向けガイドライン

コンポーネントを新規作成・改修する際は、以下のルールを厳守してください。

### 1. インポート規約 (`use super::core::*`)

ユーザーがどの階層に `iu_ui` を配置してもパス解決エラー（`cannot find crate::core`）が起きないようにするため、以下のルールを徹底します：

- **コアトレイトのインポートは必ず `super::core` を使用すること**:
  ```rust
  use super::core::{MichiuTheme, MichiuWidget, Oklch};
  ```
- 各カテゴリの `mod.rs`（`src/components/mod.rs`, `src/theme/mod.rs` 等）において、あらかじめ以下のように core を非公開再エクスポートしています：
  ```rust
  pub(crate) use super::core;
  ```
  _(コンポーネント内部で `use crate::core::*;` を直接使うことは禁止です。持ち帰り先ユーザーのクレートルートを探しに行ってしまうため)_

### 2. `registry.json` の更新タイミング

カタログ目録（`registry.json`）は Git のコミットハッシュから自動算出されます：

- 1行直すごとに頻繁に更新する必要はありません。
- **コンポーネントの追加や仕様変更がある程度まとまって完了してから**、以下のコマンドを実行して `registry.json` を再生成してください：
  ```bash
  cargo run -p registry_builder
  ```

### 3. ファイルまたはフォルダ単位で完結

- `components/` 以下の各ファイル・フォルダは、`core.rs` を除き**他のカタログ内コンポーネントに一切依存してはいけません**。
- コンポーネントの内部構造体やヘルパー関数は、そのファイル内またはフォルダ内に閉じ込めてください（過度な共通化より重複コードを推奨）。

### 4. アセットの参照規約

コンポーネント側からアセットを参照する際は、`components/` から見た相対パスでバイナリ埋め込みを行います。CLI はこの記述を正規表現で検知し、アセットを自動ダウンロード対象として登録します：

```rust
// src/iu_ui/components/tab/mod.rs
const OPEN_ICON: &[u8] = include_bytes!("../../asset/open.svg");

// src/iu_ui/components/simple_button.rs
const CLOSE_ICON: &[u8] = include_bytes!("../asset/close.svg");
```

### 5. ドキュメントへの依存スコープ明記

提供するコンポーネント構造体には、以下のように rustdoc を記載してください。

```rust
use super::core::{MichiuTheme, MichiuWidget, Oklch};

/// Accessible button component compliant with WAI-ARIA Button Pattern.
///
/// # Scope
/// - **Stand-alone**: `core.rs`（`MichiuWidget`, `MichiuTheme`）があれば、
///   この構造体と impl ブロックをコピペするだけで単体動作します。
///
/// # Required Assets
/// - None（アセットがある場合はファイル名を明記）
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

## 支援ツール・リソース

- [Accessible Palette](https://accessiblepalette.com/)
- [Radix Colors](https://www.radix-ui.com/colors)
- [oklch.com](https://oklch.com/)

---

- [WAI-ARIA Authoring Practices Guide (APG)](https://www.w3.org/WAI/ARIA/apg/patterns/)
- [Web Content Accessibility Guidelines (WCAG) 2.2](https://www.w3.org/TR/WCAG22/)
- [WCAG2ICT](https://www.w3.org/WAI/standards-guidelines/wcag/non-web-ict/)
- [EN 301 549 V4.1.1](https://www.etsi.org/deliver/etsi_en/301500_301599/301549/04.01.01_60/en_301549v040101p.pdf)
- [Section 508](https://www.section508.gov/)
- [JIS X 8341-3:2016 達成基準 早見表（レベルA & AA）](https://waic.jp/resource/jis-x-8341-3-2016/)
- [JIS規格 簡易閲覧サイト (JIS X 8341-6)](https://kikakurui.com/x8/X8341-6-2013-01.html)
- [デジタル庁デザインシステム](https://design.digital.go.jp/dads/)

---

- [Accessibility Insights for Windows](https://accessibilityinsights.io/docs/windows/overview/)

---

## ライセンス & クレジット表示免除

本カタログに含まれるすべてのソースコードおよびアセットは、本体リポジトリと同様に **MIT License** または **Apache License 2.0** のデュアルライセンスのもとで公開されています。

### 著作権表示（クレジット）に関する特記事項

当カタログ内のコードをご自身のプロジェクトにコピー＆ペースト、改変、または組み込んで利用する場合、**商用・非商用を問わず、著作権表示やライセンス条項の記載（クレジット表示義務）は不要**です。ご自身のコードベースの一部として自由にご利用いただけます。

---

Created: [2026/10/30]

Last Updated: [2026/11/09]
