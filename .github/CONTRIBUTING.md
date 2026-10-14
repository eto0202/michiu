# Contributing to michiu

> **Languages**: [English](#english) | [日本語](#原文)

---

# English

Thank you for your interest in contributing to **michiu**! We welcome all contributions, including bug reports, feature suggestions, documentation improvements, and code contributions.

## Table of Contents

- [How Can I Contribute?](#how-can-i-contribute)
- [PR Guidelines (Keep it Focused!)](#pr-guidelines-keep-it-focused)
- [Development Setup](#development-setup)
- [Pre-submission Checklist](#pre-submission-checklist)
- [Code of Conduct](#code-of-conduct)

---

## How Can I Contribute?

### 1. Reporting Bugs & Suggesting Features

- Please check existing [Issues](https://github.com/eto0202/michiu/issues) before opening a new one to avoid duplicates.
- For bug reports, please include your Windows version, minimal reproduction code, and any error logs.
- For accessibility-related issues, please refer to [ACCESSIBILITY.md](ACCESSIBILITY.md).

### 2. Submitting Pull Requests (PRs)

- **Small fixes**: Typos, documentation updates, and minor bug fixes are welcome as direct PRs.
- **Large changes / New features**: Before writing a large amount of code, please **open an issue or start a discussion first** so we can agree on the API design and approach.

---

## PR Guidelines (Keep it Focused!)

- **Keep PRs small and focused**:
  - Reviewing massive pull requests with hundreds of lines of code takes a significant amount of time and effort. Please keep changes as compact and single-purposed as possible.
  - If a feature is large, consider breaking it down into smaller, incremental PRs (That said, sending a ton of short PRs isn't good either!).
- **Draft PRs are welcome**:
  - Feel free to open a Draft PR early if you want early feedback on the implementation direction before writing too much code.

---

## Development Setup

### Prerequisites

- **OS**: Windows 10 / 11 (64-bit)
- **Rust Toolchain**: Latest stable Rust installed via [rustup](https://rustup.rs/)

### Building & Running

1. **Clone the repository**:

   ```bash
   git clone https://github.com/eto0202/michiu.git
   cd michiu
   ```

2. **Build the project**:

   ```bash
   cargo build
   ```

3. **Run tests**:

   ```bash
   cargo test
   ```

4. **Run examples**:
   ```bash
   cargo run --example file_name
   ```

---

## Pre-submission Checklist

To keep the codebase clean and CI passing, please make sure your changes pass the standard Rust checks before opening a PR:

1. **Format your code**:
   ```bash
   cargo fmt
   ```
2. **Run Clippy (Linter)**:
   Ensure there are no warnings.
   ```bash
   cargo clippy --all-targets -- -D warnings
   ```
3. **Run Tests**:
   Ensure all tests pass.
   ```bash
   cargo test
   ```

---

## Standards & Policies

By participating in this project, you are expected to uphold the following guidelines:

- [Code of Conduct](CODE_OF_CONDUCT.md): Defines our commitment to an inclusive and harassment-free community.
- [Language Policy](LANGUAGE_POLICY.md): Outlines our multilingual communication rules. You are welcome to write in your native language, accompanied by an English translation!

---

# 原文

**michiu** への貢献に興味を持っていただきありがとうございます！バグ報告、機能提案、ドキュメントの改善、コードのコントリビューションなど、あらゆる貢献を歓迎します。

## 目次

- [コントリビューションの方法](#コントリビューションの方法)
- [PR のガイドライン（コード量はほどほどに！）](#pr-のガイドラインコード量はほどほどに)
- [開発環境のセットアップ](#開発環境のセットアップ)
- [PR 提出前のチェックリスト](#pr-提出前のチェックリスト)
- [行動規範 (Code of Conduct)](#行動規範-code-of-conduct)

---

## コントリビューションの方法

### 1. バグ報告と機能提案

- 重複を避けるため、新しい Issue を作成する前に既存の [Issues](https://github.com/eto0202/michiu/issues) を確認してください。
- バグ報告の際は、Windows のバージョン、最小限の再現コード、エラーログなどを含めてください。
- アクセシビリティ関連の問題については、[ACCESSIBILITY.md](ACCESSIBILITY.md) を参照してください。

### 2. Pull Request (PR) の提出

- **軽微な修正**: タイポの修正、ドキュメント更新、小さなバグ修正などは直接 PR を作成していただいて構いません。
- **大きな変更・新機能**: 大量のコードを書く前に、**まず Issue や Discussions で相談** してください。API の設計や方針について事前に合意を取りましょう。

---

## PR のガイドライン（コード量はほどほどに！）

- **変更は小さく分割してください**:
  - 何百行・何千行にも及ぶ巨大な PR は、メンテナのレビュー負荷が非常に高く、確認やマージに膨大な時間がかかってしまいます。変更内容はできるだけ単一の目的に絞り、コンパクトに保ってください。
  - 機能が大きい場合は、複数の小さな PR に分割して提出することを推奨します（かと言って、小分けの PR を大量に送ってくるのもダメだよ！）。
- **Draft PR の活用**:
  - 方針に迷った場合や、作り込みすぎる前にフィードバックが欲しい場合は、気軽に Draft PR を作成してください。

---

## 開発環境のセットアップ

### 前提条件

- **OS**: Windows 10 / 11 (64-bit)
- **Rust Toolchain**: [rustup](https://rustup.rs/) でインストールされた最新の stable Rust

### ビルドと実行

1. **リポジトリのクローン**:

   ```bash
   git clone https://github.com/eto0202/michiu.git
   cd michiu
   ```

2. **プロジェクトのビルド**:

   ```bash
   cargo build
   ```

3. **テストの実行**:

   ```bash
   cargo test
   ```

4. **サンプルの実行**:
   ```bash
   cargo run --example file_name
   ```

---

## PR 提出前のチェックリスト

コードベースの品質を保ち、CI を通過させるために、PR を提出する前に以下の標準チェックを実行してください：

1. **コードのフォーマット**:
   ```bash
   cargo fmt
   ```
2. **Clippy（リンター）の実行**:
   警告（Warning）が出ないことを確認してください。
   ```bash
   cargo clippy --all-targets -- -D warnings
   ```
3. **テストの実行**:
   すべてのテストがパスすることを確認してください。
   ```bash
   cargo test
   ```

---

## 行動指針とポリシー

コミュニティへの参加にあたっては、以下のガイドラインの遵守をお願いします：

- [行動規範 (Code of Conduct)](CODE_OF_CONDUCT.md): すべての人が安心して参加できるコミュニティ作りのための約束事です。
- [言語ポリシー (Language Policy)](LANGUAGE_POLICY.md): 多言語コミュニケーションのルールです。母語での記述を歓迎しています（英訳の併記をお願いします）！
