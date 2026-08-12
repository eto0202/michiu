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
    ActiveFocusTrigger, CursorIcon, DndDragPayload, Element, ElementState, ImeState, LayoutPoint,
    LayoutRect, LayoutSize, Modifiers, MouseButton, Overflow, PlaybackCount, PointerEvents,
    PropertyList, ReadSignal, STATE_ACTIVED, STATE_DISABLED, STATE_DND_DRAG_IN,
    STATE_DND_DRAG_OVER, STATE_DND_DRAGGING, STATE_DRAGGED, STATE_FOCUSED, STATE_FOCUSED_VISIBLE,
    STATE_HOVERED, STATE_PRESSED, STATE_QUEUED_LAYOUT, STATE_SELECTED, STYLE_OVERFLOW,
    STYLE_PREVENT_FOCUS_STEAL, STYLE_PREVENT_FOCUS_STEAL_WITHIN, TextAlign, TransitionValue,
    UserSelect, Val, VirtualKey, WriteSignal, bind_context, handle_on_char_input, handle_on_click,
    handle_on_dnd_entity_drop, handle_on_dnd_id_drop, handle_on_file_dropped, handle_on_ime,
    handle_on_keyboard_input, handle_on_mouse_input, handle_on_right_click, with_context,
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

/// 引数の配置ルールと接頭辞。それぞれ `&mut` を先に配置。
/// 1. window (win_)
/// 2. system (sys_)
/// 3. reactive (react_)
/// 4. events (evt_)
/// 5. contents (cont_)
/// 6. topology (topo_)
/// 7. layouts (lay_)
/// 8. renders (ren_)
/// 9. outputs (out_)
pub struct Context {
    /// ウィンドウ全体の基本状態（DPI、最終境界）の保持。
    /// ウィンドウリサイズ検知、可視矩形の算出など。
    pub window: WindowStore,
    /// OS機能（DWriteレイアウトキャッシュ、IMM32位置、UIAプロパティ）および非同期STAキューの保持。
    /// IMM32候補窓の物理位置同期、DWriteレイアウト生成など。
    pub system: SystemStore,
    /// シグナル・エフェクト実体、プロバイダー依存関係の保持。
    /// プロバイダー引き当て、エフェクトの初回評価遅延処理など。
    pub reactive: ReactiveStore,
    /// ユーザー入力リスナー、およびリサイズ/ドラッグセッション状態の保持。
    /// リサイズ方向検知、オートスクロールはみ出し距離など。
    pub events: EventStore,
    /// ユーザーコンテンツの保持。
    /// キャレット点滅判定、コンテンツサイズ計測など。
    pub contents: ContentStore,
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
        let Context {
            topology,
            layouts,
            renders,
            outputs,
            contents,
            events,
            reactive,
            window,
            system,
        } = self;

        TopologyStore::despawn_internal(
            id, topology, layouts, renders, outputs, contents, events, reactive, window, system,
        );
    }

    /// 指定した要素の子要素一覧を取得します。
    #[inline]
    pub fn children_list(&self, handle: Element) -> Option<Vec<Element>> {
        self.topology
            .topo_children
            .get(handle.id)
            .map(|c| c.iter().map(|&id| Element { id }).collect())
    }

    /// 画面上でアクティブになっている要素の総数を取得します。
    #[inline]
    pub fn topo_active_entities_count(&self) -> usize {
        self.topology.topo_active_entities.len()
    }

    /// 指定された要素が現在マウスホバーされているか判定します
    #[inline]
    pub fn is_hovered(&self, id: EntityId) -> bool {
        self.topology
            .topo_active_masks
            .get(id)
            .is_some_and(|m| m.has(STATE_HOVERED))
    }

    /// 指定された要素が現在キーボードフォーカスを得ているか判定します
    #[inline]
    pub fn is_focused(&self, id: EntityId) -> bool {
        self.topology
            .topo_active_masks
            .get(id)
            .is_some_and(|m| m.has(STATE_FOCUSED))
    }

    /// 指定された要素が現在マウスやタップで押し下げられているか判定します
    #[inline]
    pub fn is_pressed(&self, id: EntityId) -> bool {
        self.topology
            .topo_active_masks
            .get(id)
            .is_some_and(|m| m.has(STATE_PRESSED))
    }

    /// 指定された要素が無効化（操作不可）状態にあるか判定します
    #[inline]
    pub fn is_disabled(&self, id: EntityId) -> bool {
        self.topology
            .topo_active_masks
            .get(id)
            .is_some_and(|m| m.has(STATE_DISABLED))
    }

    /// 指定された要素が現在アクティブ（有効選択など）状態にあるか判定します
    #[inline]
    pub fn is_actived(&self, id: EntityId) -> bool {
        self.topology
            .topo_active_masks
            .get(id)
            .is_some_and(|m| m.has(STATE_ACTIVED))
    }

    /// 指定された要素が現在テキストまたはトグル選択されているか判定します
    #[inline]
    pub fn is_selected(&self, id: EntityId) -> bool {
        self.topology
            .topo_active_masks
            .get(id)
            .is_some_and(|m| m.has(STATE_SELECTED))
    }

    /// 指定された要素が現在ドラッグ操作中にあるか判定します
    #[inline]
    pub fn is_dragged(&self, id: EntityId) -> bool {
        self.topology
            .topo_active_masks
            .get(id)
            .is_some_and(|m| m.has(STATE_DRAGGED))
    }

    /// 現在、システム内部に再描画要求（Dirtyマークされた要素）があるか判定します。
    #[inline]
    pub fn is_render_dirty(&self) -> bool {
        !self.renders.ren_dirty_entities.is_empty()
            || !self.layouts.lay_dirty_entities.is_empty()
            || self.topology.topo_is_structure_dirty
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
        let LayoutStore {
            lay_taffy_nodes,
            lay_taffy,
            lay_dirty_entities,
            ..
        } = &mut self.layouts;
        let TopologyStore {
            topo_active_masks,
            topo_parents,
            ..
        } = &mut self.topology;
        let RenderStore {
            ren_dirty_entities, ..
        } = &mut self.renders;

        LayoutStore::mark_layout_dirty(
            id,
            topo_active_masks,
            topo_parents,
            lay_taffy,
            lay_dirty_entities,
            lay_taffy_nodes,
        );
        RenderStore::mark_render_dirty(id, topo_active_masks, ren_dirty_entities);
    }

    #[inline]
    pub fn clear_layout_dirty(&mut self) {
        let TopologyStore {
            topo_active_masks, ..
        } = &mut self.topology;
        let LayoutStore {
            lay_dirty_entities, ..
        } = &mut self.layouts;

        LayoutStore::clear_layout_dirty(topo_active_masks, lay_dirty_entities);
    }

    /// 指定した要素の画面上の絶対座標（LayoutRect）を取得します。
    #[inline]
    pub fn rect(&self, id: EntityId) -> Option<LayoutRect> {
        let OutputStore { out_rects, .. } = &self.outputs;

        OutputStore::rect(id, out_rects)
    }

    /// 指定した要素の画面上のクリップ境界（LayoutRect）を取得します。
    #[inline]
    pub fn clip_rect(&self, id: EntityId) -> Option<LayoutRect> {
        let OutputStore { out_clip_rects, .. } = &self.outputs;

        OutputStore::clip_rect(id, out_clip_rects)
    }

    /// 現在フォーカスされている要素で範囲選択されている文字列を取得します。
    #[inline]
    pub fn get_selected_text(&self) -> Option<String> {
        let EventStore {
            evt_interaction_states,
            ..
        } = &self.events;
        let ContentStore {
            cont_text_contents, ..
        } = &self.contents;
        let RenderStore { ren_visual, .. } = &self.renders;
        let OutputStore {
            out_text_selections,
            ..
        } = &self.outputs;

        OutputStore::get_selected_text(
            evt_interaction_states,
            cont_text_contents,
            ren_visual,
            out_text_selections,
        )
    }

    /// 現在のスクロール位置から相対移動します。
    pub fn scroll_by(&mut self, id: EntityId, dx: f32, dy: f32) -> bool {
        let TopologyStore {
            topo_entities,
            topo_parents,
            topo_children,
            topo_active_masks,
            topo_active_entities,
            topo_session_spawned,
            topo_session_roots,
            topo_flat_dfs_sequence,
            topo_is_structure_dirty,
            ..
        } = &mut self.topology;
        let LayoutStore {
            lay_basic,
            lay_base_basic,
            lay_flex,
            lay_grid,
            lay_scrollbar_styles,
            lay_taffy_nodes,
            lay_taffy,
            lay_dirty_entities,
        } = &mut self.layouts;
        let RenderStore {
            ren_visual,
            ren_interaction,
            ren_base_visual,
            ren_dirty_entities,
            ren_active_transitions,
            ren_active_animations,
            ren_active_webviews,
            ren_last_tick_time,
        } = &mut self.renders;
        let OutputStore {
            out_rects,
            out_clip_rects,
            out_scroll_offsets,
            out_prev_rects,
            out_prev_clip_rects,
            out_selected_rects,
            out_text_selections,
            out_selection_start_index,
        } = &mut self.outputs;
        let ContentStore {
            cont_text_contents,
            cont_text_spans,
            cont_input_contents,
            cont_image_sources,
            cont_movie_properties,
            cont_webview_contents,
        } = &mut self.contents;
        let WindowStore { win_last_size, .. } = &mut self.window;
        let SystemStore {
            sys_text_engine,
            sys_dwrite_layouts,
            sys_uia_properties,
            sys_task_sender,
            sys_task_receiver,
        } = &mut self.system;
        OutputStore::scroll_by(
            id,
            dx,
            dy,
            *win_last_size,
            sys_text_engine,
            sys_dwrite_layouts,
            cont_input_contents,
            cont_text_contents,
            cont_text_spans,
            topo_active_masks,
            topo_parents,
            topo_children,
            lay_taffy,
            lay_dirty_entities,
            lay_scrollbar_styles,
            lay_taffy_nodes,
            lay_basic,
            lay_flex,
            lay_grid,
            ren_visual,
            ren_interaction,
            ren_active_transitions,
            out_rects,
            out_scroll_offsets,
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
            react_signals,
            react_subscribers,
            ..
        } = &mut self.reactive;

        ReactiveStore::create_signal(initial_value, react_signals, react_subscribers)
    }

    /// 現在のスレッドローカルコンテキストから、
    /// 親ツリーを自動的に遡って解決した型 T のシグナルに対する同期書き込み用端（WriteSignal）を取得します。
    #[inline]
    pub fn use_provided_setter<T: Send + 'static>(&self) -> WriteSignal<T> {
        let ReactiveStore {
            react_effect_to_element,
            react_providers,
            ..
        } = &self.reactive;
        let TopologyStore { topo_parents, .. } = &self.topology;

        let element_id = ReactiveStore::resolve_element_effect(react_effect_to_element)
                .unwrap_or_else(|| {
                    panic!(
                        "use_provided_setter must be called inside a dynamic reactive context or an active event handler context"
                    );
                });

        ReactiveStore::use_provided_setter_from::<T>(element_id, react_providers, topo_parents)
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
            react_providers,
            react_effect_to_element,
            ..
        } = &self.reactive;
        let TopologyStore { topo_parents, .. } = &self.topology;

        let element_id = ReactiveStore::resolve_element_effect(react_effect_to_element)
                .unwrap_or_else(|| {
                    panic!(
                        "use_provided must be called inside a dynamic style, text, content closure, or an active event handler context"
                    );
                });

        ReactiveStore::use_provided_from::<T>(element_id, react_providers, topo_parents)
                .unwrap_or_else(|| {
                    panic!(
                        "Dependency resolution failed: No Provider found in ancestor sub-tree for type: '{}'",
                        std::any::type_name::<T>()
                    )
                })
    }

    pub fn try_use_provided<T: Clone + 'static>(&self) -> Option<ReadSignal<T>> {
        let ReactiveStore {
            react_providers,
            react_effect_to_element,
            ..
        } = &self.reactive;
        let TopologyStore { topo_parents, .. } = &self.topology;

        let element_id = ReactiveStore::resolve_element_effect(react_effect_to_element)?;
        ReactiveStore::use_provided_from::<T>(element_id, react_providers, topo_parents)
    }

    /// 現在ホバーされている要素から親ツリーを遡り、適用するべき物理的な `CursorIcon` を正確に解決します。
    #[inline]
    pub fn resolve_cursor(&self, hovered_id: EntityId) -> CursorIcon {
        let RenderStore {
            ren_visual,
            ren_base_visual,
            ..
        } = &self.renders;
        let EventStore {
            evt_interaction_states,
            ..
        } = &self.events;
        let TopologyStore { topo_parents, .. } = &self.topology;

        RenderStore::resolve_cursor(
            hovered_id,
            evt_interaction_states,
            topo_parents,
            ren_visual,
            ren_base_visual,
        )
    }

    /// 描画（レンダー）ダーティ状態として登録された要素をすべてクリアします。
    #[inline]
    pub fn clear_render_dirty(&mut self) {
        let RenderStore {
            ren_dirty_entities, ..
        } = &mut self.renders;
        let TopologyStore {
            topo_active_masks, ..
        } = &mut self.topology;

        RenderStore::clear_render_dirty(topo_active_masks, ren_dirty_entities);
    }

    /// 現在、アクティブに動いているトランジション（wgpuアニメーション）があるか判定します
    #[inline]
    pub fn has_active_animations(&self) -> bool {
        let RenderStore {
            ren_active_animations,
            ren_active_transitions,
            ren_visual,
            ..
        } = &self.renders;
        let EventStore {
            evt_interaction_states,
            evt_current_pointer_position,
            ..
        } = &self.events;
        let OutputStore { out_clip_rects, .. } = &self.outputs;
        let LayoutStore {
            lay_scrollbar_styles,
            ..
        } = &self.layouts;
        let ContentStore {
            cont_input_contents,
            ..
        } = &self.contents;

        RenderStore::has_active_animations(
            evt_interaction_states,
            evt_current_pointer_position.as_ref(),
            cont_input_contents,
            lay_scrollbar_styles,
            ren_visual,
            ren_active_transitions,
            ren_active_animations,
            out_clip_rects,
        )
    }

    /// 毎フレームの描画前に呼び出され、すべてのアクティブなキーフレームアニメーションを 1 Tick 進めます
    pub fn tick_animations(&mut self) {
        let RenderStore {
            ren_active_animations,
            ren_visual,
            ren_dirty_entities,
            ..
        } = &mut self.renders;
        let TopologyStore {
            topo_active_masks,
            topo_parents,
            ..
        } = &mut self.topology;
        let LayoutStore {
            lay_basic,
            lay_taffy,
            lay_taffy_nodes,
            lay_dirty_entities,
            ..
        } = &mut self.layouts;

        RenderStore::tick_animations(
            topo_active_masks,
            topo_parents,
            lay_taffy,
            lay_basic,
            lay_dirty_entities,
            lay_taffy_nodes,
            ren_visual,
            ren_dirty_entities,
            ren_active_animations,
        );
    }

    /// 毎フレームの描画前に呼び出され、すべてのアクティブなトランジションを 1 Tick 進めます
    pub fn tick_transitions(&mut self) {
        let RenderStore {
            ren_visual,
            ren_dirty_entities,
            ren_last_tick_time,
            ren_active_transitions,
            ..
        } = &mut self.renders;
        let TopologyStore {
            topo_active_masks,
            topo_parents,
            ..
        } = &mut self.topology;
        let LayoutStore {
            lay_basic,
            lay_taffy,
            lay_taffy_nodes,
            lay_dirty_entities,
            ..
        } = &mut self.layouts;

        RenderStore::tick_transitions(
            topo_active_masks,
            topo_parents,
            lay_taffy,
            lay_basic,
            lay_dirty_entities,
            lay_taffy_nodes,
            ren_visual,
            ren_dirty_entities,
            ren_active_transitions,
            ren_last_tick_time,
        );
    }

    /// ワーカースレッドなど、どこからでも安全にクローンしてタスクを送信できるスレッドセーフな送信端を取得します。
    #[inline]
    pub fn sys_task_sender(&self) -> TaskSender {
        self.system.sys_task_sender.clone()
    }

    /// ウィンドウ生成後に起床用コールバックを登録します。
    #[inline]
    pub fn set_waker<F>(&mut self, f: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.system.sys_task_sender.waker = Some(Arc::new(f));
    }

    /// メインスレッドの毎フレーム開始時（またはイベントハンドラの先頭など）に呼び出され、
    /// バックグラウンドから届いたシグナル更新タスクなどの処理を安全に一括実行します。
    #[inline]
    pub fn process_main_thread_tasks(&mut self) {
        let _context_guard = bind_context(self);
        // キューに溜まっているクロージャをすべてメインスレッドのコンテキスト上で実行
        while let Ok(task) = self.system.sys_task_receiver.try_recv() {
            task(self);
        }
    }

    #[inline]
    pub fn entity_id_focused(&self) -> Option<EntityId> {
        self.events.evt_interaction_states.focused
    }

    #[inline]
    pub fn entity_id_dragged(&self) -> Option<EntityId> {
        self.events.evt_interaction_states.dragged
    }

    #[inline]
    pub fn entity_id_hovered(&self) -> Option<EntityId> {
        self.events.evt_interaction_states.hovered
    }

    #[inline]
    pub fn entity_id_pressed(&self) -> Option<EntityId> {
        self.events.evt_interaction_states.pressed
    }

    /// 指定された要素をプログラム駆動でクリックさせます
    #[inline]
    pub fn trigger_element_click(&mut self, id: EntityId) {
        if !self.topology.topo_entities.contains_key(id) || self.is_disabled(id) {
            return;
        }
        handle_on_click(self, id);
    }

    /// 現在のテキスト・IME状態・フォントサイズから、
    /// キャレットの物理座標や最終表示テキスト、レイアウト矩形を正確に再計算して `SoA` を更新。
    pub fn update_input_caret_position(&mut self, id: EntityId) {
        let TopologyStore {
            topo_active_masks,
            topo_parents,
            topo_children,
            ..
        } = &mut self.topology;
        let LayoutStore {
            lay_basic,
            lay_flex,
            lay_grid,
            lay_dirty_entities,
            lay_taffy,
            lay_taffy_nodes,
            lay_scrollbar_styles,
            ..
        } = &mut self.layouts;
        let RenderStore {
            ren_visual,
            ren_base_visual,
            ren_interaction,
            ren_active_transitions,
            ..
        } = &mut self.renders;
        let OutputStore {
            out_rects,
            out_scroll_offsets,
            out_text_selections,
            ..
        } = &mut self.outputs;
        let ContentStore {
            cont_text_contents,
            cont_input_contents,
            cont_text_spans,
            ..
        } = &mut self.contents;
        let SystemStore {
            sys_text_engine,
            sys_dwrite_layouts,
            ..
        } = &mut self.system;
        let WindowStore {
            win_last_size,
            win_scale_factor,
            ..
        } = &mut self.window;

        OutputStore::update_input_caret_position(
            id,
            *win_last_size,
            *win_scale_factor,
            sys_text_engine,
            sys_dwrite_layouts,
            cont_input_contents,
            cont_text_contents,
            cont_text_spans,
            topo_active_masks,
            topo_parents,
            topo_children,
            lay_taffy,
            lay_dirty_entities,
            lay_scrollbar_styles,
            lay_taffy_nodes,
            lay_basic,
            lay_flex,
            lay_grid,
            ren_visual,
            ren_base_visual,
            ren_interaction,
            ren_active_transitions,
            out_scroll_offsets,
            out_text_selections,
            out_rects,
        );
    }

    /// 毎フレーム呼び出され、ドラッグ選択中の要素に対するオートスクロールを自律駆動します。
    /// ウィンドウメッセージループ等、 `tick_transitions()` を呼び出している箇所と同じ周期で実行する。
    #[inline]
    pub fn tick_drag_autoscroll(&mut self) {
        let TopologyStore {
            topo_active_masks,
            topo_parents,
            topo_children,
            ..
        } = &mut self.topology;
        let LayoutStore {
            lay_basic,
            lay_base_basic,
            lay_flex,
            lay_grid,
            lay_scrollbar_styles,
            lay_dirty_entities,
            lay_taffy,
            lay_taffy_nodes,
            ..
        } = &mut self.layouts;
        let RenderStore {
            ren_visual,
            ren_interaction,
            ren_active_transitions,
            ren_dirty_entities,
            ..
        } = &mut self.renders;
        let ContentStore {
            cont_input_contents,
            cont_text_contents,
            cont_text_spans,
            ..
        } = &mut self.contents;
        let OutputStore {
            out_rects,
            out_scroll_offsets,
            out_clip_rects,
            ..
        } = &mut self.outputs;
        let EventStore {
            evt_current_pointer_position,
            evt_interaction_states,
            ..
        } = &mut self.events;
        let SystemStore {
            sys_text_engine,
            sys_dwrite_layouts,
            ..
        } = &mut self.system;
        let WindowStore { win_last_size, .. } = &mut self.window;

        let Some(id) = evt_interaction_states.pressed else {
            return;
        };
        let (autoscroll_occurred, active_pos) = EventStore::autoscroll_occurred(
            id,
            *win_last_size,
            sys_dwrite_layouts,
            sys_text_engine,
            *evt_current_pointer_position,
            cont_input_contents,
            cont_text_spans,
            cont_text_contents,
            topo_active_masks,
            topo_parents,
            topo_children,
            lay_taffy,
            lay_dirty_entities,
            lay_scrollbar_styles,
            lay_taffy_nodes,
            lay_basic,
            lay_flex,
            lay_grid,
            ren_visual,
            ren_active_transitions,
            ren_interaction,
            out_scroll_offsets,
            out_rects,
            out_clip_rects,
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

    #[inline]
    pub fn inject_pointer_move(&mut self, logical_pos: LayoutPoint) {
        EventStore::pointer_move_inner(self, logical_pos);
    }

    pub fn inject_pointer_button(
        &mut self,
        button: MouseButton,
        state: ElementState,
        modifiers: Modifiers,
    ) {
        EventStore::pointer_button_inner(self, button, state, modifiers);
    }

    // ダブルクリック
    pub fn inject_pointer_double_click(&mut self, modifiers: Modifiers) {
        let _context_guard = bind_context(self);
        let current_hovered = self.events.evt_interaction_states.hovered;

        if let Some(target_id) = current_hovered {
            let user_select = self.get_user_select(target_id);

            if user_select == UserSelect::Text
                && let Some(pointer_pos) = self.events.evt_current_pointer_position
            {
                if let Some(contents) = self.contents.cont_input_contents.get(target_id) {
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

                if let Some(dw_layout) = self.get_or_create_layout(target_id) {
                    let (clicked_index, is_trailing) = self
                        .system
                        .sys_text_engine
                        .hit_test_point(&dw_layout, local_x, local_y);
                    let final_index = if is_trailing {
                        clicked_index + 1
                    } else {
                        clicked_index
                    };

                    if let Some(text) = self.contents.cont_text_contents.get(target_id) {
                        let text_u16: Vec<u16> = text.encode_utf16().collect();

                        // 高精度な文節境界を抽出
                        let range = crate::find_word_boundaries(&text_u16, final_index);

                        self.outputs
                            .out_text_selections
                            .insert(target_id, range.clone());
                        // アンカー開始を文節左端にセット
                        self.outputs
                            .out_selection_start_index
                            .insert(target_id, range.start);
                        self.update_selection_rects(target_id, &dw_layout); // 選択矩形を更新

                        if let Some(contents) = self.contents.cont_input_contents.get_mut(target_id)
                        {
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

        let mut curr = self.events.evt_interaction_states.hovered;
        let mut handled = false;

        // イベントバブリング: ホバー要素から親へ辿る
        while let Some(curr_id) = curr {
            // 個別に定義された `on_mouse_wheel` ハンドラがあれば最優先実行
            if let Some(l) = self.events.evt_listeners.get_mut(curr_id)
                && let Some(mut handler) = l.on_mouse_wheel.take()
            {
                let _guard = crate::ActiveElementGuard::new(curr_id);
                handler(self, scroll_x, scroll_y);
                if let Some(l) = self.events.evt_listeners.get_mut(curr_id) {
                    l.on_mouse_wheel = Some(handler);
                }
                handled = true; // イベントが消費されたため、これ以降のコンテナスクロールは行わない
                break;
            }

            // ユーザーハンドラがない場合、要素がスクロールコンテナであるか判定
            let mask = self.topology.topo_active_masks[curr_id];
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
            curr = self.topology.topo_parents.get(curr_id).copied().flatten();
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

        if let Some(focused_id) = self.events.evt_interaction_states.focused {
            // フォーカス中に Enter または Space が押されたら自動的にクリックをエミュレートする
            if state == ElementState::Pressed
                && (key == VirtualKey::RETURN || key == VirtualKey::SPACE)
                && !self.topology.topo_active_masks[focused_id].has_input_content()
            {
                handle_on_click(self, focused_id);
                return;
            }
            // 内部で完結する全選択（Ctrl+A）のみを自動処理
            if state == ElementState::Pressed && modifiers.ctrl {
                let user_select = self.get_user_select(focused_id);

                if key == VirtualKey::A && user_select == UserSelect::Text {
                    if let Some(dw_layout) = self.get_or_create_layout(focused_id)
                        && let Some(text) = self.contents.cont_text_contents.get(focused_id)
                    {
                        let u16_len = text.encode_utf16().count();
                        let full_range = 0..u16_len;

                        self.outputs
                            .out_text_selections
                            .insert(focused_id, full_range.clone());

                        self.update_selection_rects(focused_id, &dw_layout);

                        if let Some(contents) =
                            self.contents.cont_input_contents.get_mut(focused_id)
                        {
                            contents.selected_range = full_range;
                            contents.selection_reversed = false;
                            self.update_input_caret_position(focused_id);
                        }
                        self.mark_render_dirty(focused_id);
                    }
                    return;
                }
            }
            handle_on_keyboard_input(self, focused_id, key, modifiers, state);
        }
    }

    /// キーボードフォーカスを次の適格な要素へ巡回させます
    pub fn cycle_keyboard_focus(&mut self, reverse: bool) {
        if self.topology.topo_flat_dfs_sequence.is_empty() {
            return;
        }

        let len = self.topology.topo_flat_dfs_sequence.len();

        // 現在フォーカスされている要素のインデックスを特定（無ければ探索方向の末端から開始）
        let current_focused = self.events.evt_interaction_states.focused;
        let start_idx = current_focused
            .and_then(|id| {
                self.topology
                    .topo_flat_dfs_sequence
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

            let candidate_id = self.topology.topo_flat_dfs_sequence[idx];

            if self.is_keyboard_focusable(candidate_id) {
                // 古い要素のフォーカスを外し、新しい要素へフォーカスを設定
                if let Some(old_id) = self.events.evt_interaction_states.focused {
                    self.set_focused_by_trigger(old_id, false, ActiveFocusTrigger::Keyboard);
                }
                self.set_focused_by_trigger(candidate_id, true, ActiveFocusTrigger::Keyboard);
                self.events.evt_interaction_states.focused = Some(candidate_id);

                // WebView2 要素だった場合はシステム側にフォーカスをプログラム駆動で移譲
                if self.topology.topo_active_masks[candidate_id].has_webveiw2_content() {
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
        let Some(focused_id) = self.events.evt_interaction_states.focused else {
            return;
        };
        handle_on_char_input(self, focused_id, c);
    }

    #[inline]
    pub fn inject_ime(&mut self, ime_state: ImeState) {
        let _context_guard = bind_context(self);
        let Some(focused_id) = self.events.evt_interaction_states.focused else {
            return;
        };
        handle_on_ime(self, focused_id, ime_state);
    }

    #[inline]
    pub fn inject_file_dropped(&mut self, paths: Vec<PathBuf>) {
        let _context_guard = bind_context(self);
        let Some(target_id) = self.events.evt_interaction_states.hovered else {
            return;
        };
        handle_on_file_dropped(self, target_id, paths);
    }

    /// 外部から提供されたテキストを、現在フォーカスされている入力要素にペーストします。
    #[inline]
    pub fn inject_paste(&mut self, text: &str) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.events.evt_interaction_states.focused
            && self.topology.topo_active_masks[focused_id].has_input_content()
            && let Some(contents) = self.contents.cont_input_contents.get_mut(focused_id)
        {
            OutputStore::inject_paste_internal(
                focused_id,
                text,
                contents,
                &mut self.outputs.out_text_selections,
                &mut self.outputs.out_selected_rects,
            );

            self.update_input_caret_position(focused_id);
            self.mark_render_dirty(focused_id);
        }
    }

    /// Undo (元に戻す) のインジェクション
    #[inline]
    pub fn inject_undo(&mut self) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.events.evt_interaction_states.focused
            && self.topology.topo_active_masks[focused_id].has_input_content()
            && let Some(contents) = self.contents.cont_input_contents.get_mut(focused_id)
            && let Some((prev_text, prev_sel)) = contents.undo_stack.pop()
        {
            OutputStore::inject_undo_internal(
                focused_id,
                prev_sel,
                prev_text,
                contents,
                &mut self.outputs.out_text_selections,
                &mut self.outputs.out_selected_rects,
            );

            self.update_input_caret_position(focused_id);
            self.mark_render_dirty(focused_id);
        }
    }

    /// Redo (やり直し) のインジェクション
    #[inline]
    pub fn inject_redo(&mut self) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.events.evt_interaction_states.focused
            && self.topology.topo_active_masks[focused_id].has_input_content()
            && let Some(contents) = self.contents.cont_input_contents.get_mut(focused_id)
            && let Some((next_text, next_sel)) = contents.redo_stack.pop()
        {
            OutputStore::inject_redo_internal(
                focused_id,
                next_sel,
                next_text,
                contents,
                &mut self.outputs.out_text_selections,
                &mut self.outputs.out_selected_rects,
            );

            self.update_input_caret_position(focused_id);
            self.mark_render_dirty(focused_id);
        }
    }

    /// 切り取り (Ctrl+X) の実行と削除後のテキスト取得
    #[inline]
    pub fn inject_cut(&mut self) -> Option<String> {
        let _context_guard = bind_context(self);
        let focused_id = self.events.evt_interaction_states.focused?;
        let user_select = self.get_user_select(focused_id);

        if user_select == UserSelect::Text
            && let Some(range) = self.outputs.out_text_selections.get(focused_id).cloned()
            && range.start < range.end
            && let Some(text) = self.contents.cont_text_contents.get(focused_id)
        {
            let u16_text: Vec<u16> = text.encode_utf16().collect();
            let slice = &u16_text[range.start.min(u16_text.len())..range.end.min(u16_text.len())];
            let cut_text = String::from_utf16(slice).ok()?;

            // 対象が Input コントロールである場合のみ、切り取り削除上書きを実行
            if self.topology.topo_active_masks[focused_id].has_input_content()
                && let Some(contents) = self.contents.cont_input_contents.get_mut(focused_id)
            {
                OutputStore::inject_cut_internal(
                    focused_id,
                    range,
                    contents,
                    &mut self.outputs.out_text_selections,
                    &mut self.outputs.out_selected_rects,
                );

                self.update_input_caret_position(focused_id);
                self.mark_render_dirty(focused_id);
            }

            return Some(cut_text);
        }

        None
    }

    /// マウス座標などが、要素の描画領域かつ表示枠内に収まっているかを判定。
    /// 階層的な早期枝刈りヒットテスト
    pub fn hit_test(&mut self, point: LayoutPoint) -> Option<EntityId> {
        let TopologyStore {
            topo_active_entities,
            topo_active_masks,
            topo_parents,
            topo_flat_dfs_sequence,
            topo_effective_z_indices,
            topo_sorted_entities,
            ..
        } = &mut self.topology;
        let RenderStore {
            ren_visual,
            ren_base_visual,
            ..
        } = &self.renders;
        let OutputStore {
            out_rects,
            out_clip_rects,
            ..
        } = &self.outputs;
        let EventStore {
            evt_interaction_states,
            ..
        } = &self.events;

        TopologyStore::hit_test(
            point,
            topo_active_entities,
            topo_active_masks,
            topo_flat_dfs_sequence,
            topo_parents,
            topo_effective_z_indices,
            topo_sorted_entities,
            ren_visual,
            ren_base_visual,
            evt_interaction_states,
            out_rects,
            out_clip_rects,
        )
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
        if self.layouts.lay_dirty_entities.is_empty()
            && !self.topology.topo_is_structure_dirty
            && !window_resized
            && !self.outputs.out_rects.is_empty()
        {
            return;
        }

        if self.topology.topo_is_structure_dirty {
            self.rebuild_topo_flat_dfs_sequence(root);
        }

        // 全スクロールバー関連IDを一括抽出
        let scrollbar_el_ids = self.scrollbar_el_ids();

        // 1. Taffy永続ツリーへの差分同期
        for id in &self.layouts.lay_dirty_entities {
            // スクロールバー専用要素は手動で物理座標を同期させるため、Taffyへの登録更新を完全にバイパス
            if scrollbar_el_ids.contains(id) {
                continue;
            }

            let (mut basic, flex, grid) = self.resolve_active_layouts(*id);

            // もしこの要素が現在アニメーション中（ren_active_transitions に存在）であれば、
            // resolve_active_layouts が強制マージした目標値を拒否し、
            // tick_transitions が毎フレーム更新している現在値に上書きし直して Taffy に送信。
            if let Some(active_list) = self.renders.ren_active_transitions.get(*id) {
                for t_state in active_list {
                    match t_state.property_list {
                        PropertyList::Width => {
                            if let Some(layout) = self.layouts.lay_basic.get(*id) {
                                basic.size.width = layout.size.width;
                            }
                        }
                        PropertyList::Height => {
                            if let Some(layout) = self.layouts.lay_basic.get(*id) {
                                basic.size.height = layout.size.height;
                            }
                        }
                        _ => {}
                    }
                }
            }

            let t_style = self.resolve_taffy_style(*id, &basic, &flex, grid.as_ref());
            let t_node = self.layouts.lay_taffy_nodes[*id];

            self.layouts.lay_taffy.set_style(t_node, t_style).unwrap();
        }

        // 2. Taffy のレイアウト再計算
        if let Some(&root_node) = self.layouts.lay_taffy_nodes.get(root) {
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

            let _ = self.layouts.lay_taffy.compute_layout_with_measure(
                root_node,
                taffy::Size {
                    width: taffy::AvailableSpace::Definite(window_size.width),
                    height: taffy::AvailableSpace::Definite(window_size.height),
                },
                measure_func,
            );
        }

        self.topology.topo_active_entities.clear();

        // scroll_size を正しく算出するため、スワップおよび一旦コンテンツの out_rects のみを確定
        self.swap_output_rect();

        let flat_len = self.topology.topo_flat_dfs_sequence.len();

        // 1次元非再帰・静的キャッシュバイパスループ
        for i in 0..flat_len {
            let id = self.topology.topo_flat_dfs_sequence[i];

            // スクロールバー専用子要素は手動で物理座標を強制更新するため、この走査ループから完全にスルー
            if scrollbar_el_ids.contains(&id) {
                continue;
            }

            // 親の移動・リサイズ状態を検証
            let parent_changed = self.parent_changed(id);

            // 静的キャッシュバイパス判定
            let has_style_changed = self.topology.topo_active_masks[id].has(STATE_QUEUED_LAYOUT);

            if !window_resized
                && !has_style_changed
                && !parent_changed
                && self.outputs.out_prev_rects.contains_key(id)
            {
                // 自分自身のスタイルが変わっておらず、親も動いていない、かつモニターリサイズもされていないならキャッシュ利用
                let cached_rect = self.outputs.out_prev_rects[id];
                self.outputs.out_rects.insert(id, cached_rect);

                // クリップも同様にキャッシュ再利用
                let cached_clip = self.outputs.out_prev_clip_rects[id];
                self.outputs.out_clip_rects.insert(id, cached_clip);

                self.topology.topo_active_entities.push(id);
                continue;
            }

            let (abs_rect, parent_clip) = self.calc_local_rect(id, window_size);

            self.outputs.out_rects.insert(id, abs_rect);
            let mask = self.topology.topo_active_masks[id];

            if mask.has_input_content()
                && let Some(contents) = self.contents.cont_input_contents.get_mut(id)
            {
                contents.last_bounds = Some(abs_rect);
            }

            let current_clip = if mask.has(STYLE_OVERFLOW) {
                parent_clip.intersect(&abs_rect)
            } else {
                parent_clip
            };
            self.outputs.out_clip_rects.insert(id, current_clip);

            // 常に1次元DFS順でアクティブ要素リストに登録する
            self.topology.topo_active_entities.push(id);
        }

        // スクロールバー要素（Track & Thumb）のサイズ・配置・不透明度を一括同期更新
        self.sync_scrollbar_styles();

        // スクロールバー専用要素のサイズ・位置が確定したため、
        // 差分計算を走らせてマージンやパディングを考慮した物理位置を Taffy 内部で正確に解決
        if let Some(&root_node) = self.layouts.lay_taffy_nodes.get(root) {
            let _ = self.layouts.lay_taffy.compute_layout_with_measure(
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
                            if cx.topology.topo_active_masks[id].has_input_content()
                                && let Some(contents) = cx.contents.cont_input_contents.get(id)
                                && let Some(layout_rect) = contents.last_layout
                            {
                                return taffy::Size {
                                    width: known_dims.width.unwrap_or(layout_rect.width),
                                    height: known_dims.height.unwrap_or(layout_rect.height),
                                };
                            }

                            // 2回目パスはキャッシュサイズを即時引き出して高速マッピング
                            if let Some(&rect) = cx.outputs.out_rects.get(id) {
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

        // スクロールバー要素も含めて、Taffy から最終確定位置をすべて引き出して out_rects にマウント
        self.topology.topo_active_entities.clear();

        for i in 0..flat_len {
            let id = self.topology.topo_flat_dfs_sequence[i];

            let (abs_rect, parent_clip) = self.calc_local_rect(id, window_size);

            self.outputs.out_rects.insert(id, abs_rect);
            let mask = self.topology.topo_active_masks[id];

            if mask.has_input_content()
                && let Some(contents) = self.contents.cont_input_contents.get_mut(id)
            {
                contents.last_bounds = Some(abs_rect);
            }

            let current_clip = if mask.has(STYLE_OVERFLOW) {
                parent_clip.intersect(&abs_rect)
            } else {
                parent_clip
            };
            self.outputs.out_clip_rects.insert(id, current_clip);

            self.topology.topo_active_entities.push(id);
        }

        // 全アクティブコンテナのスクロールオフセット自動クランプ同期
        for i in 0..flat_len {
            let id = self.topology.topo_flat_dfs_sequence[i];
            if self.outputs.out_scroll_offsets.contains_key(id) {
                let current = self.outputs.out_scroll_offsets[id];
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
