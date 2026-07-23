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

    /// 要素を新規に生成（Spawn）
    #[inline]
    pub(crate) fn spawn(&mut self, parent_id: Option<EntityId>) -> EntityId {
        TopologyStore::spawn(
            parent_id,
            &mut self.topology,
            &mut self.layouts,
            &mut self.renders,
        )
    }

    #[inline]
    pub(crate) fn add_child(&mut self, parent: EntityId, child: EntityId) {
        TopologyStore::add_child(parent, child, &mut self.topology, &mut self.layouts);
    }

    #[inline]
    pub(crate) fn mark_layout_dirty(&mut self, id: EntityId) {
        TopologyStore::mark_layout_dirty(id, &mut self.topology, &mut self.layouts);
    }

    #[inline]
    pub(crate) fn mark_render_dirty(&mut self, id: EntityId) {
        TopologyStore::mark_render_dirty(id, &mut self.topology, &mut self.renders);
    }

    // セッションの開始マーカーを取得
    #[inline]
    pub(crate) fn start_session(&mut self) -> usize {
        self.topology.session_spawned.len()
    }

    // ルート要素として保護するIDを登録
    #[inline]
    pub(crate) fn register_root(&mut self, id: EntityId) {
        self.topology.session_roots.push(id);
    }

    // セッションのクリーンアップを実行
    #[inline]
    pub(crate) fn end_session(&mut self, start_marker: usize) {
        // start_marker 以降に生成された要素をスキャン
        let spawned_in_session: Vec<EntityId> = self
            .topology
            .session_spawned
            .drain(start_marker..)
            .collect();

        for id in spawned_in_session {
            if TopologyStore::no_root_no_parent(id, &self.topology) {
                TopologyStore::despawn_internal(id, self);
            }
        }
        // ルートリストをクリア
        self.topology.session_roots.clear();
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

    /// Context インスタンスから直接シグナルを生成します。
    /// これにより build_ui の外側（メインスレッド上）でもシグナルを定義できます。
    #[inline]
    pub fn create_signal<T: Send + 'static>(
        &mut self,
        initial_value: T,
    ) -> (ReadSignal<T>, WriteSignal<T>) {
        ReactiveStore::create_signal(initial_value, &mut self.reactive)
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

    /// 要素の階層トポロジーを親（Ancestor）に向かって遡り、最初に見つかった型 T の ReadSignal を解決して返します
    #[inline]
    pub(crate) fn use_provided_from<T: Clone + 'static>(
        &self,
        id: EntityId,
    ) -> Option<ReadSignal<T>> {
        ReactiveStore::use_provided_from(id, &self.reactive, &self.topology)
    }

    /// 現在のスレッドローカルコンテキスト（アクティブなエフェクト、またはイベントハンドラ）から、
    /// 自動的に対象の要素を特定し、親ツリーを遡って型 T の ReadSignal を解決します。
    #[inline]
    pub fn use_provided<T: Clone + 'static>(&self) -> ReadSignal<T> {
        // ACTIVE_EFFECT（エフェクト実行中）から解決を試みる
        let element_id = ReactiveStore::resolve_element_effect(&self.reactive);

        // 親ツリーを遡って解決
        ReactiveStore::use_provided_from::<T>(element_id, &self.reactive, &self.topology)
                .unwrap_or_else(|| {
                    panic!(
                        "Dependency resolution failed: No Provider found in ancestor sub-tree for type: '{}'",
                        std::any::type_name::<T>()
                    )
                })
    }

    /// 現在のスレッドローカルコンテキストから、
    /// 親ツリーを自動的に遡って解決した型 T のシグナルに対する同期書き込み用端（WriteSignal）を取得します。
    #[inline]
    pub fn use_provided_setter<T: Send + 'static>(&self) -> WriteSignal<T> {
        ReactiveStore::use_provided_setter(&self.reactive, &self.topology)
    }

    /// 要素にエフェクトをカテゴリ指定付きで紐づけて登録します。
    /// 同一カテゴリのエフェクトが既に存在する場合、自動的に古いエフェクトを破棄してから上書きします。
    #[inline]
    pub(crate) fn register_element_effect(
        &mut self,
        element_id: EntityId,
        category: EffectCategory,
        effect_id: EffectId,
    ) {
        ReactiveStore::register_element_effect(element_id, &mut self.reactive, category, effect_id);
    }

    /// 要素に動的エフェクト（Style、Text等のリアクティブクロージャ）を安全に登録し、初期評価を実行します。
    #[inline]
    pub(crate) fn create_element_effect<F>(
        &mut self,
        element_id: EntityId,
        category: EffectCategory,
        f: F,
    ) -> EffectId
    where
        F: FnMut(&mut Context) + 'static,
    {
        ReactiveStore::create_element_effect(element_id, &mut self.reactive, category, f)
    }

    /// トポロジーが完全に完成したビルド完了後、または同期直前に、溜めてある初回評価を一挙に安全実行します
    #[inline]
    pub(crate) fn evaluate_pending_element_effects(&mut self) {
        ReactiveStore::evaluate_pending_element_effects(&mut self.reactive);
    }

    /// 指定された要素に対してシグナルコンテキストを提供します
    #[inline]
    pub(crate) fn provide_context<T: Send + 'static>(&mut self, id: EntityId, signal_id: SignalId) {
        ReactiveStore::provide_context::<T>(id, &mut self.reactive, signal_id);
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
    pub fn despawn(&mut self, handle: Element) {
        self.despawn_internal(handle.id);
    }

    /// 要素を安全に破棄（Despawn）。親が消えた場合子はフレーム末尾のクリーンアップフェーズで自動修復・一掃
    #[inline]
    pub(crate) fn despawn_internal(&mut self, id: EntityId) {
        TopologyStore::despawn_internal(id, self);
    }

    /// 親要素の特定の古い子要素を、順序（インデックス）を維持したまま新しい子要素へ直接差し替えます。
    #[inline]
    pub(crate) fn replace_child(
        &mut self,
        parent: EntityId,
        old_child: EntityId,
        new_child: EntityId,
    ) {
        TopologyStore::replace_child(
            parent,
            old_child,
            new_child,
            &mut self.layouts,
            &mut self.topology,
        );

        // 親子参照の更新
        self.topology.parents.insert(new_child, Some(parent));

        // 古い子要素（およびその子孫）を完全に安全デスポーン
        // この中で Taffy からの remove_child も安全に実行されます
        TopologyStore::despawn_internal(old_child, self);

        TopologyStore::mark_layout_dirty(parent, &mut self.topology, &mut self.layouts);
        self.layouts.is_structure_dirty = true;
    }

    /// デスポーン済みの無効な EntityId を各走査・Dirty配列から一括して排除。
    #[inline]
    pub(crate) fn gc_inactive_entities(&mut self) {
        TopologyStore::gc_inactive_entities(
            &mut self.topology,
            &mut self.layouts,
            &mut self.renders,
        );
    }

    /// 現在、システム内部に再描画要求（Dirtyマークされた要素）があるか判定します。
    #[inline]
    pub fn is_render_dirty(&self) -> bool {
        // dirty_render_entities に何か登録されている、またはレイアウトに Dirty がある場合
        !self.renders.dirty_render_entities.is_empty()
            || !self.layouts.dirty_layout_entities.is_empty()
            || self.layouts.is_structure_dirty
    }

    /// 実際の可視サイズから、物理ボーダーとパディングの厚みを引いた内枠の有効表示可能サイズを算出します。
    #[inline]
    pub(crate) fn calculate_inner_content_size(
        &self,
        visible_size: LayoutSize,
        border: EdgeInsets,
        padding: EdgeInsets,
    ) -> LayoutSize {
        LayoutStore::calculate_inner_content_size(visible_size, border, padding)
    }

    #[inline]
    pub(crate) fn get_basic_layout_mut(
        &mut self,
        id: EntityId,
        target: StyleTarget,
    ) -> Option<&mut BasicLayout> {
        RenderStore::get_basic_layout_mut(id, &mut self.renders, target)
    }

    #[inline]
    pub(crate) fn get_visual_property_mut(
        &mut self,
        id: EntityId,
        target: StyleTarget,
    ) -> Option<&mut VisualProperty> {
        RenderStore::get_visual_property_mut(id, &mut self.renders, target)
    }

    #[inline]
    pub(crate) fn get_flex_layout_mut(
        &mut self,
        id: EntityId,
        target: StyleTarget,
    ) -> Option<&mut FlexLayout> {
        RenderStore::get_flex_layout_mut(
            id,
            &mut self.renders,
            target,
            &mut self.layouts.flex_layouts,
        )
    }

    /// テキストやインプットのサイズを DirectWrite を用いて計測し、Taffy 向けサイズを返します。
    #[inline]
    pub(crate) fn measure_content(
        &mut self,
        id: EntityId,
        visual_properties: &SecondaryMap<EntityId, VisualProperty>,
        known_dims: taffy::Size<Option<f32>>,
    ) -> taffy::Size<f32> {
        ContentStore::measure_content(
            id,
            &mut self.contents,
            &self.topology.active_masks,
            visual_properties,
            &self.system.text_engine,
            known_dims,
        )
    }

    /// リサイズ方向から対応するカーソル種別へ変換するヘルパー
    #[inline]
    pub(crate) fn resize_direction_to_cursor(dir: ResizeDirection) -> CursorIcon {
        EventStore::resize_direction_to_cursor(dir)
    }

    /// マウス位置と要素の境界・リサイズ許可フラグから、該当するリサイズ方向を算出するヘルパー
    #[inline]
    pub(crate) fn detect_resize_direction(
        rect: LayoutRect,
        resizable: [bool; 4], // [top, right, bottom, left]
        pos: LayoutPoint,
        border: f32,
    ) -> Option<ResizeDirection> {
        EventStore::detect_resize_direction(rect, resizable, pos, border)
    }

    /// 各スタイルの解決を1回のルックアップと1回のカスケード解決ループに統合
    #[inline]
    pub(crate) fn resolve_active_layouts(
        &self,
        id: EntityId,
    ) -> (BasicLayout, FlexLayout, Option<GridLayout>) {
        LayoutStore::resolve_active_layouts(id, &self.topology, &self.layouts, &self.renders)
    }

    // Taffyスタイルを一括解決するヘルパー
    #[inline]
    pub(crate) fn resolve_taffy_style(
        &self,
        id: EntityId,
        basic: &BasicLayout,
        flex: &FlexLayout,
        grid: Option<&GridLayout>,
    ) -> taffy::Style {
        LayoutStore::resolve_taffy_style(id, &self.layouts, basic, flex, grid)
    }

    /// 指定された要素の現在解決されている物理ボーダー（EdgeInsets）を取得します。
    pub(crate) fn get_physical_border(&self, id: EntityId, basic: &BasicLayout) -> EdgeInsets {
        LayoutStore::get_physical_border(id, basic, &self.outputs)
    }

    /// 指定された要素の現在解決されている物理パディング（EdgeInsets）を取得します。
    pub(crate) fn get_physical_padding(&self, id: EntityId, basic: &BasicLayout) -> EdgeInsets {
        LayoutStore::get_physical_padding(id, basic, &self.outputs)
    }

    // 全スクロールバー関連IDを一括抽出
    #[inline]
    pub(crate) fn scrollbar_el_ids(&self) -> HashSet<EntityId> {
        LayoutStore::scrollbar_el_ids(&self.layouts.scrollbar_styles)
    }

    /// スクロールバー用要素（TrackやThumb）のレイアウト、不透明度、Taffyスタイルへの反映を一括して同期更新します。
    #[inline]
    pub(crate) fn update_scrollbar_element(
        &mut self,
        id: EntityId,
        size: Size<Val>,
        inset: Rect<Val>,
        opacity: f32,
    ) {
        LayoutStore::update_scrollbar_element_layout(
            id,
            &mut self.layouts,
            &mut self.renders,
            size,
            inset,
        );
        RenderStore::update_scrollbar_element_opacity(id, &mut self.renders, opacity);

        let (basic, flex, grid) =
            LayoutStore::resolve_active_layouts(id, &self.topology, &self.layouts, &self.renders);

        LayoutStore::set_taffy_style(id, &mut self.layouts, &basic, &flex, grid.as_ref());
    }

    /// 解決済みの基本スタイルを TaffyTree のノードへ即時同期して適用します。
    #[inline]
    pub(crate) fn set_taffy_style(
        &mut self,
        id: EntityId,
        basic: &BasicLayout,
        flex: &FlexLayout,
        grid: Option<&GridLayout>,
    ) {
        LayoutStore::set_taffy_style(id, &mut self.layouts, basic, flex, grid);
    }

    /// スクロールバー用要素をレイアウト上から安全に隠します。
    #[inline]
    pub(crate) fn hide_scrollbar_element(&mut self, id: EntityId) {
        LayoutStore::hide_scrollbar_element(id, &mut self.layouts, &mut self.renders);
    }

    /// 非再帰スタックによるフラットDFS配列の高速構築
    #[inline]
    pub(crate) fn rebuild_flat_dfs_sequence(&mut self, root: EntityId) {
        LayoutStore::rebuild_flat_dfs_sequence(root, &mut self.layouts, &self.topology);
    }

    #[inline]
    pub(crate) fn local_rect_from_taffy(&self, id: EntityId) -> LayoutRect {
        LayoutStore::local_rect_from_taffy(id, &self.layouts)
    }

    /// 指定された親コンテナにアタッチされている DComp / Taffy 側のすべての子ノードの物理順序を
    /// 内部 SoA リスト（self.children）の順序に沿って一括して再同期）します。
    #[inline]
    pub(crate) fn resync_taffy_children_order(&mut self, parent_id: EntityId) {
        LayoutStore::resync_taffy_children_order(parent_id, &mut self.layouts, &self.topology);
    }

    #[inline]
    pub fn clear_layout_dirty(&mut self) {
        LayoutStore::clear_layout_dirty(&mut self.layouts, &mut self.topology);
    }

    /// 指定した要素の画面上の絶対座標（LayoutRect）を取得します。
    #[inline]
    pub fn rect(&self, handle: Element) -> Option<LayoutRect> {
        self.outputs.rects.get(handle.id).copied()
    }

    /// 指定した要素の画面上のクリップ境界（LayoutRect）を取得します。
    #[inline]
    pub fn clip_rect(&self, handle: Element) -> Option<LayoutRect> {
        self.outputs.clip_rects.get(handle.id).copied()
    }

    #[inline]
    pub(crate) fn swap_output_rect(&mut self) {
        OutputStore::swap_output_rect(&mut self.outputs);
    }

    #[inline]
    pub(crate) fn parent_changed(&self, id: EntityId) -> bool {
        OutputStore::parent_changed(id, &self.outputs, &self.topology)
    }

    #[inline]
    pub(crate) fn calc_local_rect(
        &self,
        id: EntityId,
        window_size: LayoutSize,
    ) -> (LayoutRect, LayoutRect) {
        OutputStore::calc_local_rect(
            id,
            &self.outputs,
            &self.layouts,
            &self.topology,
            window_size,
        )
    }

    /// 単位（Px, Percent, Auto）を親要素のサイズまたはウィンドウ基準をベースに f32 (物理ピクセル) へ解決します。
    #[inline]
    pub(crate) fn resolve_val_to_px(&self, id: EntityId, val: Val, is_width: bool) -> Option<f32> {
        OutputStore::resolve_val_to_px(
            id,
            val,
            is_width,
            &self.topology,
            &self.outputs,
            &self.window,
        )
    }

    /// 現在テキスト選択ドラッグ中かつ、マウスポインタが要素の可視境界外にあるかを判定
    #[inline]
    pub(crate) fn is_drag_autoscroll_active(&self) -> bool {
        OutputStore::is_drag_autoscroll_active(&self.events, &self.outputs, &self.renders)
    }

    #[inline]
    pub(crate) fn trigger_keyframe_animations_if_needed(&mut self, id: EntityId) {
        RenderStore::trigger_keyframe_animations_if_needed(id, &mut self.renders);
    }

    /// 指定された動的状態（例: STATE_HOVERED）に切り替わる際、
    /// その要素に割り当てられている状態スタイルがレイアウトの再計算を必要とするか判定します。
    #[inline]
    pub(crate) fn does_state_require_layout(&self, id: EntityId, state_flag: u128) -> bool {
        RenderStore::does_state_require_layout(id, &self.renders, state_flag)
    }

    #[inline]
    pub(crate) fn resolv_focus_style(
        &self,
        id: EntityId,
        active_mask: &ComponentMask,
    ) -> Option<ThisStyle> {
        RenderStore::resolv_focus_style(id, &self.renders, active_mask, &self.topology.parents)
    }

    #[inline]
    pub(crate) fn cascade_interaction(
        &self,
        id: EntityId,
        active_mask: ComponentMask,
        target: &mut TargetStyle,
        focus_style_resolved: Option<ThisStyle>,
    ) {
        if let Some(interaction) = self.renders.interaction_properties.get(id) {
            let cascade = RenderStore::cascade_interaction_flag(
                id,
                &self.renders,
                interaction,
                &focus_style_resolved,
            );

            for (state, style_opt) in cascade {
                if active_mask.has(state)
                    && let Some(style) = style_opt
                {
                    TargetStyle::apply_visual_property(
                        target,
                        &style.inner.visual_property,
                        style.inner.mask,
                    );
                }
            }
        }
    }

    #[inline]
    pub(crate) fn cascade_within_interaction(
        &self,
        id: EntityId,
        active_mask: ComponentMask,
        target: &mut TargetStyle,
    ) {
        if active_mask.has(STYLE_INTERACTION_WITHIN)
            && let Some(interaction) = self.renders.interaction_properties.get(id)
        {
            // 自身の mask にビットが立っている場合のみツリー再帰を走らせてマージ解決
            let cascade_within = RenderStore::cascade_within_interaction_flag(id, interaction);

            for (state, style_opt) in cascade_within {
                // 子孫要素のいずれかがこの state_flag を満たしているか
                if TopologyStore::has_descendant_with_state(id, &self.topology, state)
                    && let Some(style) = style_opt
                {
                    TargetStyle::apply_visual_property(
                        target,
                        &style.inner.visual_property,
                        style.inner.mask,
                    );
                }
            }

            // All（いずれかのインタラクションがあればON）の解決
            if let Some(ref style) = interaction.any_within
                && TopologyStore::has_descendant_with_any_active_state(id, &self.topology)
            {
                TargetStyle::apply_visual_property(
                    target,
                    &style.inner.visual_property,
                    style.inner.mask,
                );
            }
        }
    }

    #[inline]
    pub(crate) fn cascade_basic_layout(
        &self,
        id: EntityId,
        active_mask: ComponentMask,
        target_layout: &mut BasicLayout,
    ) {
        RenderStore::cascade_basic_layout(id, &self.renders, active_mask, target_layout);
    }

    /// 現在の描画用データを取得 (Copy可能なプリミティブのみ)
    #[inline]
    pub(crate) fn get_current_style(&self, id: EntityId) -> CurrentStyle {
        RenderStore::get_current_style(id, &self.renders)
    }

    /// 目標値を参照経由で構築
    #[inline]
    pub(crate) fn get_target_style(&self, id: EntityId) -> TargetStyle {
        RenderStore::get_target_style(id, &self.renders)
    }

    /// 現在ホバーされている要素から親ツリーを遡り、適用するべき物理的な CursorIcon を正確に解決します。
    #[inline]
    pub fn resolve_cursor(&self, hovered_id: EntityId) -> CursorIcon {
        RenderStore::resolve_cursor(hovered_id, &self.events, &self.renders, &self.topology)
    }

    /// 描画（レンダー）ダーティ状態として登録された要素をすべてクリアします。
    #[inline]
    pub fn clear_render_dirty(&mut self) {
        RenderStore::clear_render_dirty(&mut self.renders, &mut self.topology);
    }

    /// テキスト変更やスタイル更新時にキャッシュを安全に破棄します。
    #[inline]
    pub(crate) fn clear_layout_cache(&self, id: EntityId) {
        self.system.dwrite_layouts.borrow_mut().remove(id);
    }

    /// 子孫要素のインタラクション状態（state_flag）を走査します
    #[inline]
    pub(crate) fn has_descendant_with_state(&self, parent: EntityId, state_flag: u128) -> bool {
        TopologyStore::has_descendant_with_state(parent, &self.topology, state_flag)
    }

    /// いずれか一つのアクティブなユーザーインタラクションが子孫要素でONになっているか非再帰で走査します
    #[inline]
    pub(crate) fn has_descendant_with_any_active_state(&self, parent: EntityId) -> bool {
        TopologyStore::has_descendant_with_any_active_state(parent, &self.topology)
    }

    /// ドロップ先コンテナのフレックス方向（Row / Column）に基づいて、
    /// マウスのドロップ座標がどの子要素の手前（インデックス）に位置するかを逆引き算出します。
    #[inline]
    fn calculate_insert_index(&self, parent_id: EntityId, logical_pos: LayoutPoint) -> usize {
        TopologyStore::calculate_insert_index(
            parent_id,
            logical_pos,
            &self.topology,
            &self.layouts,
            &self.outputs,
        )
    }

    /// 指定された要素（target）が、ある親要素（parent）自身、またはその子孫であるかを判定します。
    #[inline]
    pub(crate) fn is_descendant_of(&self, target: EntityId, parent: EntityId) -> bool {
        TopologyStore::is_descendant_of(target, parent, &self.topology)
    }

    /// ウィンドウサイズの変更検知
    #[inline]
    pub(crate) fn window_resize_detection(&mut self, window_size: LayoutSize) -> bool {
        WindowStore::window_resize_detection(&mut self.window, window_size)
    }

    /// 与えられたコンテナ矩形の、現在のウィンドウ領域において実際に画面上に見えている物理的な可視サイズを算出します。
    #[inline]
    pub(crate) fn calculate_visible_size(&self, container_rect: LayoutRect) -> LayoutSize {
        WindowStore::calculate_visible_size(&self.window, container_rect)
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
        // Taffy計算も、ダブルバッファスワップもすべてスキップして即時帰還する。
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

            let taffy_style = self.resolve_taffy_style(*id, &basic, &flex, grid.as_ref());
            let taffy_node = self.layouts.taffy_nodes[*id];

            self.layouts
                .taffy
                .set_style(taffy_node, taffy_style)
                .unwrap();
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

    fn sync_scrollbar_styles(&mut self) {
        let scrollbar_ids: Vec<EntityId> = self.layouts.scrollbar_styles.keys().collect();
        for id in scrollbar_ids {
            let sb_state = self.layouts.scrollbar_styles.get(id).cloned().unwrap();
            let container_rect = self.outputs.rects[id];
            let scroll_size = self.get_scroll_size(id);
            let current_scroll = self
                .outputs
                .scroll_offsets
                .get(id)
                .copied()
                .unwrap_or(LayoutPoint::ZERO);

            // 親コンテナのボーダーおよびパディング厚を取得
            let (basic, _, _) = self.resolve_active_layouts(id);

            let border = self.get_physical_border(id, &basic);
            let padding = self.get_physical_padding(id, &basic);

            // ウィンドウ境界によるクランプ可視サイズの算出
            let visible_size = self.calculate_visible_size(container_rect);
            // 枠線と余白を引いた内枠コンテンツサイズの算出
            let content_size = self.calculate_inner_content_size(visible_size, border, padding);

            // 内枠の有効表示領域と、同じく内枠基準の scroll_size を精密に比較する
            let show_v_bar = scroll_size.height > content_size.height;
            let show_h_bar = scroll_size.width > content_size.width;

            // 縦スクロールバーの同期
            if let Some(v_track) = sb_state.v_track_id {
                let show = show_v_bar && sb_state.style.display != ScrollbarDisplay::None;

                let mut v_track_visible = false;
                let mut v_track_opacity = 1.0f32;

                if show {
                    if sb_state.style.display == ScrollbarDisplay::Always
                        || sb_state.style.display == ScrollbarDisplay::Auto
                    {
                        v_track_visible = true;
                    } else if sb_state.style.display == ScrollbarDisplay::Transient
                        && let Some(last) = sb_state.last_scroll_time
                    {
                        let elapsed = last.elapsed();
                        if elapsed < Duration::from_millis(1000) {
                            v_track_visible = true;
                        } else if elapsed < Duration::from_millis(1500) {
                            v_track_visible = true;
                            v_track_opacity = 1.0 - (elapsed.as_secs_f32() - 1.0) / 0.5;
                        }
                    }
                }

                if v_track_visible {
                    let mut user_track_h = None;
                    if let Some(ref track_style) = sb_state.style.v_track
                        && let Val::Px(val) = track_style.inner.basic_layout.size.height
                    {
                        user_track_h = Some(val);
                    }

                    // 有効可視サイズを起点にすることで画面外突き出しをクランプ
                    let track_h = if let Some(h) = user_track_h {
                        h
                    } else {
                        (visible_size.height
                            - border.top
                            - border.bottom
                            - (if show_h_bar {
                                sb_state.style.width
                            } else {
                                0.0
                            }))
                        .max(0.0)
                    };

                    let track_right = 0.0;

                    // v_track の幅を Val::Auto に上書きしたため、Taffy 側で known_dims.width が None（Auto）になる
                    // Taffy 側の measure_func は  outputs.rects から値を取得しようと試みる
                    // スクロールバー要素は 1 回目のパスで走査スルーされているため、この時点では outputs.rects に座標が登録されていない
                    // 結果としてサイズ 0.0 が返り、Track の幅が 0.0 に潰れて不可視になっていた
                    // Val::Auto ではなく Val::Px(sb_state.style.width) に修正して SoA 上に実サイズを維持
                    self.update_scrollbar_element(
                        v_track,
                        Size::new(Val::Px(sb_state.style.width), Val::Px(track_h)),
                        Rect::new(Val::Px(0.0), Val::Px(track_right), Val::Auto, Val::Auto),
                        v_track_opacity,
                    );
                } else {
                    self.hide_scrollbar_element(v_track);
                }
            }

            // A-1. 縦つまみ (V-Thumb) の同期
            if let Some(v_thumb) = sb_state.v_thumb_id {
                let show = show_v_bar && sb_state.style.display != ScrollbarDisplay::None;

                let mut v_thumb_visible = false;
                let mut v_thumb_opacity = 1.0f32;

                if show {
                    if sb_state.style.display == ScrollbarDisplay::Always
                        || sb_state.style.display == ScrollbarDisplay::Auto
                    {
                        v_thumb_visible = true;
                    } else if sb_state.style.display == ScrollbarDisplay::Transient
                        && let Some(last) = sb_state.last_scroll_time
                    {
                        let elapsed = last.elapsed();
                        if elapsed < Duration::from_millis(1000) {
                            v_thumb_visible = true;
                        } else if elapsed < Duration::from_millis(1500) {
                            v_thumb_visible = true;
                            v_thumb_opacity = 1.0 - (elapsed.as_secs_f32() - 1.0) / 0.5;
                        }
                    }
                }

                if v_thumb_visible {
                    let track_h = (visible_size.height
                        - border.top
                        - border.bottom
                        - (if show_h_bar {
                            sb_state.style.width
                        } else {
                            0.0
                        }))
                    .max(0.0);

                    let mut initial_thumb_h = track_h;
                    if let Some(ref thumb_style) = sb_state.style.v_thumb
                        && let Val::Px(val) = thumb_style.inner.basic_layout.size.height
                    {
                        initial_thumb_h = val;
                    }

                    // コンテンツ比率 (分母・分子に一貫してクランプ済み表示領域を採用)
                    let view_ratio = if scroll_size.height > 0.0 {
                        (visible_size.height / scroll_size.height).min(1.0)
                    } else {
                        1.0
                    };

                    let calculated_h = initial_thumb_h * view_ratio;

                    // ユーザー指定の min_size と max_size で正確にクランプ
                    let mut min_h = 24.0;
                    if let Some(ref thumb_style) = sb_state.style.v_thumb
                        && let Val::Px(val) = thumb_style.inner.basic_layout.min_size.height
                    {
                        min_h = val;
                    }
                    let mut max_h = track_h;
                    if let Some(ref thumb_style) = sb_state.style.v_thumb
                        && let Val::Px(val) = thumb_style.inner.basic_layout.max_size.height
                    {
                        max_h = val;
                    }

                    let thumb_height = calculated_h.max(min_h).min(max_h).min(track_h);

                    let mut margin_top = 0.0;
                    let mut margin_bottom = 0.0;
                    if let Some(ref thumb_style) = sb_state.style.v_thumb {
                        if let Val::Px(val) = thumb_style.inner.basic_layout.margin.top {
                            margin_top = val;
                        }
                        if let Val::Px(val) = thumb_style.inner.basic_layout.margin.bottom {
                            margin_bottom = val;
                        }
                    }

                    let scroll_ratio = if scroll_size.height > visible_size.height {
                        (current_scroll.y / (scroll_size.height - visible_size.height))
                            .clamp(0.0, 1.0)
                    } else {
                        0.0
                    };

                    let max_thumb_y =
                        (track_h - thumb_height - margin_top - margin_bottom).max(0.0);
                    let thumb_y = max_thumb_y * scroll_ratio;

                    let mut thumb_width = sb_state.style.width;

                    let mut pad_right = 0.0;
                    let mut pad_left = 0.0;
                    if let Some(ref thumb_style) = sb_state.style.v_thumb {
                        if let Val::Px(w) = thumb_style.inner.basic_layout.size.width {
                            thumb_width = w.min(sb_state.style.width);
                        }
                        if let Length::Px(val) = thumb_style.inner.basic_layout.padding.right {
                            pad_right = val;
                        }
                        if let Length::Px(val) = thumb_style.inner.basic_layout.padding.left {
                            pad_left = val;
                        }
                    }

                    let thumb_x = if pad_right > 0.0 {
                        sb_state.style.width - thumb_width - pad_right
                    } else if pad_left > 0.0 {
                        pad_left
                    } else {
                        (sb_state.style.width - thumb_width) * 0.5
                    };

                    self.update_scrollbar_element(
                        v_thumb,
                        Size::new(Val::Px(thumb_width), Val::Px(thumb_height)),
                        Rect::new(Val::Px(thumb_y), Val::Auto, Val::Auto, Val::Px(thumb_x)),
                        v_thumb_opacity,
                    );
                } else {
                    self.hide_scrollbar_element(v_thumb);
                }
            }

            // 2. 横スクロールバー (H-Track) の同期
            if let Some(h_track) = sb_state.h_track_id {
                let show = show_h_bar && sb_state.style.display != ScrollbarDisplay::None;

                let mut h_track_visible = false;
                let mut h_track_opacity = 1.0f32;

                if show {
                    if sb_state.style.display == ScrollbarDisplay::Always
                        || sb_state.style.display == ScrollbarDisplay::Auto
                    {
                        h_track_visible = true;
                    } else if sb_state.style.display == ScrollbarDisplay::Transient
                        && let Some(last) = sb_state.last_scroll_time
                    {
                        let elapsed = last.elapsed();
                        if elapsed < Duration::from_millis(1000) {
                            h_track_visible = true;
                        } else if elapsed < Duration::from_millis(1500) {
                            h_track_visible = true;
                            h_track_opacity = 1.0 - (elapsed.as_secs_f32() - 1.0) / 0.5;
                        }
                    }
                }

                if h_track_visible {
                    let track_w = (visible_size.width
                        - border.left
                        - border.right
                        - (if show_v_bar {
                            sb_state.style.width
                        } else {
                            0.0
                        }))
                    .max(0.0);

                    let track_bottom = 0.0;

                    self.update_scrollbar_element(
                        h_track,
                        Size::new(Val::Px(track_w), Val::Px(sb_state.style.width)),
                        Rect::new(Val::Auto, Val::Auto, Val::Px(track_bottom), Val::Px(0.0)),
                        h_track_opacity,
                    );
                } else {
                    self.hide_scrollbar_element(h_track);
                }
            }

            // B-1. 横つまみ (H-Thumb) の同期
            if let Some(h_thumb) = sb_state.h_thumb_id {
                let show = show_h_bar && sb_state.style.display != ScrollbarDisplay::None;

                let mut h_thumb_visible = false;
                let mut h_thumb_opacity = 1.0f32;

                if show {
                    if sb_state.style.display == ScrollbarDisplay::Always
                        || sb_state.style.display == ScrollbarDisplay::Auto
                    {
                        h_thumb_visible = true;
                    } else if sb_state.style.display == ScrollbarDisplay::Transient
                        && let Some(last) = sb_state.last_scroll_time
                    {
                        let elapsed = last.elapsed();
                        if elapsed < Duration::from_millis(1000) {
                            h_thumb_visible = true;
                        } else if elapsed < Duration::from_millis(1500) {
                            h_thumb_visible = true;
                            h_thumb_opacity = 1.0 - (elapsed.as_secs_f32() - 1.0) / 0.5;
                        }
                    }
                }

                if h_thumb_visible {
                    let track_w = (visible_size.width
                        - border.left
                        - border.right
                        - (if show_v_bar {
                            sb_state.style.width
                        } else {
                            0.0
                        }))
                    .max(0.0);

                    let mut initial_thumb_w = track_w;
                    if let Some(ref thumb_style) = sb_state.style.h_thumb
                        && let Val::Px(w) = thumb_style.inner.basic_layout.size.width
                    {
                        initial_thumb_w = w;
                    }

                    let view_ratio = if scroll_size.width > 0.0 {
                        (visible_size.width / scroll_size.width).min(1.0)
                    } else {
                        1.0
                    };

                    let calculated_w = initial_thumb_w * view_ratio;

                    let mut min_w = 24.0;
                    if let Some(ref thumb_style) = sb_state.style.h_thumb
                        && let Val::Px(val) = thumb_style.inner.basic_layout.min_size.width
                    {
                        min_w = val;
                    }
                    let mut max_w = track_w;
                    if let Some(ref thumb_style) = sb_state.style.h_thumb
                        && let Val::Px(val) = thumb_style.inner.basic_layout.max_size.width
                    {
                        max_w = val;
                    }

                    let thumb_width = calculated_w.max(min_w).min(max_w).min(track_w);

                    let mut margin_left = 0.0;
                    let mut margin_right = 0.0;
                    if let Some(ref thumb_style) = sb_state.style.h_thumb {
                        if let Val::Px(val) = thumb_style.inner.basic_layout.margin.left {
                            margin_left = val;
                        }
                        if let Val::Px(val) = thumb_style.inner.basic_layout.margin.right {
                            margin_right = val;
                        }
                    }

                    let scroll_ratio = if scroll_size.width > visible_size.width {
                        (current_scroll.x / (scroll_size.width - visible_size.width))
                            .clamp(0.0, 1.0)
                    } else {
                        0.0
                    };

                    let max_thumb_x = (track_w - thumb_width - margin_left - margin_right).max(0.0);
                    let thumb_x = max_thumb_x * scroll_ratio;

                    let mut thumb_height = sb_state.style.width;
                    let mut pad_top = 0.0;
                    let mut pad_bottom = 0.0;
                    if let Some(ref thumb_style) = sb_state.style.h_thumb {
                        if let Val::Px(val) = thumb_style.inner.basic_layout.size.height {
                            thumb_height = val.min(sb_state.style.width);
                        }
                        if let Length::Px(val) = thumb_style.inner.basic_layout.padding.top {
                            pad_top = val;
                        }
                        if let Length::Px(val) = thumb_style.inner.basic_layout.padding.bottom {
                            pad_bottom = val;
                        }
                    }

                    let thumb_y = if pad_bottom > 0.0 {
                        sb_state.style.width - thumb_height - pad_bottom
                    } else if pad_top > 0.0 {
                        pad_top
                    } else {
                        (sb_state.style.width - thumb_height) * 0.5
                    };

                    self.update_scrollbar_element(
                        h_thumb,
                        Size::new(Val::Px(thumb_width), Val::Px(thumb_height)),
                        Rect::new(Val::Px(thumb_y), Val::Auto, Val::Auto, Val::Px(thumb_x)),
                        h_thumb_opacity,
                    );
                } else {
                    self.hide_scrollbar_element(h_thumb);
                }
            }
        }
    }

    /// 現在の全アクティブ要素から、wgpu 用の前面・背面描画バッチを生成します
    pub fn collect_render_data(&self) -> RenderData {
        let mut batches = Vec::new();
        let mut current_instances = Vec::new();
        let mut current_ids = Vec::new();
        let mut last_clip = None;

        // 現在のバッチの種類 (通常)
        let mut current_batch_type = BatchType::Normal;

        // 静的なデフォルト値（一度だけ確保して使い回す）
        let default_visual = VisualProperty::default();

        // 各要素の実効 z_index を親から子へカスケード（伝播）して計算
        let mut effective_z_indices =
            SecondaryMap::with_capacity(self.topology.active_entities.len());

        // flat_dfs_sequence は必ず親から子への順でフラットに並んでいるため、前方1方向の走査で完結
        for &id in &self.layouts.flat_dfs_sequence {
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

            // 自身に z_index 指定があればそれを最優先し、
            // なければ親の実効 z_index を継承する（双方になければデフォルト 0）
            let eff_z = self_z.or(parent_z).unwrap_or(0);
            effective_z_indices.insert(id, eff_z);
        }

        // 実効 z_index で active_entities を安定ソート
        let mut sorted_entities = self.topology.active_entities.clone();
        sorted_entities.sort_by_key(|&id| effective_z_indices.get(id).copied().unwrap_or(0));

        for &id in &sorted_entities {
            let rect = self.outputs.rects[id];
            if rect.width <= 0.0 || rect.height <= 0.0 {
                continue;
            }

            let clip = self.outputs.clip_rects[id];

            let is_webview = self.topology.active_masks[id].has(COMP_WEBVIEW_CONTENT);

            // コントローラーがまだ初期化されていない（active_webviewsに入っていない）場合は、
            // 紺色の背景を通常通り描き込み、デスクトップが透けるのを完全に防止します。
            let is_webview_ready = is_webview && self.renders.active_webviews.contains(&id);

            let (basic, _, _) = self.resolve_active_layouts(id);
            let visual = self
                .renders
                .visual_properties
                .get(id)
                .unwrap_or(&default_visual);

            let o_width = visual.outline_width.unwrap_or(EdgeInsets::ZERO);
            let o_color = visual.outline_color.unwrap_or(Color::TRANSPARENT);
            let o_lengths = visual.outline_lengths.unwrap_or(EdgeInsets::px_all(1.0));
            let o_offset = visual.outline_offset.unwrap_or(0.0);
            let o_styles = visual.outline_styles.unwrap_or([BorderStyle::Solid; 4]);
            let o_aligns = visual
                .outline_alignments
                .unwrap_or([BorderAlignment::Start; 4]);

            let mut o_flags = 0u32;
            for idx in 0..4 {
                o_flags |= (o_styles[idx] as u32) << (idx * 4);
                o_flags |= (o_aligns[idx] as u32) << (idx * 4 + 2);
            }
            let outline_offset_and_flags = [o_offset, o_flags as f32, 0.0, 0.0];

            if is_webview_ready {
                // 1. 今まで溜まっている「通常（Normal）」のバッチがあれば一旦フラッシュ
                if !current_instances.is_empty() {
                    batches.push(DrawBatch {
                        scissor_rect: last_clip.unwrap_or(LayoutRect::ZERO),
                        instances: std::mem::take(&mut current_instances),
                        entity_ids: std::mem::take(&mut current_ids),
                        batch_type: current_batch_type,
                    });
                }

                let origin = visual
                    .transform_origin
                    .map(|p| [p.x, p.y])
                    .unwrap_or([0.5, 0.5]);

                let full_transform = visual.transform.unwrap_or(IDENTITY_MATRIX);
                let packed_transform = [
                    full_transform[0], // X軸基底
                    full_transform[1], // Y軸基底
                    full_transform[3], // 平行移動部
                ];

                // 不透明度を 1.0 固定にせず、要素自身の opacity 値を引き渡す。
                // これにより、くり抜く強度がブレンドステート OneMinusSrcAlpha に正しく乗り、
                // wgpu側の親の背景色が適度に残ることでグループ合成を模倣しデスクトップ透過を防ぐ
                let punchout_opacity = visual.opacity.unwrap_or(1.0);

                // くり抜き（Punchout）用のインスタンスを作成して登録
                // wgpu のバッファの背景を、角丸を維持したまま完全に透明に上書き消去するためのインスタンス
                let punchout_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    // アルファを 1.0 で出力させることで、Destination Out ブレンドが
                    // 反応して背景アルファを完全に 0.0 にくり抜くようになります。
                    color: Color::WHITE, // 白（アルファ減算用）
                    corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO), // 角丸に沿ってくり抜く
                    border_width: EdgeInsets::ZERO, // くり抜き時は枠線は不要
                    border_color: Color::TRANSPARENT,
                    border_lengths: EdgeInsets::ZERO,
                    opacity_mode_sizing: [punchout_opacity, 0.0, 0.0, 0.0],
                    uv_max: [0.0; 2],
                    uv_min: [0.0; 2],
                    gradient_end_color: Color::TRANSPARENT,
                    gradient_angle: 0.0,
                    _padding: 0.0,
                    shadow_color: Color::TRANSPARENT,
                    shadow_params: [0.0; 4],
                    outline_color: Color::TRANSPARENT,
                    outline_lengths: EdgeInsets::ZERO,
                    outline_width: EdgeInsets::ZERO,
                    outline_offset_and_flags: [0.0; 4],
                };
                current_instances.push(punchout_instance);
                current_ids.push(id);

                // くり抜き用のバッチとして即座にフラッシュ
                batches.push(DrawBatch {
                    scissor_rect: clip,
                    instances: std::mem::take(&mut current_instances),
                    entity_ids: std::mem::take(&mut current_ids),
                    batch_type: BatchType::Punchout, // ★くり抜き用パイプラインを指示
                });

                // 3. 次に「前面装飾（通常）」用のインスタンスを作成して登録
                // (くり抜かれた透明の窓の上に、枠線、角丸のアウトライン、影などをブレンド描画する)
                let border_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    color: Color::TRANSPARENT, // 背景は透明
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
                    uv_max: [0.0; 2],
                    uv_min: [0.0; 2],
                    gradient_end_color: Color::TRANSPARENT,
                    gradient_angle: 0.0,
                    _padding: 0.0,
                    shadow_color: Color::WHITE,
                    shadow_params: [0.0; 4],
                    outline_width: o_width,
                    outline_color: o_color,
                    outline_lengths: o_lengths,
                    outline_offset_and_flags,
                };
                current_instances.push(border_instance);
                current_ids.push(id);

                current_batch_type = BatchType::Normal; // 以降はまた通常バッチに戻す
                last_clip = Some(clip);
                continue;
            }

            // 非アクティブな WebView2（静止キャッシュ画像）のバッチ隔離
            // 一般要素と絶対にバッチを混在させないことで、テクスチャ（アトラス）の相互汚染を100%防止します。
            let is_webview_static = is_webview && !is_webview_ready;

            if is_webview_static {
                // 1. 現在溜まっている一般UIインスタンスがあれば一度ここで強制フラッシュ
                if !current_instances.is_empty() {
                    batches.push(DrawBatch {
                        scissor_rect: last_clip.unwrap_or(LayoutRect::ZERO),
                        instances: std::mem::take(&mut current_instances),
                        entity_ids: std::mem::take(&mut current_ids),
                        batch_type: current_batch_type,
                    });
                }

                let origin = visual
                    .transform_origin
                    .map(|p| [p.x, p.y])
                    .unwrap_or([0.5, 0.5]);

                let full_transform = visual.transform.unwrap_or(IDENTITY_MATRIX);
                let packed_transform = [
                    full_transform[0], // X軸基底
                    full_transform[1], // Y軸基底
                    full_transform[3], // 平行移動部
                ];

                let static_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    color: Color::TRANSPARENT,
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
                    uv_max: [0.0; 2],
                    uv_min: [0.0; 2],
                    gradient_end_color: Color::TRANSPARENT,
                    gradient_angle: 0.0,
                    _padding: 0.0,
                    shadow_color: Color::TRANSPARENT,
                    shadow_params: [0.0; 4],
                    outline_color: Color::TRANSPARENT,
                    outline_lengths: EdgeInsets::ZERO,
                    outline_width: EdgeInsets::ZERO,
                    outline_offset_and_flags: [0.0; 4],
                };
                current_instances.push(static_instance);
                current_ids.push(id);

                batches.push(DrawBatch {
                    scissor_rect: clip,
                    instances: std::mem::take(&mut current_instances),
                    entity_ids: std::mem::take(&mut current_ids),
                    batch_type: BatchType::Normal, // 静止キャッシュ表示用通常描画
                });

                let border_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    color: Color::TRANSPARENT,
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
                    uv_max: [0.0; 2],
                    uv_min: [0.0; 2],
                    gradient_end_color: Color::TRANSPARENT,
                    gradient_angle: 0.0,
                    _padding: 0.0,
                    shadow_color: Color::WHITE,
                    shadow_params: [0.0; 4],
                    outline_width: o_width,
                    outline_color: o_color,
                    outline_lengths: o_lengths,
                    outline_offset_and_flags,
                };
                current_instances.push(border_instance);
                current_ids.push(id);

                batches.push(DrawBatch {
                    scissor_rect: clip,
                    instances: std::mem::take(&mut current_instances),
                    entity_ids: std::mem::take(&mut current_ids),
                    batch_type: BatchType::Normal,
                });

                last_clip = Some(clip);
                continue;
            }

            // 初回の初期化を安全にキャッチし、異なるクリップ境界の時に新しいバッチを作成する
            if let Some(prev_clip) = last_clip {
                if clip != prev_clip {
                    if !current_instances.is_empty() {
                        let batch = DrawBatch {
                            scissor_rect: prev_clip,
                            instances: std::mem::take(&mut current_instances),
                            entity_ids: std::mem::take(&mut current_ids),
                            batch_type: current_batch_type,
                        };
                        // 構築したバッチを確実にプッシュ（フラッシュバグの修正）
                        batches.push(batch);
                    }
                    last_clip = Some(clip);
                }
            } else {
                // 最初の要素（root）の時点で、確実にその要素のクリップ矩形で初期化します
                last_clip = Some(clip);
            }

            // 選択ハイライト背景のwgpu側への差し込み
            // キャッシュされた選択背景矩形群を描画
            if let Some(rects) = self.outputs.selected_rects.get(id) {
                let border = self.get_physical_border(id, &basic);
                let padding = self.get_physical_padding(id, &basic);

                let sel_bg = visual
                    .select_bg_color
                    .unwrap_or(Color::rgba_f32(0.0, 0.47, 0.84, 0.35));

                // スクロールオフセット
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

                    let full_transform = visual.transform.unwrap_or(IDENTITY_MATRIX);
                    let packed_transform = [
                        full_transform[0], // X軸基底
                        full_transform[1], // Y軸基底
                        full_transform[3], // 平行移動部
                    ];

                    let sel_instance = QuadInstance {
                        rect: sel_rect,
                        transform: packed_transform,
                        transform_origin: [0.5, 0.5],
                        color: sel_bg,
                        corner_radius: CornerRadius::ZERO,
                        border_width: EdgeInsets::ZERO,
                        border_color: Color::TRANSPARENT,
                        border_lengths: EdgeInsets::ZERO,
                        // mode = -1.0（デコレーター上書きをバイパス）
                        opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), -1.0, 0.0, 0.0],
                        uv_max: [0.0; 2],
                        uv_min: [0.0; 2],
                        gradient_end_color: Color::TRANSPARENT,
                        gradient_angle: 0.0,
                        _padding: 0.0,
                        shadow_color: Color::TRANSPARENT,
                        shadow_params: [0.0; 4],
                        outline_color: Color::TRANSPARENT,
                        outline_lengths: EdgeInsets::ZERO,
                        outline_width: EdgeInsets::ZERO,
                        outline_offset_and_flags: [0.0; 4],
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

            // テキスト要素かつ背景・枠線・影などを持つ場合、まず背景用のインスタンスを先に差し込む
            if is_text && has_bg {
                let bg_color = visual.bg_color.unwrap_or(Color::TRANSPARENT);
                let (gradient_end_color, gradient_angle, bg_mode) = match visual.bg_gradient {
                    Some(g) => (g.end_color, g.angle, 1.0f32),
                    None => (bg_color, 0.0, 0.0f32), // mode = -1.0 (装飾モードとしてテキストをバイパス)
                };

                let full_transform = visual.transform.unwrap_or(IDENTITY_MATRIX);
                let packed_transform = [
                    full_transform[0], // X軸基底
                    full_transform[1], // Y軸基底
                    full_transform[3], // 平行移動部
                ];

                let bg_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: visual
                        .transform_origin
                        .map(|p| [p.x, p.y])
                        .unwrap_or([0.5, 0.5]),
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
                    uv_max: [0.0; 2],
                    uv_min: [0.0; 2],
                    gradient_end_color,
                    gradient_angle,
                    _padding: 0.0,
                    shadow_color: Color::WHITE,
                    shadow_params: [0.0; 4],
                    outline_width: o_width,
                    outline_color: o_color,
                    outline_lengths: o_lengths,
                    outline_offset_and_flags,
                };
                current_instances.push(bg_instance);
                current_ids.push(id);
            }

            // 以下、通常のテキスト/背景描画を重ねる（選択矩形が文字の下に）
            // テキスト要素の場合は「テキストの色」、それ以外は「背景色」を color にセットする
            let color = if self.topology.active_masks[id].has(COMP_TEXT_CONTENT) {
                visual.text_color.unwrap_or(Color::BLACK)
            } else {
                visual.bg_color.unwrap_or(Color::TRANSPARENT)
            };

            // グラデーションの解決 (WebView2 がアクティブな場合は、グラデーションもキャンセルして透明化)
            let (gradient_end_color, gradient_angle, mode) = match visual.bg_gradient {
                Some(g) => (g.end_color, g.angle, 1.0f32),
                None => (color, 0.0, 0.0f32),
            };

            // テキスト要素で背景を分離描画した場合、テキストレイヤー側の枠線は不要
            let border_width = if is_text && has_bg {
                EdgeInsets::ZERO
            } else {
                EdgeInsets {
                    top: basic.border.top.into(),
                    right: basic.border.right.into(),
                    bottom: basic.border.bottom.into(),
                    left: basic.border.left.into(),
                }
            };

            let border_lengths = if is_text && has_bg {
                EdgeInsets::ZERO
            } else {
                visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0))
            };

            let border_color = if is_text && has_bg {
                Color::TRANSPARENT
            } else {
                visual.border_color.unwrap_or(Color::TRANSPARENT)
            };

            // テキスト要素で背景を分離描画した場合、テキストレイヤー側の影（BoxShadow）は不要
            let shadow_color = if is_text && has_bg {
                Color::TRANSPARENT // wgpu_renderer側で影を完全無効化させます
            } else {
                Color::WHITE
            };

            let origin = visual
                .transform_origin
                .map(|p| [p.x, p.y])
                .unwrap_or([0.5, 0.5]); // デフォルトは中心

            let full_transform = visual.transform.unwrap_or(IDENTITY_MATRIX);
            let packed_transform = [
                full_transform[0], // X軸基底
                full_transform[1], // Y軸基底
                full_transform[3], // 平行移動部
            ];

            // SoAからGPU用インスタンスデータへ変換
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
                uv_max: [0.0; 2],
                uv_min: [0.0; 2],
                gradient_end_color,
                gradient_angle,
                _padding: 0.0,
                shadow_color,
                shadow_params: [0.0; 4],
                outline_width: if is_text && has_bg {
                    EdgeInsets::ZERO
                } else {
                    o_width
                },
                outline_color: if is_text && has_bg {
                    Color::TRANSPARENT
                } else {
                    o_color
                },
                outline_lengths: if is_text && has_bg {
                    EdgeInsets::ZERO
                } else {
                    o_lengths
                },
                outline_offset_and_flags: if is_text && has_bg {
                    [0.0; 4]
                } else {
                    outline_offset_and_flags
                },
            };

            current_instances.push(instance);
            current_ids.push(id);

            let is_input = self.topology.active_masks[id].has(COMP_INPUT_CONTENT);
            let is_focused = self.events.interaction_states.focused == Some(id);

            if is_input
                && is_focused
                && let Some(contents) = self.contents.input_contents.get(id)
            {
                let now_instant = Instant::now();
                // 現在のミリ秒から点滅周期を自動計算
                let show_caret = if let Some(last) = contents.last_interacted_time
                    && now_instant.duration_since(last) < Duration::from_millis(300)
                {
                    true // キー入力や移動の操作から 500ms 未満のときは、点滅させずに常時表示
                } else if contents.is_blink {
                    let freq = contents
                        .blink_frequency
                        .unwrap_or(Duration::from_millis(530)) // Windows標準
                        .as_millis();
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                    (now / freq).is_multiple_of(2)
                } else {
                    contents.has_caret
                };

                if show_caret {
                    let visual = self
                        .renders
                        .visual_properties
                        .get(id)
                        .unwrap_or(&default_visual);
                    let font_size = visual.font_size.unwrap_or(16.0);

                    let border = self.get_physical_border(id, &basic);
                    let padding = self.get_physical_padding(id, &basic);

                    let scale = self.window.scale_factor;

                    // スクロールオフセットを取得
                    let scroll = self
                        .outputs
                        .scroll_offsets
                        .get(id)
                        .copied()
                        .unwrap_or(LayoutPoint::ZERO);

                    // 1. X座標をDPIスケーリング後の物理ピクセルグリッドに完全にスナップ
                    let logical_x =
                        rect.x + border.left + padding.left + contents.measured_caret_x - scroll.x;
                    let aligned_x = (logical_x * scale).round() / scale;

                    let line_height = contents.caret_line_height;
                    let caret_width = contents.caret_width.unwrap_or(1.5);

                    // キャレット高さを、明示指定された縮小サイズにするか、
                    // デフォルトでは行高全体の85%（文字のインク境界に完璧に一致する高さ）に設定
                    // let caret_height = contents.caret_height.unwrap_or(line_height * 0.85);
                    // 修正: デフォルトでは行高全体の100%に設定
                    let caret_height = contents.caret_height.unwrap_or(line_height);

                    // 2. キャレットサイズ縮小時も、行に対して「垂直中央配置」されるよう動的オフセットを算出
                    // let vertical_center_offset = (line_height - caret_height) * 0.5;
                    // 修正: キャレットサイズをユーザーが個別縮小指定した時のみ、垂直中央配置用のオフセットを適用
                    let vertical_center_offset = if contents.caret_height.is_some() {
                        (line_height - caret_height) * 0.5
                    } else {
                        0.0
                    };

                    let logical_y = rect.y
                        + border.top
                        + padding.top
                        + contents.measured_caret_y
                        + contents.caret_offset
                        - scroll.y;

                    let aligned_y = ((logical_y + vertical_center_offset) * scale).round() / scale;
                    let aligned_width = (caret_width * scale).round().max(1.0) / scale;
                    let aligned_height = (caret_height * scale).round().max(1.0) / scale;

                    let caret_rect =
                        LayoutRect::new(aligned_x, aligned_y, aligned_width, aligned_height);

                    let c_color = contents
                        .caret_color
                        .or(visual.text_color)
                        .unwrap_or(Color::WHITE);
                    let visual = self
                        .renders
                        .visual_properties
                        .get(id)
                        .unwrap_or(&default_visual);

                    // 通常の Solid 矩形 (mode == 0.0) としてキャレット Quad を最前面に配置
                    let caret_instance = QuadInstance {
                        rect: caret_rect,
                        transform: packed_transform,
                        transform_origin: [0.5, 0.5],
                        color: c_color,
                        corner_radius: CornerRadius::ZERO,
                        border_width: EdgeInsets::ZERO,
                        border_color: Color::TRANSPARENT,
                        border_lengths: EdgeInsets::ZERO,
                        // 前面装飾スキップ
                        opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), -1.0, 0.0, 0.0],
                        uv_max: [0.0; 2],
                        uv_min: [0.0; 2],
                        gradient_end_color: Color::TRANSPARENT,
                        gradient_angle: 0.0,
                        _padding: 0.0,
                        shadow_color: Color::TRANSPARENT,
                        shadow_params: [0.0; 4],
                        outline_width: EdgeInsets::ZERO,
                        outline_color: Color::TRANSPARENT,
                        outline_lengths: EdgeInsets::ZERO,
                        outline_offset_and_flags: [0.0; 4],
                    };

                    current_instances.push(caret_instance);
                    current_ids.push(id);
                }
            }
        }

        // 走査終了後、最後に残ったバッチをフラッシュ
        if !current_instances.is_empty() {
            batches.push(DrawBatch {
                scissor_rect: last_clip.unwrap_or(LayoutRect::ZERO),
                instances: current_instances,
                entity_ids: current_ids,
                batch_type: current_batch_type,
            });
        }

        RenderData { batches }
    }

    /// 現在、アクティブに動いているトランジション（wgpuアニメーション）があるか判定します
    pub fn has_active_animations(&self) -> bool {
        // ドラッグ選択中でポインタが可視境界外にある場合も継続
        let has_drag_autoscroll =
            OutputStore::is_drag_autoscroll_active(&self.events, &self.outputs, &self.renders);

        RenderStore::has_active_animations(
            &self.renders,
            &self.events,
            &self.layouts,
            &self.contents,
            has_drag_autoscroll,
        )
    }

    /// 毎フレーム呼び出され、ドラッグ選択中の要素に対するオートスクロールを自律駆動します。
    /// ウィンドウメッセージループ等、 tick_transitions() を呼び出している箇所と同じ周期で実行する。
    pub fn tick_drag_autoscroll(&mut self) {
        let mut autoscroll_occurred = false;
        let mut active_pos = None;

        if let Some(pressed_id) = self.events.interaction_states.pressed
            && let Some(pointer_pos) = self.events.current_pointer_position
            && let Some(clip) = self.outputs.clip_rects.get(pressed_id).copied()
        {
            let user_select = self
                .renders
                .visual_properties
                .get(pressed_id)
                .and_then(|v| v.user_select)
                .unwrap_or(UserSelect::None);

            if user_select == UserSelect::Text {
                let mut dx = 0.0f32;
                let mut dy = 0.0f32;

                // はみ出し距離
                if pointer_pos.x < clip.x {
                    dx = pointer_pos.x - clip.x; // 左はみ出し：負値
                } else if pointer_pos.x > clip.x + clip.width {
                    dx = pointer_pos.x - (clip.x + clip.width); // 右はみ出し：正値
                }

                if pointer_pos.y < clip.y {
                    dy = pointer_pos.y - clip.y;
                } else if pointer_pos.y > clip.y + clip.height {
                    dy = pointer_pos.y - (clip.y + clip.height);
                }

                // はみ出しがある場合、距離に比例したオートスクロールを実行
                if dx.abs() > 1.0 || dy.abs() > 1.0 {
                    // TODO: スクロール感度調整用メソッドを実装。
                    let speed_factor = 0.15f32;
                    let scroll_dx = dx * speed_factor;
                    let scroll_dy = dy * speed_factor;

                    if self.scroll_by(pressed_id, scroll_dx, scroll_dy) {
                        autoscroll_occurred = true;
                        active_pos = Some(pointer_pos);
                    }
                }
            }
        }

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

    /// 毎フレームの描画前に呼び出され、すべてのアクティブなトランジションを 1 Tick 進めます
    pub fn tick_transitions(&mut self) {
        let now = Instant::now();

        // (1.0 / 120.0 秒 = 約 8,333,333 ナノ秒)
        const FRAME_TIME_120FPS: Duration = Duration::from_nanos(8_333_333);
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

        for (id, transitions) in active_map.iter_mut() {
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

    /// 必要に応じてトランジションを起動、または上書き（逆再生含む）します
    pub(crate) fn trigger_transition_if_needed(
        &mut self,
        id: EntityId,
        property_list: PropertyList,
        start_value: TransitionValue,
        end_value: TransitionValue,
    ) -> bool {
        // スタイルの再評価エフェクトの実行中であるか
        let is_style_evaluating = crate::signal::ACTIVE_EFFECT.with(|cell| {
            if let Some(effect_id) = cell.get() {
                // 現在走っているエフェクトがいずれかの要素の StyleCategory::Style のものであるか走査
                self.reactive.element_effects.values().any(|list| {
                    list.iter()
                        .any(|(cat, eff_id)| *eff_id == effect_id && *cat == EffectCategory::Style)
                })
            } else {
                false
            }
        });

        // スタイルエフェクト評価中であればトランジションの開始を完全拒否して値の即時書き換え
        if is_style_evaluating {
            return false;
        }

        // 1. その要素に、このプロパティに対するトランジション設定が定義されているか検証
        if let Some(visual) = self.renders.base_visual_properties.get(id) {
            // transitions ベクタの中から、一致する PropertyList を探す
            if let Some(t) = visual
                .transitions
                .iter()
                .find(|t| t.property_list == property_list || t.property_list == PropertyList::Size)
            {
                let now = Instant::now();
                if let Some(entry) = self.renders.active_transitions.entry(id) {
                    let active_list = entry.or_insert_with(Vec::new);

                    // 2. 割り込み処理の解決（すでに同じプロパティのアニメーションが走っているか）
                    let actual_start = if let Some(existing) = active_list
                        .iter_mut()
                        .find(|et| et.property_list == property_list)
                    {
                        // すでに同じ目的地に向かってアニメーション中の場合は、
                        // 割り込みを一切行わず、そのまま既存アニメーションを走らせる
                        if existing.end_value == end_value {
                            return true;
                        }

                        // すでに駆動中の場合は、その現在の補間位置をリアルタイム計算する
                        // ※ start_time が None の場合（登録されたが一度も tick されていない場合）は
                        // 経過時間 0 として進捗 progress を 0.0 にする
                        let elapsed = existing
                            .start_time
                            .map(|st| now.duration_since(st))
                            .unwrap_or(Duration::ZERO);
                        let progress =
                            (elapsed.as_secs_f32() / existing.duration.as_secs_f32()).min(1.0);
                        let eased_t = existing.curve.evaluate(progress);

                        // 中間位置の算出（これが新しいアニメーションの開始点になる）
                        let current_interposed_val =
                            existing.start_value.lerp(&existing.end_value, eased_t);

                        // 既存のアニメーション状態をリセットし、現在地点から新しい目標値（end_value）へ向かうように上書き
                        existing.start_time = None;
                        existing.start_value = current_interposed_val;
                        existing.end_value = end_value;
                        existing.duration = t.duration;
                        existing.curve = t.curve;

                        return true; // 既存のアニメーションを上書き更新したため即時復帰
                    } else {
                        // 新規開始の場合は、渡された現在の開始値をそのまま採用
                        start_value
                    };

                    // 3. 新規トランジションをアクティブリストに登録
                    active_list.push(ActiveTransition {
                        property_list,
                        start_time: None,
                        duration: t.duration,
                        curve: t.curve,
                        start_value: actual_start,
                        end_value,
                    });

                    return true; // トランジションを正常に起動
                }
            }
        }
        false // トランジション設定がなかったため、即時適用パスへ
    }

    /// 状態の変更を検知し、アニメーション（トランジション）が必要な箇所を自動的に開始・制御します。
    pub(crate) fn resolve_element_style_state(&mut self, id: EntityId, allow_transition: bool) {
        let active_mask = self.topology.active_masks[id];

        // ビジュアルプロパティ (bg_color, opacity等) の解決
        let has_base_visual = self.renders.base_visual_properties.contains_key(id);
        let has_active_visual = self.renders.visual_properties.contains_key(id);

        // 要素がホバーやプレス時の動的スタイルを登録しているか
        let has_interaction_styles = self.renders.interaction_properties.contains_key(id);

        // スタイルを一切持たない要素は、ヒープアロケーションを避けるため完全にスキップ
        // 静的なベース装飾がなくても、ホバースタイル等を持っていれば確実にカスケード解決を通す
        if has_base_visual || has_active_visual || has_interaction_styles {
            let current = self.get_current_style(id);
            let mut target = self.get_target_style(id);

            // 自身のフォーカススタイルが無い場合、親先祖要素が自身のために定義している focused スタイルを抽出
            let focus_style_resolved = self.resolv_focus_style(id, &active_mask);

            // 疑似クラス（Hovered等）のマージ
            self.cascade_interaction(id, active_mask, &mut target, focus_style_resolved);
            self.cascade_within_interaction(id, active_mask, &mut target);

            // プレースホルダー表示状態
            let mut is_placeholder_active = false;
            if let Some(contents) = self.contents.input_contents.get(id) {
                // 文字列が空、かつ IME 変換中でない場合はプレースホルダーと判定
                let has_no_ime = contents
                    .ime_state
                    .as_ref()
                    .map(|s| s.composition_text.is_empty())
                    .unwrap_or(true);
                if contents.text.0.get().is_empty() && has_no_ime {
                    is_placeholder_active = true;
                }
            }

            // 各プロパティの即時適用の変更を評価
            let target_bg_val = target.bg_color.unwrap_or(Color::TRANSPARENT);
            let bg_changed = current.bg_color != target_bg_val;

            let target_border_val = target.border_color.unwrap_or(Color::TRANSPARENT);
            let border_changed = current.border_color != target_border_val;

            let target_outline_width_val = target.outline_width.unwrap_or(EdgeInsets::ZERO);
            let outline_width_changed = current.outline_width != target_outline_width_val;

            let target_outline_color_val = target.outline_color.unwrap_or(Color::TRANSPARENT);
            let outline_color_changed = current.outline_color != target_outline_color_val;

            let target_outline_offset_val = target.outline_offset.unwrap_or(0.0);
            let outline_offset_changed =
                (current.outline_offset - target_outline_offset_val).abs() > 0.001;

            let target_opacity_val = target.opacity.unwrap_or(1.0);
            let opacity_changed = (current.opacity - target_opacity_val).abs() > 0.001;

            let target_transform_val = target.transform.unwrap_or(IDENTITY_MATRIX);
            let transform_changed = current.transform != target_transform_val;

            let target_radius_val = target.corner_radius.unwrap_or(CornerRadius::ZERO);
            let radius_changed = current.corner_radius != target_radius_val;

            let target_shadow_val = target.shadow_params.unwrap_or(BoxShadow::none());
            let shadow_changed = current.shadow_params != target_shadow_val;

            // トランジション判定 (変更がある場合のみトリガー)
            let mut bg_triggered = false;
            if allow_transition && bg_changed && has_active_visual {
                bg_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::BackgroundColor,
                    TransitionValue::Color(current.bg_color),
                    TransitionValue::Color(target_bg_val),
                );
            }

            let mut border_triggered = false;
            if border_changed && has_active_visual {
                border_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::BorderColor,
                    TransitionValue::Color(current.border_color),
                    TransitionValue::Color(target_border_val),
                );
            }

            let mut opacity_triggered = false;
            if opacity_changed && has_active_visual {
                opacity_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::Opacity,
                    TransitionValue::Opacity(current.opacity),
                    TransitionValue::Opacity(target_opacity_val),
                );
            }

            let mut transform_triggered = false;
            if transform_changed {
                transform_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::Transform,
                    TransitionValue::Transform(current.transform),
                    TransitionValue::Transform(target_transform_val),
                );
            }

            let mut radius_triggered = false;
            if radius_changed && has_active_visual {
                radius_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::CornerRadius,
                    TransitionValue::CornerRadius(current.corner_radius),
                    TransitionValue::CornerRadius(target_radius_val),
                );
            }

            let mut shadow_triggered = false;
            if shadow_changed && has_active_visual {
                shadow_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::BoxShadow,
                    TransitionValue::BoxShadow(current.shadow_params),
                    TransitionValue::BoxShadow(target_shadow_val),
                );
            }

            // アニメーションが起動した、または明示的にベースの描画プロパティがある場合のみ
            // 遅延評価（Lazy）でマップを確保し、書き込みを行う
            if bg_triggered
                || border_triggered
                || opacity_triggered
                || transform_triggered
                || radius_triggered
                || shadow_triggered
                || bg_changed
                || border_changed
                || opacity_changed
                || transform_changed
                || radius_changed
                || shadow_changed
                || outline_width_changed
                || outline_color_changed
                || outline_offset_changed
                || self.renders.base_visual_properties.contains_key(id)
            {
                if !self.renders.visual_properties.contains_key(id) {
                    self.renders
                        .visual_properties
                        .insert(id, Default::default());
                }
                let active_vis = self.renders.visual_properties.get_mut(id).unwrap();

                if !bg_triggered {
                    active_vis.bg_color = target.bg_color;
                }
                if !border_triggered {
                    active_vis.border_color = target.border_color;
                }
                if !opacity_triggered {
                    active_vis.opacity = target.opacity;
                }
                if !transform_triggered {
                    active_vis.transform = target.transform;
                }
                if !radius_triggered {
                    active_vis.corner_radius = target.corner_radius;
                }
                if is_placeholder_active {
                    // プレースホルダー時はフォーカスに関わらず、強制的に半透明の薄いグレー
                    active_vis.text_color = Some(Color::rgb_f32(0.5, 0.5, 0.5));
                } else {
                    active_vis.text_color = target.text_color; // 通常時、または疑似状態（Hover等）のテキストカラー
                }

                // 解決した影（target_shadow）をアクティブプロパティに代入
                // アニメーション非起動時のみ行うように修正
                if !shadow_triggered {
                    active_vis.shadow_params = target.shadow_params;
                    active_vis.shadow_color = target.shadow_color;
                }

                active_vis.border_lengths = target.border_lengths;
                active_vis.border_styles = target.border_styles;
                active_vis.border_alignments = target.border_alignments;

                active_vis.outline_width = target.outline_width;
                active_vis.outline_color = target.outline_color;
                active_vis.outline_lengths = target.outline_lengths;
                active_vis.outline_styles = target.outline_styles;
                active_vis.outline_alignments = target.outline_alignments;
                active_vis.outline_offset = target.outline_offset;

                // 解決された選択色をアクティブビジュアルに代入
                active_vis.select_bg_color = target.select_bg_color;
                active_vis.select_text_color = target.select_text_color;
                // 常に即時解決する静的プロパティ群
                active_vis.user_select = self
                    .renders
                    .base_visual_properties
                    .get(id)
                    .and_then(|v| v.user_select);

                active_vis.cursor = target.cursor;
                active_vis.resizable_cursor = target.resizable_cursor;

                // コールドプロパティの即時代入
                if let Some(target_vis) = self.renders.base_visual_properties.get(id) {
                    active_vis.z_index = target_vis.z_index;
                    active_vis.backdrop = target_vis.backdrop;
                    active_vis.font_size = target_vis.font_size;
                    active_vis.font_family = target_vis.font_family.clone();
                    active_vis.font_weight = target_vis.font_weight;
                    active_vis.font_style = target_vis.font_style;
                    active_vis.bg_gradient = target_vis.bg_gradient;
                    active_vis.pointer_events = target_vis.pointer_events;
                    active_vis.transitions = target_vis.transitions.clone();
                    active_vis.keyframe_animations = target_vis.keyframe_animations.clone();
                    active_vis.focusable = target_vis.focusable;
                }

                // 即時変更があったため、レンダラーへの転送 Dirty をマーク
                self.mark_render_dirty(id);
            }
        }

        //  (Width, Height) の解決
        let has_base_layout = self.renders.base_basic_layouts.contains_key(id);
        let has_active_layout = self.layouts.basic_layouts.contains_key(id);

        // レイアウト変更のない要素は完全にスキップ
        if has_base_layout || has_active_layout {
            let active_layout = self
                .layouts
                .basic_layouts
                .get(id)
                .cloned()
                .unwrap_or_default();

            // BasicLayout は heap allocation を持たないフラットな構造（Copy同等）なので
            // cloned() によるクローンは極めて低コスト
            let base_layout = self
                .renders
                .base_basic_layouts
                .get(id)
                .cloned()
                .unwrap_or_default();
            let mut target_layout = base_layout;

            self.cascade_basic_layout(id, active_mask, &mut target_layout);

            // 単位を親/ウィンドウアラインメントを考慮した物理ピクセル(f32)へ解決
            let target_w_px = self.resolve_val_to_px(id, target_layout.size.width, true);
            let current_w_px = self.resolve_val_to_px(id, active_layout.size.width, true);
            let target_h_px = self.resolve_val_to_px(id, target_layout.size.height, false);
            let current_h_px = self.resolve_val_to_px(id, active_layout.size.height, false);

            let mut width_triggered = false;
            let mut height_triggered = false;

            // Width または 一括 Size トランジション設定が定義されているか検証
            let can_trigger_width = self
                .renders
                .visual_properties
                .get(id)
                .map(|v| {
                    v.transitions.iter().any(|t| {
                        t.property_list == PropertyList::Width
                            || t.property_list == PropertyList::Size
                    })
                })
                .unwrap_or(false);

            if allow_transition
                && can_trigger_width
                && has_active_layout
                && let (Some(cw), Some(tw)) = (current_w_px, target_w_px)
                && (cw - tw).abs() > 0.01
            // 浮動小数点誤差を無視
            {
                width_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::Width,
                    TransitionValue::Width(cw),
                    TransitionValue::Width(tw),
                );
            }

            // Height または 一括 Size トランジション設定が定義されているか検証
            let can_trigger_height = self
                .renders
                .visual_properties
                .get(id)
                .map(|v| {
                    v.transitions.iter().any(|t| {
                        t.property_list == PropertyList::Height
                            || t.property_list == PropertyList::Size
                    })
                })
                .unwrap_or(false);

            if allow_transition
                && can_trigger_height
                && has_active_layout
                && let (Some(ch), Some(th)) = (current_h_px, target_h_px)
                && (ch - th).abs() > 0.01
            {
                height_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::Height,
                    TransitionValue::Height(ch),
                    TransitionValue::Height(th),
                );
            }

            // 遅延マウント
            if !self.layouts.basic_layouts.contains_key(id) {
                self.layouts.basic_layouts.insert(id, Default::default());
            }
            let active_layout_mut = self.layouts.basic_layouts.get_mut(id).unwrap();
            *active_layout_mut = target_layout;

            if width_triggered {
                active_layout_mut.size.width = Val::Px(current_w_px.unwrap());
            }
            if height_triggered {
                active_layout_mut.size.height = Val::Px(current_h_px.unwrap());
            }

            // 最終的に解決されたレイアウトを Taffy ツリーに即時同期させるため、
            // スタイル解決の末尾でレイアウトの Dirty マークを叩きます
            self.mark_layout_dirty(id);
        }

        // スタイル解決が完了した結果、自身に新しくキーフレームアニメーション定義が
        // 読み込まれていれば、自動的にそのアニメーションの再生を開始する
        self.trigger_keyframe_animations_if_needed(id);

        if let Some(effects) = self.reactive.element_effects.get(id) {
            let text_effects: Vec<EffectId> = effects
                .iter()
                .filter(|(cat, _)| *cat == EffectCategory::Text)
                .map(|(_, eff_id)| *eff_id)
                .collect();
            for eff_id in text_effects {
                crate::execute_effect(eff_id);
            }
        }
    }

    /// 毎フレームの描画前に呼び出され、すべてのアクティブなキーフレームアニメーションを 1 Tick 進めます
    pub fn tick_animations(&mut self) {
        let now = Instant::now();

        // 借用チェッカーを回避するため、一時的にマップを take して更新
        let mut active_map = std::mem::take(&mut self.renders.active_animations);
        let mut to_remove = Vec::new();

        for (id, animations) in active_map.iter_mut() {
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

    /// 補間されたアニメーション値を SoA のアクティブプロパティへ安全に上書きします
    fn apply_animation_value(
        &mut self,
        id: EntityId,
        property: PropertyList,
        value: &TransitionValue,
    ) {
        if !self.renders.visual_properties.contains_key(id) {
            self.renders
                .visual_properties
                .insert(id, Default::default());
        }
        let v = self.renders.visual_properties.get_mut(id).unwrap();

        match *value {
            TransitionValue::Color(c) => {
                if property == PropertyList::BackgroundColor {
                    v.bg_color = Some(c);
                } else if property == PropertyList::BorderColor {
                    v.border_color = Some(c);
                }
            }
            TransitionValue::Opacity(o) => {
                v.opacity = Some(o);
            }
            TransitionValue::Transform(m) => {
                v.transform = Some(m);
            }
            TransitionValue::CornerRadius(cr) => {
                v.corner_radius = Some(cr);
            }
            TransitionValue::Width(w) => {
                if let Some(layout) = self.layouts.basic_layouts.get_mut(id) {
                    layout.size.width = Val::Px(w);
                }
                self.mark_layout_dirty(id); // レイアウト再計算を要求（スローパス）
            }
            TransitionValue::Height(h) => {
                if let Some(layout) = self.layouts.basic_layouts.get_mut(id) {
                    layout.size.height = Val::Px(h);
                }
                self.mark_layout_dirty(id);
            }
            TransitionValue::BoxShadow(shadow) => {
                v.shadow_params = Some(shadow);
                v.shadow_color = Some(shadow.color);
            }
        }
    }

    /// マウス座標などが、要素の描画領域かつ表示枠内に収まっているかを判定。
    /// 階層的な早期枝刈りヒットテスト
    pub fn hit_test(&self, point: LayoutPoint) -> Option<EntityId> {
        // 各要素の実効 z_index を、親から子へカスケードして算出
        let mut effective_z_indices =
            SecondaryMap::with_capacity(self.topology.active_entities.len());
        for &id in &self.layouts.flat_dfs_sequence {
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
            if let Some(rect) = self.outputs.rects.get(id)
                && rect.contains(point)
            {
                return Some(id);
            }
        }
        None
    }

    /// 階層的な境界判定ヘルパー（非対象のブランチをまるごとスキップ）
    fn hit_test_recursive(&self, id: EntityId, point: LayoutPoint) -> Option<EntityId> {
        // 1. 親などの overflow: hidden 等でクリップされている表示範囲をチェック
        // クリップ領域外であれば、この要素もそのすべての子孫要素も画面上に見えていないため、走査を即座にスキップ（枝刈り）
        if let Some(clip) = self.outputs.clip_rects.get(id)
            && !clip.contains(point)
        {
            return None;
        }

        // 2. 子要素を逆順（前面優先）で再帰降下
        if let Some(children) = self.topology.children.get(id) {
            let child_len = children.len();
            for i in (0..child_len).rev() {
                let child_id = children[i];
                if let Some(hit) = self.hit_test_recursive(child_id, point) {
                    return Some(hit);
                }
            }
        }

        // pointer_events: none の場合は、自分自身の矩形判定のみをスルーする (子要素は上を辿れるため除外しない)
        // visual_properties (動的) に無ければ base_visual_properties (静的) を見に行く
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

        if pointer_events != PointerEvents::None
            && let Some(rect) = self.outputs.rects.get(id)
            && rect.contains(point)
        {
            return Some(id);
        }

        None
    }

    /// 各インタラクション状態（ステート）を更新し、レイアウト変更を伴うか自動的に判別して Dirty フラグを制御する共通ヘルパー
    #[inline(always)]
    fn update_state(&mut self, id: EntityId, state_flag: u128, active: bool) {
        if let Some(mask) = self.topology.active_masks.get_mut(id) {
            let was_active = mask.has(state_flag);
            if was_active != active {
                // 1. ビットマスクの更新
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

                        // 先祖要素が one of the within スタイルを1つでも持っている場合のみ深く入る
                        if parent_mask.has(STYLE_INTERACTION_WITHIN) {
                            self.resolve_element_style_state(parent_id, true);

                            if self.does_state_require_layout(parent_id, state_flag) {
                                self.mark_layout_dirty(parent_id);
                            } else {
                                self.mark_render_dirty(parent_id);
                            }
                        }
                    }
                    curr = parent_id;
                }

                // 2. 残りの状態遷移イベントの自動トリガー解決
                if active {
                    match state_flag {
                        // 無効化（Disabled）状態が有効になった瞬間
                        STATE_DISABLED => {
                            let mut on_dis = self
                                .events
                                .event_listeners
                                .get_mut(id)
                                .and_then(|l| l.on_disable.take());
                            if let Some(mut handler) = on_dis {
                                let _guard = crate::ActiveElementGuard::new(id);
                                handler(self);
                                if let Some(l) = self.events.event_listeners.get_mut(id) {
                                    l.on_disable = Some(handler);
                                }
                            }
                        }
                        // アクティブ（Actived：STATE_ACTIVED）状態が有効になった瞬間
                        STATE_ACTIVED => {
                            let mut on_act = self
                                .events
                                .event_listeners
                                .get_mut(id)
                                .and_then(|l| l.on_active.take());
                            if let Some(mut handler) = on_act {
                                let _guard = crate::ActiveElementGuard::new(id);
                                handler(self);
                                if let Some(l) = self.events.event_listeners.get_mut(id) {
                                    l.on_active = Some(handler);
                                }
                            }
                        }
                        // セレクト（Selected：STATE_SELECTED）状態が有効になった瞬間
                        STATE_SELECTED => {
                            let mut on_sel = self
                                .events
                                .event_listeners
                                .get_mut(id)
                                .and_then(|l| l.on_select.take());
                            if let Some(mut handler) = on_sel {
                                let _guard = crate::ActiveElementGuard::new(id);
                                handler(self);
                                if let Some(l) = self.events.event_listeners.get_mut(id) {
                                    l.on_select = Some(handler);
                                }
                            }
                        }
                        _ => {}
                    }
                }

                // 3. レイアウト再計算（スローパス）か描画更新（ファストパス）かを自動判定
                if self.does_state_require_layout(id, state_flag) {
                    self.mark_layout_dirty(id);
                } else {
                    self.mark_render_dirty(id);
                }
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

    fn sync_resizing_drag(&mut self, logical_pos: LayoutPoint, state: ResizingState) {
        let id = state.entity_id;
        let delta_x = logical_pos.x - state.start_mouse_pos.x;
        let delta_y = logical_pos.y - state.start_mouse_pos.y;

        let start_rect = state.start_rect;
        let position = self
            .layouts
            .basic_layouts
            .get(id)
            .map(|l| l.position)
            .unwrap_or(Position::Relative);

        // 1-1. 最小サイズ・最大クランプ値の解決
        let (min_w, max_w, min_h, max_h) = {
            let basic = self
                .layouts
                .basic_layouts
                .get(id)
                .copied()
                .unwrap_or_default();

            let ref_w = start_rect.width;
            let ref_h = start_rect.height;

            let b = self.get_physical_border(id, &basic);
            let p = self.get_physical_padding(id, &basic);

            // 枠線と余白を足した、物理的にこれ以上小さくできない限界サイズ
            let abs_min_w = b.left + b.right + p.left + p.right;
            let abs_min_h = b.top + b.bottom + p.top + p.bottom;

            // ユーザー指定の min_size / max_size を物理ピクセルに解決
            let user_min_w = match basic.min_size.width {
                Val::Px(v) => v,
                Val::Percent(_) => self
                    .resolve_val_to_px(id, basic.min_size.width, true)
                    .unwrap_or(0.0),
                Val::Auto => 0.0, // Autoのときは最小値制約なし
            };
            let user_min_h = match basic.min_size.height {
                Val::Px(v) => v,
                Val::Percent(_) => self
                    .resolve_val_to_px(id, basic.min_size.height, false)
                    .unwrap_or(0.0),
                Val::Auto => 0.0,
            };
            let user_max_w = match basic.max_size.width {
                Val::Px(v) => v,
                Val::Percent(_) => self
                    .resolve_val_to_px(id, basic.max_size.width, true)
                    .unwrap_or(f32::MAX),
                Val::Auto => f32::MAX, // Autoのときは最大値制限なし
            };
            let user_max_h = match basic.max_size.height {
                Val::Px(v) => v,
                Val::Percent(_) => self
                    .resolve_val_to_px(id, basic.max_size.height, false)
                    .unwrap_or(f32::MAX),
                Val::Auto => f32::MAX,
            };

            (
                abs_min_w.max(user_min_w).max(10.0), // 最低限 10px は維持
                user_max_w,
                abs_min_h.max(user_min_h).max(10.0),
                user_max_h,
            )
        };

        let mut new_w = start_rect.width;
        let mut new_h = start_rect.height;

        let mut delta_inset_top = 0.0;
        let mut delta_inset_left = 0.0;

        // 配置モードによるサイズ変更と位置補正の切り分け
        if position == Position::Absolute {
            // 絶対配置（Absolute）物理サイズ変更と、Top / Left 引っ張り時の Inset 同期移動補正を行う
            match state.direction {
                ResizeDirection::Right => {
                    new_w = (start_rect.width + delta_x).clamp(min_w, max_w);
                }
                ResizeDirection::Bottom => {
                    new_h = (start_rect.height + delta_y).clamp(min_h, max_h);
                }
                ResizeDirection::BottomRight => {
                    new_w = (start_rect.width + delta_x).clamp(min_w, max_w);
                    new_h = (start_rect.height + delta_y).clamp(min_h, max_h);
                }
                ResizeDirection::Left => {
                    let potential_w = start_rect.width - delta_x;
                    new_w = potential_w.clamp(min_w, max_w);
                    // サイズが大きくなった分（start - new_w）だけ、正確に左端（left）を左（マイナス）へ補正
                    delta_inset_left = start_rect.width - new_w;
                }
                ResizeDirection::Top => {
                    let potential_h = start_rect.height - delta_y;
                    new_h = potential_h.clamp(min_h, max_h);
                    // サイズが大きくなった分だけ、正確に上端（top）を上（マイナス）へ補正
                    delta_inset_top = start_rect.height - new_h;
                }
                ResizeDirection::TopLeft => {
                    let potential_w = start_rect.width - delta_x;
                    new_w = potential_w.clamp(min_w, max_w);
                    delta_inset_left = start_rect.width - new_w;

                    let potential_h = start_rect.height - delta_y;
                    new_h = potential_h.clamp(min_h, max_h);
                    delta_inset_top = start_rect.height - new_h;
                }
                ResizeDirection::TopRight => {
                    new_w = (start_rect.width + delta_x).clamp(min_w, max_w);

                    let potential_h = start_rect.height - delta_y;
                    new_h = potential_h.clamp(min_h, max_h);
                    delta_inset_top = start_rect.height - new_h;
                }
                ResizeDirection::BottomLeft => {
                    let potential_w = start_rect.width - delta_x;
                    new_w = potential_w.clamp(min_w, max_w);
                    delta_inset_left = start_rect.width - new_w;

                    new_h = (start_rect.height + delta_y).clamp(min_h, max_h);
                }
            }
        } else {
            // 相対配置（Relative）フローを崩さないため inset は変更せず、
            // 引っ張る方向（Top/Left時はマイナス乗算）に合わせてサイズ（幅・高さ）のみを増減させる
            match state.direction {
                ResizeDirection::Right | ResizeDirection::Left => {
                    let factor = if state.direction == ResizeDirection::Left {
                        -1.0
                    } else {
                        1.0
                    };
                    new_w = (start_rect.width + delta_x * factor).clamp(min_w, max_w);
                }
                ResizeDirection::Bottom | ResizeDirection::Top => {
                    let factor = if state.direction == ResizeDirection::Top {
                        -1.0
                    } else {
                        1.0
                    };
                    new_h = (start_rect.height + delta_y * factor).clamp(min_h, max_h);
                }
                ResizeDirection::TopLeft => {
                    new_w = (start_rect.width - delta_x).clamp(min_w, max_w);
                    new_h = (start_rect.height - delta_y).clamp(min_h, max_h);
                }
                ResizeDirection::TopRight => {
                    new_w = (start_rect.width + delta_x).clamp(min_w, max_w);
                    new_h = (start_rect.height - delta_y).clamp(min_h, max_h);
                }
                ResizeDirection::BottomLeft => {
                    new_w = (start_rect.width - delta_x).clamp(min_w, max_w);
                    new_h = (start_rect.height + delta_y).clamp(min_h, max_h);
                }
                ResizeDirection::BottomRight => {
                    new_w = (start_rect.width + delta_x).clamp(min_w, max_w);
                    new_h = (start_rect.height + delta_y).clamp(min_h, max_h);
                }
            }
        }

        // 基本サイズ情報を SoA のアクティブレイアウトへ書き込み
        if let Some(layout) = self.layouts.basic_layouts.get_mut(id) {
            layout.size.width = Val::Px(new_w);
            layout.size.height = Val::Px(new_h);

            // 絶対配置時のみ位置を動的に補正し、制約衝突を回避するため right / bottom を Auto 化
            if position == Position::Absolute {
                if let Val::Px(start_top) = state.start_inset.top {
                    layout.inset.top = Val::Px(start_top + delta_inset_top);
                }
                if let Val::Px(start_left) = state.start_inset.left {
                    layout.inset.left = Val::Px(start_left + delta_inset_left);
                }
                layout.inset.right = Val::Auto;
                layout.inset.bottom = Val::Auto;
            }
        }

        // base_basic_layouts にも同時に書き込み、解決処理（resolve）によるリセットを完全に防ぐ
        if let Some(layout) = self.renders.base_basic_layouts.get_mut(id) {
            layout.size.width = Val::Px(new_w);
            layout.size.height = Val::Px(new_h);

            if position == Position::Absolute {
                if let Val::Px(start_top) = state.start_inset.top {
                    layout.inset.top = Val::Px(start_top + delta_inset_top);
                }
                if let Val::Px(start_left) = state.start_inset.left {
                    layout.inset.left = Val::Px(start_left + delta_inset_left);
                }
                layout.inset.right = Val::Auto;
                layout.inset.bottom = Val::Auto;
            }
        }
        // Taffy 測定キャッシュをバイパスし再計算をマーク
        self.mark_layout_dirty(id);
        self.mark_render_dirty(id);
    }

    fn sync_scrollbar_drag(&mut self, logical_pos: LayoutPoint) {
        let mut scrollbar_dragged = false;
        let mut active_drag_target: Option<(EntityId, bool, bool)> = None;

        for (id, state) in self.layouts.scrollbar_styles.iter() {
            if state.v_thumb_dragged {
                active_drag_target = Some((id, true, false));
                break;
            } else if state.h_thumb_dragged {
                active_drag_target = Some((id, false, true));
                break;
            }
        }

        if let Some((current_id, is_vertical, is_horiazon)) = active_drag_target {
            let (sb_state, container_rect, scroll_size) = {
                let sb_state = self
                    .layouts
                    .scrollbar_styles
                    .get(current_id)
                    .cloned()
                    .unwrap();
                let container_rect = self
                    .outputs
                    .rects
                    .get(current_id)
                    .copied()
                    .unwrap_or(LayoutRect::ZERO);
                let scroll_size = self.get_scroll_size(current_id);
                (sb_state, container_rect, scroll_size)
            };

            let visible_size = self.calculate_visible_size(container_rect);

            if is_vertical {
                let track_id = sb_state.v_track_id.unwrap();
                let thumb_id = sb_state.v_thumb_id.unwrap();
                let track_rect = self.outputs.rects[track_id];
                let thumb_rect = self.outputs.rects[thumb_id];

                // サムのマージンを差し引く
                let mut margin_top = 0.0;
                let mut margin_bottom = 0.0;
                if let Some(ref thumb_style) = sb_state.style.v_thumb {
                    if let Val::Px(val) = thumb_style.inner.basic_layout.margin.top {
                        margin_top = val;
                    }
                    if let Val::Px(val) = thumb_style.inner.basic_layout.margin.bottom {
                        margin_bottom = val;
                    }
                }

                // 同期処理と同じく、マージンを含めた実際の有効可動域を正確に計算
                let track_range =
                    track_rect.height - thumb_rect.height - margin_top - margin_bottom;
                if track_range > 0.0 {
                    let dy = logical_pos.y - sb_state.drag_start_mouse.y;
                    let max_scroll_y = scroll_size.height - visible_size.height;

                    if max_scroll_y > 0.0 {
                        let ratio = max_scroll_y / track_range;
                        let target_scroll_y = sb_state.drag_start_offset.y + dy * ratio;

                        let current_x = self
                            .outputs
                            .scroll_offsets
                            .get(current_id)
                            .map(|o| o.x)
                            .unwrap_or(0.0);
                        self.scroll_to(current_id, current_x, target_scroll_y);
                    }
                }
            } else if is_horiazon {
                let track_id = sb_state.h_track_id.unwrap();
                let thumb_id = sb_state.h_thumb_id.unwrap();
                let track_rect = self.outputs.rects[track_id];
                let thumb_rect = self.outputs.rects[thumb_id];

                let mut margin_left = 0.0;
                let mut margin_right = 0.0;
                if let Some(ref thumb_style) = sb_state.style.h_thumb {
                    if let Val::Px(val) = thumb_style.inner.basic_layout.margin.left {
                        margin_left = val;
                    }
                    if let Val::Px(val) = thumb_style.inner.basic_layout.margin.right {
                        margin_right = val;
                    }
                }

                let track_range = track_rect.width - thumb_rect.width - margin_left - margin_right;
                if track_range > 0.0 {
                    let dx = logical_pos.x - sb_state.drag_start_mouse.x;
                    let max_scroll_x = scroll_size.width - visible_size.width;

                    if max_scroll_x > 0.0 {
                        let ratio = max_scroll_x / track_range;
                        let target_scroll_x = sb_state.drag_start_offset.x + dx * ratio;

                        let current_y = self
                            .outputs
                            .scroll_offsets
                            .get(current_id)
                            .map(|o| o.y)
                            .unwrap_or(0.0);
                        self.scroll_to(current_id, target_scroll_x, current_y);
                    }
                }
            }

            self.mark_render_dirty(current_id);
            scrollbar_dragged = true;
        }
    }

    pub fn inject_pointer_move(&mut self, logical_pos: LayoutPoint) {
        let _context_guard = bind_context(self);

        let prev_pos = self.events.current_pointer_position;
        self.events.current_pointer_position = Some(logical_pos);

        // リサイズ中のドラッグ同期処理
        if let Some(state) = self.events.resizing_state.clone() {
            self.sync_resizing_drag(logical_pos, state);
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
        let mut current_id = target_id;
        let mut found_resize_hover = None;

        while let Some(id) = current_id {
            if self.topology.active_masks[id].has(STYLE_RESIZABLE) {
                let rect = self.outputs.rects[id];
                let resizable_flags = self
                    .layouts
                    .basic_layouts
                    .get(id)
                    .map(|l| l.resizable)
                    .unwrap_or([false; 4]);

                // 境界外周に 6.0px のあそびを持たせてヒット判定
                let detect_border = 6.0f32;
                if let Some(dir) = Context::detect_resize_direction(
                    rect,
                    resizable_flags,
                    logical_pos,
                    detect_border,
                ) {
                    found_resize_hover = Some((id, dir));
                    break; // 最も前面寄りのリサイズ親要素を優先採用
                }
            }
            current_id = self.topology.parents.get(id).copied().flatten();
        }

        if let Some((id, dir)) = found_resize_hover {
            self.events.active_resize_hover = Some((id, dir));

            if let Some(vis) = self.renders.visual_properties.get_mut(id) {
                // 要素に resizable_cursor の個別指定があれば、方向に応じて該当カーソルを抽出
                let custom_cursor = if let Some(arr) = vis.resizable_cursor {
                    let idx = match dir {
                        ResizeDirection::Top | ResizeDirection::Bottom => 0, // Ns
                        ResizeDirection::Left | ResizeDirection::Right => 1, // Ew
                        ResizeDirection::TopRight | ResizeDirection::BottomLeft => 2, // Nesw
                        ResizeDirection::TopLeft | ResizeDirection::BottomRight => 3, // Nwse
                    };
                    arr[idx]
                } else {
                    None
                };

                // 独自指定があればそれを使い、無ければライブラリの自動マッピングを使用
                vis.cursor =
                    Some(custom_cursor.unwrap_or_else(|| Context::resize_direction_to_cursor(dir)));
            }
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
            let user_select = self
                .renders
                .visual_properties
                .get(pressed_id)
                .and_then(|v| v.user_select)
                .unwrap_or(UserSelect::None);

            if user_select == UserSelect::Text
                && let Some(start_pos) = self.outputs.selection_start_index.get(pressed_id).copied()
            {
                // プレースホルダー選択のドラッグ遮断
                if let Some(contents) = self.contents.input_contents.get(pressed_id) {
                    let text_val = contents.text.0.get();
                    let is_placeholder = text_val.is_empty()
                        && contents
                            .ime_state
                            .as_ref()
                            .map(|s| s.composition_text.is_empty())
                            .unwrap_or(true);

                    if is_placeholder && !contents.placeholder_select {
                        return;
                    }
                }

                let rect = self.outputs.rects[pressed_id];
                let (basic, _, _) = self.resolve_active_layouts(pressed_id);
                let border = self.get_physical_border(pressed_id, &basic);
                let padding = self.get_physical_padding(pressed_id, &basic);

                let scroll = self
                    .outputs
                    .scroll_offsets
                    .get(pressed_id)
                    .copied()
                    .unwrap_or(LayoutPoint::ZERO);

                let local_x = logical_pos.x - (rect.x + border.left + padding.left) + scroll.x;
                let local_y = logical_pos.y - (rect.y + border.top + padding.top) + scroll.y;

                if let Some(layout) = self.get_or_create_layout(pressed_id) {
                    let (current_index, is_trailing) = self
                        .system
                        .text_engine
                        .hit_test_point(&layout, local_x, local_y);
                    let final_index = if is_trailing {
                        current_index + 1
                    } else {
                        current_index
                    };

                    let range = if start_pos <= final_index {
                        // 順選択（右方向ドラッグ）
                        if let Some(contents) = self.contents.input_contents.get_mut(pressed_id) {
                            contents.selection_reversed = false;
                        }
                        start_pos..final_index
                    } else {
                        // 逆選択（左方向ドラッグ）
                        if let Some(contents) = self.contents.input_contents.get_mut(pressed_id) {
                            contents.selection_reversed = true;
                        }
                        final_index..start_pos
                    };

                    self.outputs
                        .text_selections
                        .insert(pressed_id, range.clone());

                    self.update_selection_rects(pressed_id);

                    if let Some(contents) = self.contents.input_contents.get_mut(pressed_id) {
                        contents.selected_range = range;
                        crate::update_input_caret_position(self, pressed_id);
                    }
                    self.mark_render_dirty(pressed_id);
                }
            }
        }

        // ヒットテスト
        let target_id = self.hit_test(logical_pos);

        // ホバー（Enter/Leave）状態の解決
        if target_id != self.events.interaction_states.hovered {
            // 旧ホバー要素からマウスが去った
            if let Some(old_id) = self.events.interaction_states.hovered {
                self.set_hovered(old_id, false);

                let mut on_leave = self
                    .events
                    .event_listeners
                    .get_mut(old_id)
                    .and_then(|l| l.on_mouse_leave.take());
                if let Some(mut handler) = on_leave {
                    let _guard = crate::ActiveElementGuard::new(old_id);
                    handler(self);
                    if let Some(l) = self.events.event_listeners.get_mut(old_id) {
                        l.on_mouse_leave = Some(handler);
                    }
                }
            }

            // 新ホバー要素にマウスが入った
            if let Some(new_id) = target_id {
                self.set_hovered(new_id, true);

                // on_mouse_enter
                let mut on_enter = self
                    .events
                    .event_listeners
                    .get_mut(new_id)
                    .and_then(|l| l.on_mouse_enter.take());
                if let Some(mut handler) = on_enter {
                    let _guard = crate::ActiveElementGuard::new(new_id);
                    handler(self);
                    if let Some(l) = self.events.event_listeners.get_mut(new_id) {
                        l.on_mouse_enter = Some(handler);
                    }
                }

                // on_hover
                let mut on_hover = self
                    .events
                    .event_listeners
                    .get_mut(new_id)
                    .and_then(|l| l.on_hover.take());
                if let Some(mut handler) = on_hover {
                    let _guard = crate::ActiveElementGuard::new(new_id);
                    handler(self);
                    if let Some(l) = self.events.event_listeners.get_mut(new_id) {
                        l.on_hover = Some(handler);
                    }
                }
            }

            self.events.interaction_states.hovered = target_id;
        }

        // カーソル移動イベントの伝播
        if let Some(target_id) = target_id {
            // on_cursor_moved
            let mut on_move = self
                .events
                .event_listeners
                .get_mut(target_id)
                .and_then(|l| l.on_cursor_moved.take());
            if let Some(mut handler) = on_move {
                let rect = self.outputs.rects[target_id];
                let relative_pos = LayoutPoint::new(logical_pos.x - rect.x, logical_pos.y - rect.y);
                let _guard = crate::ActiveElementGuard::new(target_id);
                handler(self, relative_pos);
                if let Some(l) = self.events.event_listeners.get_mut(target_id) {
                    l.on_cursor_moved = Some(handler);
                }
            }
        }

        // ドラッグイベントの伝播
        if let Some(pressed_id) = self.events.interaction_states.pressed
            && let Some(prev) = prev_pos
        {
            let delta = LayoutPoint::new(logical_pos.x - prev.x, logical_pos.y - prev.y);
            if delta.x != 0.0 || delta.y != 0.0 {
                self.set_dragged(pressed_id, true);
                self.events.interaction_states.dragged = Some(pressed_id);

                // D&D 設定（STYLE_DRAGGABLE）を持っている場合のセッションのキック
                if self.topology.active_masks[pressed_id].has(STYLE_DRAGGABLE)
                    && self.events.active_drag_state.is_none()
                {
                    let drag_prop = self
                        .events
                        .drag_properties
                        .get(pressed_id)
                        .copied()
                        .unwrap();
                    let start_rect = self.outputs.rects[pressed_id];

                    // 開始時のクリック位置と要素左上の相対的なズレを計算
                    let click_offset = LayoutPoint::new(
                        logical_pos.x - start_rect.x,
                        logical_pos.y - start_rect.y,
                    );

                    // ウィンドウの真のルート要素をライブラリ側で自己解決
                    let root_entity = self
                        .find_root_entity()
                        .expect("Root EntityId not found in Context");

                    // プレースホルダーアタッチ先親要素の決定
                    let (parent_id_opt, parent_rect, parent_border_left, parent_border_top) =
                        match drag_prop.placeholder_parent {
                            DragPlaceholderParent::Root => (
                                Some(root_entity),
                                self.outputs
                                    .rects
                                    .get(root_entity)
                                    .copied()
                                    .unwrap_or(LayoutRect::ZERO),
                                0.0,
                                0.0,
                            ),
                            DragPlaceholderParent::Custom(p_id) => {
                                let p_rect = self
                                    .outputs
                                    .rects
                                    .get(p_id)
                                    .copied()
                                    .unwrap_or(LayoutRect::ZERO);
                                let b_l = if let Some(l) = self.layouts.basic_layouts.get(p_id) {
                                    match l.border.left {
                                        Length::Px(v) => v,
                                        _ => 0.0,
                                    }
                                } else {
                                    0.0
                                };
                                let b_t = if let Some(l) = self.layouts.basic_layouts.get(p_id) {
                                    match l.border.top {
                                        Length::Px(v) => v,
                                        _ => 0.0,
                                    }
                                } else {
                                    0.0
                                };
                                (Some(p_id), p_rect, b_l, b_t)
                            }
                        };

                    // プレースホルダー（クローン）をアタッチ先親の直下へ spawn して生成
                    let placeholder_id = self.spawn(parent_id_opt);
                    if let Some(p_id) = parent_id_opt {
                        self.add_child(p_id, placeholder_id);
                    }

                    // 元要素のレイアウトおよびビジュアル情報をコピーして初期マウント
                    if let Some(basic) = self.renders.base_basic_layouts.get(pressed_id).copied() {
                        self.renders
                            .base_basic_layouts
                            .insert(placeholder_id, basic);
                        self.layouts.basic_layouts.insert(placeholder_id, basic);
                    }
                    if let Some(visual) =
                        self.renders.base_visual_properties.get(pressed_id).cloned()
                    {
                        self.renders
                            .base_visual_properties
                            .insert(placeholder_id, visual.clone());
                        self.renders
                            .visual_properties
                            .insert(placeholder_id, visual);
                    }
                    if let Some(interaction) =
                        self.renders.interaction_properties.get(pressed_id).cloned()
                    {
                        self.renders
                            .interaction_properties
                            .insert(placeholder_id, interaction);
                    }

                    // ドラッグ元の元の要素は非可視（または半透明）にするため STATE_DRAGGING 状態をセット
                    self.set_drag_state(pressed_id, STATE_DRAGGING, true);

                    // プレースホルダー側は absolute 配置化し、STATE_DRAG_OVER 状態をセット
                    self.set_drag_state(placeholder_id, STATE_DRAG_OVER, true);
                    if let Some(layout) = self.layouts.basic_layouts.get_mut(placeholder_id) {
                        layout.position = Position::Absolute;
                        layout.size.width = Val::Px(start_rect.width);
                        layout.size.height = Val::Px(start_rect.height);
                    }
                    if let Some(layout) = self.renders.base_basic_layouts.get_mut(placeholder_id) {
                        layout.position = Position::Absolute;
                        layout.size.width = Val::Px(start_rect.width);
                        layout.size.height = Val::Px(start_rect.height);
                    }

                    // 元の要素が持つ本物の子要素トポロジーを、一時的にプレースホルダー配下へ自動アタッチ
                    if let Some(src_children) = self.topology.children.get(pressed_id).cloned() {
                        for child_id in src_children {
                            // 子要素の親ポインタをプレースホルダーに付け替え
                            self.topology.parents.insert(child_id, Some(placeholder_id));

                            // プレースホルダー側の子要素リストへ追加
                            if let Some(ph_children) =
                                self.topology.children.get_mut(placeholder_id)
                            {
                                ph_children.push(child_id);
                            }

                            // Taffy 側の親子構造も、一時的にプレースホルダーに繋ぎ替え
                            if let Some(&src_node) = self.layouts.taffy_nodes.get(pressed_id)
                                && let Some(&ph_node) = self.layouts.taffy_nodes.get(placeholder_id)
                                && let Some(&child_node) = self.layouts.taffy_nodes.get(child_id)
                            {
                                let _ = self.layouts.taffy.remove_child(src_node, child_node);
                                let _ = self.layouts.taffy.add_child(ph_node, child_node);
                            }
                        }

                        // 元の要素の子要素リストは一時的にクリア（プレースホルダーに避難しているため）
                        if let Some(src_children_mut) = self.topology.children.get_mut(pressed_id) {
                            src_children_mut.clear();
                        }
                        self.mark_layout_dirty(pressed_id);
                        self.mark_layout_dirty(placeholder_id);
                    }

                    // プレースホルダー自体はヒットテストを完全に透過させる
                    if let Some(vis) = self.renders.visual_properties.get_mut(placeholder_id) {
                        vis.pointer_events = Some(PointerEvents::None);
                    }
                    if let Some(vis) = self.renders.base_visual_properties.get_mut(placeholder_id) {
                        vis.pointer_events = Some(PointerEvents::None);
                    }
                    if let Some(mask) = self.topology.active_masks.get_mut(placeholder_id) {
                        mask.set(STYLE_POINTER_EVENTS);
                    }

                    // プレースホルダーアタッチ前の、本当の元の親要素のIDを安全に記録
                    let original_parent = self.topology.parents.get(pressed_id).copied().flatten();

                    // セッション開始
                    self.events.active_drag_state = Some(ActiveDragState {
                        source_entity: pressed_id,
                        placeholder_entity: placeholder_id,
                        current_drop_target: None,
                        start_mouse_pos: logical_pos,
                        start_rect,
                        click_offset,
                        original_parent,
                    });

                    // ドラッグ開始コールバックに、Original(pressed_id) と Placeholder(placeholder_id) の両ハンドルを渡して実行
                    let mut start_listener_opt = self
                        .events
                        .event_listeners
                        .get_mut(pressed_id)
                        .and_then(|l| l.on_drag_start.take());
                    if let Some(mut listener) = start_listener_opt {
                        {
                            let _guard = crate::ActiveElementGuard::new(pressed_id);
                            listener(
                                self,
                                Element::from(pressed_id),
                                Element::from(placeholder_id),
                            );
                        }
                        if let Some(l) = self.events.event_listeners.get_mut(pressed_id) {
                            l.on_drag_start = Some(listener);
                        }
                    }
                }

                // on_drag
                let mut on_drag = self
                    .events
                    .event_listeners
                    .get_mut(pressed_id)
                    .and_then(|l| l.on_drag.take());
                if let Some(mut handler) = on_drag {
                    let _guard = crate::ActiveElementGuard::new(pressed_id);
                    handler(self, delta);
                    if let Some(l) = self.events.event_listeners.get_mut(pressed_id) {
                        l.on_drag = Some(handler);
                    }
                }
            }
        }

        // D&D プレースホルダーの移動とドロップ先ホバー検知
        if let Some(mut drag_state) = self.events.active_drag_state.clone() {
            let src_id = drag_state.source_entity;
            let placeholder_id = drag_state.placeholder_entity;
            let drag_prop = self.events.drag_properties.get(src_id).copied().unwrap();

            // ウィンドウの真のルート要素をライブラリ側で自己解決
            let root_entity = self
                .find_root_entity()
                .expect("Root EntityId not found in Context");

            // 5-1. アタッチ先親コンテナ基準での相対ローカル座標を逆算して追従（Inset更新）
            let (parent_rect, b_l, b_t) = match drag_prop.placeholder_parent {
                DragPlaceholderParent::Root => (
                    self.outputs
                        .rects
                        .get(root_entity)
                        .copied()
                        .unwrap_or(LayoutRect::ZERO),
                    0.0,
                    0.0,
                ),
                DragPlaceholderParent::Custom(p_id) => {
                    let p_rect = self
                        .outputs
                        .rects
                        .get(p_id)
                        .copied()
                        .unwrap_or(LayoutRect::ZERO);
                    let b_l = if let Some(l) = self.layouts.basic_layouts.get(p_id) {
                        match l.border.left {
                            Length::Px(v) => v,
                            _ => 0.0,
                        }
                    } else {
                        0.0
                    };
                    let b_t = if let Some(l) = self.layouts.basic_layouts.get(p_id) {
                        match l.border.top {
                            Length::Px(v) => v,
                            _ => 0.0,
                        }
                    } else {
                        0.0
                    };
                    (p_rect, b_l, b_t)
                }
            };

            // マウスのドラッグ開始時クリックオフセットを用いて、ローカル Top-Left 座標を算出
            let local_x = logical_pos.x - (parent_rect.x + b_l) - drag_state.click_offset.x;
            let local_y = logical_pos.y - (parent_rect.y + b_t) - drag_state.click_offset.y;

            if let Some(layout) = self.layouts.basic_layouts.get_mut(placeholder_id) {
                layout.inset.left = Val::Px(local_x);
                layout.inset.top = Val::Px(local_y);
                layout.inset.right = Val::Auto;
                layout.inset.bottom = Val::Auto;
            }
            if let Some(layout) = self.renders.base_basic_layouts.get_mut(placeholder_id) {
                layout.inset.left = Val::Px(local_x);
                layout.inset.top = Val::Px(local_y);
                layout.inset.right = Val::Auto;
                layout.inset.bottom = Val::Auto;
            }

            self.mark_layout_dirty(placeholder_id);
            self.mark_render_dirty(placeholder_id);

            // 5-2. 現在ホバー侵入中のドロップターゲット要素を検知
            let hit_id_opt = self.hit_test(logical_pos);
            let mut found_drop_target = None;

            if let Some(hit_id) = hit_id_opt {
                let mut current_id = Some(hit_id);
                while let Some(id) = current_id {
                    // ヒットした要素がドラッグ元（src_id）自身、またはその子孫である場合は
                    // ドロップ先として誤認されるのを完全に防ぐため、スルーしてさらに上の親を辿る
                    if id == src_id || self.is_descendant_of(id, src_id) {
                        current_id = self.topology.parents.get(id).copied().flatten();
                        continue;
                    }

                    if id != placeholder_id && self.topology.active_masks[id].has(STYLE_DROPPABLE) {
                        found_drop_target = Some(id);
                        break;
                    }
                    current_id = self.topology.parents.get(id).copied().flatten();
                }
            }

            // ドロップ先のホバー切り替えイベントを解決（STATE_DRAG_IN の同期）
            if found_drop_target != drag_state.current_drop_target {
                if let Some(old_target) = drag_state.current_drop_target {
                    self.set_drag_state(old_target, STATE_DRAG_IN, false);
                }
                if let Some(new_target) = found_drop_target {
                    self.set_drag_state(new_target, STATE_DRAG_IN, true);
                }
                drag_state.current_drop_target = found_drop_target;
                self.events.active_drag_state = Some(drag_state.clone());
            }

            // コールバックを一時的に take して借用を分離した後に実行
            match drag_prop.drag_mode {
                DragPayload::Element => {
                    let mut listener_opt = self
                        .events
                        .event_listeners
                        .get_mut(src_id)
                        .and_then(|l| l.on_entity_drag.take());
                    if let Some(mut listener) = listener_opt {
                        {
                            let _guard = crate::ActiveElementGuard::new(src_id);
                            listener(
                                self,
                                Element::from(src_id),
                                found_drop_target.map(Element::from),
                            );
                        }
                        // 再度元の場所へ戻す
                        if let Some(l) = self.events.event_listeners.get_mut(src_id) {
                            l.on_entity_drag = Some(listener);
                        }
                    }
                }
                DragPayload::EntityId => {
                    let mut listener_opt = self
                        .events
                        .event_listeners
                        .get_mut(src_id)
                        .and_then(|l| l.on_id_drag.take());
                    if let Some(mut listener) = listener_opt {
                        {
                            let _guard = crate::ActiveElementGuard::new(src_id);
                            listener(self, src_id, found_drop_target);
                        }
                        if let Some(l) = self.events.event_listeners.get_mut(src_id) {
                            l.on_id_drag = Some(listener);
                        }
                    }
                }
            }
        }
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
                        let rect = self.outputs.rects[id];
                        let position = self
                            .layouts
                            .basic_layouts
                            .get(id)
                            .map(|l| l.position)
                            .unwrap_or(Position::Relative);

                        // 親要素の矩形を取得
                        // 親要素の矩形と、その「左・上ボーダーの厚み」を正確に取得する
                        let (parent_rect, parent_border_left, parent_border_top) =
                            if let Some(Some(parent_id)) = self.topology.parents.get(id) {
                                let p_rect = self
                                    .outputs
                                    .rects
                                    .get(*parent_id)
                                    .copied()
                                    .unwrap_or(LayoutRect::ZERO);

                                let border_l = if let Some(layout) =
                                    self.layouts.basic_layouts.get(*parent_id)
                                {
                                    match layout.border.left {
                                        Length::Px(v) => v,
                                        Length::Percent(p) => p_rect.width * (p / 100.0),
                                    }
                                } else {
                                    0.0
                                };
                                let border_t = if let Some(layout) =
                                    self.layouts.basic_layouts.get(*parent_id)
                                {
                                    match layout.border.top {
                                        Length::Px(v) => v,
                                        Length::Percent(p) => p_rect.height * (p / 100.0),
                                    }
                                } else {
                                    0.0
                                };

                                (p_rect, border_l, border_t)
                            } else {
                                (LayoutRect::ZERO, 0.0, 0.0)
                            };

                        // 親コンテナのボーダー内側を基準点として物理相対位置を逆算
                        let local_x = rect.x - (parent_rect.x + parent_border_left);
                        let local_y = rect.y - (parent_rect.y + parent_border_top);

                        // 【解決】絶対配置の場合、開始時に Top-Left 基準に完全に正規化（コンバート）する
                        // これにより、もともと right / bottom 基準で配置されていた要素であっても、
                        // ドラッグ開始の瞬間に左上へ吹っ飛ぶ現象を完全に阻止します。
                        let mut start_inset = Rect {
                            top: Val::Px(0.0),
                            right: Val::Px(0.0),
                            bottom: Val::Px(0.0),
                            left: Val::Px(0.0),
                        };
                        if position == Position::Absolute {
                            start_inset.top = Val::Px(local_y);
                            start_inset.left = Val::Px(local_x);
                            start_inset.right = Val::Auto;
                            start_inset.bottom = Val::Auto;

                            // SoA 側も、この Top-Left 座標で即時上書きアップデート
                            if let Some(layout) = self.layouts.basic_layouts.get_mut(id) {
                                layout.inset = start_inset;
                            }
                            if let Some(layout) = self.renders.base_basic_layouts.get_mut(id) {
                                layout.inset = start_inset;
                            }
                        } else {
                            // 相対配置時は、通常通りそのままのインセットを使用
                            start_inset = self
                                .layouts
                                .basic_layouts
                                .get(id)
                                .map(|l| l.inset)
                                .unwrap_or(BasicLayout::default().inset);
                        }

                        let start_pos = self
                            .events
                            .current_pointer_position
                            .unwrap_or(LayoutPoint::ZERO);

                        self.events.resizing_state = Some(ResizingState {
                            entity_id: id,
                            direction: dir,
                            start_mouse_pos: start_pos,
                            start_rect: rect,
                            start_inset,
                        });

                        // リサイズ中の要素は pressed とマーク（多重干渉防止）
                        self.events.interaction_states.pressed = Some(id);
                        self.mark_render_dirty(id);
                        return; // リサイズドラッグが開始されたため、通常のクリック・フォーカス処理を完全にバイパス
                    }
                }

                let mut clicked_scrollbar = false;

                if let Some(pointer_pos) = self.events.current_pointer_position
                    && let Some(target_id) = current_hovered
                {
                    // 1. ヒットした要素がサム、またはトラックであるかを判定
                    let mut parent_container = None;
                    let mut is_v_thumb = false;
                    let mut is_h_thumb = false;
                    let mut is_v_track = false;
                    let mut is_h_track = false;

                    for (c_id, sb_state) in self.layouts.scrollbar_styles.iter() {
                        if sb_state.v_thumb_id == Some(target_id) {
                            parent_container = Some(c_id);
                            is_v_thumb = true;
                            break;
                        } else if sb_state.h_thumb_id == Some(target_id) {
                            parent_container = Some(c_id);
                            is_h_thumb = true;
                            break;
                        } else if sb_state.v_track_id == Some(target_id) {
                            parent_container = Some(c_id);
                            is_v_track = true;
                            break;
                        } else if sb_state.h_track_id == Some(target_id) {
                            parent_container = Some(c_id);
                            is_h_track = true;
                            break;
                        }
                    }

                    if let Some(c_id) = parent_container {
                        clicked_scrollbar = true;

                        let (sb_state, container_rect, scroll_size) = {
                            let sb_state =
                                self.layouts.scrollbar_styles.get(c_id).cloned().unwrap();
                            let container_rect = self
                                .outputs
                                .rects
                                .get(c_id)
                                .copied()
                                .unwrap_or(LayoutRect::ZERO);
                            let scroll_size = self.get_scroll_size(c_id);
                            (sb_state, container_rect, scroll_size)
                        };

                        let offset = self
                            .outputs
                            .scroll_offsets
                            .get(c_id)
                            .copied()
                            .unwrap_or(LayoutPoint::ZERO);

                        if is_v_thumb || is_h_thumb {
                            // A. サムをクリックした場合：ドラッグを開始
                            if let Some(st) = self.layouts.scrollbar_styles.get_mut(c_id) {
                                if is_v_thumb {
                                    st.v_thumb_dragged = true;
                                } else {
                                    st.h_thumb_dragged = true;
                                }
                                st.drag_start_mouse = pointer_pos;
                                st.drag_start_offset = offset;
                            }
                            self.events.interaction_states.pressed = Some(target_id); // サム要素自体を pressed に設定
                            self.mark_render_dirty(target_id);
                        } else if is_v_track || is_h_track {
                            let visible_size = self.calculate_visible_size(container_rect);

                            // B. レールをクリックした場合：ダイレクトジャンプスクロールを実行
                            if is_v_track {
                                let track_rect = self.outputs.rects[target_id];
                                let thumb_rect = self.outputs.rects[sb_state.v_thumb_id.unwrap()];
                                let relative_y = pointer_pos.y - track_rect.y;

                                let track_range = track_rect.height - thumb_rect.height;
                                let scroll_ratio = if track_range > 0.0 {
                                    ((relative_y - thumb_rect.height * 0.5) / track_range)
                                        .clamp(0.0, 1.0)
                                } else {
                                    0.0
                                };

                                let target_y =
                                    scroll_ratio * (scroll_size.height - visible_size.height);
                                self.scroll_to(c_id, offset.x, target_y);

                                let new_offset = self
                                    .outputs
                                    .scroll_offsets
                                    .get(c_id)
                                    .copied()
                                    .unwrap_or(LayoutPoint::ZERO);
                                if let Some(st) = self.layouts.scrollbar_styles.get_mut(c_id) {
                                    st.v_thumb_dragged = true;
                                    st.drag_start_mouse = pointer_pos;
                                    st.drag_start_offset = new_offset;
                                }
                                self.events.interaction_states.pressed =
                                    Some(sb_state.v_thumb_id.unwrap());
                                self.mark_render_dirty(sb_state.v_thumb_id.unwrap());
                            } else {
                                let track_rect = self.outputs.rects[target_id];
                                let thumb_rect = self.outputs.rects[sb_state.h_thumb_id.unwrap()];
                                let relative_x = pointer_pos.x - track_rect.x;

                                let track_range = track_rect.width - thumb_rect.width;
                                let scroll_ratio = if track_range > 0.0 {
                                    ((relative_x - thumb_rect.width * 0.5) / track_range)
                                        .clamp(0.0, 1.0)
                                } else {
                                    0.0
                                };

                                let target_x =
                                    scroll_ratio * (scroll_size.width - visible_size.width);
                                self.scroll_to(c_id, target_x, offset.y);

                                let new_offset = self
                                    .outputs
                                    .scroll_offsets
                                    .get(c_id)
                                    .copied()
                                    .unwrap_or(LayoutPoint::ZERO);
                                if let Some(st) = self.layouts.scrollbar_styles.get_mut(c_id) {
                                    st.h_thumb_dragged = true;
                                    st.drag_start_mouse = pointer_pos;
                                    st.drag_start_offset = new_offset;
                                }
                                self.events.interaction_states.pressed =
                                    Some(sb_state.h_thumb_id.unwrap());
                                self.mark_render_dirty(sb_state.h_thumb_id.unwrap());
                            }
                        }
                    }
                }

                if clicked_scrollbar {
                    return; // 背後の一般子要素へのイベント透過を防止
                }

                if let Some(target_id) = current_hovered {
                    self.events.interaction_states.pressed = Some(target_id);
                    self.set_pressed(target_id, true);

                    let user_select = self
                        .renders
                        .visual_properties
                        .get(target_id)
                        .and_then(|v| v.user_select)
                        .unwrap_or(UserSelect::None);

                    let is_input = self.topology.active_masks[target_id].has(COMP_INPUT_CONTENT);

                    if user_select == UserSelect::Text
                        && !is_input
                        && let Some(pointer_pos) = self.events.current_pointer_position
                    {
                        let rect = self.outputs.rects[target_id];
                        let (basic, _, _) = self.resolve_active_layouts(target_id);
                        let border_left = match basic.border.left {
                            Length::Px(v) => v,
                            _ => 0.0,
                        };
                        let padding_left = match basic.padding.left {
                            Length::Px(v) => v,
                            _ => 0.0,
                        };
                        let border_top = match basic.border.top {
                            Length::Px(v) => v,
                            _ => 0.0,
                        };
                        let padding_top = match basic.padding.top {
                            Length::Px(v) => v,
                            _ => 0.0,
                        };

                        let scroll = self
                            .outputs
                            .scroll_offsets
                            .get(target_id)
                            .copied()
                            .unwrap_or(LayoutPoint::ZERO);

                        let local_x =
                            pointer_pos.x - (rect.x + border_left + padding_left) + scroll.x;
                        let local_y =
                            pointer_pos.y - (rect.y + border_top + padding_top) + scroll.y;

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

                            if modifiers.shift {
                                // 共通の Shift選択拡張
                                let anchor = self
                                    .outputs
                                    .selection_start_index
                                    .get(target_id)
                                    .copied()
                                    .unwrap_or(final_index);
                                if !self.outputs.selection_start_index.contains_key(target_id) {
                                    self.outputs
                                        .selection_start_index
                                        .insert(target_id, final_index);
                                }
                                let range = if anchor <= final_index {
                                    anchor..final_index
                                } else {
                                    final_index..anchor
                                };
                                self.outputs.text_selections.insert(target_id, range);
                                self.update_selection_rects(target_id);
                            } else {
                                // 共通の通常クリックリセット
                                self.outputs
                                    .selection_start_index
                                    .insert(target_id, final_index);
                                self.outputs
                                    .text_selections
                                    .insert(target_id, final_index..final_index);
                                self.outputs.selected_rects.remove(target_id);
                            }

                            self.mark_render_dirty(target_id);
                        }
                    }

                    // フォーカス可能要素のみにフォーカスを制限
                    let is_focusable = self.topology.active_masks[target_id]
                        .has(COMP_INPUT_CONTENT)
                        || self.topology.active_masks[target_id].has(COMP_WEBVIEW_CONTENT)
                        || (self.topology.active_masks[target_id].has(STYLE_FOCUSABLE)
                            && self
                                .renders
                                .visual_properties
                                .get(target_id)
                                .and_then(|v| v.focusable)
                                .map(|f| match f {
                                    Focusable::SelfStyle(trigger) | Focusable::Inherit(trigger) => {
                                        trigger == FocusTrigger::Mouse
                                            || trigger == FocusTrigger::Both
                                    }
                                    Focusable::None => false,
                                })
                                .unwrap_or(false));

                    if is_focusable {
                        // フォーカスの自動切り替え
                        if self.events.interaction_states.focused != Some(target_id) {
                            if let Some(old_focus_id) = self.events.interaction_states.focused {
                                self.set_focused(old_focus_id, false);

                                // on_blur
                                let mut on_blur = self
                                    .events
                                    .event_listeners
                                    .get_mut(old_focus_id)
                                    .and_then(|l| l.on_blur.take());
                                if let Some(mut handler) = on_blur {
                                    let _guard = crate::ActiveElementGuard::new(old_focus_id);
                                    handler(self);
                                    if let Some(l) =
                                        self.events.event_listeners.get_mut(old_focus_id)
                                    {
                                        l.on_blur = Some(handler);
                                    }
                                }
                            }

                            // 新しいフォーカス可能要素にフォーカスを設定
                            self.set_focused(target_id, true);

                            // on_focus
                            let mut on_focus = self
                                .events
                                .event_listeners
                                .get_mut(target_id)
                                .and_then(|l| l.on_focus.take());
                            if let Some(mut handler) = on_focus {
                                let _guard = crate::ActiveElementGuard::new(target_id);
                                handler(self);
                                if let Some(l) = self.events.event_listeners.get_mut(target_id) {
                                    l.on_focus = Some(handler);
                                }
                            }
                            self.events.interaction_states.focused = Some(target_id);
                        }
                    } else {
                        // フォーカス不可能な要素をクリックした場合は、
                        // 現在フォーカスされているインプットからフォーカスを完全に外し状態をクリアする
                        if let Some(old_focus_id) = self.events.interaction_states.focused {
                            self.set_focused(old_focus_id, false);

                            let mut on_blur = self
                                .events
                                .event_listeners
                                .get_mut(old_focus_id)
                                .and_then(|l| l.on_blur.take());
                            if let Some(mut handler) = on_blur {
                                let _guard = crate::ActiveElementGuard::new(old_focus_id);
                                handler(self);
                                if let Some(l) = self.events.event_listeners.get_mut(old_focus_id) {
                                    l.on_blur = Some(handler);
                                }
                            }
                            self.events.interaction_states.focused = None;
                        }
                    }

                    // on_mouse_input
                    let mut on_input = self
                        .events
                        .event_listeners
                        .get_mut(target_id)
                        .and_then(|l| l.on_mouse_input.take());
                    if let Some(mut handler) = on_input {
                        let _guard = crate::ActiveElementGuard::new(target_id);
                        handler(self, button, modifiers, state);
                        if let Some(l) = self.events.event_listeners.get_mut(target_id) {
                            l.on_mouse_input = Some(handler);
                        }
                    }
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
                    let placeholder_id = drag_state.placeholder_entity;
                    let drag_prop = self.events.drag_properties.get(src_id).copied().unwrap();

                    // 疑似クラス（STATE_DRAGGING, STATE_DRAG_IN）を解除
                    self.set_drag_state(src_id, STATE_DRAGGING, false);
                    if let Some(target_id) = drag_state.current_drop_target {
                        self.set_drag_state(target_id, STATE_DRAG_IN, false);
                    }

                    // プレースホルダー要素を親および Taffy から安全にデスポーン
                    // このタイミングではまだ despawn_internal せず最後に移動させます。
                    self.events.interaction_states.pressed = None;
                    self.events.interaction_states.dragged = None;

                    let drop_success = drag_state.current_drop_target;

                    // A. 実体移動（DragMode::Entity）の場合のツリートポロジー書き換え
                    if let Some(target_id) = drop_success
                        && drag_prop.drag_mode == DragPayload::Element
                        && let Some(drop_prop) = self.events.drop_properties.get(target_id).copied()
                    {
                        // 1. まずドラッグ元要素を現在の親の children リストから安全に引き抜いて削除
                        if let Some(src_parent_id) = drag_state.original_parent {
                            if let Some(src_children) =
                                self.topology.children.get_mut(src_parent_id)
                            {
                                src_children.retain(|x| *x != src_id);
                            }
                            // 旧親側の Taffy 順序も再同期
                            self.resync_taffy_children_order(src_parent_id);
                            self.mark_layout_dirty(src_parent_id);
                        }

                        // ドラッグ元要素の配置（Position）の取得
                        let position = self
                            .layouts
                            .basic_layouts
                            .get(src_id)
                            .map(|l| l.position)
                            .unwrap_or(Position::Relative);

                        if position == Position::Absolute {
                            // 【絶対配置（Absolute）】: 位置移動（補正）を伴うアタッチ
                            if drag_prop.update_position {
                                // 1. プレースホルダーの最終的な絶対画面座標を取得
                                let ph_abs_rect = self
                                    .outputs
                                    .rects
                                    .get(placeholder_id)
                                    .copied()
                                    .unwrap_or(LayoutRect::ZERO);

                                // 2. 新しい親（target_id）の絶対画面座標とボーダー厚みを取得
                                let target_rect = self
                                    .outputs
                                    .rects
                                    .get(target_id)
                                    .copied()
                                    .unwrap_or(LayoutRect::ZERO);
                                let (border_l, border_t) = if let Some(layout) =
                                    self.layouts.basic_layouts.get(target_id)
                                {
                                    let border = self.get_physical_border(target_id, layout);
                                    (border.left, border.top)
                                } else {
                                    (0.0, 0.0)
                                };

                                // 3. 新しい親を基準にした新しいローカル相対位置を逆算して割り出す
                                let new_inset_left = ph_abs_rect.x - (target_rect.x + border_l);
                                let new_inset_top = ph_abs_rect.y - (target_rect.y + border_t);

                                let new_inset = Rect {
                                    top: Val::Px(new_inset_top),
                                    right: Val::Auto,
                                    bottom: Val::Auto,
                                    left: Val::Px(new_inset_left),
                                };

                                if let Some(layout) = self.layouts.basic_layouts.get_mut(src_id) {
                                    layout.inset = new_inset;
                                }
                                if let Some(layout) =
                                    self.renders.base_basic_layouts.get_mut(src_id)
                                {
                                    layout.inset = new_inset;
                                }
                            }

                            // ドロップ先コンテナ（target_id）の末尾の子要素としてマウント
                            self.add_child(target_id, src_id);
                        } else {
                            // 【相対配置（Relative）】: マウス座標に基づいた子要素の動的並び替えアタッチ
                            if drag_prop.update_position {
                                let mouse_pos = self
                                    .events
                                    .current_pointer_position
                                    .unwrap_or(LayoutPoint::ZERO);
                                let insert_idx = self.calculate_insert_index(target_id, mouse_pos);

                                if let Some(parent_children) =
                                    self.topology.children.get_mut(target_id)
                                {
                                    // 算出されたインデックス位置へ挿入
                                    parent_children.insert(insert_idx, src_id);
                                }
                                self.topology.parents.insert(src_id, Some(target_id));

                                // Taffy 側のノード順序を物理並び替え結果に沿って一括して再同期
                                self.resync_taffy_children_order(target_id);
                            } else {
                                // 自動更新オフの場合は末尾に通常アタッチ
                                self.add_child(target_id, src_id);
                            }
                            self.mark_layout_dirty(target_id);
                        }

                        self.layouts.is_structure_dirty = true;
                    }

                    // 避難していた本物の子要素トポロジーを、元の要素（src_id）の配下へ自動復元
                    if let Some(ph_children) = self.topology.children.get(placeholder_id).cloned() {
                        for child_id in ph_children {
                            // 子要素の親ポインタを元の要素に書き戻し
                            self.topology.parents.insert(child_id, Some(src_id));

                            // 元の要素の子要素リストへ復旧
                            if let Some(src_children) = self.topology.children.get_mut(src_id) {
                                src_children.push(child_id);
                            }

                            // Taffy 側の親子構造も、元の要素に繋ぎ戻し
                            if let Some(&src_node) = self.layouts.taffy_nodes.get(src_id)
                                && let Some(&ph_node) = self.layouts.taffy_nodes.get(placeholder_id)
                                && let Some(&child_node) = self.layouts.taffy_nodes.get(child_id)
                            {
                                let _ = self.layouts.taffy.remove_child(ph_node, child_node);
                                let _ = self.layouts.taffy.add_child(src_node, child_node);
                            }
                        }

                        // プレースホルダー側は空にして破棄に備える
                        if let Some(ph_children_mut) =
                            self.topology.children.get_mut(placeholder_id)
                        {
                            ph_children_mut.clear();
                        }
                        self.mark_layout_dirty(src_id);
                        self.mark_layout_dirty(placeholder_id);
                    }

                    // コールバックを一時的に take して借用を完全に切り離して実行する
                    match drag_prop.drag_mode {
                        DragPayload::Element => {
                            let mut listener_opt = self
                                .events
                                .event_listeners
                                .get_mut(src_id)
                                .and_then(|l| l.on_entity_drop.take());
                            if let Some(mut listener) = listener_opt {
                                {
                                    let _guard = crate::ActiveElementGuard::new(src_id);
                                    listener(
                                        self,
                                        Element::from(src_id),
                                        drop_success.map(Element::from),
                                    );
                                }
                                if let Some(l) = self.events.event_listeners.get_mut(src_id) {
                                    l.on_entity_drop = Some(listener);
                                }
                            }
                        }
                        DragPayload::EntityId => {
                            let mut listener_opt = self
                                .events
                                .event_listeners
                                .get_mut(src_id)
                                .and_then(|l| l.on_id_drop.take());
                            if let Some(mut listener) = listener_opt {
                                {
                                    let _guard = crate::ActiveElementGuard::new(src_id);
                                    listener(self, src_id, drop_success);
                                }
                                if let Some(l) = self.events.event_listeners.get_mut(src_id) {
                                    l.on_id_drop = Some(listener);
                                }
                            }
                        }
                    }

                    // 位置情報の安全な回収がすべて完了した、この最末尾で初めてプレースホルダーを破棄
                    self.despawn_internal(placeholder_id);

                    // 離脱直後に位置を再移動評価して、通常のホバーを正しく復元
                    if let Some(pos) = self.events.current_pointer_position {
                        self.inject_pointer_move(pos);
                    }

                    self.mark_render_dirty(src_id);
                    return; // 早期リターン
                }

                let mut dirty_ids = smallvec::SmallVec::<[EntityId; 4]>::new();
                for (id, state) in self.layouts.scrollbar_styles.iter_mut() {
                    if state.v_thumb_dragged || state.h_thumb_dragged {
                        state.v_thumb_dragged = false;
                        state.h_thumb_dragged = false;
                        dirty_ids.push(id);
                    }
                }

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

                    // 1. on_mouse_input の発火（ボタンの種類を問わず常に呼ぶ）
                    let mut on_input = self
                        .events
                        .event_listeners
                        .get_mut(pressed_id)
                        .and_then(|l| l.on_mouse_input.take());
                    if let Some(mut handler) = on_input {
                        let _guard = crate::ActiveElementGuard::new(pressed_id);
                        handler(self, button, modifiers, state);
                        if let Some(l) = self.events.event_listeners.get_mut(pressed_id) {
                            l.on_mouse_input = Some(handler);
                        }
                    }

                    // 2. 同一要素上で離された場合の各種クリック解決
                    if self.events.interaction_states.hovered == Some(pressed_id) {
                        match button {
                            // 左クリックの解決
                            MouseButton::Left => {
                                let mut on_click = self
                                    .events
                                    .event_listeners
                                    .get_mut(pressed_id)
                                    .and_then(|l| l.on_click.take());
                                if let Some(mut handler) = on_click {
                                    let _guard = crate::ActiveElementGuard::new(pressed_id);
                                    handler(self);
                                    if let Some(l) = self.events.event_listeners.get_mut(pressed_id)
                                    {
                                        l.on_click = Some(handler);
                                    }
                                }
                            }
                            // 右クリックの解決（追加）
                            MouseButton::Right => {
                                let mut on_right = self
                                    .events
                                    .event_listeners
                                    .get_mut(pressed_id)
                                    .and_then(|l| l.on_right_click.take());
                                if let Some(mut handler) = on_right {
                                    let _guard = crate::ActiveElementGuard::new(pressed_id);
                                    handler(self);
                                    if let Some(l) = self.events.event_listeners.get_mut(pressed_id)
                                    {
                                        l.on_right_click = Some(handler);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    self.events.interaction_states.pressed = None;
                }
            }
        }
    }

    /// ウィンドウ内の最上位ルート要素の EntityId を自律解決して返します。
    pub(crate) fn find_root_entity(&self) -> Option<EntityId> {
        // すでにフラットシーケンスが構築されていればその先頭、
        // 無ければ parents マップをスキャンして親が None の生存要素をフォールバック解決します
        self.layouts.flat_dfs_sequence.first().copied().or_else(|| {
            self.topology
                .parents
                .iter()
                .find(|&(id, &parent_id_opt)| {
                    // 親が None かつ、要素 id 自体が slotmap (entities) に生存しているか
                    parent_id_opt.is_none() && self.topology.entities.contains_key(id)
                })
                .map(|(id, _)| id)
        })
    }

    // ダブルクリック
    pub fn inject_pointer_double_click(&mut self, modifiers: Modifiers) {
        let _context_guard = bind_context(self);
        let current_hovered = self.events.interaction_states.hovered;

        if let Some(target_id) = current_hovered {
            let user_select = self
                .renders
                .visual_properties
                .get(target_id)
                .and_then(|v| v.user_select)
                .unwrap_or(UserSelect::None);

            if user_select == UserSelect::Text
                && let Some(pointer_pos) = self.events.current_pointer_position
            {
                if let Some(contents) = self.contents.input_contents.get(target_id) {
                    let text_val = contents.text.0.get();
                    let is_placeholder = text_val.is_empty()
                        && contents
                            .ime_state
                            .as_ref()
                            .map(|s| s.composition_text.is_empty())
                            .unwrap_or(true);

                    if is_placeholder && !contents.placeholder_select {
                        return;
                    }
                }

                let rect = self.outputs.rects[target_id];
                let (basic, _, _) = self.resolve_active_layouts(target_id);
                let border = self.get_physical_border(target_id, &basic);
                let padding = self.get_physical_padding(target_id, &basic);

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
                            crate::update_input_caret_position(self, target_id);
                        }

                        self.mark_render_dirty(target_id);
                    }
                }
            }
        }
    }

    /// 外部で計算された論理ピクセルスクロール移動量 (scroll_x, scroll_y) を注入し、
    /// バブリングによる自動スクロール処理、またはユーザーイベントハンドラへの配送を行います。
    pub fn inject_mouse_wheel(&mut self, scroll_x: f32, scroll_y: f32) {
        let _context_guard = bind_context(self);

        let mut curr = self.events.interaction_states.hovered;
        let mut handled = false;

        // イベントバブリング: ホバー要素から親へ辿る
        while let Some(curr_id) = curr {
            // 個別に定義された `on_mouse_wheel` ハンドラがあれば最優先実行
            let mut on_wheel = self
                .events
                .event_listeners
                .get_mut(curr_id)
                .and_then(|l| l.on_mouse_wheel.take());

            if let Some(mut handler) = on_wheel {
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
            {
                let mut on_click = self
                    .events
                    .event_listeners
                    .get_mut(focused_id)
                    .and_then(|l| l.on_click.take());

                if let Some(mut handler) = on_click {
                    let _guard = crate::ActiveElementGuard::new(focused_id);
                    handler(self);
                    if let Some(l) = self.events.event_listeners.get_mut(focused_id) {
                        l.on_click = Some(handler);
                    }
                }
                return;
            }
            // 内部で完結する全選択（Ctrl+A）のみを自動処理
            if state == ElementState::Pressed && modifiers.ctrl {
                let user_select = self
                    .renders
                    .visual_properties
                    .get(focused_id)
                    .and_then(|v| v.user_select)
                    .unwrap_or(UserSelect::None);

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
                            crate::update_input_caret_position(self, focused_id);
                        }
                        self.mark_render_dirty(focused_id);
                    }
                    return;
                }
            }

            let mut on_key = self
                .events
                .event_listeners
                .get_mut(focused_id)
                .and_then(|l| l.on_keyboard_input.take());
            if let Some(mut handler) = on_key {
                let _guard = crate::ActiveElementGuard::new(focused_id);
                handler(self, key, modifiers, state);
                if let Some(l) = self.events.event_listeners.get_mut(focused_id) {
                    l.on_keyboard_input = Some(handler);
                }
            }
        }
    }

    /// キーボードフォーカスを次の適格な要素へ巡回させます
    pub fn cycle_keyboard_focus(&mut self, reverse: bool) {
        if self.layouts.flat_dfs_sequence.is_empty() {
            return;
        }

        let len = self.layouts.flat_dfs_sequence.len();

        // 現在フォーカスされている要素のインデックスを特定（無ければ探索方向の末端から開始）
        let current_focused = self.events.interaction_states.focused;
        let start_idx = current_focused
            .and_then(|id| self.layouts.flat_dfs_sequence.iter().position(|&x| x == id))
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

            let candidate_id = self.layouts.flat_dfs_sequence[idx];

            if self.is_keyboard_focusable(candidate_id) {
                // 古い要素のフォーカスを外し、新しい要素へフォーカスを設定
                if let Some(old_id) = self.events.interaction_states.focused {
                    self.set_focused(old_id, false);
                }
                self.set_focused(candidate_id, true);
                self.events.interaction_states.focused = Some(candidate_id);

                // WebView2 要素だった場合はシステム側にフォーカスをプログラム駆動で移譲
                if self.topology.active_masks[candidate_id].has(COMP_WEBVIEW_CONTENT) {
                    // 通常のレンダラーから focus_webview を呼び出すためここでは何もしない
                }

                self.mark_render_dirty(candidate_id);
                break;
            }
        }
    }

    /// 対象の要素がキーボードフォーカス可能であるかを総合検証します
    fn is_keyboard_focusable(&self, id: EntityId) -> bool {
        // 生存確認、および無効化（Disabled）状態でないか検証
        if !self.topology.entities.contains_key(id) || self.is_disabled(id) {
            return false;
        }

        // 暗黙的または明示的にキーボードフォーカスを要求しているか
        let is_target = self.topology.active_masks[id].has(COMP_INPUT_CONTENT)
            || self.topology.active_masks[id].has(COMP_WEBVIEW_CONTENT)
            || (self.topology.active_masks[id].has(STYLE_FOCUSABLE)
                && self
                    .renders
                    .visual_properties
                    .get(id)
                    .and_then(|v| v.focusable)
                    .map(|f| match f {
                        Focusable::SelfStyle(trigger) | Focusable::Inherit(trigger) => {
                            trigger == FocusTrigger::Keyboard || trigger == FocusTrigger::Both
                        }
                        Focusable::None => false,
                    })
                    .unwrap_or(false));

        if !is_target {
            return false;
        }

        // 自分自身、および親先祖ツリーに非表示（Display::None）が1つも含まれていないか検証
        let mut curr = Some(id);
        while let Some(curr_id) = curr {
            if let Some(layout) = self.layouts.basic_layouts.get(curr_id)
                && layout.display == Display::None
            {
                return false;
            }
            curr = self.topology.parents.get(curr_id).copied().flatten();
        }

        true
    }

    pub fn inject_character(&mut self, c: char) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.events.interaction_states.focused {
            let mut on_char = self
                .events
                .event_listeners
                .get_mut(focused_id)
                .and_then(|l| l.on_char_input.take());
            if let Some(mut handler) = on_char {
                let _guard = crate::ActiveElementGuard::new(focused_id);
                handler(self, c);
                if let Some(l) = self.events.event_listeners.get_mut(focused_id) {
                    l.on_char_input = Some(handler);
                }
            }
        }
    }

    pub fn inject_ime(&mut self, ime_state: ImeState) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.events.interaction_states.focused {
            let mut on_ime = self
                .events
                .event_listeners
                .get_mut(focused_id)
                .and_then(|l| l.on_ime.take());
            if let Some(mut handler) = on_ime {
                let _guard = crate::ActiveElementGuard::new(focused_id);
                handler(self, ime_state);
                if let Some(l) = self.events.event_listeners.get_mut(focused_id) {
                    l.on_ime = Some(handler);
                }
            }
        }
    }

    pub fn inject_file_dropped(&mut self, paths: Vec<PathBuf>) {
        let _context_guard = bind_context(self);
        if let Some(target_id) = self.events.interaction_states.hovered {
            let mut on_drop = self
                .events
                .event_listeners
                .get_mut(target_id)
                .and_then(|l| l.on_file_dropped.take());
            if let Some(mut handler) = on_drop {
                let _guard = crate::ActiveElementGuard::new(target_id);
                handler(self, paths);
                if let Some(l) = self.events.event_listeners.get_mut(target_id) {
                    l.on_file_dropped = Some(handler);
                }
            }
        }
    }

    /// 現在の選択範囲（text_selections）に基づき、
    /// 描画用の物理選択矩形（selected_rects）を自動再計算して SoA キャッシュを更新します。
    #[inline]
    pub(crate) fn update_selection_rects(&mut self, id: EntityId) {
        if let Some(range) = self.outputs.text_selections.get(id).cloned()
            && range.start < range.end
            && let Some(layout) = self.get_or_create_layout(id)
        {
            let rects = OutputStore::calc_selection_rects(id, layout, range, &mut self.outputs);
            self.outputs.selected_rects.insert(id, rects);
            return;
        }
        // 範囲が 0、または選択なしの時は自動クリーンアップ
        self.outputs.selected_rects.remove(id);
    }

    /// キャッシュされたレイアウトがあればそれを返し、無ければ安全に生成して保持します。
    #[inline]
    pub(crate) fn get_or_create_layout(&self, id: EntityId) -> Option<IDWriteTextLayout> {
        if let Some(layout) = self.system.dwrite_layouts.borrow().get(id) {
            return Some(layout.clone());
        }

        SystemStore::create_text_layout(id, &self.system, &self.contents, &self.renders)
    }

    /// 現在フォーカスされている要素で範囲選択されている文字列を取得します。
    #[inline]
    pub fn get_selected_text(&self) -> Option<String> {
        OutputStore::get_selected_text(&self.events, &self.renders, &self.outputs, &self.contents)
    }

    /// 外部から提供されたテキストを、現在フォーカスされている入力要素にペーストします。
    #[inline]
    pub fn inject_paste(&mut self, text: &str) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.events.interaction_states.focused
            && self.topology.active_masks[focused_id].has(COMP_INPUT_CONTENT)
            && let Some(contents) = self.contents.input_contents.get_mut(focused_id)
        {
            OutputStore::inject_paste_internal(focused_id, text, &mut self.outputs, contents);

            crate::update_input_caret_position(self, focused_id);
            self.mark_render_dirty(focused_id);
        }
    }

    /// Undo (元に戻す) のインジェクション
    #[inline]
    pub fn inject_undo(&mut self) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.events.interaction_states.focused
            && self.topology.active_masks[focused_id].has(COMP_INPUT_CONTENT)
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

            crate::update_input_caret_position(self, focused_id);
            self.mark_render_dirty(focused_id);
        }
    }

    /// Redo (やり直し) のインジェクション
    #[inline]
    pub fn inject_redo(&mut self) {
        let _context_guard = bind_context(self);
        if let Some(focused_id) = self.events.interaction_states.focused
            && self.topology.active_masks[focused_id].has(COMP_INPUT_CONTENT)
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

            crate::update_input_caret_position(self, focused_id);
            self.mark_render_dirty(focused_id);
        }
    }

    /// 切り取り (Ctrl+X) の実行と削除後のテキスト取得
    #[inline]
    pub fn inject_cut(&mut self) -> Option<String> {
        let _context_guard = bind_context(self);
        let focused_id = self.events.interaction_states.focused?;
        let user_select = self
            .renders
            .visual_properties
            .get(focused_id)
            .and_then(|v| v.user_select)
            .unwrap_or(UserSelect::None);

        if user_select == UserSelect::Text
            && let Some(range) = self.outputs.text_selections.get(focused_id).cloned()
            && range.start < range.end
            && let Some(text) = self.contents.text_contents.get(focused_id)
        {
            let u16_text: Vec<u16> = text.encode_utf16().collect();
            let slice = &u16_text[range.start.min(u16_text.len())..range.end.min(u16_text.len())];
            let cut_text = String::from_utf16(slice).ok()?;

            // 対象が Input コントロールである場合のみ、切り取り削除上書きを実行
            if self.topology.active_masks[focused_id].has(COMP_INPUT_CONTENT)
                && let Some(contents) = self.contents.input_contents.get_mut(focused_id)
            {
                OutputStore::inject_cut_internal(focused_id, range, &mut self.outputs, contents);

                crate::update_input_caret_position(self, focused_id);
                self.mark_render_dirty(focused_id);
            }

            return Some(cut_text);
        }

        None
    }

    /// 指定された要素の子要素全体のスクロール領域を親ローカル座標系で算出します。
    pub fn get_scroll_size(&self, id: EntityId) -> LayoutSize {
        let mut max_x = 0.0f32;
        let mut max_y = 0.0f32;

        // 自身に内包されたインラインコンテンツの計測サイズを初期値とする
        if self.topology.active_masks[id].has(COMP_INPUT_CONTENT)
            && let Some(contents) = self.contents.input_contents.get(id)
            && let Some(layout_rect) = contents.last_layout
        {
            max_x = layout_rect.width + contents.caret_width.unwrap_or(1.5);
            max_y = layout_rect.height;
        } else if self.topology.active_masks[id].has(COMP_TEXT_CONTENT)
            && let Some(layout) = self.get_or_create_layout(id)
        {
            let size = self.system.text_engine.get_layout_size(&layout);
            max_x = size.width;
            max_y = size.height;
        }

        // 親要素自体のボーダー・パディング厚を取得
        let (basic, _, _) = self.resolve_active_layouts(id);
        let border = self.get_physical_border(id, &basic);
        let padding = self.get_physical_padding(id, &basic);

        let offset_x = border.left + padding.left;
        let offset_y = border.top + padding.top;

        // スクロールバー要素のIDを取得して除外対象にする
        let (v_track_opt, h_track_opt) =
            if let Some(sb_state) = self.layouts.scrollbar_styles.get(id) {
                (sb_state.v_track_id, sb_state.h_track_id)
            } else {
                (None, None)
            };

        if let Some(children_list) = self.topology.children.get(id) {
            for &child_id in children_list {
                // スクロールバーのトラックはサイズ計算から除外
                if Some(child_id) == v_track_opt || Some(child_id) == h_track_opt {
                    continue;
                }

                // 絶対配置要素（スクロールバーのサムなど）もスクロール領域サイズ計算から除外
                let is_absolute = self
                    .layouts
                    .basic_layouts
                    .get(child_id)
                    .map(|l| l.position == Position::Absolute)
                    .unwrap_or(false);
                if is_absolute {
                    continue;
                }

                if let Some(&rect) = self.outputs.rects.get(child_id) {
                    let parent_rect = self
                        .outputs
                        .rects
                        .get(id)
                        .copied()
                        .unwrap_or(LayoutRect::ZERO);
                    let scroll_offset = self
                        .outputs
                        .scroll_offsets
                        .get(id)
                        .copied()
                        .unwrap_or(LayoutPoint::ZERO);

                    // 親の左上（border+padding除外）を原点 (0,0) とした子要素の右下端
                    let local_right =
                        rect.x - parent_rect.x + scroll_offset.x + rect.width - offset_x;
                    let local_bottom =
                        rect.y - parent_rect.y + scroll_offset.y + rect.height - offset_y;

                    max_x = max_x.max(local_right);
                    max_y = max_y.max(local_bottom);
                }
            }
        }

        LayoutSize::new(max_x, max_y)
    }

    /// スクロールオフセットを目標位置へクランプした上で代入。
    /// オフセットに変化が生じた場合は true を返し、レイアウトのDirtyマークを打つ。
    pub fn scroll_to(&mut self, id: EntityId, mut x: f32, mut y: f32) -> bool {
        let rect = match self.outputs.rects.get(id).copied() {
            Some(r) => r,
            None => return false,
        };

        let scroll_size = self.get_scroll_size(id);

        // 親コンテナのボーダーおよびパディング厚を取得
        let (basic, _, _) = self.resolve_active_layouts(id);
        let border = self.get_physical_border(id, &basic);
        let padding = self.get_physical_padding(id, &basic);

        let visible_size = self.calculate_visible_size(rect);
        let content_size = self.calculate_inner_content_size(visible_size, border, padding);

        // コンテンツサイズと内枠表示領域サイズの差分として、正確な最大スクロール量を算出
        let max_scroll_x = (scroll_size.width - content_size.width).max(0.0);
        let max_scroll_y = (scroll_size.height - content_size.height).max(0.0);

        x = x.clamp(0.0, max_scroll_x);
        y = y.clamp(0.0, max_scroll_y);

        // スロットが存在しない場合はあらかじめ挿入して初期化
        if !self.outputs.scroll_offsets.contains_key(id) {
            self.outputs.scroll_offsets.insert(id, LayoutPoint::ZERO);
        }

        let current = self.outputs.scroll_offsets.get_mut(id).unwrap();
        if (current.x - x).abs() > 0.01 || (current.y - y).abs() > 0.01 {
            current.x = x;
            current.y = y;

            // スクロールバー状態の最終スクロール時刻を更新
            if let Some(sb_state) = self.layouts.scrollbar_styles.get_mut(id) {
                sb_state.last_scroll_time = Some(Instant::now());
            }

            // オフセット変化に伴い、子孫全体の絶対座標を再同期させる
            self.mark_layout_dirty(id);
            true
        } else {
            false
        }
    }

    /// スクロールコンテナのスタイル設定に連動し、
    /// トラック・サムに相当する要素（Element）を遅延生成して親子関係にアタッチします。
    pub(crate) fn ensure_scrollbar_elements(
        &mut self,
        id: EntityId,
        sb: &ScrollbarStyle,
        merge: bool,
    ) {
        if !self.layouts.scrollbar_styles.contains_key(id) {
            self.layouts.scrollbar_styles.insert(
                id,
                ScrollBarState {
                    style: sb.clone(),
                    v_track_id: None,
                    v_thumb_id: None,
                    h_track_id: None,
                    h_thumb_id: None,
                    v_thumb_hovered: false,
                    v_thumb_dragged: false,
                    h_thumb_hovered: false,
                    h_thumb_dragged: false,
                    drag_start_mouse: LayoutPoint::ZERO,
                    drag_start_offset: LayoutPoint::ZERO,
                    last_scroll_time: None,
                },
            );
        }

        let mut state = self.layouts.scrollbar_styles.get(id).cloned().unwrap();
        state.style = sb.clone();
        let mut changed = false;

        if sb.display != ScrollbarDisplay::None {
            // A. 縦スクロールバー (V-Track)
            let v_track = if let Some(v_track) = state.v_track_id {
                v_track
            } else {
                let v_track = self.spawn(Some(id));
                self.add_child(id, v_track);
                state.v_track_id = Some(v_track);
                changed = true;
                v_track
            };

            // トラックは常に絶対配置（コンテナの右端に固定）
            let track_style = sb
                .v_track
                .clone()
                .unwrap_or_default()
                .absolute()
                .z_index(9999)
                .width(sb.width)
                .inset((0.0, 0.0, 0.0, crate::auto()))
                .pointer_events_auto(); // イベントを透過させない

            let el = Element::from(v_track);
            el.style_internal(self, track_style, merge);

            // A-1. 縦つまみ (V-Thumb、V-Track の子要素としてアタッチ)
            let v_thumb = if let Some(v_thumb) = state.v_thumb_id {
                v_thumb
            } else {
                let v_thumb = self.spawn(Some(v_track));
                self.add_child(v_track, v_thumb);
                state.v_thumb_id = Some(v_thumb);
                changed = true;
                v_thumb
            };

            let mut thumb_width = sb.width;
            if let Some(ref thumb_style) = sb.v_thumb
                && let Val::Px(w) = thumb_style.inner.basic_layout.size.width
            {
                thumb_width = w.min(sb.width);
            }

            // サムは V-Track の絶対座標を原点とし、Y方向のみ absolute スライド
            let thumb_style = sb
                .v_thumb
                .clone()
                .unwrap_or_default()
                .absolute()
                .width(thumb_width)
                .inset((0.0, crate::auto(), crate::auto(), crate::auto()))
                .pointer_events_auto();

            let el = Element::from(v_thumb);
            el.style_internal(self, thumb_style, merge);

            // B. 横スクロールバー (H-Track)
            let h_track = if let Some(h_track) = state.h_track_id {
                h_track
            } else {
                let h_track = self.spawn(Some(id));
                self.add_child(id, h_track);
                state.h_track_id = Some(h_track);
                changed = true;
                h_track
            };

            let track_style = sb
                .h_track
                .clone()
                .unwrap_or_default()
                .absolute()
                .z_index(9999)
                .height(sb.width)
                .inset((crate::auto(), 0.0, 0.0, 0.0))
                .pointer_events_auto();

            let el = Element::from(h_track);
            el.style_internal(self, track_style, merge);

            // B-1. 横つまみ (H-Thumb、H-Track の子要素としてアタッチ)
            let h_thumb = if let Some(h_thumb) = state.h_thumb_id {
                h_thumb
            } else {
                let h_thumb = self.spawn(Some(h_track));
                self.add_child(h_track, h_thumb);
                state.h_thumb_id = Some(h_thumb);
                changed = true;
                h_thumb
            };

            let mut thumb_height = sb.width;
            if let Some(ref thumb_style) = sb.h_thumb
                && let Val::Px(h) = thumb_style.inner.basic_layout.size.height
            {
                thumb_height = h.min(sb.width);
            }

            let thumb_style = sb
                .h_thumb
                .clone()
                .unwrap_or_default()
                .absolute()
                .height(thumb_height)
                .inset((crate::auto(), crate::auto(), crate::auto(), 0.0))
                .pointer_events_auto();

            let el = Element::from(h_thumb);
            el.style_internal(self, thumb_style, merge);
        }

        if changed {
            *self.layouts.scrollbar_styles.get_mut(id).unwrap() = state;
            self.layouts.is_structure_dirty = true; // flat_dfs_sequence の更新契機
        }
    }

    /// 現在のスクロール位置から相対移動します。
    pub fn scroll_by(&mut self, id: EntityId, dx: f32, dy: f32) -> bool {
        let current = self
            .outputs
            .scroll_offsets
            .get(id)
            .copied()
            .unwrap_or(LayoutPoint::ZERO);
        self.scroll_to(id, current.x + dx, current.y + dy)
    }

    /// 指定された要素が現在マウスホバーされているか判定します
    #[inline]
    pub fn is_hovered(&self, id: EntityId) -> bool {
        self.topology
            .active_masks
            .get(id)
            .map(|m| m.has(STATE_HOVERED))
            .unwrap_or(false)
    }

    /// 指定された要素が現在キーボードフォーカスを得ているか判定します
    #[inline]
    pub fn is_focused(&self, id: EntityId) -> bool {
        self.topology
            .active_masks
            .get(id)
            .map(|m| m.has(STATE_FOCUSED))
            .unwrap_or(false)
    }

    /// 指定された要素が現在マウスやタップで押し下げられているか判定します
    #[inline]
    pub fn is_pressed(&self, id: EntityId) -> bool {
        self.topology
            .active_masks
            .get(id)
            .map(|m| m.has(STATE_PRESSED))
            .unwrap_or(false)
    }

    /// 指定された要素が無効化（操作不可）状態にあるか判定します
    #[inline]
    pub fn is_disabled(&self, id: EntityId) -> bool {
        self.topology
            .active_masks
            .get(id)
            .map(|m| m.has(STATE_DISABLED))
            .unwrap_or(false)
    }

    /// 指定された要素が現在アクティブ（有効選択など）状態にあるか判定します
    #[inline]
    pub fn is_actived(&self, id: EntityId) -> bool {
        self.topology
            .active_masks
            .get(id)
            .map(|m| m.has(STATE_ACTIVED))
            .unwrap_or(false)
    }

    /// 指定された要素が現在テキストまたはトグル選択されているか判定します
    #[inline]
    pub fn is_selected(&self, id: EntityId) -> bool {
        self.topology
            .active_masks
            .get(id)
            .map(|m| m.has(STATE_SELECTED))
            .unwrap_or(false)
    }

    /// 指定された要素が現在ドラッグ操作中にあるか判定します
    #[inline]
    pub fn is_dragged(&self, id: EntityId) -> bool {
        self.topology
            .active_masks
            .get(id)
            .map(|m| m.has(STATE_DRAGGED))
            .unwrap_or(false)
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
        let mut on_click = self
            .events
            .event_listeners
            .get_mut(id)
            .and_then(|l| l.on_click.take());

        if let Some(mut handler) = on_click {
            let _guard = crate::ActiveElementGuard::new(id);
            handler(self);
            if let Some(l) = self.events.event_listeners.get_mut(id) {
                l.on_click = Some(handler);
            }
        }
    }

    /// 指定した要素の子要素一覧を取得します。
    pub fn children_list(&self, handle: Element) -> Option<Vec<Element>> {
        self.topology
            .children
            .get(handle.id)
            .map(|c| c.iter().map(|&id| Element { id }).collect())
    }

    /// 画面上でアクティブ（有効）になっている要素の総数を取得します。
    pub fn active_entities_count(&self) -> usize {
        self.topology.active_entities.len()
    }
}

#[cfg(test)]
mod tests;
