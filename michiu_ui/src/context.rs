#![allow(unused)]
pub mod content_store;
pub mod event_store;
pub mod layout_store;
pub mod output_store;
pub mod reactive_store;
pub mod render_store;
pub mod system_store;
pub mod topology_store;
pub mod window_store;

pub use content_store::*;
pub use event_store::*;
pub use layout_store::*;
pub use output_store::*;
pub use reactive_store::*;
pub use render_store::*;
pub use system_store::*;
pub use topology_store::*;
pub use window_store::*;

use crate::*;
use slotmap::{KeyData, SecondaryMap, SlotMap, SparseSecondaryMap, new_key_type};
use smallvec::SmallVec;
use std::{
    any::TypeId,
    borrow::Cow,
    cell::RefCell,
    collections::{HashMap, HashSet},
    marker::PhantomData,
    path::PathBuf,
    sync::{
        Arc,
        mpsc::{Receiver, Sender},
    },
    time::{Duration, Instant},
};
use taffy::TaffyTree;
use windows::Win32::Graphics::DirectWrite::{DWRITE_HIT_TEST_METRICS, IDWriteTextLayout};

new_key_type! {
    /// UI内の各要素（Entity）を識別する一意な世代管理ID
    pub struct EntityId;
}

// 利用者用 Context を用意して安定APIはそちらで公開
// pub struct EventContext<'a> {
//    cx: &'a mut Context,
// }
// RawContext 側で全てのAPIを公開
// Facade化するのもあり
pub struct Context {
    pub topology: TopologyStore,
    pub layouts: LayoutStore,
    pub renders: RenderStore,
    pub outputs: OutputStore,
    pub contents: ContentStore,
    pub events: EventStore,
    pub reactive: ReactiveStore,
    pub window: WindowStore,
    pub system: SystemStore,
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();

        Self {
            topology: TopologyStore::new(),
            layouts: LayoutStore::new(),
            renders: RenderStore::new(),
            outputs: OutputStore::new(),
            contents: ContentStore::new(),
            events: EventStore::new(),
            reactive: ReactiveStore::new(),
            window: WindowStore::new(),
            system: SystemStore::new(
                TaskSender {
                    inner: tx,
                    waker: None,
                },
                rx,
            ),
        }
    }

    /// 一括解放
    pub fn clear(&mut self) {
        self.topology.clear();
        self.layouts.clear();
        self.renders.clear();
        self.outputs.clear();
        self.contents.clear();
        self.events.clear();
        self.reactive.clear();
        self.window.clear();
        self.system.clear();
    }

    /// 現在、システム内部に再描画要求（Dirtyマークされた要素）があるか判定します。
    #[inline]
    pub fn is_render_dirty(&self) -> bool {
        // dirty_render_entities に何か登録されている、またはレイアウトに Dirty がある場合
        !self.renders.dirty_render_entities.is_empty()
            || !self.layouts.dirty_layout_entities.is_empty()
            || self.layouts.is_structure_dirty
    }

    /// 現在イベントハンドラを実行している要素（自分自身）の EntityId を取得します
    #[inline]
    pub fn current_element_id(&self) -> Option<EntityId> {
        crate::signal::ACTIVE_ELEMENT.with(|cell| cell.get())
    }

    /// 現在イベントハンドラを実行している要素（自分自身） を取得します
    #[inline]
    pub fn try_current(&self) -> Option<Element> {
        self.current_element_id().map(|id| Element { id })
    }

    /// 現在イベントハンドラを実行している要素（自分自身） を取得します
    #[inline]
    pub fn current(&self) -> Element {
        Element {
            id: self
                .current_element_id()
                .expect("Context::current() called outside event dispatch"),
        }
    }

    #[inline]
    pub(crate) fn mark_dirty(&mut self, id: EntityId) {
        self.mark_layout_dirty(id);
        self.mark_render_dirty(id);
    }

    #[inline]
    pub fn entity_id_focused(&self) -> Option<EntityId> {
        self.events.interaction_states.focused
    }

    #[inline]
    pub fn entity_id_dragged(&self) -> Option<EntityId> {
        self.events.interaction_states.dragged
    }

    #[inline]
    pub fn entity_id_hovered(&self) -> Option<EntityId> {
        self.events.interaction_states.hovered
    }

    #[inline]
    pub fn entity_id_pressed(&self) -> Option<EntityId> {
        self.events.interaction_states.pressed
    }

    /// 指定された要素をプログラム駆動でクリックさせます
    pub fn trigger_element_click(&mut self, id: EntityId) {
        if !self.topology.entities.contains_key(id) || self.is_disabled(id) {
            return;
        }
        if let Some(mut listeners) = self.events.event_listeners.get_mut(id)
            && let Some(mut handler) = listeners.on_click.take()
        {
            let _guard = crate::ActiveElementGuard::new(id);
            handler(self);
            if let Some(l) = self.events.event_listeners.get_mut(id) {
                l.on_click = Some(handler);
            }
        }
    }

    /// 毎フレーム呼び出され、ドラッグ選択中の要素に対するオートスクロールを自律駆動します。
    /// ウィンドウメッセージループ等、 tick_transitions() を呼び出している箇所と同じ周期で実行する。
    #[inline]
    pub fn tick_drag_autoscroll(&mut self) {
        let (autoscroll_occurred, active_pos) = self.autoscroll_occurred();

        if autoscroll_occurred && let Some(pos) = active_pos {
            // スクロールによりテキストが流れたため、
            // 現在のポインタ座標で仮想的にポインタ移動を再トリガーし、
            // 選択文字インデックスおよびキャレット位置を同期
            self.inject_pointer_move(pos);

            if let Some(pressed_id) = self.events.interaction_states.pressed {
                self.mark_render_dirty(pressed_id);
            }
        }
    }

    /// ホバー（Hovered：マウスホバー）状態を更新します。
    ///
    /// ホバースタイル内にレイアウト変更プロパティ（幅やマージン等）が含まれていれば自動的にレイアウト再計算が要求され、
    /// 色や不透明度の変化だけであれば最速の描画更新（ファストパス）として処理されます。
    #[inline]
    pub fn set_hovered(&mut self, id: EntityId, hovered: bool) {
        self.update_state(id, STATE_HOVERED, hovered);
    }

    /// フォーカス（Focused：キーボードタブフォーカス等）状態を更新します。
    #[inline]
    pub fn set_focused(&mut self, id: EntityId, focused: bool) {
        self.update_state(id, STATE_FOCUSED, focused);
    }

    /// プレス（Pressed：クリック押し下げ、タップ中）状態を更新します。
    #[inline]
    pub fn set_pressed(&mut self, id: EntityId, pressed: bool) {
        self.update_state(id, STATE_PRESSED, pressed);
    }

    /// 無効化（Disabled：ボタンの操作不可など）状態を更新します。
    #[inline]
    pub fn set_disabled(&mut self, id: EntityId, disabled: bool) {
        self.update_state(id, STATE_DISABLED, disabled);
    }

    /// アクティブ（Actived：タブのトグル選択中など）状態を更新します。
    #[inline]
    pub fn set_actived(&mut self, id: EntityId, actived: bool) {
        self.update_state(id, STATE_ACTIVED, actived);
    }

    /// セレクト（Selected：チェックボックス、リストなどの選択）状態を更新します。
    #[inline]
    pub fn set_selected(&mut self, id: EntityId, selected: bool) {
        self.update_state(id, STATE_SELECTED, selected);
    }

    /// ドラッグ（Dragged：スライダーノブやスプリッターのドラッグ中）状態を更新します。
    #[inline]
    pub fn set_dragged(&mut self, id: EntityId, dragged: bool) {
        self.update_state(id, STATE_DRAGGED, dragged);
    }

    /// 要素のドラッグ・ドロップ擬似状態（STATE_DRAGGING, STATE_DRAG_IN, STATE_DRAG_OVER）を制御します。
    #[inline]
    pub(crate) fn set_drag_state(&mut self, id: EntityId, flag: u128, active: bool) {
        self.update_state(id, flag, active);
    }

    /// 各インタラクション状態（ステート）を更新し、レイアウト変更を伴うか自動的に判別して Dirty フラグを制御する共通ヘルパー
    #[inline(always)]
    fn update_state(&mut self, id: EntityId, state_flag: u128, active: bool) {
        let Some(mask) = self.topology.active_masks.get_mut(id) else {
            return;
        };

        let was_active = mask.has(state_flag);
        if (was_active == active) {
            return;
        }

        if active {
            mask.set(state_flag);
        } else {
            mask.unset(state_flag);
        }

        // 状態変化の発生時に、即座に動的なスタイルを解決する
        self.resolve_element_style_state(id, true);

        // STYLE_INTERACTION_WITHIN マスク判定による親先祖の早期バイパス
        let mut curr = id;
        while let Some(Some(parent_id)) = self.topology.parents.get(curr).copied() {
            if self.topology.entities.contains_key(parent_id) {
                let parent_mask = self.topology.active_masks[parent_id];

                // 先祖要素が within スタイルを1つでも持っている場合のみ深く入る
                if !parent_mask.has(STYLE_INTERACTION_WITHIN) {
                    return;
                }

                self.resolve_element_style_state(parent_id, true);

                if self.does_state_require_layout(parent_id, state_flag) {
                    self.mark_layout_dirty(parent_id);
                    self.mark_render_dirty(id);
                } else {
                    self.mark_render_dirty(parent_id);
                }
            }
            curr = parent_id;
        }

        // 残りの状態遷移イベントの解決
        if active {
            match state_flag {
                // Disabledになった瞬間
                STATE_DISABLED => {
                    if let Some(mut listeners) = self.events.event_listeners.get_mut(id)
                        && let Some(mut handler) = listeners.on_disable.take()
                    {
                        let _guard = crate::ActiveElementGuard::new(id);
                        handler(self);
                        if let Some(l) = self.events.event_listeners.get_mut(id) {
                            l.on_disable = Some(handler);
                        }
                    };
                }
                // アクティブになった瞬間
                STATE_ACTIVED => {
                    if let Some(mut listeners) = self.events.event_listeners.get_mut(id)
                        && let Some(mut handler) = listeners.on_active.take()
                    {
                        let _guard = crate::ActiveElementGuard::new(id);
                        handler(self);
                        if let Some(l) = self.events.event_listeners.get_mut(id) {
                            l.on_active = Some(handler);
                        }
                    };
                }
                // セレクトになった瞬間
                STATE_SELECTED => {
                    if let Some(mut listeners) = self.events.event_listeners.get_mut(id)
                        && let Some(mut handler) = listeners.on_select.take()
                    {
                        let _guard = crate::ActiveElementGuard::new(id);
                        handler(self);
                        if let Some(l) = self.events.event_listeners.get_mut(id) {
                            l.on_select = Some(handler);
                        }
                    };
                }
                _ => {}
            }
        }

        if self.does_state_require_layout(id, state_flag) {
            self.mark_layout_dirty(id);
            self.mark_render_dirty(id);
        } else {
            self.mark_render_dirty(id);
        }
    }

    /// キャッシュコヒーレントな直列DFS同期（1次元直線ループ同期）
    /// Taffy自動計算を完全内包
    pub fn sync_layout_and_render_list(&mut self, root: EntityId, window_size: LayoutSize) {
        // 同期処理の開始時に自身をバインドする
        let _context_guard = bind_context(self);
        // レイアウトが再計算される前に、溜まっているすべてのエフェクトを評価完了させる
        self.evaluate_pending_element_effects();
        // ウィンドウサイズの変更検知
        let window_resized = self.window_resize_detection(window_size);

        // 構造変更がなく、スタイル変更（レイアウト変更要求）もなく、ウィンドウサイズも変わっていないなら、
        // すべてスキップして早期リターン。
        if self.layouts.dirty_layout_entities.is_empty()
            && !self.layouts.is_structure_dirty
            && !window_resized
            && !self.outputs.rects.is_empty()
        {
            return;
        }

        if self.layouts.is_structure_dirty {
            self.rebuild_flat_dfs_sequence(root);
        }

        // 全スクロールバー関連IDを一括抽出
        let scrollbar_el_ids = self.scrollbar_el_ids();

        // 1. Taffy永続ツリーへの差分同期
        for id in &self.layouts.dirty_layout_entities {
            // スクロールバー専用要素は手動で物理座標を同期させるため、Taffyへの登録更新を完全にバイパス
            if scrollbar_el_ids.contains(id) {
                continue;
            }

            let (mut basic, flex, grid) = self.resolve_active_layouts(*id);

            // もしこの要素が現在アニメーション中（active_transitions に存在）であれば、
            // resolve_active_layouts が強制マージした目標値を拒否し、
            // tick_transitions が毎フレーム更新している現在値に上書きし直して Taffy に送信。
            if let Some(active_list) = self.renders.active_transitions.get(*id) {
                for t_state in active_list {
                    match t_state.property_list {
                        PropertyList::Width => {
                            if let Some(layout) = self.layouts.basic_layouts.get(*id) {
                                basic.size.width = layout.size.width;
                            }
                        }
                        PropertyList::Height => {
                            if let Some(layout) = self.layouts.basic_layouts.get(*id) {
                                basic.size.height = layout.size.height;
                            }
                        }
                        _ => {}
                    }
                }
            }

            let t_style = self.resolve_taffy_style(*id, &basic, &flex, grid.as_ref());
            let t_node = self.layouts.taffy_nodes[*id];

            self.layouts.taffy.set_style(t_node, t_style).unwrap();
        }

        // 2. Taffy のレイアウト再計算
        if let Some(&root_node) = self.layouts.taffy_nodes.get(root) {
            // 計測関数をクロージャとして定義
            let measure_func = |known_dims: taffy::Size<Option<f32>>,
                                available_space: taffy::Size<taffy::AvailableSpace>,
                                _node_id: taffy::NodeId,
                                context: Option<&mut EntityId>,
                                _style: &taffy::Style|
             -> taffy::Size<f32> {
                // 幅と高さの両方がすでにスタイル（known_dims）として解決されている場合はそれを最優先する
                if let (Some(w), Some(h)) = (known_dims.width, known_dims.height) {
                    return taffy::Size {
                        width: w,
                        height: h,
                    };
                }

                if let Some(&id) = context.as_deref() {
                    // テキスト内容を持っているかチェック
                    // (クロージャの外側の self (= Context) は直接キャプチャできないため、
                    //  一時的に bind_context されているスレッドローカル経由で取得)
                    return with_context(|cx| {
                        cx.measure_content(id, &self.renders.visual_properties, known_dims)
                    });
                }
                taffy::Size::ZERO
            };

            let _ = self.layouts.taffy.compute_layout_with_measure(
                root_node,
                taffy::Size {
                    width: taffy::AvailableSpace::Definite(window_size.width),
                    height: taffy::AvailableSpace::Definite(window_size.height),
                },
                measure_func,
            );
        }

        self.topology.active_entities.clear();

        // scroll_size を正しく算出するため、スワップおよび一旦コンテンツの rects のみを確定
        self.swap_output_rect();

        let flat_len = self.layouts.flat_dfs_sequence.len();

        // 1次元非再帰・静的キャッシュバイパスループ
        for i in 0..flat_len {
            let id = self.layouts.flat_dfs_sequence[i];

            // スクロールバー専用子要素は手動で物理座標を強制更新するため、この走査ループから完全にスルー
            if scrollbar_el_ids.contains(&id) {
                continue;
            }

            // 親の移動・リサイズ状態を検証
            let parent_changed = self.parent_changed(id);

            // 静的キャッシュバイパス判定
            let has_style_changed = self.topology.active_masks[id].has(STATE_QUEUED_LAYOUT);

            if !window_resized
                && !has_style_changed
                && !parent_changed
                && self.outputs.prev_rects.contains_key(id)
            {
                // 自分自身のスタイルが変わっておらず、親も動いていない、かつモニターリサイズもされていないならキャッシュ利用
                let cached_rect = self.outputs.prev_rects[id];
                self.outputs.rects.insert(id, cached_rect);

                // クリップも同様にキャッシュ再利用
                let cached_clip = self.outputs.prev_clip_rects[id];
                self.outputs.clip_rects.insert(id, cached_clip);

                self.topology.active_entities.push(id);
                continue;
            }

            let (abs_rect, parent_clip) = self.calc_local_rect(id, window_size);

            self.outputs.rects.insert(id, abs_rect);
            let mask = self.topology.active_masks[id];

            if mask.has(COMP_INPUT_CONTENT)
                && let Some(contents) = self.contents.input_contents.get_mut(id)
            {
                contents.last_bounds = Some(abs_rect);
            }

            let current_clip = if mask.has(STYLE_OVERFLOW) {
                parent_clip.intersect(&abs_rect)
            } else {
                parent_clip
            };
            self.outputs.clip_rects.insert(id, current_clip);

            // 常に1次元DFS順でアクティブ要素リストに登録する
            self.topology.active_entities.push(id);
        }

        // スクロールバー要素（Track & Thumb）のサイズ・配置・不透明度を一括同期更新
        self.sync_scrollbar_styles();

        // スクロールバー専用要素のサイズ・位置が確定したため、
        // 差分計算を走らせてマージンやパディングを考慮した物理位置を Taffy 内部で正確に解決
        if let Some(&root_node) = self.layouts.taffy_nodes.get(root) {
            let _ = self.layouts.taffy.compute_layout_with_measure(
                root_node,
                taffy::Size {
                    width: taffy::AvailableSpace::Definite(window_size.width),
                    height: taffy::AvailableSpace::Definite(window_size.height),
                },
                |known_dims: taffy::Size<Option<f32>>,
                 _available_space: taffy::Size<taffy::AvailableSpace>,
                 _node_id: taffy::NodeId,
                 context: Option<&mut EntityId>,
                 _style: &taffy::Style|
                 -> taffy::Size<f32> {
                    if let Some(&id) = context.as_deref() {
                        return with_context(|cx| {
                            if cx.topology.active_masks[id].has(COMP_INPUT_CONTENT)
                                && let Some(contents) = cx.contents.input_contents.get(id)
                                && let Some(layout_rect) = contents.last_layout
                            {
                                return taffy::Size {
                                    width: known_dims.width.unwrap_or(layout_rect.width),
                                    height: known_dims.height.unwrap_or(layout_rect.height),
                                };
                            }

                            // 2回目パスはキャッシュサイズを即時引き出して高速マッピング
                            if let Some(&rect) = cx.outputs.rects.get(id) {
                                taffy::Size {
                                    width: known_dims.width.unwrap_or(rect.width),
                                    height: known_dims.height.unwrap_or(rect.height),
                                }
                            } else {
                                taffy::Size::ZERO
                            }
                        });
                    }
                    taffy::Size::ZERO
                },
            );
        }

        // スクロールバー要素も含めて、Taffy から最終確定位置をすべて引き出して rects にマウント
        self.topology.active_entities.clear();

        for i in 0..flat_len {
            let id = self.layouts.flat_dfs_sequence[i];

            let (abs_rect, parent_clip) = self.calc_local_rect(id, window_size);

            self.outputs.rects.insert(id, abs_rect);
            let mask = self.topology.active_masks[id];

            if mask.has(COMP_INPUT_CONTENT)
                && let Some(contents) = self.contents.input_contents.get_mut(id)
            {
                contents.last_bounds = Some(abs_rect);
            }

            let current_clip = if mask.has(STYLE_OVERFLOW) {
                parent_clip.intersect(&abs_rect)
            } else {
                parent_clip
            };
            self.outputs.clip_rects.insert(id, current_clip);

            self.topology.active_entities.push(id);
        }

        // 全アクティブコンテナのスクロールオフセット自動クランプ同期
        for i in 0..flat_len {
            let id = self.layouts.flat_dfs_sequence[i];
            if self.outputs.scroll_offsets.contains_key(id) {
                let current = self.outputs.scroll_offsets[id];
                // 枠サイズの変更があった場合など、現在の位置からはみ出していれば自動クランプ調整
                self.scroll_to(id, current.x, current.y);
            }
        }

        // 全ての座標確定と絶対クリップ範囲の同期が完了した最末尾で、
        // 一括して Dirty フラグの完全クリアおよびキューリストのリセットを実行
        self.clear_layout_dirty();
    }

    /// 現在の全アクティブ要素から、wgpu 用の前面・背面描画バッチを生成します
    pub(crate) fn collect_render_data(&self) -> RenderData {
        let mut batches = Vec::new();
        let mut current_instances = Vec::new();
        let mut current_ids = Vec::new();
        let mut last_clip = None;

        // 現在のバッチの種類 (通常)
        let mut current_batch_type = BatchType::Normal;

        // 静的なデフォルト値（一度だけ確保して使い回す）
        let default_visual = VisualProperty::default();

        // 各要素の実効 z_index を親から子へカスケード（伝播）して計算
        let effective_z_indices = self.compute_effective_z_indices();

        // 実効 z_index で active_entities を安定ソート
        let mut sorted_entities = self.topology.active_entities.clone();
        sorted_entities.sort_by_key(|&id| effective_z_indices.get(id).copied().unwrap_or(0));

        // 溜まっているインスタンスを DrawBatch としてフラッシュ
        fn flush_batch(
            batches: &mut Vec<DrawBatch>,
            instances: &mut Vec<QuadInstance>,
            ids: &mut Vec<EntityId>,
            scissor_rect: LayoutRect,
            batch_type: BatchType,
        ) {
            if !instances.is_empty() {
                batches.push(DrawBatch {
                    scissor_rect,
                    instances: std::mem::take(instances),
                    entity_ids: std::mem::take(ids),
                    batch_type,
                });
            }
        }

        for &id in &sorted_entities {
            let rect = self.outputs.rects[id];
            if rect.width <= 0.0 || rect.height <= 0.0 {
                continue;
            }

            let clip = self.outputs.clip_rects[id];
            let is_webview = self.topology.active_masks[id].has(COMP_WEBVIEW_CONTENT);

            // コントローラーがまだ初期化されていない場合は通常通り背景を描画し透過を防止
            let is_webview_ready = is_webview && self.renders.active_webviews.contains(&id);

            let (basic, _, _) = self.resolve_active_layouts(id);
            let visual = self
                .renders
                .visual_properties
                .get(id)
                .unwrap_or(&default_visual);

            // 共通パラメータの展開
            let (packed_transform, origin) = self.get_transform_and_origin(visual);
            let (o_width, o_color, o_lengths, outline_offset_and_flags) =
                self.get_outline_params(visual);

            // --- 1. WebView (アクティブ) の個別処理 ---
            if is_webview_ready {
                // 溜まっている「通常（Normal）」のバッチがあれば一旦フラッシュ
                flush_batch(
                    &mut batches,
                    &mut current_instances,
                    &mut current_ids,
                    last_clip.unwrap_or(LayoutRect::ZERO),
                    current_batch_type,
                );

                let punchout_opacity = visual.opacity.unwrap_or(1.0);
                let punchout_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    color: Color::WHITE,
                    corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
                    opacity_mode_sizing: [punchout_opacity, 0.0, 0.0, 0.0],
                    ..Default::default()
                };
                current_instances.push(punchout_instance);
                current_ids.push(id);

                // くり抜き用のバッチとして即座にフラッシュ
                flush_batch(
                    &mut batches,
                    &mut current_instances,
                    &mut current_ids,
                    clip,
                    BatchType::Punchout,
                );

                // 前面装飾（通常）用のインスタンス
                let border_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
                    border_width: EdgeInsets {
                        top: basic.border.top.into(),
                        right: basic.border.right.into(),
                        bottom: basic.border.bottom.into(),
                        left: basic.border.left.into(),
                    },
                    border_color: visual.border_color.unwrap_or(Color::TRANSPARENT),
                    border_lengths: visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0)),
                    opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), 0.0, 0.0, 0.0],
                    outline_width: o_width,
                    outline_color: o_color,
                    outline_lengths: o_lengths,
                    outline_offset_and_flags,
                    ..Default::default()
                };
                current_instances.push(border_instance);
                current_ids.push(id);

                current_batch_type = BatchType::Normal;
                last_clip = Some(clip);
                continue;
            }

            // --- 2. WebView (非アクティブ・静止キャッシュ) の処理 ---
            let is_webview_static = is_webview && !is_webview_ready;
            if is_webview_static {
                // 一般UIインスタンスがあれば強制フラッシュ
                flush_batch(
                    &mut batches,
                    &mut current_instances,
                    &mut current_ids,
                    last_clip.unwrap_or(LayoutRect::ZERO),
                    current_batch_type,
                );

                let static_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
                    border_width: EdgeInsets {
                        top: basic.border.top.into(),
                        right: basic.border.right.into(),
                        bottom: basic.border.bottom.into(),
                        left: basic.border.left.into(),
                    },
                    border_color: visual.border_color.unwrap_or(Color::TRANSPARENT),
                    border_lengths: visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0)),
                    opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), 0.0, 0.0, 0.0],
                    ..Default::default()
                };
                current_instances.push(static_instance);
                current_ids.push(id);

                flush_batch(
                    &mut batches,
                    &mut current_instances,
                    &mut current_ids,
                    clip,
                    BatchType::Normal,
                );

                let border_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
                    border_width: EdgeInsets {
                        top: basic.border.top.into(),
                        right: basic.border.right.into(),
                        bottom: basic.border.bottom.into(),
                        left: basic.border.left.into(),
                    },
                    border_color: visual.border_color.unwrap_or(Color::TRANSPARENT),
                    border_lengths: visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0)),
                    opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), 0.0, 0.0, 0.0],
                    shadow_color: Color::WHITE,
                    outline_width: o_width,
                    outline_color: o_color,
                    outline_lengths: o_lengths,
                    outline_offset_and_flags,
                    ..Default::default()
                };
                current_instances.push(border_instance);
                current_ids.push(id);

                flush_batch(
                    &mut batches,
                    &mut current_instances,
                    &mut current_ids,
                    clip,
                    BatchType::Normal,
                );

                last_clip = Some(clip);
                continue;
            }

            // 一般要素
            if let Some(prev_clip) = last_clip {
                if clip != prev_clip {
                    flush_batch(
                        &mut batches,
                        &mut current_instances,
                        &mut current_ids,
                        prev_clip,
                        current_batch_type,
                    );
                    last_clip = Some(clip);
                }
            } else {
                last_clip = Some(clip);
            }

            // 選択ハイライト背景のwgpu側への差し込み
            if let Some(rects) = self.outputs.selected_rects.get(id) {
                let border = self.get_physical_border(id, &basic);
                let padding = self.get_physical_padding(id, &basic);
                let sel_bg = visual
                    .select_bg_color
                    .unwrap_or(Color::rgba_f32(0.0, 0.47, 0.84, 0.35));

                let scroll = self
                    .outputs
                    .scroll_offsets
                    .get(id)
                    .copied()
                    .unwrap_or(LayoutPoint::ZERO);

                for metric_rect in rects {
                    let sel_rect = LayoutRect::new(
                        rect.x + border.left + padding.left + metric_rect.x - scroll.x,
                        rect.y + border.top + padding.top + metric_rect.y - scroll.y,
                        metric_rect.width,
                        metric_rect.height,
                    );

                    let sel_instance = QuadInstance {
                        rect: sel_rect,
                        transform: packed_transform,
                        color: sel_bg,
                        opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), -1.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    current_instances.push(sel_instance);
                    current_ids.push(id);
                }
            }

            // 背景色とテキストの多重描画の解決
            let is_text = self.topology.active_masks[id].has(COMP_TEXT_CONTENT);
            let has_bg = visual.bg_color.is_some()
                || visual.bg_gradient.is_some()
                || visual.border_color.is_some()
                || visual.shadow_params.is_some();

            let box_sizing_val = match basic.box_sizing {
                BoxSizing::BorderBox => 0.0f32,
                BoxSizing::ContentBox => 1.0f32,
            };

            if is_text && has_bg {
                let bg_color = visual.bg_color.unwrap_or(Color::TRANSPARENT);
                let (gradient_end_color, gradient_angle, bg_mode) = match visual.bg_gradient {
                    Some(g) => (g.end_color, g.angle, 1.0f32),
                    None => (bg_color, 0.0, 0.0f32),
                };

                let bg_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    color: bg_color,
                    corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
                    border_width: EdgeInsets {
                        top: basic.border.top.into(),
                        right: basic.border.right.into(),
                        bottom: basic.border.bottom.into(),
                        left: basic.border.left.into(),
                    },
                    border_color: visual.border_color.unwrap_or(Color::TRANSPARENT),
                    border_lengths: visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0)),
                    opacity_mode_sizing: [
                        visual.opacity.unwrap_or(1.0),
                        bg_mode,
                        box_sizing_val,
                        0.0,
                    ],
                    gradient_end_color,
                    gradient_angle,
                    shadow_color: Color::WHITE,
                    shadow_params: [0.0; 4],
                    outline_width: o_width,
                    outline_color: o_color,
                    outline_lengths: o_lengths,
                    outline_offset_and_flags,
                    ..Default::default()
                };
                current_instances.push(bg_instance);
                current_ids.push(id);
            }

            // 通常のテキスト / 背景のレンダリング
            let color = if is_text {
                visual.text_color.unwrap_or(Color::BLACK)
            } else {
                visual.bg_color.unwrap_or(Color::TRANSPARENT)
            };

            let (gradient_end_color, gradient_angle, mode) = match visual.bg_gradient {
                Some(g) => (g.end_color, g.angle, 1.0f32),
                None => (color, 0.0, 0.0f32),
            };

            // テキスト要素で背景を分離描画した場合、テキストレイヤー側の装飾をクリア
            let bypass_decorations = is_text && has_bg;
            let border_width = if bypass_decorations {
                EdgeInsets::ZERO
            } else {
                EdgeInsets {
                    top: basic.border.top.into(),
                    right: basic.border.right.into(),
                    bottom: basic.border.bottom.into(),
                    left: basic.border.left.into(),
                }
            };
            let border_lengths = if bypass_decorations {
                EdgeInsets::ZERO
            } else {
                visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0))
            };
            let border_color = if bypass_decorations {
                Color::TRANSPARENT
            } else {
                visual.border_color.unwrap_or(Color::TRANSPARENT)
            };
            let shadow_color = if bypass_decorations {
                Color::TRANSPARENT
            } else {
                Color::WHITE
            };
            let outline_width = if bypass_decorations {
                EdgeInsets::ZERO
            } else {
                o_width
            };
            let outline_color = if bypass_decorations {
                Color::TRANSPARENT
            } else {
                o_color
            };
            let outline_lengths = if bypass_decorations {
                EdgeInsets::ZERO
            } else {
                o_lengths
            };
            let outline_offset_and_flags = if bypass_decorations {
                [0.0; 4]
            } else {
                outline_offset_and_flags
            };

            let instance = QuadInstance {
                rect,
                transform: packed_transform,
                transform_origin: origin,
                color,
                corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
                border_width,
                border_color,
                border_lengths,
                opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), mode, 0.0, 0.0],
                gradient_end_color,
                gradient_angle,
                shadow_color,
                outline_width,
                outline_color,
                outline_lengths,
                outline_offset_and_flags,
                ..Default::default()
            };

            current_instances.push(instance);
            current_ids.push(id);

            // インプット要素のキャレット描画
            let is_input = self.topology.active_masks[id].has(COMP_INPUT_CONTENT);
            let is_focused = self.events.interaction_states.focused == Some(id);

            if is_input
                && is_focused
                && let Some(contents) = self.contents.input_contents.get(id)
                && self.should_show_caret(contents)
            {
                let border = self.get_physical_border(id, &basic);
                let padding = self.get_physical_padding(id, &basic);
                let scale = self.window.scale_factor;
                let scroll = self
                    .outputs
                    .scroll_offsets
                    .get(id)
                    .copied()
                    .unwrap_or(LayoutPoint::ZERO);

                let caret_rect =
                    self.calculate_caret_rect(rect, border, padding, contents, scale, scroll);
                let c_color = contents
                    .caret_color
                    .or(visual.text_color)
                    .unwrap_or(Color::WHITE);

                let caret_instance = QuadInstance {
                    rect: caret_rect,
                    transform: packed_transform,
                    color: c_color,
                    opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), -1.0, 0.0, 0.0],
                    ..Default::default()
                };

                current_instances.push(caret_instance);
                current_ids.push(id);
            }
        }

        // 走査終了後、最後に残ったバッチをフラッシュ
        flush_batch(
            &mut batches,
            &mut current_instances,
            &mut current_ids,
            last_clip.unwrap_or(LayoutRect::ZERO),
            current_batch_type,
        );

        RenderData { batches }
    }

    
}

#[cfg(test)]
mod tests;
