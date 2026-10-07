## [0.0.3] - 2026-10-07

### Breaking Changes

- `poll_event()` no longer returns `Option<MichiuEvent>`.
  - Use `event_pump.poll_event(|event, id, raw| { ... })` instead.
- `wait_event()` now takes a closure and returns `Result<bool>`.
  - Use `event_pump.wait_event(|event, id, raw| { ... })` instead.
- Other variant has been added to `MichiuEvent`.
  - Unhandled library events are now dispatched here.
- `UnsafeRaw` event variant has been removed.
  - Raw Win32 message data is now directly accessible via the `RawEvent` closure argument.
- `WM_PAINT` no longer automatically calls `BeginPaint` and `EndPaint`.
  - Use `begin_paint` to obtain a `GDI` context. `EndPaint` is automatically called when PaintContext is dropped.
- Redesign `Event::MouseWheel` fields to `raw_delta_x` and `raw_delta_y` typed as `Unvalidated<WheelDelta>`.
  - `translate_and_dispatch` now preserves the raw `i32` value inside `WheelDelta` without implicit division.
  - Use `WheelDelta::raw()` to obtain the raw value, or `WheelDelta::notches()` to get the value normalized by `120`.
- Rename `ComposedRenderer` to `MichiuRenderer`
  - Move `dispatch_raw_input_to_external_visual` to a method of `MichiuRenderer`
- Rename `EventPump` to `MichiuEventPump`
- Rename `MichiuEvent` to `MichiuAnyEvent`
- Rename `Event` to `MichiuEvent`
- Rename `RawEvent` to `MichiuRawEvent`
- Rename `EventSender` to `MichiuEventSender`
- Rename `EventBus` to `MichiuEventBus`
- Rename `ComContext` to `MichiuComContext`
- Rename `Window` to `MichiuWindow`
  - Move `init_dpi_awareness` to a method of `MichiuWindow`
- Rename `WindowBuilder` to `MichiuWindowBuilder`
- Rename `Icon` to `MichiuIcon`
- Rename `Tray` to `MichiuTray`
- Rename `TrayBuilder` to `MichiuTrayBuilder`
- Rename `TrayMenuItem` to `MichiuTrayMenuItem`
- Rename `CustomTrayMenu` to `MichiuCustomTrayMenu`
- Rename `BuilderConfig` to `MichiuBuilderConfig`

### 🚀 Features

- Fixed the parsing of `CssMap`
- Add a sample to `michiu`
- Changes for the integration of `michiu`
- Add `MichiuAppBuilder` and `MichiuApp` to `michiu`.

### 🐛 Bug Fixes

- Modify the `michiu` sample

### 🚜 Refactor

- Change `div()` to `flex()` (#27)
- [**breaking**] Redesign EventPump to use callback-based dispatch
- [**breaking**] Update `MouseWheel` and IME event handling
- [**breaking**] Changes in Preparation for the `michiu` Integration
- Overhaul workspace structure and update documentation

### ⚙️ Miscellaneous Tasks

- Changes to `README.md`

## [0.0.2] - 2026-09-30

### 🚀 Features

- ExternalVisual の実装と webview2 の分離 (#16)
- ExternalVisual と webview2 周りの不具合の修正 (#16)
- Webview2 周りの調整 (#16)
- ExternalStyle の実装 (#18)
- ExternalStyleSet の実装 (#18)
- ExternalStyleSet の実装 (18)
- 簡単なドキュメントを追加 (#18)
- 依存関係の追加 (#21)
- AccessibilityStore を実装(#21)
- 必要な構造体を定義 (#21)
- 構造体を修正 (#21)
- 構造体を修正 (#21)
- AccessibilityStore に関連メソッドを実装 (#21)
- スナップショットの作成 (#21)
- ベース機能を実装 (#21)
- `tag` の統合とサンプルに `accessibility` を追加
- A11y 関連を修正 (#21)

### 🚜 Refactor

- Sample_collection に統合
- サンプルの修正
- バグ修正とファイル整理
- Context の tag 関連を修正
- ユーザー定義の推論タグの修正 (#21)
- A11y() メソッドと推論ロジックの修正 (#21)

### Other

- `CHANGELOG.md` を追加
- `release.yml` を追加
- `cliff.toml` を追加

## [0.0.1] - 2026-09-21

- Initial release

[unreleased]: https://github.com/eto0202/michiu/compare/v0.0.3...HEAD
[0.0.3]: https://github.com/eto0202/michiu/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/eto0202/michiu/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/eto0202/michiu/releases/tag/v0.0.1
