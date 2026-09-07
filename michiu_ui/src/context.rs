#![allow(unused)]
pub mod config;
pub mod content_store;
pub mod event_store;
pub mod layout_store;
pub mod output_store;
pub mod pipeline;
pub mod reactive_store;
pub mod render_store;
pub mod state_store;
pub mod system_store;
pub mod topology_store;
pub mod window_store;

pub use config::*;
pub use content_store::*;
pub use event_store::*;
pub use layout_store::*;
pub use output_store::*;
pub use pipeline::*;
pub use reactive_store::*;
pub use render_store::*;
pub use state_store::*;
pub use system_store::*;
pub use topology_store::*;
pub use window_store::*;

use crate::{
    BasicLayout, ComponentMask, CursorIcon, Element, ElementState, FlexLayout, GridLayout,
    ImeState, InteractionState, LayoutPoint, LayoutRect, LayoutSize, Modifiers, MouseButton,
    Overflow, PlaybackCount, PointerEvents, PropertyList, ReadSignal, RendererView, SignalId,
    TextAlign, TransitionValue, UserSelect, Val, VirtualKey, VisualProperty, WriteSignal,
    bind_context, handle_on_char_input, handle_on_click, handle_on_dnd_entity_drop,
    handle_on_dnd_id_drop, handle_on_file_dropped, handle_on_ime, handle_on_keyboard_input,
    handle_on_mouse_input, handle_on_right_click, with_context,
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

pub struct Context {
    pub window: WindowStore,
    pub system: SystemStore,
    pub reactive: ReactiveStore,
    pub events: EventStore,
    pub contents: ContentStore,
    pub topology: TopologyStore,
    pub states: StateStore,
    pub layouts: LayoutStore,
    pub renders: RenderStore,
    pub outputs: OutputStore,
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            topology: TopologyStore::new(),
            states: StateStore::new(),
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

    #[inline]
    #[must_use]
    pub fn with_capacity(capacity: &CapacityConfig) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            window: WindowStore::new(),
            system: SystemStore::with_capacity(
                TaskSender {
                    inner: tx,
                    waker: None,
                },
                rx,
                capacity,
            ),
            reactive: ReactiveStore::with_capacity(capacity),
            events: EventStore::with_capacity(capacity),
            contents: ContentStore::with_capacity(capacity),
            topology: TopologyStore::with_capacity(capacity),
            states: StateStore::with_capacity(capacity),
            layouts: LayoutStore::with_capacity(capacity),
            renders: RenderStore::with_capacity(capacity),
            outputs: OutputStore::with_capacity(capacity),
        }
    }

    /// 一括解放
    #[inline]
    pub fn clear(&mut self) {
        self.topology.clear();
        self.states.clear();
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
            &mut self.states,
            &mut self.layouts,
            &mut self.renders,
            &mut self.outputs,
        );
    }

    /// 指定した要素の親要素を取得します。
    #[inline]
    pub fn parent_element(&self, handle: Element) -> Option<Element> {
        self.topology
            .topo_parents
            .get(handle.id)
            .and_then(|c| c.map(Element::from))
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

    /// 指定された要素に現在設定されている最新の `BasicLayout` を取得
    #[inline]
    #[must_use]
    pub fn try_basic_layout(&self, id: EntityId) -> Option<BasicLayout> {
        self.layouts.lay_basic.get(id).copied()
    }

    /// 指定された要素に現在設定されている最新の `FlexLayout` を取得
    #[inline]
    #[must_use]
    pub fn get_flex_layout(&self, id: EntityId) -> Option<FlexLayout> {
        self.layouts.lay_flex.get(id).copied()
    }

    /// 指定された要素に現在設定されている最新の `GridLayout` を取得
    #[inline]
    #[must_use]
    pub fn get_grid_layout(&self, id: EntityId) -> Option<GridLayout> {
        self.layouts.lay_grid.get(id).cloned()
    }

    /// 指定された要素に現在設定されている最新の `VisualProperty` を取得
    #[inline]
    #[must_use]
    pub fn get_visual_property(&self, id: EntityId) -> Option<VisualProperty> {
        self.renders.rnd_visual.get(id).cloned()
    }

    /// 指定された要素が現在マウスホバーされているか判定します
    #[inline]
    pub fn is_hovered(&self, id: EntityId) -> bool {
        self.topology
            .topo_active_masks
            .get(id)
            .is_some_and(|m| m.has(ComponentMask::STATE_HOVERED))
    }

    /// 指定された要素が現在キーボードフォーカスを得ているか判定します
    #[inline]
    pub fn is_focused(&self, id: EntityId) -> bool {
        self.topology
            .topo_active_masks
            .get(id)
            .is_some_and(|m| m.has(ComponentMask::STATE_FOCUSED))
    }

    /// 指定された要素が現在マウスやタップで押し下げられているか判定します
    #[inline]
    pub fn is_pressed(&self, id: EntityId) -> bool {
        self.topology
            .topo_active_masks
            .get(id)
            .is_some_and(|m| m.has(ComponentMask::STATE_PRESSED))
    }

    /// 指定された要素が無効化（操作不可）状態にあるか判定します
    #[inline]
    pub fn is_disabled(&self, id: EntityId) -> bool {
        self.topology
            .topo_active_masks
            .get(id)
            .is_some_and(|m| m.has(ComponentMask::STATE_DISABLED))
    }

    /// 指定された要素が現在アクティブ（有効選択など）状態にあるか判定します
    #[inline]
    pub fn is_actived(&self, id: EntityId) -> bool {
        self.topology
            .topo_active_masks
            .get(id)
            .is_some_and(|m| m.has(ComponentMask::STATE_ACTIVED))
    }

    /// 指定された要素が現在テキストまたはトグル選択されているか判定します
    #[inline]
    pub fn is_selected(&self, id: EntityId) -> bool {
        self.topology
            .topo_active_masks
            .get(id)
            .is_some_and(|m| m.has(ComponentMask::STATE_SELECTED))
    }

    /// 指定された要素が現在ドラッグ操作中にあるか判定します
    #[inline]
    pub fn is_dragged(&self, id: EntityId) -> bool {
        self.topology
            .topo_active_masks
            .get(id)
            .is_some_and(|m| m.has(ComponentMask::STATE_DRAGGED))
    }

    /// 現在、システム内部に再描画要求（Dirtyマークされた要素）があるか判定します。
    #[inline]
    pub fn has_dirty(&self) -> bool {
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
        TopologyStore::mark_dirty(
            id,
            &mut self.topology.topo_active_masks,
            &self.topology.topo_parents,
            &mut self.layouts.lay_dirty_entities,
            &mut self.layouts.lay_taffy_tree,
            &self.layouts.lay_taffy_nodes,
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
        self.outputs.out_rects.get(id).copied()
    }

    /// 指定した要素の画面上のクリップ境界（LayoutRect）を取得します。
    #[inline]
    pub fn clip_rect(&self, id: EntityId) -> Option<LayoutRect> {
        self.outputs.out_clip_rects.get(id).copied()
    }

    /// 現在フォーカスされている要素で範囲選択されている文字列を取得します。
    #[inline]
    pub fn get_selected_text(&self) -> Option<String> {
        TextEditStore::get_selected_text(
            &self.events.evt_interaction_states,
            &self.contents.cont_text_contents,
            &self.renders.rnd_visual,
            &self.states.edit.edit_selections,
        )
        .map(|m| m.into())
    }

    /// 現在のスクロール位置 (x, y) を取得
    #[inline]
    #[must_use]
    pub fn scroll_offset(&self, id: EntityId) -> Option<LayoutPoint> {
        self.states.scroll.sc_offsets.get(id).copied()
    }

    /// 現在のスクロール位置から相対移動します。
    #[inline]
    pub fn scroll_by(&mut self, id: EntityId, dx: f32, dy: f32) -> bool {
        ScrollStore::scroll_by(
            id,
            dx,
            dy,
            self.window.win_last_size,
            &mut self.topology.topo_active_masks,
            &self.topology.topo_parents,
            &mut self.layouts.lay_dirty_entities,
            &mut self.layouts.lay_taffy_tree,
            &mut self.layouts.scrollbar.bar_styles,
            &self.layouts.lay_taffy_nodes,
            &self.layouts.lay_resolved_basic,
            &self.renders.rnd_visual,
            &self.renders.rnd_interaction,
            &self.renders.rnd_active_transitions,
            &mut self.states.scroll.sc_offsets,
            &self.outputs.out_rects,
            &self.states.scroll.sc_sizes,
        )
    }

    #[inline]
    pub fn cut_text(&self) -> Option<Cow<'_, str>> {
        self.contents.cont_cut_text.clone().map(|f| f.0)
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

    /// 指定された要素もしくはルート要素に対してシグナルコンテキストを提供します
    #[inline]
    pub fn provide<T: Send + 'static>(&mut self, id: Option<EntityId>, read_signal: ReadSignal<T>) {
        ReactiveStore::provide::<T>(
            id,
            read_signal,
            &mut self.reactive.react_providers,
            &self.topology.topo_entities,
            &self.topology.topo_parents,
            &self.topology.topo_flat_dfs_sequence,
        );
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
    pub fn has_active_frame(&self) -> bool {
        RenderStore::has_active_frame(
            &self.events.evt_interaction_states,
            self.events.evt_current_pointer_position.as_ref(),
            &self.contents.cont_input_contents,
            &self.layouts.scrollbar.bar_styles,
            &self.renders.rnd_visual,
            &self.renders.rnd_active_transitions,
            &self.renders.rnd_active_animations,
            &self.outputs.out_clip_rects,
        )
    }

    /// 毎フレームの描画前に呼び出され、すべてのアクティブなキーフレームアニメーションを 1 Tick 進めます
    pub fn tick_system_frame(&mut self, tick: &TickType) {
        Pipeline::tick_system_frame(self, tick);
    }

    /// ワーカースレッドなど、どこからでも安全にクローンしてタスクを送信できるスレッドセーフな送信端を取得します。
    #[inline]
    pub fn task_sender(&self) -> TaskSender {
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
    pub fn interaction_id(&self, interaction: InteractionState) -> Option<EntityId> {
        match interaction {
            InteractionState::Hovered => self.events.evt_interaction_states.hovered,
            InteractionState::Focused => self.events.evt_interaction_states.focused,
            InteractionState::Pressed => self.events.evt_interaction_states.pressed,
            InteractionState::Dragged => self.events.evt_interaction_states.dragged,
        }
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
        FocusStore::auto_focus_switch_by_trigger(self, id, trigger);
    }

    #[inline]
    pub fn set_states(&mut self, id: EntityId, flag: &StateFlag, actived: bool) {
        Pipeline::set_states(self, id, flag, actived);
    }

    #[inline]
    pub fn interaction_states(&mut self, id: Option<EntityId>, interaction: InteractionState) {
        match interaction {
            InteractionState::Hovered => self.events.evt_interaction_states.hovered = id,
            InteractionState::Focused => self.events.evt_interaction_states.focused = id,
            InteractionState::Pressed => self.events.evt_interaction_states.pressed = id,
            InteractionState::Dragged => self.events.evt_interaction_states.dragged = id,
        }
    }

    #[inline]
    pub fn inject_user_action(&mut self, action: UserAction) {
        Pipeline::inject_user_action(self, action);
    }

    /// キーボードフォーカスを次の適格な要素へ巡回させます
    #[inline]
    pub fn cycle_keyboard_focus(&mut self, reverse: bool) {
        EventStore::cycle_keyboard_focus_internal(self, reverse);
    }

    /// マウス座標などが、要素の描画領域かつ表示枠内に収まっているかを判定。
    /// 階層的な早期枝刈りヒットテスト
    #[inline]
    pub fn hit_test(&mut self, point: LayoutPoint) -> Option<EntityId> {
        TopologyStore::hit_test(
            point,
            self.window.win_last_size,
            &self.events.evt_interaction_states,
            &mut self.topology.topo_active_masks,
            &mut self.topology.topo_dfs_indices,
            &mut self.topology.topo_effective_z_indices,
            &mut self.topology.topo_sorted_entities,
            &mut self.topology.topo_sort_cache,
            &mut self.topology.topo_is_sort_dirty,
            &self.topology.topo_active_entities,
            &self.topology.topo_parents,
            &self.topology.topo_flat_dfs_sequence,
            &self.renders.rnd_visual,
            &self.renders.rnd_base_visual,
            &mut self.outputs.out_clip_rects,
            &self.outputs.out_rects,
        )
    }

    /// キャッシュコヒーレントな直列DFS同期（1次元直線ループ同期）
    /// Taffy自動計算を完全内包
    #[inline]
    pub fn sync_layout_and_render(&mut self, root: EntityId, window_size: LayoutSize) {
        Pipeline::sync_layout_and_render(self, root, window_size);
    }
}

#[cfg(test)]
mod tests;
