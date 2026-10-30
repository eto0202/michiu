# Accessibility (a11y) Guide & Policy

> **Languages**: [English](#english) | [日本語](#原文)

---

# English

[michiu](https://github.com/eto0202/michiu) aims to enable developers to build accessible desktop applications on Windows that are easy for everyone to use.

Accessibility is achieved through a combination of the **foundation provided by the library** and **the application developer’s design**. This document describes the library’s accessibility foundation, best practices for developers, and recommended verification procedures.

For specific instructions on how to use the accessibility APIs provided by the library, please refer to `accessibility.rs` in the `examples` folder.

## Table of Contents

- [Library Accessibility Infrastructure](#library-accessibility-infrastructure)
- [Guidelines for App Developers](#guidelines-for-app-developers)
  - [1. Accessible Names](#1-accessible-names)
  - [2. Logical Focus Order](#2-logical-focus-order)
  - [3. Contrast and Avoiding Reliance on Color](#3-contrast-and-avoiding-reliance-on-color)
- [How to Test](#how-to-test)
  - [Step 1: Automated Checks and UI Tree Verification](#step-1-automated-checks-and-ui-tree-verification)
  - [Step 2: Keyboard Navigation Testing](#step-2-keyboard-navigation-testing)
  - [Step 3: Testing with a Screen Reader](#step-3-testing-with-a-screen-reader)
- [Known Issues and Roadmap for the Library](#known-issues-and-roadmap-for-the-library)
- [Feedback and Issue Reporting](#feedback-and-issue-reporting)

---

## Library Accessibility Infrastructure

This library integrates **[AccessKit](https://accesskit.dev/)** as an accessibility abstraction layer. This enables it to work with the OS-native **Windows UI Automation (UIA)** and provides the following:

1. **Default Semantics**:
   - Through automatic inference, it conveys the appropriate default _Role_ and _State_ to UIA via AccessKit based on the attributes and capabilities of the `Entity`.
2. **Focus and Keyboard Navigation**:
   - Provides a focus navigation mechanism compliant with OS standards via the `focusable` API.
3. **Developer APIs**:
   - Provides APIs for assigning accessibility labels and descriptions to each `Entity`.

---

## Guidelines for App Developers

No matter how well the library lays the groundwork, an app will not be accessible unless developers configure the appropriate information. When developing apps using this library, please keep the following points in mind.

### 1. Accessible Names

- For buttons consisting solely of icons or graphical input fields, be sure to use this library’s label configuration API to specify text that screen readers will read aloud.
  - Example:
  ```rust
    div_n().a11y_label(Role::Button, "Accessible Name")
  ```

### 2. Logical Focus Order

- Design the UI so that the navigation order using the Tab key matches the visual layout of the screen (typically from left to right and top to bottom).

### 3. Contrast and Avoiding Reliance on Color

- Ensure sufficient contrast between the background color and the text.
- Do not rely solely on color (such as “red”) to convey important states, such as errors; explicitly indicate them using icons and text as well.

---

## How to Test

When verifying app accessibility, we recommend incorporating the following tools and procedures into your development workflow.

### Step 1: Automated Checks and UI Tree Verification

Use Microsoft tools to verify that nodes are being correctly passed to the UIA via AccessKit.

- **[Accessibility Insights for Windows](https://accessibilityinsights.io/docs/windows/overview/)** (Recommended)
  - Simply hover your cursor over the app window to visualize the structure of the accessibility tree.
  - The “FastPass” feature automatically detects major issues, such as missing labels, with a single click.
- **Inspect.exe** (included with the Windows SDK)
  - You can verify in detail whether UIA properties (Name, Role, BoundingRectangle, etc.) are being output as intended.

### Step 2: Keyboard Navigation Testing

Try navigating the app without using the mouse.

- **Tab / Shift+Tab**: Does focus move sequentially to all interactive entities?
- **Focus Visualization**: Is a border displayed that makes it immediately clear where the focus is currently located? Outlines are typically used for this.
  - Example:
  ```rust
    ii().focusable_self_keyboard().focused(
        ii().outline_solid(2.0)
            .outline_offset(2.0)
            .outline_color(Color::WHITE),
    )
  ```
- **Enter / Space / Arrow Keys**: Can you execute buttons or select items in lists using only the keyboard?

### Step 3: Testing with a Screen Reader

Check how the content is actually conveyed through audio.

- **Windows Narrator**: You can start and stop it at any time by pressing `Win + Ctrl + Enter`.
- **NVDA**: A practical screen reader for Windows.
  - While navigating with the keyboard, verify that the component’s name (Label), type (e.g., Button), and state (e.g., Checked) are read aloud correctly.

---

## Known Issues and Roadmap for the Library

We are currently working on expanding the following features in the library:

- [ ] Optimizing the automatic inference of AccessKit node trees in complex components.
- [ ] Reading scrollbars and scroll positions.
- [ ] Responding to input from accessibility tools.
- [ ] Refining `ACCESSIBILITY.md`.
- [ ] Refining issue templates.
- [ ] Providing more detailed explanations of API usage (`accessibility.rs`) and expanding the documentation.
- [ ] Expansion of `a11y_catalog`

---

## Feedback and Issue Reporting

If you discover any issues with the library’s internal AccessKit integration, keyboard traps, focus bugs, or other problems, please report them via a [GitHub Issue](https://github.com/eto0202/michiu/issues).

- **Open an issue** using the [accessibility issue template](https://github.com/eto0202/michiu/issues/new?template=accessibility_issue.md).

If possible, please include details such as “detection results from Accessibility Insights” or “Narrator behavior” and set the label to `accessibility` to help us investigate the issue more efficiently.

---

# 原文

[michiu](https://github.com/eto0202/michiu) は、開発者が Windows 上で誰もが使いやすいアクセシブルなデスクトップアプリケーションを構築できることを目指します。

アクセシビリティは **ライブラリの提供する基盤** と **アプリケーション開発者の設計** の共同作業によって実現されます。このドキュメントでは、本ライブラリのアクセシビリティ基盤、開発者向けのベストプラクティス、および推奨する検証手順について説明します。

ライブラリが提供するアクセシビリティ API の具体的な使用方法については、 `examples` フォルダにある `accessibility.rs` を確認してください。

## 目次

- [ライブラリのアクセシビリティ基盤](#ライブラリのアクセシビリティ基盤)
- [アプリ開発者へのガイドライン](#アプリ開発者へのガイドライン)
  - [1. ラベル（Accessible Name）の付与](#1-ラベルaccessible-nameの付与)
  - [2. 論理的なフォーカス順序](#2-論理的なフォーカス順序)
  - [3. コントラストと色への依存回避](#3-コントラストと色への依存回避)
- [アプリ開発時の推奨検証手順 (How to Test)](#アプリ開発時の推奨検証手順-how-to-test)
  - [ステップ 1: 自動チェック・UI ツリーの確認](#ステップ-1-自動チェックui-ツリーの確認)
  - [ステップ 2: キーボード操作テスト](#ステップ-2-キーボード操作テスト)
  - [ステップ 3: スクリーンリーダーによる読み上げテスト](#ステップ-3-スクリーンリーダーによる読み上げテスト)
- [ライブラリ側の既知の課題とロードマップ](#ライブラリ側の既知の課題とロードマップ)
- [フィードバックと問題の報告](#フィードバックと問題の報告)

---

## ライブラリのアクセシビリティ基盤

本ライブラリは、アクセシビリティ抽象化レイヤーとして **[AccessKit](https://accesskit.dev/)** を統合しています。これにより、OS ネイティブの **Windows UI Automation (UIA)** と連携し、以下を提供します：

1. **デフォルトのセマンティクス**:
   - 自動推論により、`Entity` が持つ属性および機能に応じてデフォルトで適切な Role や State を AccessKit 経由で UIA に伝達します。
2. **フォーカスとキーボードナビゲーション**:
   - `focusable` API を通じて OS 標準に沿ったフォーカス移動の仕組みを提供します。
3. **開発者向け API**:
   - 各 `Entity` に対して、アクセシビリティラベルや説明文を付与するための API を提供します。

---

## アプリ開発者へのガイドライン

どれだけライブラリが基盤を整えても、開発者が適切な情報を設定しなければアクセシブルなアプリにはなりません。本ライブラリを使ってアプリを開発する際は、以下の点に配慮してください。

### 1. ラベル（Accessible Name）の付与

- アイコンのみのボタンやグラフィカルな入力欄には、必ず本ライブラリのラベル設定 API を使って、スクリーンリーダー用の読み上げテキストを指定してください。
  - 例:
  ```rust
    div_n().a11y_label(Role::Button, "Accessible Name")
  ```

### 2. 論理的なフォーカス順序

- Tab キーによる移動順序が、画面の視覚的な配置（通常は左から右、上から下）と一致するように UI を組み立ててください。

### 3. コントラストと色への依存回避

- 背景色とテキストのコントラスト比を十分に確保してください。
- エラーなどの重要な状態を「赤色」などの色情報だけで伝えず、アイコンやテキストでも明示してください。

---

## アプリ開発時の推奨検証手順 (How to Test)

アプリのアクセシビリティを検証する際は、以下のツールと手順を開発フローに組み込むことを推奨します。

### ステップ 1: 自動チェック・UI ツリーの確認

Microsoft 製のツールを使って、AccessKit 経由で UIA に正しくノードが渡っているかを確認します。

- **[Accessibility Insights for Windows](https://accessibilityinsights.io/docs/windows/overview/)**（推奨）
  - アプリのウィンドウにカーソルを合わせるだけで、アクセシビリティツリーの構造を視覚化できます。
  - 「FastPass」機能を使えば、ラベルの欠落などの主要な問題をワンクリックで自動検出してくれます。
- **Inspect.exe** (Windows SDK 付属)
  - UIA プロパティ（Name, Role, BoundingRectangle 等）が意図通り出力されているかを詳細に確認できます。

### ステップ 2: キーボード操作テスト

マウスを使わずにアプリを操作してみます。

- **Tab / Shift+Tab**: すべての操作可能な要素に順番にフォーカスが当たるか。
- **フォーカスの可視化**: 現在どこにフォーカスがあるか一目でわかる枠線が表示されるか。一般的にはアウトラインを使用します。
  - 例:
  ```rust
    ii().focusable_self_keyboard().focused(
        ii().outline_solid(2.0)
            .outline_offset(2.0)
            .outline_color(Color::WHITE),
    )
  ```
- **Enter / Space / 矢印キー**: ボタンの実行やリストの選択がキーボードのみで行えるか。

### ステップ 3: スクリーンリーダーによる読み上げテスト

実際に音声でどう伝わるかを確認します。

- **Windows ナレーター (Narrator)**: `Win + Ctrl + Enter` でいつでも起動・終了できます。
- **NVDA**: Windows 向けの実用スクリーンリーダー。
  - キーボード操作をしながら、コンポーネントの名前（Label）、種類（Button なのか等）、状態（Checked なのか等）が正しく読み上げられるか確認してください。

---

## ライブラリ側の既知の課題とロードマップ

現在、ライブラリ側では以下の機能拡充に取り組んでいます：

- [ ] 複雑なコンポーネントにおける AccessKit ノードツリーの自動推論の最適化。
- [ ] スクロールバー、スクロール位置の読み取り。
- [ ] アクセシビリティツール側からの入力への応答。
- [ ] `ACCESSIBILITY.md` のブラッシュアップ。
- [ ] Issue テンプレートのブラッシュアップ。
- [ ] API のより具体的な使用方法の解説 (`accessibility.rs`) とドキュメントの拡充。
- [ ] `a11y_catalog` の拡充

---

## フィードバックと問題の報告

ライブラリ内部の AccessKit 連携の不具合、キーボードトラップ、フォーカスのバグなどを発見した場合は、[GitHub Issue](https://github.com/eto0202/michiu/issues) にて報告してください。

- **Open an issue** using the [accessibility issue template](https://github.com/eto0202/michiu/issues/new?template=accessibility_issue.md).

可能であれば「Accessibility Insights での検出結果」や「ナレーターの挙動」を添えて、ラベル `accessibility` を設定していただけると調査がスムーズになります。

---

Created: [2026/10/14]

Last Updated: [2026/10/30]
