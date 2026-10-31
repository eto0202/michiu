## [0.1.2] - 2026-11-01

### 🚀 Features

- _(a11y_catalog)_ Add samples and updated `README.md`

### 🐛 Bug Fixes

- Fix absolute positioning by resolving containing block (#58) (#59)

## [0.1.1] - 2026-10-30

### 🚀 Features

- _(a11y_catalog)_ Introduce accessible component catalog and preview (#57)

## [0.1.0] - 2026-10-30

### Breaking Changes

- Upgrade taffy to 0.15:
  - Remove Grid layout
  - Adapt alignment handling to `AlignItems::Normal` (remove Option wrapping)
  - Add `Fixed`, `Static`, and `Sticky` to `Position` enum
  - Maintain `Position::Relative` as library default
  - Update resize dragging and scrollable bounds calculation for Fixed position

### ⚙️ Miscellaneous Tasks

- Fix the translation of `LANGUAGE_POLICY.md`
- _(security)_ Add security policy and restrict workflow permissions
- Add `pull_request_template`.

### 💼 Other

- [**breaking**] Update dependencies and adapt to taffy 0.15

### ⚙️ Miscellaneous Tasks

- Fix the translation of `LANGUAGE_POLICY.md`

## [0.0.5] - 2026-10-29

### Breaking Changes

- Replace `LinearGradient` builder/factory methods:
  - `LinearGradient::two_stops`
  - `LinearGradient::new`
  - `LinearGradient::four_corners`
  - `LinearGradient::to_corner`
  - Remove legacy fields (gradient_start_color, gradient_end_color, etc.)
- Rename `find_root_entity` -> `find_root`
- Rename `parent_entity` -> `parent`
- `EntityId` and `Element` are unified as `Entity`
- Rename `DndDragPayload` -> `DndDragMode`
  - Rename `Element` -> `Interactive`
  - Rename `EntityId` -> `Passive`
  - Rename `on_dnd_element_drag` -> `on_dnd_drag_interactive`
  - Rename `on_dnd_id_drag` -> `on_dnd_drag_passive`
  - Rename `on_dnd_element_drop` -> `on_dnd_drop_interactive`
  - Rename `on_dnd_id_drop` -> `on_dnd_drop_passive`
- Rename `ThisStyle` -> `Modifier`
  - Rename `StyleInner` -> `ModInner`
  - Rename `StyleValue` -> `ModValue`
  - Rename `IntoStyle_` -> `IntoMod_`
  - Rename `ts()` -> `ii()`, `m!()`
  - Rename `StyleTarget` -> `ModTarget`
- Rename element.rs -> entity.rs
- Rename style.rs -> modifier.rs

### 🚀 Features

- Add extension traits for `LPARAM` and `WPARAM`
- Add Oklab/Oklch support and color manipulation builders
- [**breaking**] Implement 4-stop and four-corner gradients
- Add a sample of `LinearGradient::new` to `sample_collection`

### 📚 Documentation

- Added `# Errors` docs to the public API

### 🚜 Refactor

- Introduce conversion traits to reduce type mapping boilerplate
- [**breaking**] Rename style-related types and functions to modifier
- [**breaking**] Unify Element and EntityId into Entity
- [**breaking**] Remove redundant _entity suffix from method names
- Address clippy cast warnings and error handling
- Reorder ComponentMask flags and add unit tests
- Fix the rendering order of `collect_render_data`
- Fix `sample_collection`

### 🧪 Testing

- Add unit tests for `FlexDirection` and `LinearGradient`
- Add unit tests for Transform
- Add unit tests for ByteIndex, CharIndex, and RangeExt
- Add unit tests for MichiuString
- Add unit tests for `debug_store.rs`
- Add unit tests for `soa.rs`
- Add unit tests for `TopologyStore`
- Add unit tests for `entity.rs`
- Add unit tests for `signal.rs`
- Add unit tests for `ReactiveStore`
- Add unit tests for `LayoutStore`

### ⚙️ Miscellaneous Tasks

- _(michiu_ui)_ Modify `README.md`

## [0.0.4] - 2026-10-17

### Breaking Changes

- Rename `inject_user_action` to `inject_action`
- Rename `UserAction` to `EventAction`

### 🚀 Features

- Add `a11y_label` to `Entity`
- Add ‘context_menu’ sample
- Add a custom title bar to the `examples/sample_collection`
- Fix the custom title bar in `examples/sample_collection`
- Add a sample `transform.rs` file
- Add a sample to `transform.rs`
- Add a sample to `scrollbar.rs`
- Add a sample to `gradient.rs`

### 🐛 Bug Fixes

- Fix the custom title bar in `examples/sample_collection` (#41)

### 📚 Documentation

- Update the documentation for `michiu`
- [**breaking**] Add documentation to `utils.rs`
- Add a top-level document to `michiu_ui`
- Remove the original text from `lib.rs` (#42)
- Add a top-level document to `context.rs`
- Add a document to `Context`
- Add documentation to `pipeline.rs`
- Add documentation to `element.rs`

### 🚜 Refactor

- Rename the sample file `image.rs` to `external_texture.rs`
- [**breaking**] Renamed `inject_user_action` and `UserAction`

### ⚙️ Miscellaneous Tasks

- Modify the `Cargo.toml` and `README.md`
- Add `ACCESSIBILITY.md` and `accessibility_issue.md`.
- Add `CODE_OF_CONDUCT.md`
- Add `CONTRIBUTING.md` and `LANGUAGE_POLICY.md`
- Update `CONTRIBUTING.md` and `README.md`.
- Move `CODE_OF_CONDUCT.md` and `CONTRIBUTING.md`.
- Modify the `assets` folder
- Rename the folder and update `README.md`

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

[unreleased]: https://github.com/eto0202/michiu/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/eto0202/michiu/compare/v0.1.0...v0.1.2
[0.1.1]: https://github.com/eto0202/michiu/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/eto0202/michiu/compare/v0.0.5...v0.1.0
[0.0.5]: https://github.com/eto0202/michiu/compare/v0.0.4...v0.0.5
[0.0.4]: https://github.com/eto0202/michiu/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.com/eto0202/michiu/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/eto0202/michiu/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/eto0202/michiu/releases/tag/v0.0.1
