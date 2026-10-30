# Accessibility Component Catalog (`a11y_catalog`)

> **Languages**: [English](#english) | [日本語](#原文)

# English

This crate provides a catalog of accessible UI components, inference tags, and themes compliant with WCAG (Web Content Accessibility Guidelines) and WAI-ARIA specifications.

Instead of enforcing a library-level dependency, it is designed around the philosophy of **"bringing only the files and functions you need into your own project."**

---

## Running the Catalog App

You can directly preview and verify component appearance, keyboard interactions, and screen reader behavior (e.g., Windows Narrator).

```bash
# Run from the workspace root
cargo run -p a11y_catalog
```

---

## Directory Structure

Components are managed as single files or folder modules depending on their complexity.

```text
a11y_catalog/
├── Cargo.toml
├── README.md
└── src/
    ├── main.rs                  # Application entry point
    ├── app.rs                   # Application root setup
    ├── logger.rs                # Error logging
    ├── components.rs            # Module declarations and catalog registry
    │
    ├── app/                     # Internal catalog app UI
    │   ├── header.rs
    │   ├── main_area.rs
    │   ├── sidebar.rs
    │   └── theme.rs
    │
    ├── assets/                  # Static assets such as cursor icons and images
    │
    └── components/              # Exportable component collection
        ├── button.rs            # [Single File] Lightweight components
        ├── tabs/                # [Folder Module] Complex components split across files
        │   ├── mod.rs
        │   ├── item.rs
        │   └── panel.rs
        ├── a11y_theme/          # Accessible themes meeting contrast ratios
        ├── inference_tag/       # Reusable inference tags (A11yInferenceTag implementations)
        └── utils/               # Internal helpers and utilities
```

---

## How to Use

Choose the approach that best fits your needs:

### 1. Copying Files or Folders into Your Project

- **Single file**: Download and place files like `components/button.rs` into your project.
- **Folder module**: Copy entire directories like `components/tabs/` into your project.
  In either case, **components have zero dependencies on other catalog components**, allowing immediate compilation by simply declaring the module (`mod`).

### 2. Copying Individual Functions

Check the rustdoc documentation for each function. If marked as self-contained, you can copy-paste the function block directly into your existing code.

### 3. When Assets Are Required

Some components may require external resources like custom cursor icons or images.
Copy the assets specified in the function's documentation (under `src/assets/`) into your project's resource directory.

---

## Design Principles

Follow these strict guidelines when contributing new components or themes:

### 1. Self-Contained

- Each file or directory under `components/` **must never depend on other components within the catalog**.
- To prevent cascading dependencies (e.g., "copying a button also requires shared utilities and other components"), **code duplication (such as private internal helpers) is intentionally accepted and encouraged**, except where logic becomes excessively bloated.

### 2. Single File vs. Folder Module

- **Single File (`name.rs`)**: Standard components should be consolidated into one file.
- **Folder Module (`name/`)**: When components require complex state machines or multiple structs (e.g., tabs, comboboxes, modals), organize them in a dedicated folder. **All internal dependencies must strictly remain inside that folder.**

### 3. Minimal Public API

- Keep public APIs as small as possible—ideally a **single public function** (e.g., `pub fn tabs(...) -> Entity`).
- Internal structs, helper functions, and Modifier composition logic should remain private or scoped to `pub(crate)`.

### 4. Asset Placement and Explicit Documentation

- Place external resources (cursor icons, images, etc.) in **`src/assets/`**.
- Explicitly state the paths of any required assets in the function documentation.

### 5. Separation and Documentation of Event Loop Handlers

- Certain components require processing outside their own render tree, specifically **within the application event loop (main loop or window events)**.
  - Examples: Outside-click detection for modals/popovers, listening for the `Esc` key, or app-wide focus trap handling.
- When such handling is required, **provide dedicated event handlers (e.g., `pub fn handle_event(...)`) within the same component module** so consumers can wire them up with minimal glue code.
- Explicitly detail the required event loop integrations and how to call them in the rustdoc.

### 6. Explicit Dependency Scope in Documentation

Document all provided functions with clear preconditions using the following template:

```rust
/// Complex accessible modal component with focus trapping.
///
/// # Scope
/// - **Folder-required**: Requires copying the entire `components/modal/` directory.
///
/// # Required Assets
/// - None
///
/// # Event Loop Requirements
/// - Must call `modal::handle_key_event(&event)` from the window key event handler
///   (or main loop) to handle dismissals on `Esc`.
/// - Must forward pointer events to `modal::handle_pointer_event(&event)` to detect
///   clicks outside the modal container.
///
/// # Feature Details
/// - Complies with the WAI-ARIA Dialog (Modal) pattern.
/// - Traps focus inside the modal dialog.
pub fn modal(...) -> Entity { ... }

/// Complex accessible tabs component.
///
/// # Scope
/// - **Folder-required**: Requires copying the entire `components/tabs/` directory.
///
/// # Required Assets
/// - None
///
/// # Event Loop Requirements
/// - None (Self-contained within the component)
///
/// # Feature Details
/// -
pub fn tabs(...) -> Entity { ... }

/// Accessible button component with custom cursor.
///
/// # Scope
/// - **File-required**: Requires copying the entire file.
///
/// # Required Assets
/// - `src/assets/cursor_pointer.png`
///
/// # Event Loop Requirements
/// - None
///
/// # Feature Details
/// -
pub fn custom_cursor_button(...) -> Entity { ... }

/// Simple accessible button.
///
/// # Scope
/// - **Stand-alone**: Can be used by copying only this function.
///
/// # Required Assets
/// - None
///
/// # Event Loop Requirements
/// - None
///
/// # Feature Details
/// -
pub fn simple_button(...) -> Entity { ... }
```

---

## Accessibility & Design Specifications

- **WCAG / WAI-ARIA Compliance**:
  - Keyboard interactions (Tab, Shift+Tab, Arrow keys, Home/End, Enter, Space, Esc, etc.) must operate according to standard specifications.
  - Focus order, AccessKit node `Role`, and states (Selected, Checked, etc.) must be accurately synchronized.
- **Styling Flexibility**:
  - Visual styling (colors, corner radii, padding, etc.) can be freely customized **as long as it meets WCAG standards (contrast ratios, focus ring visibility, etc.)**. We encourage clean, unopinionated defaults that are easy to adapt to any brand.

---

## Rules for Listing in the Catalog App (`src/app/`)

When introducing a new component, include preview displays in the catalog app (`main_area.rs`, etc.) to allow manual verification.

- **UI Components (`components/`)**:
  - Implement previews where clicks, keyboard navigation, and screen reader announcements can be verified in real time.
- **Themes (`components/a11y_theme/`)**:
  - Provide a contrast comparison display showing foreground/background colors and focus indicators.
- **Inference Tags (`components/inference_tag/`)**:
  - Since inference tags are purely functional, **direct UI previews are not required**.

---

## License

All source code and assets contained in this catalog are dual-licensed under the **MIT License** or **Apache License 2.0**, consistent with the main repository.

### Attribution Waiver

When copying, modifying, or integrating code and assets from this catalog (including `src/components/`, internal helpers, and assets) into your own projects, **no attribution, copyright notice, or inclusion of the license terms is required**, for both commercial and non-commercial use. Feel free to use and adapt them directly as part of your own codebase.

---

# 原文

このクレートは、WCAG（Web Content Accessibility Guidelines）および WAI-ARIA 仕様に準拠した、アクセシブルな UI コンポーネント・推論タグ・テーマのカタログ集です。

クレートとしての依存を強制せず、**「必要なファイルや関数だけを自分のプロジェクトに持ち帰る」** 思想で設計されています。

---

## カタログアプリの起動

収録されているコンポーネントの見た目、キーボード操作、スクリーンリーダー（Windows ナレーター等）の挙動を直接プレビュー・検証できます。

```bash
# ワークスペースルートから実行
cargo run -p a11y_catalog
```

---

## ディレクトリ構成

コンポーネントは規模に応じて単一ファイルまたはフォルダ（モジュール）で管理されています。

```text
a11y_catalog/
├── Cargo.toml
├── README.md
└── src/
    ├── main.rs                  # アプリエントリーポイント
    ├── app.rs                   # アプリケーションのルート構築
    ├── logger.rs                # エラーログ出力
    ├── components.rs            # components 配下のモジュール宣言・カタログ一覧
    │
    ├── app/                     # カタログアプリ自身の UI 実装
    │   ├── header.rs
    │   ├── main_area.rs
    │   ├── sidebar.rs
    │   └── theme.rs
    │
    ├── assets/                  # カーソルアイコンや画像などの静的リソース
    │
    └── components/              # ユーザーが持ち帰るコンポーネント集
        ├── button.rs            # [単一ファイル] 小規模なコンポーネント
        ├── tabs/                # [フォルダモジュール] 肥大化・複数ファイルに分割されたコンポーネント
        │   ├── mod.rs
        │   ├── item.rs
        │   └── panel.rs
        ├── a11y_theme/          # コントラスト比等を満たしたアクセシブルなテーマ定義
        ├── inference_tag/       # 再利用可能な推論タグ (A11yInferenceTag 実装)
        └── utils/               # その他ヘルパー・ユーティリティ
```

---

## 使い方

コンポーネントの導入方法は規模や用途に合わせて選択できます。

### 1. ファイルまたはフォルダごとプロジェクトに配置する

- **単一ファイルの場合**: `components/button.rs` などを自分のプロジェクトにダウンロード・配置します。
- **フォルダモジュールの場合**: `components/tabs/` などのフォルダを丸ごと自分のプロジェクトへ配置します。
  どちらの場合も**カタログ内の他のコンポーネントへの依存は一切ない**ため、配置して `mod` 宣言するだけで即座にビルド可能です。

### 2. 関数単位でコピペする

各ファイルの関数のドキュメント（rustdoc）を確認し、単体動作が可能なものであれば関数ブロックごと自分のコードへ貼り付けて利用できます。

### 3. アセット（画像・カーソルアイコン等）が必要な場合

コンポーネントによってはカスタムカーソルや画像等のリソースを必要とする場合があります。
関数のドキュメントに指定されたアセット（`src/assets/` 配下）を自分のプロジェクトの適切なリソースフォルダへコピーして使用してください。

---

## 設計原則

新しいコンポーネントやテーマを追加する際は、以下の原則を厳守してください。

### 1. 自己完結

- `components/` 以下の各ファイル・フォルダは、**他のカタログ内コンポーネントに一切依存してはいけません**。
- 「ボタンをコピーしたら共通ユーティリティや別コンポーネントも必要になった」という事態を完全に防ぐため、**極端に肥大化するロジックを除き、コードの重複（内部ヘルパー関数など）は意図的に許容・推奨**します。

### 2. 1ファイル / フォルダモジュールの基準

- **単一ファイル（`name.rs`）**: 通常のコンポーネントは1ファイルにまとめます。
- **フォルダモジュール（`name/`）**: 構造体や状態遷移が多くコードが肥大化する場合（タブ、コンボボックス、モーダル等）は、フォルダを作成して内部を分割してください。 **ただし、そのフォルダ内だけで依存関係を完結させてください。**

### 3. 最小限のパブリック API

- 外部に公開する API は、可能な限り**単一（または最小限）の `pub fn`**（例: `pub fn tabs(...) -> Entity`）としてください。
- 内部で使用する構造体、プライベートなヘルパー関数、Modifier の構築ロジックなどは極力 `pub(crate)` や非 `pub` に留めてください。

### 4. アセットの配置と明記

- カーソルアイコンや画像など外部リソースが必要な場合は **`src/assets/`** に配置してください。
- 関数のドキュメント内に必要なアセットのパスを必ず記載してください。

### 5. イベントループ・外部ハンドラの分離と明記

- コンポーネントによっては、内部の描画・状態管理だけでなく、**イベントループ側（メインループやウィンドウイベント）での追加処理** を必要とする場合があります。
  - 例: モーダルやポップオーバーの枠外クリックでのクローズや、`Esc` キー監視
  - 例: アプリ全体を対象とするフォーカストラップの監視
- このような処理が必要な場合は、**イベントループに流し込むための専用ハンドラ関数（例: `pub fn handle_event(...)`）をコンポーネント側で同梱して提供** し、利用者が最小限の配線コードを追加するだけで動くようにしてください。
- 必要なイベントループ側の処理・呼び出し方法は、rustdoc 内に必ず明記してください。

### 6. ドキュメントへの依存スコープ明記

提供する関数には、以下のように利用条件を rustdoc で明記してください。

```rust
/// Complex accessible modal component with focus trapping.
///
/// # Scope
/// - **Folder-required**: `components/modal/` フォルダ丸ごとの配置が必要です。
///
/// # Required Assets
/// - None
///
/// # Event Loop Requirements
/// - ウィンドウのキーイベント処理（またはメインループ）側で、`Esc` キー押下時にダイアログを閉じるため
///   本モジュールの `modal::handle_key_event(&event)` を呼び出す必要があります。
/// - ポップアップの枠外クリック検知のため、マウスクリックイベントを `modal::handle_pointer_event(&event)` へ流す必要があります。
///
/// # Feature Details
/// - WAI-ARIA Dialog (Modal) パターンに準拠。
/// - フォーカスがモーダル外へ逃げないようトラップします。
pub fn modal(...) -> Entity { ... }

/// Complex accessible tabs component.
///
/// # Scope
/// - **Folder-required**: `components/tabs/` フォルダ丸ごとの配置が必要です。
///
/// # Required Assets
/// - None
///
/// # Event Loop Requirements
/// - None（コンポーネント内部で完結）
///
/// # Feature Details
/// -
pub fn tabs(...) -> Entity { ... }

/// Accessible button component with custom cursor.
///
/// # Scope
/// - **File-required**: このファイル全体の配置が必要です。
///
/// # Required Assets
/// - `src/assets/cursor_pointer.png`
///
/// # Event Loop Requirements
/// - None
///
/// # Feature Details
/// -
pub fn custom_cursor_button(...) -> Entity { ... }

/// Simple accessible button.
///
/// # Scope
/// - **Stand-alone**: この関数のみをコピペして利用可能です。
///
/// # Required Assets
/// - None
///
/// # Event Loop Requirements
/// - None
///
/// # Feature Details
/// -
pub fn simple_button(...) -> Entity { ... }
```

---

## アクセシビリティ・デザイン仕様

- **WCAG / WAI-ARIA 準拠**:
  - キーボード操作（Tab, Shift+Tab, 矢印キー移動, Home/End, Enter, Space, Esc 等）が仕様に沿って動作すること。
  - フォーカス順序、AccessKit ノードの `Role`、状態（Selected, Checked など）が適切に反映されていること。
- **デザインの自由度**:
  - 見た目（カラー、角丸、パディングなど）は **WCAG の基準（コントラスト比やフォーカス枠の視認性など）を満たしていれば自由** です。プロダクトに合わせて改造しやすい、プレーンかつ洗練されたスタイルを推奨します。

---

## カタログアプリ (`src/app/`) への掲載ルール

コンポーネントを追加した際は、利用者が動作を確認できるようアプリ内（`main_area.rs` 等）にもプレビュー表示を追加することを推奨します。

- **UI コンポーネント (`components/`)**:
  - アプリ内で実際にクリック・キーボード遷移・ナレーター読み上げがテストできるプレビューを実装してください。
- **テーマ (`components/a11y_theme/`)**:
  - 背景色と文字色、フォーカスリングの対比など、簡単な色比較（コントラスト確認）ができる表示を用意してください。
- **推論タグ (`components/inference_tag/`)**:
  - タグ単体での UI 表示は難しいため、**カタログアプリ内での無理なプレビュー実装は不要**です。

---

## ライセンス

本カタログに含まれるすべてのソースコードおよびアセットは、本体リポジトリと同様に **MIT License** または **Apache License 2.0** のデュアルライセンスのもとで公開されています。

### 著作権表示（クレジット）に関する特記事項

当カタログ内のコード（`src/components/`、その他ヘルパー、アセットを含む）をご自身のプロジェクトにコピー＆ペースト、改変、または組み込んで利用する場合、**商用・非商用を問わず、著作権表示やライセンス条項の記載（クレジット表示義務）は不要**です。ご自身のコードベースの一部として自由にご利用いただけます。

---

Created: [2026/10/14]

Last Updated: [2026/10/30]
