引数の爆発と参照元の混乱を防ぐため、以下の命名規則と引数配置ルールを徹底する。

---

## 1. ストア接頭辞

各配列のフィールド名、および関数の引数名には、所属するストアを示す以下の接頭辞を必ず付与。

| 優先度 | ストア名   | 接頭辞    | 主な役割                                             |
| :----- | :--------- | :-------- | :--------------------------------------------------- |
| 1      | `window`   | `win_`    | OS/ウィンドウ基本状態 (最終境界、DPIなど)            |
| 2      | `system`   | `sys_`    | OS機能/非同期キュー (DWrite、IMM32など)              |
| 3      | `reactive` | `react_`  | シグナル・エフェクト実体と依存関係                   |
| 4      | `events`   | `evt_`    | 入力リスナー                                         |
| 5      | `dnd`      | `dnd_`    | ドラッグ状態                                         |
| 6      | `resize`   | `res_` | リサイズ状態                                         |
| 7      | `contents` | `cont_`   | ユーザーデータ (テキスト内容、外部テクスチャなど)    |
| 8      | `topology` | `topo_`   | ツリー構造、親子関係、DFS走査順序                    |
| 9      | `layouts`  | `lay_`    | 解決済みレイアウトスタイル (Taffy連携、paddingなど)  |
| 10     | `renders`  | `rnd_`    | ビジュアルスタイル、アニメーション・トランジション   |
| 11     | `outputs`  | `out_`    | 最終計算完了後の物理絶対座標キャッシュ、スクロール量 |

---

## 2. 引数の配置ルール

多引数関数を実装する際は、以下の６つの基準に沿って引数をソート。

1. `Context` がある場合は引数名を `cx` に統一。
2. `EntityId`
3. 関数に渡す細かいデータ。
4. 配列は **ストアの優先度順 (1 〜 9)** に並べる。
5. 同一ストアの引数の中では、**可変参照 (`&mut`) を先、不変参照 (`&`) を後**に配置。
6. ブロックごとに、各Storeの配列の並び順と対応させる (手遅れ)。

### 例

```rust
pub(crate) fn some_helper_function(
    id: EntityId,
    cx: &mut Context,
    window_size: LayoutSize,

    // 6. topology
    topo_active_masks: &mut ActiveMasks, // &mut
    topo_parents: &Parents,              // &
    topo_children: &Children,            // &

    // 7. layouts
    lay_dirty_layouts: &mut DirtyLayouts,
    lay_basic_layouts: &BasicLayouts,

    // 9. outputs
    out_scroll_offsets: &mut ScrollOffsets,
    out_rects: &Rects,
) {
    // 実装...
}
```

---

## 3. 一方通行データフロー（Context -> Pipeline -> Stores）

処理の全体像を一方向のストリームに統一

- 外部/アダプタ:
  - `Context` のメソッドを呼び出す。
- Context (`context.rs`):
  - 自ら直接ストアの同期や変更は行わず、実体を `Pipeline` のメソッドに丸投げ。
- Pipeline (`pipeline.rs`):
  - 処理の進行順を一意に記述。
  - `&mut Context` を分割借用し、処理の流れの中で各ストアに必要なリソースを順番に配る。
- 各 Store (`topology_store.rs` 等):
  - 他のストアの存在を一切知らない状態にする。
  - 自ストアが所有する SoA 配列の確保・初期化・破棄、および自ストアに閉じた値のクエリや書き換えに特化する。

---

## 4. Pipeline 内の設計

- 上位オーケストレーター:
  - `&mut Context` を引数に取る。
  - cx の個別フィールド（`cx.events` 等）へアクセスして処理順に下位ヘルパーへ渡す。
- 下位ヘルパー:
  - `Context` 全体を引数に取らず、SoA 配列や一部ストアのみを直接引数に取る。
  - ルール1, 2を厳密に適用。

---

## 5. ファイル構成

- 各ストア定義 (`context/*_store/mod.rs`):
  - 構造体、SoA 配列、単純な初期化・追加・削除・ゲッターのみを記述。
- パイプラインモジュール (`context/pipeline.rs`):
  - ライフサイクルの主要な関数（イベント起点〜wgpu用パッキングまで）を処理の進行順に配置。
