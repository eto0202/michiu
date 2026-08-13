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
    ActiveFocusTrigger, ComponentMask, CursorIcon, DndDragPayload, Element, ElementState, ImeState,
    LayoutPoint, LayoutRect, LayoutSize, Modifiers, MouseButton, Overflow, PlaybackCount,
    PointerEvents, PropertyList, ReadSignal, STATE_ACTIVED, STATE_DISABLED, STATE_DND_DRAG_IN,
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
    #[inline]
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
        TopologyStore::despawn_internal(
            id,
            &mut self.window,
            &mut self.system,
            &mut self.reactive,
            &mut self.events,
            &mut self.contents,
            &mut self.topology,
            &mut self.layouts,
            &mut self.renders,
            &mut self.outputs,
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
        !self.renders.rnd_dirty_entities.is_empty()
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
        LayoutStore::mark_layout_dirty(
            id,
            &mut self.topology.topo_active_masks,
            &self.topology.topo_parents,
            &mut self.layouts.lay_taffy,
            &mut self.layouts.lay_dirty_entities,
            &self.layouts.lay_taffy_nodes,
        );
        RenderStore::mark_render_dirty(
            id,
            &mut self.topology.topo_active_masks,
            &mut self.renders.rnd_dirty_entities,
        );
    }

    #[inline]
    pub fn clear_layout_dirty(&mut self) {
        LayoutStore::clear_layout_dirty(
            &mut self.topology.topo_active_masks,
            &mut self.layouts.lay_dirty_entities,
        );
    }

    /// 指定した要素の画面上の絶対座標（LayoutRect）を取得します。
    #[inline]
    pub fn rect(&self, id: EntityId) -> Option<LayoutRect> {
        OutputStore::rect(id, &self.outputs.out_rects)
    }

    /// 指定した要素の画面上のクリップ境界（LayoutRect）を取得します。
    #[inline]
    pub fn clip_rect(&self, id: EntityId) -> Option<LayoutRect> {
        OutputStore::clip_rect(id, &self.outputs.out_clip_rects)
    }

    /// 現在フォーカスされている要素で範囲選択されている文字列を取得します。
    #[inline]
    pub fn get_selected_text(&self) -> Option<String> {
        OutputStore::get_selected_text(
            &self.events.evt_interaction_states,
            &self.contents.cont_text_contents,
            &self.renders.rnd_visual,
            &self.outputs.out_text_selections,
        )
    }

    /// 現在のスクロール位置から相対移動します。
    #[inline]
    pub fn scroll_by(&mut self, id: EntityId, dx: f32, dy: f32) -> bool {
        OutputStore::scroll_by(
            id,
            dx,
            dy,
            self.window.win_last_size,
            &self.system.sys_text_engine,
            &self.system.sys_dwrite_layouts,
            &self.contents.cont_input_contents,
            &self.contents.cont_text_contents,
            &self.contents.cont_text_spans,
            &mut self.topology.topo_active_masks,
            &self.topology.topo_parents,
            &self.topology.topo_children,
            &mut self.layouts.lay_taffy,
            &mut self.layouts.lay_dirty_entities,
            &mut self.layouts.lay_scrollbar_styles,
            &self.layouts.lay_taffy_nodes,
            &self.layouts.lay_basic,
            &self.layouts.lay_flex,
            &self.layouts.lay_grid,
            &self.renders.rnd_visual,
            &self.renders.rnd_interaction,
            &self.renders.rnd_active_transitions,
            &mut self.outputs.out_scroll_offsets,
            &self.outputs.out_rects,
        )
    }

    /// Context インスタンスから直接シグナルを生成します。
    /// これにより `build_ui` の外側（メインスレッド上）でもシグナルを定義できます。
    #[inline]
    pub fn create_signal<T: Send + 'static>(
        &mut self,
        initial_value: T,
    ) -> (ReadSignal<T>, WriteSignal<T>) {
        ReactiveStore::create_signal(
            initial_value,
            &mut self.reactive.react_signals,
            &mut self.reactive.react_subscribers,
        )
    }

    /// 現在のスレッドローカルコンテキストから、
    /// 親ツリーを自動的に遡って解決した型 T のシグナルに対する同期書き込み用端（WriteSignal）を取得します。
    #[inline]
    pub fn use_provided_setter<T: Send + 'static>(&self) -> WriteSignal<T> {
        let element_id = ReactiveStore::resolve_element_effect(&self.reactive.react_effect_to_element)
                .unwrap_or_else(|| {
                    panic!(
                        "use_provided_setter must be called inside a dynamic reactive context or an active event handler context"
                    );
                });

        ReactiveStore::use_provided_setter_from::<T>(element_id, &self.reactive.react_providers, &self.topology.topo_parents)
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
        let element_id = ReactiveStore::resolve_element_effect(&self.reactive.react_effect_to_element)
                .unwrap_or_else(|| {
                    panic!(
                        "use_provided must be called inside a dynamic style, text, content closure, or an active event handler context"
                    );
                });

        ReactiveStore::use_provided_from::<T>(element_id, &self.reactive.react_providers, &self.topology.topo_parents)
                .unwrap_or_else(|| {
                    panic!(
                        "Dependency resolution failed: No Provider found in ancestor sub-tree for type: '{}'",
                        std::any::type_name::<T>()
                    )
                })
    }

    pub fn try_use_provided<T: Clone + 'static>(&self) -> Option<ReadSignal<T>> {
        let element_id =
            ReactiveStore::resolve_element_effect(&self.reactive.react_effect_to_element)?;
        ReactiveStore::use_provided_from::<T>(
            element_id,
            &self.reactive.react_providers,
            &self.topology.topo_parents,
        )
    }

    /// 現在ホバーされている要素から親ツリーを遡り、適用するべき物理的な `CursorIcon` を正確に解決します。
    #[inline]
    pub fn resolve_cursor(&self, hovered_id: EntityId) -> CursorIcon {
        RenderStore::resolve_cursor(
            hovered_id,
            &self.events.evt_interaction_states,
            &self.topology.topo_parents,
            &self.renders.rnd_visual,
            &self.renders.rnd_base_visual,
        )
    }

    /// 描画（レンダー）ダーティ状態として登録された要素をすべてクリアします。
    #[inline]
    pub fn clear_render_dirty(&mut self) {
        RenderStore::clear_render_dirty(
            &mut self.topology.topo_active_masks,
            &mut self.renders.rnd_dirty_entities,
        );
    }

    /// 現在、アクティブに動いているトランジション,アニメーションがあるか判定します
    #[inline]
    pub fn has_active_animations(&self) -> bool {
        RenderStore::has_active_animations(
            &self.events.evt_interaction_states,
            self.events.evt_current_pointer_position.as_ref(),
            &self.contents.cont_input_contents,
            &self.layouts.lay_scrollbar_styles,
            &self.renders.rnd_visual,
            &self.renders.rnd_active_transitions,
            &self.renders.rnd_active_animations,
            &self.outputs.out_clip_rects,
        )
    }

    /// 毎フレームの描画前に呼び出され、すべてのアクティブなキーフレームアニメーションを 1 Tick 進めます
    pub fn tick_animations(&mut self) {
        RenderStore::tick_animations(
            &mut self.topology.topo_active_masks,
            &self.topology.topo_parents,
            &mut self.layouts.lay_taffy,
            &mut self.layouts.lay_basic,
            &mut self.layouts.lay_dirty_entities,
            &self.layouts.lay_taffy_nodes,
            &mut self.renders.rnd_visual,
            &mut self.renders.rnd_dirty_entities,
            &mut self.renders.rnd_active_animations,
        );
    }

    /// 毎フレームの描画前に呼び出され、すべてのアクティブなトランジションを 1 Tick 進めます
    pub fn tick_transitions(&mut self) {
        RenderStore::tick_transitions(
            &mut self.topology.topo_active_masks,
            &self.topology.topo_parents,
            &mut self.layouts.lay_taffy,
            &mut self.layouts.lay_basic,
            &mut self.layouts.lay_dirty_entities,
            &self.layouts.lay_taffy_nodes,
            &mut self.renders.rnd_visual,
            &mut self.renders.rnd_dirty_entities,
            &mut self.renders.rnd_active_transitions,
            &mut self.renders.rnd_last_tick_time,
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

    #[inline]
    pub fn auto_focus_switch_by_trigger(&mut self, id: EntityId, trigger: ActiveFocusTrigger) {
        EventStore::auto_focus_switch_by_trigger(self, id, trigger);
    }

    /// 現在のテキスト・IME状態・フォントサイズから、
    /// キャレットの物理座標や最終表示テキスト、レイアウト矩形を正確に再計算して `SoA` を更新。
    #[inline]
    pub fn update_input_caret_position(&mut self, id: EntityId) {
        OutputStore::update_input_caret_position(
            id,
            self.window.win_last_size,
            self.window.win_scale_factor,
            &self.system.sys_text_engine,
            &self.system.sys_dwrite_layouts,
            &mut self.contents.cont_input_contents,
            &mut self.contents.cont_text_contents,
            &self.contents.cont_text_spans,
            &mut self.topology.topo_active_masks,
            &self.topology.topo_parents,
            &self.topology.topo_children,
            &mut self.layouts.lay_taffy,
            &mut self.layouts.lay_dirty_entities,
            &mut self.layouts.lay_scrollbar_styles,
            &self.layouts.lay_taffy_nodes,
            &self.layouts.lay_basic,
            &self.layouts.lay_flex,
            &self.layouts.lay_grid,
            &mut self.renders.rnd_visual,
            &self.renders.rnd_base_visual,
            &self.renders.rnd_interaction,
            &self.renders.rnd_active_transitions,
            &mut self.outputs.out_scroll_offsets,
            &mut self.outputs.out_text_selections,
            &self.outputs.out_rects,
        );
    }

    /// 毎フレーム呼び出され、ドラッグ選択中の要素に対するオートスクロールを自律駆動します。
    /// ウィンドウメッセージループ等、 `tick_transitions()` を呼び出している箇所と同じ周期で実行する。
    #[inline]
    pub fn tick_drag_autoscroll(&mut self) {
        let Some(id) = self.events.evt_interaction_states.pressed else {
            return;
        };
        let (autoscroll_occurred, active_pos) = EventStore::autoscroll_occurred(
            id,
            self.window.win_last_size,
            &self.system.sys_dwrite_layouts,
            &self.system.sys_text_engine,
            self.events.evt_current_pointer_position,
            &self.contents.cont_input_contents,
            &self.contents.cont_text_spans,
            &self.contents.cont_text_contents,
            &mut self.topology.topo_active_masks,
            &self.topology.topo_parents,
            &self.topology.topo_children,
            &mut self.layouts.lay_taffy,
            &mut self.layouts.lay_dirty_entities,
            &mut self.layouts.lay_scrollbar_styles,
            &self.layouts.lay_taffy_nodes,
            &self.layouts.lay_basic,
            &self.layouts.lay_flex,
            &self.layouts.lay_grid,
            &self.renders.rnd_visual,
            &self.renders.rnd_active_transitions,
            &self.renders.rnd_interaction,
            &mut self.outputs.out_scroll_offsets,
            &self.outputs.out_rects,
            &self.outputs.out_clip_rects,
        );

        if autoscroll_occurred && let Some(pos) = active_pos {
            // スクロールによりテキストが流れたため、
            // 現在のポインタ座標で仮想的にポインタ移動を再トリガーし、
            // 選択文字インデックスおよびキャレット位置を同期
            EventStore::inject_pointer_move_internal(self, pos);
            RenderStore::mark_render_dirty(
                id,
                &mut self.topology.topo_active_masks,
                &mut self.renders.rnd_dirty_entities,
            );
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
        EventStore::set_focused_by_trigger(self, id, focused, trigger);
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
        EventStore::inject_pointer_move_internal(self, logical_pos);
    }

    #[inline]
    pub fn inject_pointer_button(
        &mut self,
        button: MouseButton,
        state: ElementState,
        modifiers: Modifiers,
    ) {
        EventStore::inject_pointer_button_internal(self, button, state, modifiers);
    }

    #[inline]
    pub fn inject_pointer_double_click(&mut self, modifiers: Modifiers) {
        EventStore::inject_pointer_double_click_internal(self, modifiers);
    }

    /// 外部で計算された論理ピクセルスクロール移動量 (`scroll_x`, `scroll_y`) を注入し、
    /// バブリングによる自動スクロール処理、またはユーザーイベントハンドラへの配送を行います。
    #[inline]
    pub fn inject_mouse_wheel(&mut self, scroll_x: f32, scroll_y: f32) {
        EventStore::inject_mouse_wheel_internal(self, scroll_x, scroll_y);
    }

    #[inline]
    pub fn inject_keyboard_key(
        &mut self,
        key: VirtualKey,
        state: ElementState,
        modifiers: Modifiers,
    ) {
        EventStore::inject_keyboard_key_internal(self, key, state, modifiers);
    }

    /// キーボードフォーカスを次の適格な要素へ巡回させます
    #[inline]
    pub fn cycle_keyboard_focus(&mut self, reverse: bool) {
        EventStore::cycle_keyboard_focus_internal(self, reverse);
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
        EventStore::inject_paste_internal(self, text);
    }

    /// Undo (元に戻す) のインジェクション
    #[inline]
    pub fn inject_undo(&mut self) {
        EventStore::inject_undo_internal(self);
    }

    /// Redo (やり直し) のインジェクション
    #[inline]
    pub fn inject_redo(&mut self) {
        EventStore::inject_redo_internal(self);
    }

    /// 切り取り (Ctrl+X) の実行と削除後のテキスト取得
    #[inline]
    pub fn inject_cut(&mut self) -> Option<String> {
        EventStore::inject_cut_internal(self)
    }

    /// マウス座標などが、要素の描画領域かつ表示枠内に収まっているかを判定。
    /// 階層的な早期枝刈りヒットテスト
    #[inline]
    pub fn hit_test(&mut self, point: LayoutPoint) -> Option<EntityId> {
        TopologyStore::hit_test(
            point,
            &self.events.evt_interaction_states,
            &mut self.topology.topo_sorted_entities,
            &mut self.topology.topo_effective_z_indices,
            &self.topology.topo_active_masks,
            &self.topology.topo_active_entities,
            &self.topology.topo_parents,
            &self.topology.topo_flat_dfs_sequence,
            &self.renders.rnd_visual,
            &self.renders.rnd_base_visual,
            &self.outputs.out_rects,
            &self.outputs.out_clip_rects,
        )
    }

    /// キャッシュコヒーレントな直列DFS同期（1次元直線ループ同期）
    /// Taffy自動計算を完全内包
    #[inline]
    pub fn sync_layout_and_render_list(&mut self, root: EntityId, window_size: LayoutSize) {
        OutputStore::sync_layout_and_render_list_internal(self, root, window_size);
    }
}

#[cfg(test)]
mod tests;
