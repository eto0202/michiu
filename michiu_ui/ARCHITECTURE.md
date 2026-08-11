# プロジェクト設計

引数の爆発と参照元の混乱を防ぐため、以下の命名規則と引数配置ルールを徹底する。

---

## 1. ストア接頭辞

各配列のフィールド名、および関数の引数名には、所属するストアを示す以下の接頭辞を必ず付与。

| 優先度 | ストア名   | 接頭辞   | 主な役割                                             |
| :----- | :--------- | :------- | :--------------------------------------------------- |
| 1      | `window`   | `win_`   | OS/ウィンドウ基本状態 (最終境界、DPIなど)            |
| 2      | `system`   | `sys_`   | OS機能/非同期キュー (DWrite、IMM32など)              |
| 3      | `reactive` | `react_` | シグナル・エフェクト実体と依存関係                   |
| 4      | `events`   | `evt_`   | 入力リスナー、ドラッグ・リサイズ状態                 |
| 5      | `contents` | `cont_`  | ユーザーデータ (テキスト内容、キャレット点滅など)    |
| 6      | `topology` | `topo_`  | ツリー構造、親子関係、DFS走査順序                    |
| 7      | `layouts`  | `lay_`   | 解決済みレイアウトスタイル (Taffy連携、paddingなど)  |
| 8      | `renders`  | `ren_`   | ビジュアルスタイル、アニメーション・トランジション   |
| 9      | `outputs`  | `out_`   | 最終計算完了後の物理絶対座標キャッシュ、スクロール量 |

---

## 2. 引数の配置ルール

多引数関数を実装する際は、以下の2つの基準に沿って引数をソート。

1. **ストアの優先度順 (1 〜 9)** に並べる。
2. 同一ストアの引数の中では、**可変参照 (`&mut`) を先、不変参照 (`&`) を後**に配置。

### 例

```rust
pub(crate) fn some_helper_function(
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
