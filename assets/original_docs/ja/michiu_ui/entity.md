# `entity`

UIツリーを構成する [`Entity`] の生成、ツリー構造の組み立て、イベントハンドリング、
および [`Modifier`] による宣言的スタイリングを担当するモジュールです。

---

## アーキテクチャ

`michiu_ui` には `Button` や `TextInput` といった固定化されたウィジェット構造体は存在しません。
すべてのノードは単なる `Entity` であり、
その ID が内部のどの SoA にスロットを保持しているかによって、その要素の能力や振る舞いが決定されます。

```rust
pub struct Entity(KeyData)
```

### 機能のアタッチ（取り付けあるいは所属・登録）という考え方

メソッドチェーンは単なるプロパティ設定ではなく、「その要素（ID）を特定の SoA テーブルへ登録する操作」 です。

- **`.text(...)`**: テキストコンテンツ SoA に登録され、要素自身が文字として振る舞う。
- **`.input(...)`**: 入力コンテンツ SoA に登録され、要素自身が直接編集可能なテキストフィールドとして振る舞う。
- **`.on_click(...)`**: イベント SoA にリスナーが登録され、クリック可能なインタラクション性を獲得する。
- **`.draggable(...)`**: DnD SoA に登録され、ドラッグ移動の能力を獲得する。

### なぜスタイル（`Modifier`）が分離されているのか？

`Entity` に直接 `.bg_color(...)` を生やさず、独立した [`Modifier`]（[`ii()`](<crate::ii()>)）を引数で渡すのには2つの理由があります。

1.  **DODによる整合性**:
    `Entity` 自体が色やサイズを保持しているわけではなく、内部のスタイル SoA テーブルに ID を登録しているだけです。機能のアタッチと同様に「スタイル定義を外から注入する」という一貫したメンタルモデルに基づいています。
2.  **スタイルの再利用性と可読性（CoWによるゼロコスト共有）**:
    CSS クラスのようにスタイル定義を変数として共通化し、複数の要素へ自由に適用できます。
    `Modifier` の内部は [`Arc`] で包まれているため、クローンしてもメモリの再確保は発生せず、プロパティを変更・上書きした時のみ CoW（`Arc::make_mut`） によって安全かつ最小のコストで分岐されます。

---

## 統一されたサフィックス（命名規則）

- **`_d` サフィックス**:
  祖先要素から `.provide()` された型（`Theme` 等）を暗黙的に取得する動的クロージャを受け取ります（内部で `use_provided` を自動実行）。
- **`_with` サフィックス**:
  イベントハンドラにおいて、第1引数としてコンテキスト（`cx: &mut Context`）を受け取ります。

---

## UI構築の基本ステップ

1.  `Context` を初期化します。
2.  [`build_ui`] を使用してUIツリーを宣言します（ルート要素 `Entity` を返す必要があります）。
3.  要素生成関数（`div`, `h_flex` 等）でベース要素を初期化します。
4.  `modify()` または `modify_d()`（あるいは生成時引数）でスタイルを適用します。
5.  子要素、テキスト、入力、イベントハンドラ等の機能をアタッチします。

```rust
use michiu_ui::prelude::*;

let root = build_ui(&mut context, || {
    h_flex(ii().p(12.0).size(500.0).bg_color(Color::WHITE))
        .child(
            div(ii().size((40.0, 40.0)).bg_color(Color::BLUE))
                .hovered(ii().bg_color(Color::RED))
                .on_click(|| println!("Clicked!"))
        )
        .label("Button Text", ii().text_color(Color::BLACK))
});
```

---

## 要素生成プリミティブ（組み込み関数一覧）

| 関数                                | 初期設定される `Display` / レイアウト属性 | 主な用途                                       |
| :---------------------------------- | :---------------------------------------- | :--------------------------------------------- |
| **`div(modifier)` / `div_d`**       | `Display::Block`                          | 一般的なブロック要素                           |
| **`div_n()`**                       | `NO_STYLE: Option<Modifier> = None`       | スタイル処理を完全にスキップする最軽量コンテナ |
| **`flex(modifier)` / `flex_d`**     | `Display::Flex`                           | 自由なFlexboxコンテナ                          |
| **`h_flex(modifier)` / `h_flex_d`** | `Display::Flex` + `FlexDirection::Row`    | **水平方向（横並び）** コンテナ                |
| **`v_flex(modifier)` / `v_flex_d`** | `Display::Flex` + `FlexDirection::Column` | **垂直方向（縦並び）** コンテナ                |
| **`hidden_box(modifier)`**          | `Display::None`                           | 条件付きレンダリングや非表示領域               |

---

## スタイリング（`Modifier` / `ii()`）の仕様

スタイルは `.modify()` または `.modify_d()` で適用します。静的な `Modifier` と動的クロージャの両方を受け入れ、混用も可能です。
同じプロパティを複数回設定した場合、後に設定したプロパティが優先（後勝ち上書き）されます。

### 1. 疑似クラス（インタラクションスタイル）

ユーザー操作に応じて変化するスタイルを1階層ネストして定義できます（ネスト内の疑似クラスは無視されます）。

- **基本疑似クラス**:
  `.hovered(...)`, `.focused(...)`, `.focused_visible(...)`（キーボードフォーカス時のみ適用）, `.pressed(...)`, `.disabled(...)`, `.actived(...)`, `.selected(...)`, `.dragged(...)`
- **`_within`（子起点）**:
  自身の子孫要素に指定のインタラクションが発生した際、自身（親）にスタイルを適用（例: `.hovered_within(...)`）。
- **`_parent`（親起点）**:
  自身の親要素に指定のインタラクションが発生した際、自身（子）にスタイルを適用（例: 親が押された時に文字色を変える `.pressed_parent(...)`）。

### 2. フォーカス制御属性（`focusable`）

デフォルトでは入力要素のみがフォーカス可能です。一般要素をフォーカス可能にするには属性を付与します。

- **デバイスサフィックス**:
  `_mouse`（マウスのみ）, `_keyboard`（キーボードのみ）, `_both`（両方）
- **適用対象サフィックス**:
  - `_self`: 自身に設定された `.focused(...)` や `.focused_visible(...)` を適用。
  - `_inherit`: **親に設定されたフォーカススタイルを自身にも自動継承・適用**。親要素の疑似クラス設定を子要素全体で再利用・統一する際に有効です。
  - `_none`: フォーカス不可属性。
- **フォーカス奪取防止**:
  - `.prevent_focus_steal(true)`: フォーカス中の要素からフォーカスを奪わない。
  - `.prevent_focus_steal_within(true)`: 自身の子要素がフォーカスを奪わない。

### 3. ドラッグ＆ドロップ属性（`draggable` / `droppable`）

- **`DndDragPayload` の挙動の違い**:
  - `Entity`: ドロップ確定時にライブラリがツリー構造（SoA）を自動的に書き換えて要素を移動。
  - `Entity`: ツリー構造は変更せず、DnDイベント（ID通知）のみを発火。
- **プレースホルダー配置**:
  - `.dnd_draggable_root(...)`: プレースホルダーをルート直下に生成し、絶対配置でカーソルに追従。
  - `.dnd_draggable_parent(...)`: 任意の親要素の子として追加し、絶対配置でカーソルに追従。
- **DnD専用疑似クラス**:
  - `.dnd_draggable_original(...)`: ドラッグ中の元要素のスタイル。
  - `.dnd_draggable_placeholder(...)`: カーソルに追従するプレースホルダーのスタイル。
  - `.dnd_drag_over(...)`: ドラッグ要素が自身に重なった受け入れ側のスタイル。

### 4. その他のスタイル属性

- **リサイズ (`resizable`)**: 上下左右の境界ドラッグによるリサイズを有効化。`.resizable_cursor(...)` でリサイズカーソルを設定（カーソル設定において最も優先されます）。
- **カーソル優先順位**: `resizable_cursor`（最優先） > `cursor(...)`（要素個別） > `cursor_global(...)`（自身および子孫のベースカーソル）
- **マテリアル (`backdrop`)**: Mica や Acrylic を適用（ウィンドウ背景色は透過である必要があります）。
- **デバッグ支援 (`debug...`)**: 枠線色をあらかじめ設定したボーダーやアウトラインを提供するショートハンド。
- **スタイルのマージ**: `.merge(other_modifier)` で別の `Modifier` を合成。

---

## 機能とイベントのアタッチ

### 1. テキストとラベルの厳密な違い

見た目は似ていますが、内部の SoA 構造およびツリー階層が明確に異なります。

- **`entity.text(content)` / `text_d`（自身をテキスト化）**:
  **自身（`entity`）** がテキストコンテンツ SoA に直接登録されます。要素自身がテキスト計測（`measure_content`）の対象となり、自身が直接マウスイベントを受け取ります。動的変更時はテキストプロパティ自体が更新されます。
  ※ ショートハンド `text(content)` は `div_n().text(content)` と同等です。
- **`entity.label(content, modifier)` / `label_d`（テキスト子要素の自動生成）**:
  **新しく生成された子要素** がテキスト SoA に登録され、`entity` の子要素としてぶら下がります。
  この子要素には自動的に `PointerEvents::None` が設定されるため、親のクリック判定を遮りません。ボタン枠の中にテキストを中央配置する場合などに最適です。動的変更時は子要素が差し替えられます。

### 2. 子要素の追加と動的差し替え

- **単一/複数追加**: `.child(entity)`, `.children([entity_1, entity_2])`（動的版: `.child_d`, `.children_d`）
- **動的コンテンツ差し替え (`set_contents`)**:
  親要素を破棄せず、子要素のみを丸ごと差し替えます。以前の子要素に紐づいていた古いエフェクトは安全に破棄されます。

### 3. 入力・外部コンテンツのアタッチ

- **入力機能**: `.input(contents)` / `input_d`（ショートハンド `input(...)` は `div_n().input(...)` と同等）
- **外部テクスチャ**: `.external_texture(...)`
- **外部ビジュアル**: `.external_visual(...)`（DirectComposition によるハードウェア合成）

### 4. アクセシビリティ機能のアタッチ

- `.a11y(...)`: アクセシビリティノードを追加
- `.a11y_with(|node| ...)`: ノードを追加した上で可変参照を受け取って詳細設定
- `.a11y_role(...)`: ロールのみ追加
- `.a11y_label(...)`: ロールとラベルのみ追加

### 5. Tag 機能によるクエリ

- **登録**: `.tag::<MyTag>()`, `.tag_a11y::<CustomInferenceTag>()`（自作の `trait A11yInferenceTag` 登録用）
- **自身の子孫から探索**:
  - `entity.query_descendant::<T>()` / `entity.try_query_descendant::<T>()`
  - `entity.query_descendants::<T>()`（マッチした全要素を `Vec` で取得）
  - `entity.for_each_descendants::<T>(|child| ...)`（クロージャによる遅延生成・走査）
  - 全体や特定の要素を起点に検索する場合は `cx.query_first::<T>()` などの `Context` メソッドを使用します。

### 6. イベントハンドラ一覧

すべてのハンドラには通常版と `_with` 版（`|cx|` 付き）が存在します。同種のハンドラが重複して登録された場合、定義順にすべて順次実行されます。

| カテゴリ                 | メソッド                                                       | 発火契機・補足                           |
| :----------------------- | :------------------------------------------------------------- | :--------------------------------------- |
| **ポインタ・マウス**     | `on_click`, `on_click_with`                                    | 同一要素上で押して離された時             |
|                          | `on_right_click`, `on_right_click_with`                        | 右クリック確定時                         |
|                          | `on_mouse_input`, `on_mouse_input_with`                        | マウスボタンの押下・離上                 |
|                          | `on_mouse_enter`, `on_mouse_enter_with`                        | ポインタの侵入                           |
|                          | `on_mouse_leave`, `on_mouse_leave_with`                        | ポインタの離脱                           |
|                          | `on_cursor_moved`, `on_cursor_moved_with`                      | 要素上でのカーソル移動                   |
|                          | `on_mouse_wheel`, `on_mouse_wheel_with`                        | ホイール回転（消費時はバブリング停止）   |
|                          | `on_drag`, `on_drag_with`                                      | マウスドラッグ中                         |
|                          | `on_hover`, `on_hover_with`                                    | ホバー継続中                             |
| **キーボード・テキスト** | `on_char_input`, `on_char_input_with`                          | 文字入力確定時                           |
|                          | `on_keyboard_input`, `on_keyboard_input_with`                  | キーの物理押下・離上                     |
|                          | `on_ime`, `on_ime_with`                                        | IME 変換イベント                         |
| **ファイルD&D**          | `on_file_dropped`, `on_file_dropped_with`                      | OSからのファイルドロップ確定             |
|                          | `on_file_drag_enter`, `on_file_drag_enter_with`                | ファイルドラッグの要素侵入               |
|                          | `on_file_drag_leave`, `on_file_drag_leave_with`                | ファイルドラッグの要素離脱               |
| **状態遷移**             | `on_focus`, `on_focus_with`                                    | フォーカス獲得時（`focus(true)` に連動） |
|                          | `on_blur`, `on_blur_with`                                      | フォーカス喪失時                         |
|                          | `on_disable`, `on_disable_with`                                | 無効化時（`disable(true)` に連動）       |
|                          | `on_active`, `on_active_with`                                  | 有効化時（`active(true)` に連動）        |
|                          | `on_select`, `on_select_with`                                  | 選択状態変化時（`select(true)` に連動）  |
| **アプリ内DnD**          | `on_dnd_drag_start`, `on_dnd_drag_start_with`                  | ドラッグ閾値超過による開始               |
|                          | `on_dnd_drag_interactive(_with)`, `on_dnd_drag_passive(_with)` | ドラッグ中（Mode種別で分岐）             |
|                          | `on_dnd_drop_interactive(_with)`, `on_dnd_drop_passive(_with)` | ドロップ確定時                           |

---

## スレッド安全性と実行コンテキスト

- **[`Entity`] のメソッド**: 内部の SoA ストアと直接通信するため、メインスレッド（UIスレッド）限定です。
- **[`Modifier`]**: 中身はスレッドセーフな [`Arc`] であり、単純なデータ構造であるためスレッド間を自由に転送可能です。値の書き換えは常に `Arc::make_mut` を通じて排他参照が得られた時のみ実行されるため、データ競合のリスクはありません。
  ```rust
    #[derive(Debug, Clone, Default)]
    pub struct Modifier {
        pub(crate) inner: Arc<ModInner>,
    }
  ```
