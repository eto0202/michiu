# `Pipeline`

フレーム進行のオーケストレーターであり、
`Context` の各ストアから必要なSoAのみを分割借用して、各処理フェーズを直列で繫ぐパイプラインモジュールです。

入力イベントの受信から、D&D、テキスト編集、IME同期、2パスレイアウト計算、
GPUバッチ生成とアクセシビリティ送信まで、すべての内部ステップが確定的なシーケンスとして定義されています。

---

## パイプライン関数の内部仕様

1.  `inject_action`: 入力イベントのキューイングと正規化
2.  `begin_frame`: イベントキューの消化とインタラクション・編集解決
3.  `update_state`: ビットマスクによる要素状態の局所更新とスタイル波及
4.  `set_states`: `StateFlag` からのコンポーネントマスク変換
5.  `sync_layout`: Taffy連携・スクロールバーなどを同期する2パスレイアウト計算
6.  `tick_system_frame`: アニメーション、トランジション、オートスクロールのクロック進行
7.  `collect_render_data`: アトラス解決、パンチアウト、インスタンスバッチの構築
8.  `handle_accessibility`: AccessKit ツリーのスナップショット構築とワーカー転送

---

### 1. `inject_action(cx: &mut Context, action: EventAction)`

OSやウィンドウメッセージから届いた生のアクションを `cx.events.evt_pending_actions` に蓄積します。

- **PointerMove の最適化**: 最新のポインタ座標で既存の未消化移動アクションを上書き。
- **MouseWheel の蓄積**: 同一フレーム内で連続したスクロール量は合算して蓄積。

---

### 2. `begin_frame(cx: &mut Context)`

蓄積された `evt_pending_actions` を `std::mem::take` して古い順にイテレート・ディスパッチします。

#### A. `PointerMove` の処理フロー

1.  **リサイズドラッグの同期** (`ResizeStore::sync_resizing_drag`): ウィンドウ/要素のリサイズドラッグ中であれば、以降のホバー処理を早期リターン。
2.  **スクロールバードラッグの同期** (`ScrollStore::sync_scrollbar_drag`): バーの掴み移動を処理。
3.  **ソート済みヒットテスト** (`TopologyStore::hit_test`):
    `topo_is_sort_dirty` フラグを評価し、必要であれば可視性・トランスフォーム・z-indexを考慮したソート配列（`topo_sorted_entities`）を再構築。ヒット判定は最前面から `rev()`（逆順走査）で実行。
4.  **リサイズ境界の探索** (`ResizeStore::found_resize_hover`): ヒット要素から先祖へ遡上し、リサイズ可能境界にいるかを判定。
5.  **カーソル形状の適用** (`ResizeStore::apply_resizable_cursor_style`): 境界上ならリサイズ用カーソルをセット。
6.  **通常カーソルの復帰** (`RenderStore::resolve_element_style_state`): 境界から外れた場合は要素本来のカーソルへリセット。
7.  **テキスト選択のドラッグ追従**:
    要素が押下状態（`pressed`）かつ `UserSelect::Text` の場合、テキストバッファを同期し、選択ハイライト範囲を更新 (`TextEditStore::handle_text_selection_click`)。
8.  **ホバー状態の解決** (`EventStore::resolve_hover_state`):
    新旧ホバー要素に対して `Pipeline::update_state` を実行し、`on_mouse_leave`, `on_mouse_enter`, `on_hover` を発火。
9.  **移動イベント伝播** (`handle_on_cursor_moved`)。
10. **D&D ドラッグセッションの更新** (`DndStore::propagate_dnd_drag_events`):
    ドラッグ閾値を超えた場合にセッションを開始 (`handle_on_dnd_drag_start`)、`handle_on_drag` を通知。
11. **D&D プレースホルダーの移動と侵入検知**:
    - ルート要素基準で相対ローカル座標を逆算して追従 (`update_inset_based_relative_local`)。
    - ホバー侵入中のドロップターゲットを検知 (`detect_drop_target_during_intrusion`)。
    - `DndDragPayload` に応じて `handle_on_dnd_entity_drag` または `handle_on_dnd_id_drag` を発火。

#### B. `PointerButton` の処理フロー

- **Pressed (押下時)**:
  1.  **リサイズドラッグ判定** (左クリック時のみ): リサイズ開始時は即座に処理を確定し早期リターン。
  2.  **スクロールバー判定**: バー上のクリックであれば背面への透過を防ぎ早期リターン。
  3.  **一般要素の押下**: `Pipeline::update_state` で押下フラグを更新。テキスト要素なら選択ハイライトを開始。
  4.  **フォーカス解決**: フォーカス奪取防止 (`should_prevent_focus_steal`) を考慮しつつ、新要素へフォーカス切替（`on_blur`, `on_focus` 発火）。属性がなければフォーカス解除。
  5.  `handle_on_mouse_input` の呼び出し。
- **Released (離上時)**:
  1.  **リサイズ終了**: ドラッグ状態を終了し、仮想 `PointerMove` を発生させてカーソルを最新化。
  2.  **D&D ドロップ確定**: トポロジーの書き換え（ドラッグ要素の削除、挿入、子ツリー復元）、ドロップハンドラ発火、プレースホルダー破棄。
  3.  **スクロールバー更新**: ドラッグ終了に伴うダーティフラグをセット。
  4.  **クリック判定**: 押下と同一要素上で離された場合、`handle_on_click` または `handle_on_right_click` を確定発火。

#### C. その他のアクション処理

- **`PointerDoubleClick`**: 単語境界のヒットテストを行い、テキストのダブルクリック範囲選択を実行。
- **`MouseWheel`**: ホバー要素から親へ遡り、ユーザーハンドラ (`on_mouse_wheel`) を優先実行。未消費なら直近のスクロールコンテナで `scroll_by` を実行してバブリング停止。
- **`KeyboardKey`**: Tabキー押下のフォーカス巡回 (`cycle_keyboard_focus_internal`)、Enter/Spaceでのクリックエミュレート、Ctrl+Aでの内部全選択を自動解決。
- **`Paste` / `Cut` / `Undo` / `Redo`**: キャレット位置と入力コンテンツ (`cont_input_contents`) の履歴バッファを同期更新。

最後に空になったバッファを `cx.events` に返却。

---

### 3. `update_state(cx: &mut Context, id: EntityId, state_flag: u128, actived: bool)`

インタラクション状態の変化をビットマスクで評価し、最小限のレイアウト汚染とスタイル解決を即座に実行します。

1.  **重複評価のバイパス**: 現在のアクティブフラグと比較し、同一であれば早期リターン。
2.  **フラグの更新**: SoA の状態ビットを反転。
3.  **アクティブレイアウトキャッシュの即時更新**:
    状態変化（ホバー、フォーカス等）に伴う動的スタイルを解決し、差分をキャッシュへ反映 (`update_resolved_active_layout_cache`)。
4.  **子要素へのスタイル波及**:
    子要素が `ComponentMask::STYLE_INTERACTION_PARENT` を持つ場合、親の状態遷移に連動して子のスタイルも再解決。
5.  **先祖ツリーの早期バイパス**:
    `ComponentMask::STYLE_INTERACTION_WITHIN` を持たない先祖ツリーは探索をスキップし、関係のある先祖のみレイアウトを再計算。
6.  **状態イベントの発火**: `handle_on_disable`, `handle_on_active`, `handle_on_select` を解決。

---

### 4. `set_states(cx: &mut Context, id: EntityId, flag: &StateFlag, actived: bool)`

[`StateFlag`] 列挙型を、低レイヤの `ComponentMask` ビット（`u128`）へとマッピングして `update_state` を呼び出します。

- **キーボードフォーカス判定**:
  `Focused` 遷移時、トリガーがキーボード操作であれば `STATE_FOCUSED_VISIBLE`（フォーカスリング用フラグ）を同時付与。

---

### 5. `sync_layout(cx: &mut Context, root: EntityId, window_size: LayoutSize)`

スクロールバーの出現に伴うコンテンツ領域の変動を同期させる2パスのレイアウト計算です。

1.  **保留エフェクトのフラッシュ** (`evaluate_pending_element_effects`):
    レイアウト計算に突入する前に、蓄積されているすべてのシグナル・エフェクトを一挙に評価完了させます。
2.  **早期リターン判定**:
    構造ダーティ (`topo_is_structure_dirty`)、スタイルダーティ (`lay_dirty_entities`)、ウィンドウサイズ変動のいずれも無ければ完全にスキップ。
3.  **DFSツリーシーケンスの再構築**: ツリー構造に変更があれば、深さ優先探索順のフラット配列 (`topo_flat_dfs_sequence`) を再生成。
4.  **Taffy 差分同期**: 変更があった要素のみ Taffy 側のレイアウトスタイルを更新。
5.  **Taffy パス 1（仮レイアウト計算）**:
    固定サイズおよびテキスト計測（`measure_content`）を用いて 1 回目のレイアウトを計算。
6.  **出力座標の確定**: スクロールサイズを正確に測定するため、ダブルバッファをスワップして一旦 `out_rects` を確定。
7.  **スクロールコンテナの事前計算**: 全スクロールコンテナの `scroll_size` を算出し、スクロールバーの必要性を判定。
8.  **スクロールバースタイルの同期** (`sync_bar_styles`): スクロールバーの幅・可視性をスタイルへ反映。
9.  **Taffy パス 2（確定レイアウト計算）**:
    スクロールバーによって削られたコンテンツ領域を考慮し、キャッシュされた計測サイズを用いて 2 回目のレイアウトを最終確定。
10. **IME & キャレット位置の同期**:
    フォーカス中の入力要素がある場合、Win32 IMM32 API（`sync_imm_window_position`）を呼び出し、OSの IME 変換候補ウィンドウの位置をキャレットの絶対座標へ追従。
11. **スクロール位置の自動クランプ**: コンテナリサイズによってはみ出したスクロール位置を限界値へ補正。

---

### 6. `tick_system_frame(cx: &mut Context, tick: &TickType)`

時間経過に伴う補間処理を 1 ステップ進めます。

- **Transition**: トランジション補間値を 1 tick 進行 (`tick_transitions`)。
- **Animation**: アニメーションフレームを 1 tick 進行 (`tick_animations`)。
- **AutoScroll**:
  オートスクロールを 1 tick 進行。スクロールでテキストが流れた場合、現在のポインタ座標で仮想的な PointerMove を即座に再トリガーし、ドラッグ中の選択文字インデックスをリアルタイムに追従させます。

---

### 7. `collect_render_data(cx: &mut Context, view: &mut RendererView)`

SoA に蓄積された全UI要素をスキャンし、GPUインスタンスバッファとドローバッチを構築します。

1.  **フォントアトラスの事前スキャンと自動再構築**:
    描画対象となるすべての文字グリフを動的テクスチャアトラスへ登録。
    ※ グリフの追加によってテクスチャアトラスのオーバーフロー（満杯によるクリア）が発生した場合、即座に最初から全グリフを再キャッシュ構築するフォールバックを備えています。
2.  **ソート済みエンティティの巡回**: トランスフォームの累積マトリクスをその場で解決しながら各要素を走査。
3.  **`ExternalVisual`（実体）の処理**:
    くり抜き用インスタンスを追加し、パンチアウト用バッチとして即座に単独フラッシュ。その後、前面装飾用インスタンスを追加。
4.  **`ExternalTexture`（静止画）の処理**:
    テクスチャバインディングが固有に切り替わるため、独立したテクスチャバッチとして即座にフラッシュ。
5.  **テキストおよび装飾インスタンスの生成**:
    - テキスト選択ハイライト矩形
    - テキスト背景 + グリフごとの個別インスタンス + 前面装飾
    - 背景のない単体ボーダー描画のフォールバック
    - フォーカス中の点滅キャレット
6.  **バッチフラッシュ**: 最後に通常のUIクアッドバッチをフラッシュして完了。

---

### 8. `handle_accessibility(cx: &mut Context)`

Windows UI Automation (AccessKit) との通信を担当します。

1.  アクセシビリティ機能が無効化されている場合は早期リターン。
2.  再利用可能なスナップショットバッファを取得。
3.  現在のトポロジーおよびテキスト情報から AccessKit ノードツリーを構築 (`build_accessibility_snapshot`)。
4.  ツリーに変化がない（バッファが空の）場合はワーカースレッドへのディスパッチをスキップ。
5.  差分が存在する場合のみ、ワーカースレッドへツリーのスナップショットを送信。

---

## 2本のフラット配列による線形走査

`michiu_ui` は、データ指向設計の利点を生かし、フレームワークの根幹となる処理を2本のフラットな一次元配列へと集約しています。

1.  **DFS順シーケンス配列 (`topo_flat_dfs_sequence`)**:
    ツリーの深さ優先順序を一次元に並べた配列。レイアウト計算（Taffy）や親子のスタイル解決は、この配列を先頭から末尾へ線形走査するだけで完了します。

2.  **ソート済みエンティティ配列 (`topo_sorted_entities`)**:
    可視性、トランスフォーム、および z-index を考慮して並び替えた一次元配列。
    - **Drawバッチ生成**: 奥から手前へ正順で線形走査してインスタンスをパッキング。
    - **ヒットテスト**: 手前から奥へ逆順 (`rev()`) で線形走査して最前面の要素に当たった瞬間に早期リターン。

描画とヒットテストで同一の配列を反転利用することで、メモリの二重確保を抑制しています。
