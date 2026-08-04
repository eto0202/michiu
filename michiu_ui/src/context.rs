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

use crate::{
    ActiveFocusTrigger, CursorIcon, DragPayload, Element, ElementState, ImeState, LayoutPoint,
    LayoutRect, LayoutSize, Modifiers, MouseButton, Overflow, PlaybackCount, PointerEvents,
    PropertyList, ReadSignal, STATE_ACTIVED, STATE_DISABLED, STATE_DRAG_IN, STATE_DRAG_OVER,
    STATE_DRAGGED, STATE_DRAGGING, STATE_FOCUSED, STATE_FOCUSED_VISIBLE, STATE_HOVERED,
    STATE_PRESSED, STATE_QUEUED_LAYOUT, STATE_SELECTED, STYLE_OVERFLOW, STYLE_PREVENT_FOCUS_STEAL,
    STYLE_PREVENT_FOCUS_STEAL_WITHIN, TextAlign, TransitionValue, UserSelect, Val, VirtualKey,
    WriteSignal, bind_context, with_context,
};
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
    /// ツリー構造の構築、親子関係の管理、DFS走査順序。
    /// despawn 連鎖、DFS配列の構築、子孫/親の状態バブリング走査など。
    pub topology: TopologyStore,
    /// 解決済み基本/Flex/Gridスタイルデータの保持。
    /// TaffyTreeの同期、およびスクロールバー用要素のレイアウト。
    /// `taffy_style` への同期、物理ボーダー/パディング、コンテナ内径サイズの算出など。
    pub layouts: LayoutStore,
    /// 解決済みビジュアルスタイルの保持。
    /// DComp/wgpu用アニメーション・トランジションの時間軸Tick駆動、疑似スタイルのカスケード解決。
    /// `does_state_require_layout` 判定、フォーカス/ホバー等のカスケードマージなど。
    pub renders: RenderStore,
    /// 計算完了後の物理絶対座標、クリップ範囲の保持。
    /// キャレット・テキスト選択範囲の物理領域キャッシュ。
    /// `resolve_val_to_px` (単位の解決)、キャレット矩形の算出など。
    pub outputs: OutputStore,
    /// ユーザーコンテンツの保持。
    /// キャレット点滅判定、コンテンツサイズ計測など。
    pub contents: ContentStore,
    /// ユーザー入力リスナー、およびリサイズ/ドラッグセッション状態の保持。
    /// リサイズ方向検知、オートスクロールはみ出し距離など。
    pub events: EventStore,
    /// シグナル・エフェクト実体、プロバイダー依存関係の保持。
    /// プロバイダー引き当て、エフェクトの初回評価遅延処理など。
    pub reactive: ReactiveStore,
    /// ウィンドウ全体の基本状態（DPI、最終境界）の保持。
    /// ウィンドウリサイズ検知、可視矩形の算出など。
    pub window: WindowStore,
    /// OS機能（DWriteレイアウトキャッシュ、IMM32位置、UIAプロパティ）および非同期STAキューの保持。
    /// IMM32候補窓の物理位置同期、DWriteレイアウト生成など。
    pub system: SystemStore,
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    #[must_use]
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

    /// 外部公開用API: ハンドルを指定して要素を安全に破棄します。
    ///
    /// 親を持たないルート要素の破棄（手動での寿命管理）に使用します。
    /// 子要素が存在する場合は、自動的に再帰破棄されます。
    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.despawn_internal(id);
    }

    /// 指定した要素の子要素一覧を取得します。
    #[inline]
    pub fn children_list(&self, handle: Element) -> Option<Vec<Element>> {
        self.topology
            .children
            .get(handle.id)
            .map(|c| c.iter().map(|&id| Element { id }).collect())
    }

    /// 画面上でアクティブ（有効）になっている要素の総数を取得します。
    #[inline]
    pub fn active_entities_count(&self) -> usize {
        self.topology.active_entities.len()
    }

    /// 指定された要素が現在マウスホバーされているか判定します
    #[inline]
    pub fn is_hovered(&self, id: EntityId) -> bool {
        self.topology
            .active_masks
            .get(id)
            .is_some_and(|m| m.has(STATE_HOVERED))
    }

    /// 指定された要素が現在キーボードフォーカスを得ているか判定します
    #[inline]
    pub fn is_focused(&self, id: EntityId) -> bool {
        self.topology
            .active_masks
            .get(id)
            .is_some_and(|m| m.has(STATE_FOCUSED))
    }

    /// 指定された要素が現在マウスやタップで押し下げられているか判定します
    #[inline]
    pub fn is_pressed(&self, id: EntityId) -> bool {
        self.topology
            .active_masks
            .get(id)
            .is_some_and(|m| m.has(STATE_PRESSED))
    }

    /// 指定された要素が無効化（操作不可）状態にあるか判定します
    #[inline]
    pub fn is_disabled(&self, id: EntityId) -> bool {
        self.topology
            .active_masks
            .get(id)
            .is_some_and(|m| m.has(STATE_DISABLED))
    }

    /// 指定された要素が現在アクティブ（有効選択など）状態にあるか判定します
    #[inline]
    pub fn is_actived(&self, id: EntityId) -> bool {
        self.topology
            .active_masks
            .get(id)
            .is_some_and(|m| m.has(STATE_ACTIVED))
    }

    /// 指定された要素が現在テキストまたはトグル選択されているか判定します
    #[inline]
    pub fn is_selected(&self, id: EntityId) -> bool {
        self.topology
            .active_masks
            .get(id)
            .is_some_and(|m| m.has(STATE_SELECTED))
    }

    /// 指定された要素が現在ドラッグ操作中にあるか判定します
    #[inline]
    pub fn is_dragged(&self, id: EntityId) -> bool {
        self.topology
            .active_masks
            .get(id)
            .is_some_and(|m| m.has(STATE_DRAGGED))
    }

    /// 現在、システム内部に再描画要求（Dirtyマークされた要素）があるか判定します。
    #[inline]
    pub fn is_render_dirty(&self) -> bool {
        // dirty_render_entities に何か登録されている、またはレイアウトに Dirty がある場合
        !self.renders.dirty_render_entities.is_empty()
            || !self.layouts.dirty_layout_entities.is_empty()
            || self.topology.is_structure_dirty
    }

    /// 現在イベントハンドラを実行している要素（自分自身）の `EntityId` を取得します
    #[inline]
    pub fn current_element_id(&self) -> Option<EntityId> {
        crate::signal::ACTIVE_ELEMENT.with(std::cell::Cell::get)
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
    pub fn mark_dirty(&mut self, id: EntityId) {
        self.mark_layout_dirty(id);
        self.mark_render_dirty(id);
    }

    #[inline]
    pub fn clear_layout_dirty(&mut self) {
        let TopologyStore { active_masks, .. } = &mut self.topology;
        let LayoutStore {
            dirty_layout_entities,
            ..
        } = &mut self.layouts;

        LayoutStore::clear_layout_dirty(dirty_layout_entities, active_masks);
    }

    /// 指定した要素の画面上の絶対座標（LayoutRect）を取得します。
    #[inline]
    pub fn rect(&self, id: EntityId) -> Option<LayoutRect> {
        let OutputStore { rects, .. } = &self.outputs;

        OutputStore::rect(id, rects)
    }

    /// 指定した要素の画面上のクリップ境界（LayoutRect）を取得します。
    #[inline]
    pub fn clip_rect(&self, id: EntityId) -> Option<LayoutRect> {
        self.outputs.clip_rects.get(id).copied()
    }

    /// 現在フォーカスされている要素で範囲選択されている文字列を取得します。
    #[inline]
    pub fn get_selected_text(&self) -> Option<String> {
        OutputStore::get_selected_text(&self.events, &self.renders, &self.outputs, &self.contents)
    }

    /// 現在のスクロール位置から相対移動します。
    pub fn scroll_by(&mut self, id: EntityId, dx: f32, dy: f32) -> bool {
        let TopologyStore {
            entities,
            parents,
            children,
            active_masks,
            active_entities,
            session_spawned,
            session_roots,
            flat_dfs_sequence,
            is_structure_dirty,
        } = &mut self.topology;
        let LayoutStore {
            basic_layouts,
            base_basic_layouts,
            flex_layouts,
            grid_layouts,
            scrollbar_styles,
            taffy_nodes,
            taffy,
            dirty_layout_entities,
        } = &mut self.layouts;
        let RenderStore {
            visual_properties,
            interaction_properties,
            base_visual_properties,
            dirty_render_entities,
            active_transitions,
            active_animations,
            active_webviews,
            last_tick_time,
        } = &mut self.renders;
        let OutputStore {
            rects,
            clip_rects,
            scroll_offsets,
            prev_rects,
            prev_clip_rects,
            selected_rects,
            text_selections,
            selection_start_index,
        } = &mut self.outputs;
        let ContentStore {
            text_contents,
            text_spans,
            input_contents,
            image_sources,
            movie_properties,
            webview_contents,
        } = &mut self.contents;
        let WindowStore {
            last_window_size, ..
        } = &mut self.window;
        let SystemStore {
            text_engine,
            dwrite_layouts,
            uia_properties,
            task_sender,
            task_receiver,
        } = &mut self.system;
        OutputStore::scroll_by(
            id,
            dx,
            dy,
            active_masks,
            input_contents,
            text_engine,
            text_contents,
            visual_properties,
            text_spans,
            dwrite_layouts,
            basic_layouts,
            flex_layouts,
            grid_layouts,
            active_transitions,
            parents,
            children,
            interaction_properties,
            rects,
            scrollbar_styles,
            scroll_offsets,
            *last_window_size,
            taffy_nodes,
            taffy,
            dirty_layout_entities,
        )
    }

    /// Context インスタンスから直接シグナルを生成します。
    /// これにより `build_ui` の外側（メインスレッド上）でもシグナルを定義できます。
    #[inline]
    pub fn create_signal<T: Send + 'static>(
        &mut self,
        initial_value: T,
    ) -> (ReadSignal<T>, WriteSignal<T>) {
        let ReactiveStore {
            signals,
            subscribers,
            ..
        } = &mut self.reactive;

        ReactiveStore::create_signal(initial_value, signals, subscribers)
    }

    /// 現在のスレッドローカルコンテキストから、
    /// 親ツリーを自動的に遡って解決した型 T のシグナルに対する同期書き込み用端（WriteSignal）を取得します。
    #[inline]
    pub fn use_provided_setter<T: Send + 'static>(&self) -> WriteSignal<T> {
        let ReactiveStore {
            effect_to_element,
            providers,
            ..
        } = &self.reactive;
        let TopologyStore { parents, .. } = &self.topology;

        let element_id = ReactiveStore::resolve_element_effect(effect_to_element)
                .unwrap_or_else(|| {
                    panic!(
                        "use_provided_setter must be called inside a dynamic reactive context or an active event handler context"
                    );
                });

        ReactiveStore::use_provided_setter_from::<T>(element_id, providers, parents)
                .unwrap_or_else(|| {
                    panic!(
                        "Dependency resolution failed: No Provider Setter found in ancestor sub-tree for type: '{}'",
                        std::any::type_name::<T>()
                    )
                })
    }

    /// 現在のスレッドローカルコンテキスト（アクティブなエフェクト、またはイベントハンドラ）から、
    /// 自動的に対象の要素を特定し、親ツリーを遡って型 T の `ReadSignal` を解決します。
    #[inline]
    pub fn use_provided<T: Clone + 'static>(&self) -> ReadSignal<T> {
        let ReactiveStore {
            providers,
            effect_to_element,
            ..
        } = &self.reactive;
        let TopologyStore { parents, .. } = &self.topology;

        let element_id = ReactiveStore::resolve_element_effect(effect_to_element)
                .unwrap_or_else(|| {
                    panic!(
                        "use_provided must be called inside a dynamic style, text, content closure, or an active event handler context"
                    );
                });

        ReactiveStore::use_provided_from::<T>(element_id, providers, parents)
                .unwrap_or_else(|| {
                    panic!(
                        "Dependency resolution failed: No Provider found in ancestor sub-tree for type: '{}'",
                        std::any::type_name::<T>()
                    )
                })
    }

    pub fn try_use_provided<T: Clone + 'static>(&self) -> Option<ReadSignal<T>> {
        let ReactiveStore {
            providers,
            effect_to_element,
            ..
        } = &self.reactive;
        let TopologyStore { parents, .. } = &self.topology;

        let element_id = ReactiveStore::resolve_element_effect(effect_to_element)?;
        ReactiveStore::use_provided_from::<T>(element_id, providers, parents)
    }

    /// 現在ホバーされている要素から親ツリーを遡り、適用するべき物理的な `CursorIcon` を正確に解決します。
    #[inline]
    pub fn resolve_cursor(&self, hovered_id: EntityId) -> CursorIcon {
        let RenderStore {
            visual_properties,
            base_visual_properties,
            ..
        } = &self.renders;
        let EventStore {
            interaction_states, ..
        } = &self.events;
        let TopologyStore { parents, .. } = &self.topology;

        RenderStore::resolve_cursor(
            hovered_id,
            interaction_states,
            visual_properties,
            base_visual_properties,
            parents,
        )
    }

    /// 描画（レンダー）ダーティ状態として登録された要素をすべてクリアします。
    #[inline]
    pub fn clear_render_dirty(&mut self) {
        let RenderStore {
            dirty_render_entities,
            ..
        } = &mut self.renders;
        let TopologyStore { active_masks, .. } = &mut self.topology;

        RenderStore::clear_render_dirty(dirty_render_entities, active_masks);
    }

    /// 現在、アクティブに動いているトランジション（wgpuアニメーション）があるか判定します
    #[inline]
    pub fn has_active_animations(&self) -> bool {
        let RenderStore {
            active_animations,
            active_transitions,
            visual_properties,
            ..
        } = &self.renders;
        let EventStore {
            interaction_states,
            current_pointer_position,
            ..
        } = &self.events;
        let OutputStore { clip_rects, .. } = &self.outputs;
        let LayoutStore {
            scrollbar_styles, ..
        } = &self.layouts;
        let ContentStore { input_contents, .. } = &self.contents;

        RenderStore::has_active_animations(
            interaction_states,
            current_pointer_position.as_ref(),
            clip_rects,
            visual_properties,
            active_transitions,
            active_animations,
            input_contents,
            scrollbar_styles,
        )
    }

    /// 毎フレームの描画前に呼び出され、すべてのアクティブなキーフレームアニメーションを 1 Tick 進めます
    pub fn tick_animations(&mut self) {
        let now = Instant::now();

        // 借用チェッカーを回避するため、一時的にマップを take して更新
        let mut active_map = std::mem::take(&mut self.renders.active_animations);
        let mut to_remove = Vec::new();

        for (id, animations) in &mut active_map {
            let mut i = 0;
            while i < animations.len() {
                let anim = &mut animations[i];
                let elapsed = now.duration_since(anim.start_time);
                let elapsed_secs = elapsed.as_secs_f32();
                let duration_secs = anim.duration.as_secs_f32();

                // 1. 現在の周回回数（ループインデックス）の算出
                let current_iteration = (elapsed_secs / duration_secs).floor() as u32;

                // ループ制限に達しているかチェック
                let is_finished = match anim.iteration_count {
                    PlaybackCount::Count(max_count) => current_iteration >= max_count,
                    PlaybackCount::Infinite => false,
                };

                if is_finished {
                    // ループ終了：目標の最終値（end_value）で固定してアニメーションを破棄
                    self.apply_animation_value(id, anim.property, &anim.end_value);
                    animations.remove(i);
                    continue;
                }

                // 2. 現在のループ内での正規化進行度 (0.0 ～ 1.0) の計算
                let local_time = elapsed_secs % duration_secs;
                let progress = if duration_secs > 0.0 {
                    (local_time / duration_secs).min(1.0)
                } else {
                    1.0
                };
                let eased_t = anim.curve.evaluate(progress);

                // 3. 値の補間
                let current_val = anim.start_value.lerp(&anim.end_value, eased_t);

                // 4. SoA へ補間された動的スタイル値を上書き書き戻し
                self.apply_animation_value(id, anim.property, &current_val);

                // レンダラーへ再描画要求（ファストパス）
                self.mark_render_dirty(id);

                i += 1;
            }

            if animations.is_empty() {
                to_remove.push(id);
            }
        }

        // 空になったエントリをクリーンアップ
        for id in to_remove {
            active_map.remove(id);
        }
        self.renders.active_animations = active_map;
    }

    /// 毎フレームの描画前に呼び出され、すべてのアクティブなトランジションを 1 Tick 進めます
    pub fn tick_transitions(&mut self) {
        const FRAME_TIME_120FPS: Duration = Duration::from_nanos(8_333_333);
        let now = Instant::now();

        // (1.0 / 120.0 秒 = 約 8,333,333 ナノ秒)
        if let Some(last) = self.renders.last_tick_time
            && now.duration_since(last) < FRAME_TIME_120FPS
        {
            return;
        }

        // 実行制限を通過したため、基準時刻を更新して処理を継続
        self.renders.last_tick_time = Some(now);

        // 借用チェッカーを回避するため、一時的にマップを take して更新する
        let mut active_map = std::mem::take(&mut self.renders.active_transitions);

        // 完了して空になった要素のIDを記録する一時配列
        let mut to_remove = Vec::new();

        for (id, transitions) in &mut active_map {
            let mut i = 0;
            while i < transitions.len() {
                let t_state = &mut transitions[i];

                // start_time が None なら、このフレームの時刻 now を格納しその値を取り出す。
                let start_time = *t_state.start_time.get_or_insert(now);
                let elapsed = now.duration_since(start_time);

                // 進行度 (0.0 ～ 1.0)
                let progress = (elapsed.as_secs_f32() / t_state.duration.as_secs_f32()).min(1.0);
                let eased_t = t_state.curve.evaluate(progress);

                // Lerpによる新しい値の決定
                let current_val = t_state.start_value.lerp(&t_state.end_value, eased_t);

                // SoA（Context のアクティブなプロパティ）に補間された値を書き戻す
                match current_val {
                    TransitionValue::Color(c) => {
                        if let Some(v) = self.renders.visual_properties.get_mut(id) {
                            if t_state.property_list == PropertyList::BackgroundColor {
                                v.bg_color = Some(c);
                            } else if t_state.property_list == PropertyList::BorderColor {
                                v.border_color = Some(c);
                            }
                        }
                        self.mark_render_dirty(id);
                    }
                    TransitionValue::Opacity(o) => {
                        if let Some(v) = self.renders.visual_properties.get_mut(id) {
                            v.opacity = Some(o);
                        }
                        self.mark_render_dirty(id);
                    }
                    TransitionValue::Transform(m) => {
                        if let Some(v) = self.renders.visual_properties.get_mut(id) {
                            v.transform = Some(m);
                        }
                        self.mark_render_dirty(id);
                    }
                    TransitionValue::CornerRadius(cr) => {
                        if let Some(v) = self.renders.visual_properties.get_mut(id) {
                            v.corner_radius = Some(cr);
                        }
                        self.mark_render_dirty(id);
                    }
                    TransitionValue::Width(w) => {
                        if let Some(layout) = self.layouts.basic_layouts.get_mut(id) {
                            layout.size.width = Val::Px(w); // ピクセル値で上書き
                        }
                        self.mark_layout_dirty(id); // レイアウト再計算をマーク

                        // キャッシュを毎フレーム強制バイパスさせるためにマスクを再セット
                        if let Some(mask) = self.topology.active_masks.get_mut(id) {
                            mask.set(STATE_QUEUED_LAYOUT);
                        }
                    }
                    // 縦幅（Height）の毎フレームアニメーション補間
                    TransitionValue::Height(h) => {
                        if let Some(layout) = self.layouts.basic_layouts.get_mut(id) {
                            layout.size.height = Val::Px(h);
                        }
                        self.mark_layout_dirty(id);

                        if let Some(mask) = self.topology.active_masks.get_mut(id) {
                            mask.set(STATE_QUEUED_LAYOUT);
                        }
                    }
                    // 影（BoxShadow）の毎フレームの書き戻し処理
                    TransitionValue::BoxShadow(shadow) => {
                        if let Some(v) = self.renders.visual_properties.get_mut(id) {
                            v.shadow_params = Some(shadow);
                            v.shadow_color = Some(shadow.color);
                        }
                        self.mark_render_dirty(id);
                    }
                }

                // アニメーション完了判定
                if progress >= 1.0 {
                    transitions.remove(i);
                } else {
                    i += 1;
                }

                // トランジションが空になった要素をマーク
                if transitions.is_empty() {
                    to_remove.push(id);
                }
            }
        }

        // 空になったエントリをマップから完全削除（クリーンアップ）
        for id in to_remove {
            active_map.remove(id);
        }

        self.renders.active_transitions = active_map;
    }

    /// ワーカースレッドなど、どこからでも安全にクローンしてタスクを送信できるスレッドセーフな送信端を取得します。
    #[inline]
    pub fn task_sender(&self) -> TaskSender {
        self.system.task_sender.clone()
    }

    /// ウィンドウ生成後に起床用コールバックを登録します。
    #[inline]
    pub fn set_waker<F>(&mut self, f: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.system.task_sender.waker = Some(Arc::new(f));
    }

    /// メインスレッドの毎フレーム開始時（またはイベントハンドラの先頭など）に呼び出され、
    /// バックグラウンドから届いたシグナル更新タスクなどの処理を安全に一括実行します。
    #[inline]
    pub fn process_main_thread_tasks(&mut self) {
        let _context_guard = bind_context(self);
        // キューに溜まっているクロージャをすべてメインスレッドのコンテキスト上で実行
        while let Ok(task) = self.system.task_receiver.try_recv() {
            task(self);
        }
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
    #[inline]
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

    /// 現在のテキスト・IME状態・フォントサイズから、
    /// キャレットの物理座標や最終表示テキスト、レイアウト矩形を正確に再計算して `SoA` を更新。
    pub fn update_input_caret_position(&mut self, id: EntityId) {
        self.clear_layout_cache(id); // IMEやタイピング中の古いキャッシュを破棄

        let scroll_ime_info = OutputStore::scroll_ime_info(
            id,
            &mut self.outputs,
            &mut self.contents,
            &mut self.renders,
            &self.system,
        );

        let Some((caret, caret_offset, is_multiline)) = scroll_ime_info else {
            return;
        };
        let (basic, _, _) = self.resolve_active_layouts(id);
        let rect = self.rect(id).unwrap_or_default();
        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

        let mut scroll = self
            .outputs
            .scroll_offsets
            .get(id)
            .copied()
            .unwrap_or(LayoutPoint::ZERO);
        let scale = self.window.scale_factor;

        if rect.width > 0.0 && rect.height > 0.0 {
            let viewport_w =
                (rect.width - border.left - border.right - padding.left - padding.right).max(0.0);
            let viewport_h =
                (rect.height - border.top - border.bottom - padding.top - padding.bottom).max(0.0);

            let text_size = if let Some(contents) = self.contents.input_contents.get(id)
                && let Some(layout_rect) = contents.last_layout
            {
                LayoutSize::new(layout_rect.width, layout_rect.height)
            } else {
                LayoutSize::ZERO
            };
            let (_, flex, _) = self.resolve_active_layouts(id);
            let content_w =
                (rect.width - border.left - border.right - padding.left - padding.right).max(0.0);
            let align_offset_x = match flex.text_align {
                TextAlign::Center => ((content_w - text_size.width) * 0.5).max(0.0),
                TextAlign::Right => (content_w - text_size.width).max(0.0),
                _ => 0.0,
            };
            let content_h =
                (rect.height - border.top - border.bottom - padding.top - padding.bottom).max(0.0);
            let align_offset_y = ((content_h - text_size.height) * 0.5).max(0.0);

            let aligned_caret_x = caret.x + align_offset_x;
            let aligned_caret_y = caret.y + align_offset_y;

            // マージンを設定するとキー移動時にキャレット位置がずれるため削除
            // let margin_x = 0.0; // 左右端のあそび（マージン）

            // 1. 横方向スクロール (X軸)
            if aligned_caret_x < scroll.x {
                scroll.x = aligned_caret_x.max(0.0);
            } else if aligned_caret_x + caret.width > scroll.x + viewport_w {
                scroll.x = (aligned_caret_x + caret.width - viewport_w).max(0.0);
            }

            // 2. 縦方向スクロール (Y軸 - マルチラインのみ)
            if is_multiline {
                if aligned_caret_y < scroll.y {
                    scroll.y = aligned_caret_y.max(0.0);
                } else if aligned_caret_y + caret.height > scroll.y + viewport_h {
                    scroll.y = (aligned_caret_y + caret.height - viewport_h).max(0.0);
                }
            } else {
                scroll.y = 0.0;
            }

            self.scroll_to(id, scroll.x, scroll.y);
        }

        // IMM32 による IME 変換候補ウィンドウの位置同期を自動実行
        SystemStore::sync_imm_window_position(
            rect,
            scale,
            border,
            padding,
            caret,
            caret_offset,
            scroll,
        );
    }

    /// 毎フレーム呼び出され、ドラッグ選択中の要素に対するオートスクロールを自律駆動します。
    /// ウィンドウメッセージループ等、 `tick_transitions()` を呼び出している箇所と同じ周期で実行する。
    #[inline]
    pub fn tick_drag_autoscroll(&mut self) {
        let TopologyStore {
            active_masks,
            parents,
            children,
            ..
        } = &mut self.topology;
        let LayoutStore {
            basic_layouts,
            base_basic_layouts,
            flex_layouts,
            grid_layouts,
            scrollbar_styles,
            dirty_layout_entities,
            taffy,
            taffy_nodes,
            ..
        } = &mut self.layouts;
        let RenderStore {
            visual_properties,
            interaction_properties,
            active_transitions,
            dirty_render_entities,
            ..
        } = &mut self.renders;
        let ContentStore {
            input_contents,
            text_contents,
            text_spans,
            ..
        } = &mut self.contents;
        let OutputStore {
            rects,
            scroll_offsets,
            clip_rects,
            ..
        } = &mut self.outputs;
        let EventStore {
            current_pointer_position,
            interaction_states,
            ..
        } = &mut self.events;
        let SystemStore {
            text_engine,
            dwrite_layouts,
            ..
        } = &mut self.system;
        let WindowStore {
            last_window_size, ..
        } = &mut self.window;

        let Some(id) = interaction_states.pressed else {
            return;
        };
        let (autoscroll_occurred, active_pos) = EventStore::autoscroll_occurred(
            id,
            *current_pointer_position,
            clip_rects,
            active_masks,
            input_contents,
            text_engine,
            text_contents,
            visual_properties,
            text_spans,
            dwrite_layouts,
            basic_layouts,
            flex_layouts,
            grid_layouts,
            active_transitions,
            parents,
            children,
            interaction_properties,
            rects,
            scrollbar_styles,
            scroll_offsets,
            *last_window_size,
            taffy_nodes,
            taffy,
            dirty_layout_entities,
        );

        if autoscroll_occurred && let Some(pos) = active_pos {
            // スクロールによりテキストが流れたため、
            // 現在のポインタ座標で仮想的にポインタ移動を再トリガーし、
            // 選択文字インデックスおよびキャレット位置を同期
            self.inject_pointer_move(pos);

            self.mark_render_dirty(id);
        }
    }

    /// ホバー（Hovered：マウスホバー）状態を更新します。
    ///
    /// ホバースタイル内にレイアウト変更プロパティ（幅やマージン等）が含まれていれば自動的にレイアウト再計算が要求され、
    /// 色や不透明度の変化だけであれば最速の描画更新（ファストパス）として処理されます。
    #[inline]
    pub fn set_hovered(&mut self, id: EntityId, hovered: bool) {
        EventStore::update_state(self, id, STATE_HOVERED, hovered);
    }

    /// フォーカス（Focused：キーボードタブフォーカス等）状態を更新します。
    #[inline]
    pub fn set_focused(&mut self, id: EntityId, focused: bool) {
        self.set_focused_by_trigger(id, focused, ActiveFocusTrigger::Mouse);
    }

    /// 入力トリガー源を考慮してフォーカス状態を更新します。
    #[inline]
    pub fn set_focused_by_trigger(
        &mut self,
        id: EntityId,
        focused: bool,
        trigger: ActiveFocusTrigger,
    ) {
        EventStore::update_state(self, id, STATE_FOCUSED, focused);
        let show_visible = focused && (trigger == ActiveFocusTrigger::Keyboard);
        EventStore::update_state(self, id, STATE_FOCUSED, focused);
    }

    /// プレス（Pressed：クリック押し下げ、タップ中）状態を更新します。
    #[inline]
    pub fn set_pressed(&mut self, id: EntityId, pressed: bool) {
        EventStore::update_state(self, id, STATE_PRESSED, pressed);
    }

    /// 無効化（Disabled：ボタンの操作不可など）状態を更新します。
    #[inline]
    pub fn set_disabled(&mut self, id: EntityId, disabled: bool) {
        EventStore::update_state(self, id, STATE_DISABLED, disabled);
    }

    /// アクティブ（Actived：タブのトグル選択中など）状態を更新します。
    #[inline]
    pub fn set_actived(&mut self, id: EntityId, actived: bool) {
        EventStore::update_state(self, id, STATE_ACTIVED, actived);
    }

    /// セレクト（Selected：チェックボックス、リストなどの選択）状態を更新します。
    #[inline]
    pub fn set_selected(&mut self, id: EntityId, selected: bool) {
        EventStore::update_state(self, id, STATE_SELECTED, selected);
    }

    /// ドラッグ（Dragged：スライダーノブやスプリッターのドラッグ中）状態を更新します。
    #[inline]
    pub fn set_dragged(&mut self, id: EntityId, dragged: bool) {
        EventStore::update_state(self, id, STATE_DRAGGED, dragged);
    }

    /// `要素のドラッグ・ドロップ擬似状態（STATE_DRAGGING`, `STATE_DRAG_IN`, `STATE_DRAG_OVER）を制御します`。
    #[inline]
    pub(crate) fn set_drag_state(&mut self, id: EntityId, flag: u128, active: bool) {
        EventStore::update_state(self, id, flag, active);
    }

    pub fn inject_pointer_move(&mut self, logical_pos: LayoutPoint) {
        let _context_guard = bind_context(self);

        let prev_pos = self.events.current_pointer_position;
        self.events.current_pointer_position = Some(logical_pos);

        // リサイズ中のドラッグ同期処理
        if let Some(state) = self.events.resizing_state.clone() {
            self.sync_resizing_drag(logical_pos, &state);
            return; // リサイズドラッグ中は、通常のホバーやドラッグ判定を完全にスキップして早期リターン
        }

        self.sync_scrollbar_drag(logical_pos);

        // マウスボタン押し下げ中は、他の要素へのインタラクション漏洩を防ぐためヒット先を押し下げ要素に強制ロック
        let target_id = if let Some(pressed_id) = self.events.interaction_states.pressed {
            Some(pressed_id)
        } else {
            self.hit_test(logical_pos)
        };

        // 直前のリサイズホバー対象を退避
        let prev_resize_hover = self.events.active_resize_hover;
        // リサイズホバー情報を一旦リセット
        self.events.active_resize_hover = None;

        // ヒットした要素、およびその親先祖に向かってツリーを遡上
        let (current_id, found_resize_hover) = self.found_resize_hover(target_id, logical_pos);

        if let Some((id, dir)) = found_resize_hover {
            self.events.active_resize_hover = Some((id, dir));
            let vis = self.renders.visual_properties.get(id).unwrap();
            self.apply_resizable_cursor_style(id, dir);
            self.mark_render_dirty(id);
        }

        // 枠線から外れた、または異なる要素に変わった場合
        if let Some((prev_id, _)) = prev_resize_hover {
            let now_id = self.events.active_resize_hover.map(|(id, _)| id);

            // 異なるホバー状態になった場合、旧要素のカーソル上書きを破棄し本来のスタイルに即時強制リセット
            if Some(prev_id) != now_id {
                // スタイルの再解決を叩き、上書きされていた vis.cursor を本来のカーソル（通常ホバー/ベース等）へ復旧
                self.resolve_element_style_state(prev_id, false);
                self.mark_render_dirty(prev_id);
            }
        }

        if let Some(pressed_id) = self.events.interaction_states.pressed {
            let user_select = self.get_user_select(pressed_id);

            if user_select == UserSelect::Text
                && let Some(start_pos) = self.outputs.selection_start_index.get(pressed_id).copied()
            {
                // プレースホルダー選択のドラッグ遮断
                if let Some(contents) = self.contents.input_contents.get(pressed_id) {
                    let is_placeholder = contents.text.0.get().is_empty();
                    let is_ime = contents
                        .ime_state
                        .as_ref()
                        .is_none_or(|s| s.composition_text.is_empty());

                    if is_placeholder && is_ime && !contents.placeholder_select {
                        return;
                    }
                }

                let local = self.pressed_local_point(pressed_id, logical_pos);
                self.handle_text_selection_click(pressed_id, start_pos, local);
            }
        }

        // ヒットテスト
        let target_id = self.hit_test(logical_pos);

        // ホバー（Enter/Leave）状態の解決
        if target_id != self.events.interaction_states.hovered {
            EventStore::resolve_hover_state(self, target_id);
        }

        // カーソル移動イベントの伝播
        self.propagate_cursor_move_events(target_id, logical_pos);

        // ドラッグイベントの伝播
        self.propagate_drag_events(prev_pos, logical_pos);

        // D&D プレースホルダーの移動とドロップ先ホバー検知
        let Some(mut drag_state) = self.events.active_drag_state.clone() else {
            return;
        };

        // ウィンドウの真のルート要素を解決
        let root = self
            .find_root_entity()
            .expect("Root EntityId not found in Context");

        let src_id = drag_state.source_entity;
        let placeholder_id = drag_state.placeholder_entity;
        let drag_prop = self.events.drag_properties.get(src_id).copied().unwrap();

        // アタッチ先親コンテナ基準での相対ローカル座標を逆算して追従（Inset更新）
        self.update_inset_based_relative_local(
            root,
            placeholder_id,
            logical_pos,
            drag_prop,
            &drag_state,
        );

        // 現在ホバー侵入中のドロップターゲット要素を検知
        let found_drop_target =
            self.detect_drop_target_during_intrusion(src_id, placeholder_id, logical_pos);

        // ドロップ先のホバー切り替えイベントを解決（STATE_DRAG_IN の同期）
        self.sync_state_drag_in(&mut drag_state, found_drop_target);

        self.callback_drag_prop(src_id, found_drop_target, drag_prop);
    }

    pub fn inject_pointer_button(
        &mut self,
        button: MouseButton,
        state: ElementState,
        modifiers: Modifiers,
    ) {
        let _context_guard = bind_context(self);
        let current_hovered = self.events.interaction_states.hovered;

        match state {
            ElementState::Pressed => {
                if button == MouseButton::Left {
                    // リサイズドラッグの開始判定
                    if let Some((id, dir)) = self.events.active_resize_hover {
                        self.state_pressed_resize_drag(id, dir);
                        self.mark_render_dirty(id);
                        return; // リサイズドラッグが開始されたため、通常のクリック・フォーカス処理を完全にバイパス
                    }
                }

                if let Some(pointer_pos) = self.events.current_pointer_position
                    && let Some(target_id) = current_hovered
                {
                    // ヒットした要素がサム、またはトラックであるかを判定
                    let clicked_scrollbar =
                        self.hit_decision_element_scrollbar(target_id, pointer_pos);
                    if clicked_scrollbar {
                        return; // 背後の一般子要素へのイベント透過を防止
                    }
                }

                if let Some(target_id) = current_hovered {
                    self.events.interaction_states.pressed = Some(target_id);
                    self.set_pressed(target_id, true);

                    let user_select = self.get_user_select(target_id);

                    let is_input = self.topology.active_masks[target_id].has_input_content();

                    if user_select == UserSelect::Text
                        && !is_input
                        && let Some(pointer_pos) = self.events.current_pointer_position
                    {
                        self.handle_user_select_text(target_id, pointer_pos, modifiers.shift);
                    }

                    // prevent_focus_steal の解決
                    let mut prevent_steal = false;
                    let mut curr = Some(target_id);
                    while let Some(curr_id) = curr {
                        let mask = self
                            .topology
                            .active_masks
                            .get(curr_id)
                            .copied()
                            .unwrap_or_default();
                        if mask.has(STYLE_PREVENT_FOCUS_STEAL)
                            && curr_id == target_id
                            && self
                                .renders
                                .visual_properties
                                .get(curr_id)
                                .and_then(|v| v.prevent_focus_steal)
                                .unwrap_or(false)
                        {
                            prevent_steal = true;
                            break;
                        }
                        if mask.has(STYLE_PREVENT_FOCUS_STEAL_WITHIN)
                            && self
                                .renders
                                .visual_properties
                                .get(curr_id)
                                .and_then(|v| v.prevent_focus_steal_within)
                                .unwrap_or(false)
                        {
                            prevent_steal = true;
                            break;
                        }
                        curr = self.topology.parents.get(curr_id).copied().flatten();
                    }
                    if !prevent_steal {
                        // フォーカス可能要素のみにフォーカスを制限
                        let is_focusable = self.restrict_focusable_element(target_id);
                        if is_focusable {
                            // フォーカスの自動切り替え
                            self.auto_focus_switch_by_trigger(target_id, ActiveFocusTrigger::Mouse);
                        } else {
                            // フォーカス不可能な要素をクリックした場合は、
                            // 現在フォーカスされているインプットからフォーカスを完全に外し状態をクリアする
                            self.handle_remove_focus();
                        }
                    }

                    self.handle_on_mouse_input(target_id, button, modifiers, state);
                }
            }
            ElementState::Released => {
                // リサイズドラッグの終了処理
                if let Some(state) = self.events.resizing_state.take() {
                    let id = state.entity_id;
                    self.events.interaction_states.pressed = None;

                    // 元のリサイズホバーカーソル表示を維持するために再検出をマーク
                    // リサイズ状態が解除された「この瞬間」に現在の座標で move を再キックし、
                    // すり抜けていた通常のホバー・離脱判定（Leave）を正確に評価させる
                    if let Some(pos) = self.events.current_pointer_position {
                        self.inject_pointer_move(pos);
                    }
                    self.mark_render_dirty(id);
                    return;
                }

                // D&D ドラッグ終了・ドロップ確定処理
                if let Some(drag_state) = self.events.active_drag_state.take() {
                    let src_id = drag_state.source_entity;
                    let holder = drag_state.placeholder_entity;
                    let drag_prop = self.events.drag_properties.get(src_id).copied().unwrap();

                    // 疑似クラス（STATE_DRAGGING, STATE_DRAG_IN）を解除
                    self.set_drag_state(src_id, STATE_DRAGGING, false);
                    if let Some(target_id) = drag_state.current_drop_target {
                        self.set_drag_state(target_id, STATE_DRAG_IN, false);
                    }

                    // プレースホルダー要素を親および Taffy から安全にデスポーン
                    // このタイミングでは despawn_internal せず最後に移動
                    self.events.interaction_states.pressed = None;
                    self.events.interaction_states.dragged = None;

                    let drop_success = drag_state.current_drop_target;

                    // 実体移動（DragMode::Entity）の場合のツリートポロジー書き換え
                    if let Some(target_id) = drop_success
                        && drag_prop.drag_mode == DragPayload::Element
                        && let Some(prop) = self.events.drop_properties.get(target_id).copied()
                    {
                        // ドラッグ元要素を現在の親の children リストから安全に引き抜いて削除
                        self.remove_dragged_elemet(src_id, &drag_state);

                        self.rewrite_tree_topology(
                            src_id,
                            target_id,
                            holder,
                            drag_prop,
                            &drag_state,
                        );
                        self.topology.is_structure_dirty = true;
                    }

                    if let Some(ph_children) = self.topology.children.get(holder).cloned() {
                        // 避難していた本物の子要素トポロジーを、元の要素（src_id）の配下へ自動復元
                        self.restore_child(src_id, holder, ph_children);
                        // プレースホルダー側は空にして破棄に備える
                        if let Some(ph_children_mut) = self.topology.children.get_mut(holder) {
                            ph_children_mut.clear();
                        }
                        self.mark_layout_dirty(src_id);
                        self.mark_layout_dirty(holder);
                    }

                    match drag_prop.drag_mode {
                        DragPayload::Element => {
                            self.callback_on_entity_drop(src_id, drop_success);
                        }
                        DragPayload::EntityId => {
                            self.callback_on_id_drop(src_id, drop_success);
                        }
                    }

                    // 位置情報の安全な回収がすべて完了した、この最末尾で初めてプレースホルダーを破棄
                    self.despawn_internal(holder);

                    // 離脱直後に位置を再移動評価して、通常のホバーを正しく復元
                    if let Some(pos) = self.events.current_pointer_position {
                        self.inject_pointer_move(pos);
                    }

                    self.mark_render_dirty(src_id);
                    return;
                }

                let dirty_ids = self.get_scrollbar_dirty_ids();
                for id in dirty_ids {
                    self.mark_render_dirty(id);
                }

                if let Some(pressed_id) = self.events.interaction_states.pressed {
                    self.set_pressed(pressed_id, false);
                    self.set_dragged(pressed_id, false);
                    self.events.interaction_states.dragged = None;

                    if let Some(contents) = self.contents.input_contents.get_mut(pressed_id) {
                        contents.is_selecting = false;
                    }

                    // on_mouse_input の発火（ボタンの種類を問わず常に呼ぶ）
                    self.callback_on_mouse_input(pressed_id, button, modifiers, state);

                    // 同一要素上で離された場合の各種クリック解決
                    if self.events.interaction_states.hovered == Some(pressed_id) {
                        match button {
                            // 左クリックの解決
                            MouseButton::Left => {
                                self.callback_on_click(pressed_id);
                            }
                            MouseButton::Right => {
                                self.callback_on_right_click(pressed_id);
                            }
                            _ => {}
                        }
                    }

                    self.events.interaction_states.pressed = None;
                }
            }
        }
    }

    // ダブルクリック
    pub fn inject_pointer_double_click(&mut self, modifiers: Modifiers) {
        let _context_guard = bind_context(self);
        let current_hovered = self.events.interaction_states.hovered;

        if let Some(target_id) = current_hovered {
            let user_select = self.get_user_select(target_id);

            if user_select == UserSelect::Text
                && let Some(pointer_pos) = self.events.current_pointer_position
            {
                if let Some(contents) = self.contents.input_contents.get(target_id) {
                    let text_val = contents.text.0.get();
                    let is_placeholder = text_val.is_empty()
                        && contents
                            .ime_state
                            .as_ref()
                            .is_none_or(|s| s.composition_text.is_empty());

                    if is_placeholder && !contents.placeholder_select {
                        return;
                    }
                }

                let rect = self.rect(target_id).unwrap_or_default();
                let (basic, _, _) = self.resolve_active_layouts(target_id);
                let (border, padding) =
                    LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

                let local_x = pointer_pos.x - (rect.x + border.left + padding.left);
                let local_y = pointer_pos.y - (rect.y + border.top + padding.top);

                if let Some(layout) = self.get_or_create_layout(target_id) {
                    let (clicked_index, is_trailing) = self
                        .system
                        .text_engine
                        .hit_test_point(&layout, local_x, local_y);
                    let final_index = if is_trailing {
                        clicked_index + 1
                    } else {
                        clicked_index
                    };

                    if let Some(text) = self.contents.text_contents.get(target_id) {
                        let text_u16: Vec<u16> = text.encode_utf16().collect();

                        // 高精度な文節境界を抽出
                        let range = crate::find_word_boundaries(&text_u16, final_index);

                        self.outputs
                            .text_selections
                            .insert(target_id, range.clone());
                        // アンカー開始を文節左端にセット
                        self.outputs
                            .selection_start_index
                            .insert(target_id, range.start);
                        self.update_selection_rects(target_id); // 選択矩形を更新

                        if let Some(contents) = self.contents.input_contents.get_mut(target_id) {
                            contents.selected_range = range;
                            contents.selection_reversed = false; // キャレットは右端に配置
                            self.update_input_caret_position(target_id);
                        }

                        self.mark_render_dirty(target_id);
                    }
                }
            }
        }
    }

    /// 外部で計算された論理ピクセルスクロール移動量 (`scroll_x`, `scroll_y`) を注入し、
    /// バブリングによる自動スクロール処理、またはユーザーイベントハンドラへの配送を行います。
    pub fn inject_mouse_wheel(&mut self, scroll_x: f32, scroll_y: f32) {
        let _context_guard = bind_context(self);

        let mut curr = self.events.interaction_states.hovered;
        let mut handled = false;

        // イベントバブリング: ホバー要素から親へ辿る
        while let Some(curr_id) = curr {
            // 個別に定義された `on_mouse_wheel` ハンドラがあれば最優先実行
            if let Some(l) = self.events.event_listeners.get_mut(curr_id)
                && let Some(mut handler) = l.on_mouse_wheel.take()
            {
                let _guard = crate::ActiveElementGuard::new(curr_id);
                handler(self, scroll_x, scroll_y);
                if let Some(l) = self.events.event_listeners.get_mut(curr_id) {
                    l.on_mouse_wheel = Some(handler);
                }
                handled = true; // イベントが消費されたため、これ以降のコンテナスクロールは行わない
                break;
            }

            // ユーザーハンドラがない場合、要素がスクロールコンテナであるか判定
            let mask = self.topology.active_masks[curr_id];
            if mask.has(STYLE_OVERFLOW) {
                let (basic, _, _) = self.resolve_active_layouts(curr_id);

                let mut scrolled = false;

                // 縦方向スクロール
                if scroll_y != 0.0
                    && (basic.overflow.y == Overflow::Scroll
                        || basic.overflow.y == Overflow::Hidden)
                    && self.scroll_by(curr_id, 0.0, scroll_y)
                {
                    scrolled = true;
                }

                // 横方向スクロール
                if scroll_x != 0.0
                    && (basic.overflow.x == Overflow::Scroll
                        || basic.overflow.x == Overflow::Hidden)
                    && self.scroll_by(curr_id, scroll_x, 0.0)
                {
                    scrolled = true;
                }

                if scrolled {
                    handled = true;
                    break; // スクロールを実行したためバブリングを終了
                }
            }

            // 先祖へ伝播
            curr = self.topology.parents.get(curr_id).copied().flatten();
        }
    }

    pub fn inject_keyboard_key(
        &mut self,
        key: VirtualKey,
        state: ElementState,
        modifiers: Modifiers,
    ) {
        let _context_guard = bind_context(self);

        // Tabキー押下時は個別のフォーカス対象へのイベント配信前に巡回処理を実行
        if state == ElementState::Pressed && key == VirtualKey::TAB {
            self.cycle_keyboard_focus(modifiers.shift);
            return;
        }

        if let Some(focused_id) = self.events.interaction_states.focused {
            // フォーカス中に Enter または Space が押されたら自動的にクリックをエミュレートする
            if state == ElementState::Pressed
                && (key == VirtualKey::RETURN || key == VirtualKey::SPACE)
                && !self.topology.active_masks[focused_id].has_input_content()
            {
                self.callback_on_click(focused_id);
                return;
            }
            // 内部で完結する全選択（Ctrl+A）のみを自動処理
            if state == ElementState::Pressed && modifiers.ctrl {
                let user_select = self.get_user_select(focused_id);

                if key == VirtualKey::A && user_select == UserSelect::Text {
                    if let Some(layout) = self.get_or_create_layout(focused_id)
                        && let Some(text) = self.contents.text_contents.get(focused_id)
                    {
                        let u16_len = text.encode_utf16().count();
                        let full_range = 0..u16_len;

                        self.outputs
                            .text_selections
                            .insert(focused_id, full_range.clone());

                        self.update_selection_rects(focused_id);

                        if let Some(contents) = self.contents.input_contents.get_mut(focused_id) {
                            contents.selected_range = full_range;
                            contents.selection_reversed = false;
                            self.update_input_caret_position(focused_id);
                        }
                        self.mark_render_dirty(focused_id);
                    }
                    return;
                }
            }

            self.callback_on_keyboard_input(focused_id, key, modifiers, state);
        }
    }

    /// キーボードフォーカスを次の適格な要素へ巡回させます
    pub fn cycle_keyboard_focus(&mut self, reverse: bool) {
        if self.topology.flat_dfs_sequence.is_empty() {
            return;
        }

        let len = self.topology.flat_dfs_sequence.len();

        // 現在フォーカスされている要素のインデックスを特定（無ければ探索方向の末端から開始）
        let current_focused = self.events.interaction_states.focused;
        let start_idx = current_focused
            .and_then(|id| {
                self.topology
                    .flat_dfs_sequence
                    .iter()
                    .position(|&x| x == id)
            })
            .unwrap_or(if reverse { len - 1 } else { 0 });

        let mut idx = start_idx;
        loop {
            // インデックスの増減と循環
            if reverse {
                idx = if idx == 0 { len - 1 } else { idx - 1 };
            } else {
                idx = if idx == len - 1 { 0 } else { idx + 1 };
            }

            // 1周して元の位置に戻ってきた場合は、他にフォーカス可能な要素がないため終了
            if idx == start_idx {
                break;
            }

            let candidate_id = self.topology.flat_dfs_sequence[idx];

            if self.is_keyboard_focusable(candidate_id) {
                // 古い要素のフォーカスを外し、新しい要素へフォーカスを設定
                if let Some(old_id) = self.events.interaction_states.focused {
                    self.set_focused_by_trigger(old_id, false, ActiveFocusTrigger::Keyboard);
                }
                self.set_focused_by_trigger(candidate_id, true, ActiveFocusTrigger::Keyboard);
                self.events.interaction_states.focused = Some(candidate_id);

                // WebView2 要素だった場合はシステム側にフォーカスをプログラム駆動で移譲
                if self.topology.active_masks[candidate_id].has_webveiw2_content() {
                    // 通常のレンダラーから focus_webview を呼び出すためここでは何もしない
                }

                self.mark_render_dirty(candidate_id);
                break;
            }
        }
    }

    #[inline]
    pub fn inject_character(&mut self, c: char) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.events.interaction_states.focused
            && let Some(l) = self.events.event_listeners.get_mut(focused_id)
            && let Some(mut handler) = l.on_char_input.take()
        {
            let _guard = crate::ActiveElementGuard::new(focused_id);
            handler(self, c);
            if let Some(l) = self.events.event_listeners.get_mut(focused_id) {
                l.on_char_input = Some(handler);
            }
        }
    }

    #[inline]
    pub fn inject_ime(&mut self, ime_state: ImeState) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.events.interaction_states.focused
            && let Some(l) = self.events.event_listeners.get_mut(focused_id)
            && let Some(mut handler) = l.on_ime.take()
        {
            let _guard = crate::ActiveElementGuard::new(focused_id);
            handler(self, ime_state);
            if let Some(l) = self.events.event_listeners.get_mut(focused_id) {
                l.on_ime = Some(handler);
            }
        }
    }

    #[inline]
    pub fn inject_file_dropped(&mut self, paths: Vec<PathBuf>) {
        let _context_guard = bind_context(self);
        if let Some(target_id) = self.events.interaction_states.hovered
            && let Some(l) = self.events.event_listeners.get_mut(target_id)
            && let Some(mut handler) = l.on_file_dropped.take()
        {
            let _guard = crate::ActiveElementGuard::new(target_id);
            handler(self, paths);
            if let Some(l) = self.events.event_listeners.get_mut(target_id) {
                l.on_file_dropped = Some(handler);
            }
        }
    }

    /// 外部から提供されたテキストを、現在フォーカスされている入力要素にペーストします。
    #[inline]
    pub fn inject_paste(&mut self, text: &str) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.events.interaction_states.focused
            && self.topology.active_masks[focused_id].has_input_content()
            && let Some(contents) = self.contents.input_contents.get_mut(focused_id)
        {
            OutputStore::inject_paste_internal(focused_id, text, &mut self.outputs, contents);

            self.update_input_caret_position(focused_id);
            self.mark_render_dirty(focused_id);
        }
    }

    /// Undo (元に戻す) のインジェクション
    #[inline]
    pub fn inject_undo(&mut self) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.events.interaction_states.focused
            && self.topology.active_masks[focused_id].has_input_content()
            && let Some(contents) = self.contents.input_contents.get_mut(focused_id)
            && let Some((prev_text, prev_sel)) = contents.undo_stack.pop()
        {
            OutputStore::inject_undo_internal(
                focused_id,
                prev_sel,
                prev_text,
                &mut self.outputs,
                contents,
            );

            self.update_input_caret_position(focused_id);
            self.mark_render_dirty(focused_id);
        }
    }

    /// Redo (やり直し) のインジェクション
    #[inline]
    pub fn inject_redo(&mut self) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.events.interaction_states.focused
            && self.topology.active_masks[focused_id].has_input_content()
            && let Some(contents) = self.contents.input_contents.get_mut(focused_id)
            && let Some((next_text, next_sel)) = contents.redo_stack.pop()
        {
            OutputStore::inject_redo_internal(
                focused_id,
                next_sel,
                next_text,
                &mut self.outputs,
                contents,
            );

            self.update_input_caret_position(focused_id);
            self.mark_render_dirty(focused_id);
        }
    }

    /// 切り取り (Ctrl+X) の実行と削除後のテキスト取得
    #[inline]
    pub fn inject_cut(&mut self) -> Option<String> {
        let _context_guard = bind_context(self);
        let focused_id = self.events.interaction_states.focused?;
        let user_select = self.get_user_select(focused_id);

        if user_select == UserSelect::Text
            && let Some(range) = self.outputs.text_selections.get(focused_id).cloned()
            && range.start < range.end
            && let Some(text) = self.contents.text_contents.get(focused_id)
        {
            let u16_text: Vec<u16> = text.encode_utf16().collect();
            let slice = &u16_text[range.start.min(u16_text.len())..range.end.min(u16_text.len())];
            let cut_text = String::from_utf16(slice).ok()?;

            // 対象が Input コントロールである場合のみ、切り取り削除上書きを実行
            if self.topology.active_masks[focused_id].has_input_content()
                && let Some(contents) = self.contents.input_contents.get_mut(focused_id)
            {
                OutputStore::inject_cut_internal(focused_id, range, &mut self.outputs, contents);

                self.update_input_caret_position(focused_id);
                self.mark_render_dirty(focused_id);
            }

            return Some(cut_text);
        }

        None
    }

    /// マウス座標などが、要素の描画領域かつ表示枠内に収まっているかを判定。
    /// 階層的な早期枝刈りヒットテスト
    pub fn hit_test(&self, point: LayoutPoint) -> Option<EntityId> {
        // 各要素の実効 z_index を、親から子へカスケードして算出
        let mut effective_z_indices =
            SecondaryMap::with_capacity(self.topology.active_entities.len());
        for &id in &self.topology.flat_dfs_sequence {
            let self_z = self
                .renders
                .visual_properties
                .get(id)
                .and_then(|v| v.z_index);

            let parent_z = self
                .topology
                .parents
                .get(id)
                .copied()
                .flatten()
                .and_then(|pid| effective_z_indices.get(pid).copied());

            let eff_z = self_z.or(parent_z).unwrap_or(0);
            effective_z_indices.insert(id, eff_z);
        }

        // 実効 z_index に基づいて active_entities を安定ソート
        let mut sorted_entities = self.topology.active_entities.clone();
        sorted_entities.sort_by_key(|&id| effective_z_indices.get(id).copied().unwrap_or(0));

        for &id in sorted_entities.iter().rev() {
            // ドラッグ中かつゴースト化した元の実体要素、およびプレースホルダー要素はヒットテストを強制スルーさせる
            if Some(id) == self.events.interaction_states.dragged
                || self.topology.active_masks[id].has(STATE_DRAG_OVER)
            {
                continue;
            }

            // 親などの overflow 等でクリップされている表示範囲外ならスキップ
            if let Some(clip) = self.outputs.clip_rects.get(id)
                && !clip.contains(point)
            {
                continue;
            }

            // pointer-events 設定の解決
            let pointer_events = self
                .renders
                .visual_properties
                .get(id)
                .and_then(|v| v.pointer_events)
                .or_else(|| {
                    self.renders
                        .base_visual_properties
                        .get(id)
                        .and_then(|v| v.pointer_events)
                })
                .unwrap_or(PointerEvents::Auto);

            if pointer_events == PointerEvents::None {
                continue; // 透過設定
            }

            // 物理範囲にヒットしたかを検証
            if let Some(rect) = self.rect(id)
                && rect.contains(point)
            {
                return Some(id);
            }
        }
        None
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
            && !self.topology.is_structure_dirty
            && !window_resized
            && !self.outputs.rects.is_empty()
        {
            return;
        }

        if self.topology.is_structure_dirty {
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
                    return with_context(|cx| cx.measure_content(id, known_dims));
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

        let flat_len = self.topology.flat_dfs_sequence.len();

        // 1次元非再帰・静的キャッシュバイパスループ
        for i in 0..flat_len {
            let id = self.topology.flat_dfs_sequence[i];

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

            if mask.has_input_content()
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
                            if cx.topology.active_masks[id].has_input_content()
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
            let id = self.topology.flat_dfs_sequence[i];

            let (abs_rect, parent_clip) = self.calc_local_rect(id, window_size);

            self.outputs.rects.insert(id, abs_rect);
            let mask = self.topology.active_masks[id];

            if mask.has_input_content()
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
            let id = self.topology.flat_dfs_sequence[i];
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
}

#[cfg(test)]
mod tests;
