use crate::{
    ActiveInteractionStates, BaseVisualPropertiesSecondary, CapacityConfig, ClipRectsSecondary,
    ComponentMask, ContentStore, Context, DebugStore, DirtyLayoutEntitiesVec,
    DirtyRenderEntitiesVec, EntityId, EventStore, FlexLayoutsSecondary, IDENTITY_MATRIX,
    LayoutPoint, LayoutRect, LayoutSize, LayoutStore, MichiuSoA, MichiuTagRegistry, OptionTraceExt,
    OutputStore, PointerEvents, ReactiveStore, RectsSecondary, RenderStore, StateStore,
    SystemStore, TaffyNodesSecondary, TaffyResultTraceExt, TaffyTreeEntityId,
    VisualPropertiesSecondary, WindowStore, define_secondary, define_smallvec, define_vec,
};
#[cfg(feature = "trace-lifecycle")]
use crate::{MichiuTrace, trace_lifecycle};
use slotmap::{SecondaryMap, SlotMap};
use smallvec::SmallVec;
#[cfg(feature = "trace-lifecycle")]
use std::sync::Arc;

// ソート計算用
struct StackFrame {
    id: EntityId,
    matrix: [[f32; 4]; 4],
}

#[derive(
    Debug, Clone, Default, derive_more::Deref, derive_more::DerefMut, derive_more::IntoIterator,
)]
#[into_iterator(owned, ref, ref_mut)]
pub struct EntitiesSlot(pub(crate) SlotMap<EntityId, ()>);

define_secondary!(pub struct ParentsSecondary(Option<EntityId>));
define_secondary!(pub struct ChildrenSecondary(SmallVec<[EntityId; 8]>));
define_secondary!(pub struct ActiveMasksSecondary(ComponentMask));
define_secondary!(pub struct EffectiveZindicesSecondary(i32));

define_vec!(pub struct ActiveEntitiesVec(EntityId));
define_vec!(pub struct SessionSpawnedVec(EntityId));
define_vec!(pub struct FlatDfsSequenceVec(EntityId));
define_vec!(pub struct SortedEntitiesVec(EntityId));
define_vec!(pub struct SortCacheVec((EntityId, i32, u32)));

define_smallvec!(pub struct SessionRootsVec(EntityId, 4));
define_smallvec!(pub struct WebviewEntitiesVec(EntityId, 4));
define_smallvec!(pub struct DespawnedQueueVec(EntityId, 4));

pub struct TopologyStore {
    /// 全要素の生存期間を管理するプライマリマップ
    pub(crate) topo_entities: EntitiesSlot,
    /// 画面に表示されているアクティブな全要素のIDを詰め込んだ1次元配列。
    /// 描画やイベント走査はこの1つの配列のみを回す。
    pub(crate) topo_active_entities: ActiveEntitiesVec,
    /// 各要素がどのSoAプロパティ（コンポーネント）を有効化しているかを示すビットマスク
    pub(crate) topo_active_masks: ActiveMasksSecondary,
    /// 単方向の親ID参照。親子ポインタを排除した木構造の表現
    pub(crate) topo_parents: ParentsSecondary,
    /// 子要素のIDリスト。ヒープ割り当てを防ぐため `SmallVec` を採用
    pub(crate) topo_children: ChildrenSecondary,
    /// 現在のビルドセッションで新しく生成（Spawn）された要素のリスト
    pub(crate) topo_session_spawned: SessionSpawnedVec,
    /// セッション終了時に、親がいなくても破棄してはならないルート要素のリスト
    pub(crate) topo_session_roots: SessionRootsVec,
    pub(crate) topo_flat_dfs_sequence: FlatDfsSequenceVec,
    // 実効 z-index の作業用マップ
    pub(crate) topo_effective_z_indices: EffectiveZindicesSecondary,
    pub(crate) topo_sorted_entities: SortedEntitiesVec,
    pub(crate) topo_sort_cache: SortCacheVec,
    pub(crate) topo_is_structure_dirty: bool,
    pub(crate) topo_is_sort_dirty: bool,
    pub(crate) topo_webview_entities: WebviewEntitiesVec,
    pub(crate) topo_despawned_queue: DespawnedQueueVec,
    pub(crate) topo_tag_registry: MichiuTagRegistry,
}

impl Default for TopologyStore {
    fn default() -> Self {
        TopologyStore::new()
    }
}

impl TopologyStore {
    #[must_use]
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            topo_entities: EntitiesSlot(SlotMap::with_key()),
            topo_active_entities: ActiveEntitiesVec(Vec::new()),
            topo_active_masks: ActiveMasksSecondary(SecondaryMap::new()),
            topo_parents: ParentsSecondary(SecondaryMap::new()),
            topo_children: ChildrenSecondary(SecondaryMap::new()),
            topo_session_spawned: SessionSpawnedVec(Vec::new()),
            topo_session_roots: SessionRootsVec(SmallVec::new()),
            topo_flat_dfs_sequence: FlatDfsSequenceVec(Vec::new()),
            topo_effective_z_indices: EffectiveZindicesSecondary(SecondaryMap::new()),
            topo_sorted_entities: SortedEntitiesVec(Vec::new()),
            topo_sort_cache: SortCacheVec(Vec::new()),
            topo_is_structure_dirty: true,
            topo_is_sort_dirty: true,
            topo_webview_entities: WebviewEntitiesVec(SmallVec::new()),
            topo_despawned_queue: DespawnedQueueVec(SmallVec::new()),
            topo_tag_registry: MichiuTagRegistry::new(),
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn with_capacity(c: &CapacityConfig) -> Self {
        Self {
            topo_entities: EntitiesSlot(SlotMap::with_capacity_and_key(c.topo_entities)),
            topo_active_entities: ActiveEntitiesVec(Vec::with_capacity(c.topo_active_entities)),
            topo_active_masks: ActiveMasksSecondary(SecondaryMap::with_capacity(
                c.topo_active_masks,
            )),
            topo_parents: ParentsSecondary(SecondaryMap::with_capacity(c.topo_parents)),
            topo_children: ChildrenSecondary(SecondaryMap::with_capacity(c.topo_children)),
            topo_session_spawned: SessionSpawnedVec(Vec::with_capacity(c.topo_session_spawned)),
            topo_session_roots: SessionRootsVec(SmallVec::with_capacity(c.topo_session_roots)),
            topo_flat_dfs_sequence: FlatDfsSequenceVec(Vec::with_capacity(
                c.topo_flat_dfs_sequence,
            )),
            topo_effective_z_indices: EffectiveZindicesSecondary(SecondaryMap::with_capacity(
                c.topo_effective_z_indices,
            )),
            topo_sorted_entities: SortedEntitiesVec(Vec::with_capacity(c.topo_sorted_entities)),
            topo_sort_cache: SortCacheVec(Vec::with_capacity(c.topo_sort_cache)),
            topo_webview_entities: WebviewEntitiesVec(SmallVec::with_capacity(
                c.topo_webview_entities,
            )),
            topo_despawned_queue: DespawnedQueueVec(SmallVec::with_capacity(
                c.topo_despawned_queue,
            )),
            ..Default::default()
        }
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.topo_entities.clear();
        self.topo_active_entities.clear();
        self.topo_active_masks.clear();
        self.topo_parents.clear();
        self.topo_children.clear();
        self.topo_flat_dfs_sequence.clear();
        self.topo_effective_z_indices.clear();
        self.topo_sorted_entities.clear();
        self.topo_sort_cache.clear();
        self.topo_is_structure_dirty = true;
        self.topo_is_sort_dirty = true;
        self.topo_webview_entities.clear();
        self.topo_despawned_queue.clear();
    }

    #[inline]
    pub(crate) fn despawn(&mut self, id: EntityId) {
        self.topo_entities.remove(id);
        self.topo_active_entities.retain(|&x| x != id);
        self.topo_active_masks.remove(id);
        self.topo_parents.remove(id);
        self.topo_session_spawned.retain(|&x| x != id);
        self.topo_session_roots.retain(|x| *x != id);
        self.topo_flat_dfs_sequence.retain(|&x| x != id);
        self.topo_effective_z_indices.remove(id);
        self.topo_sorted_entities.retain(|&x| x != id);
        self.topo_sort_cache.retain(|&x| x.0 != id);
        self.topo_webview_entities.retain(|x| *x != id);
        self.topo_despawned_queue.retain(|x| *x != id);
        self.topo_tag_registry.unregister_entity(id);
    }
}

impl TopologyStore {
    /// 要素を新規に生成
    #[inline]
    pub(crate) fn spawn(
        parent_id: Option<EntityId>,
        topo_entities: &mut EntitiesSlot,
        topo_active_entities: &mut ActiveEntitiesVec,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &mut ParentsSecondary,
        topo_children: &mut ChildrenSecondary,
        topo_session_spawned: &mut SessionSpawnedVec,
        topo_is_structure_dirty: &mut bool,
        topo_is_sort_dirty: &mut bool,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &mut TaffyNodesSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        debug: &mut DebugStore,
    ) -> EntityId {
        let id = topo_entities.insert(());
        topo_parents.insert(id, parent_id);
        topo_children.insert(id, SmallVec::new());
        topo_active_masks.insert(id, ComponentMask::new(0));
        topo_active_entities.push(id);
        topo_session_spawned.push(id);
        // Taffyノードとの同期
        let node = lay_taffy_tree
            .new_leaf_with_context(taffy::Style::default(), id)
            .unwrap_or_trace(Some(id), debug);
        lay_taffy_nodes.insert(id, node);

        *topo_is_structure_dirty = true;
        *topo_is_sort_dirty = true;
        RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);

        id
    }

    /// 親子関係の追加
    #[inline]
    pub(crate) fn add_child(
        parent: EntityId,
        child: EntityId,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &mut ParentsSecondary,
        topo_children: &mut ChildrenSecondary,
        topo_is_structure_dirty: &mut bool,
        topo_is_sort_dirty: &mut bool,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &mut TaffyNodesSecondary,
        debug: &mut DebugStore,
    ) {
        // 子がすでに別の親に属している場合は、古い親からデタッチ
        if let Some(old_parent) = *topo_parents.at(child)
            && old_parent != parent
        {
            TopologyStore::detach_from_parent(
                child,
                topo_parents,
                topo_children,
                topo_is_structure_dirty,
                topo_is_sort_dirty,
            );

            // 古い親の Taffy ノードから安全にデタッチ
            let old_parent_node = *lay_taffy_nodes.at(old_parent);
            let child_node = *lay_taffy_nodes.at(child);

            if let Ok(taffy_children) = lay_taffy_tree.children(old_parent_node)
                && taffy_children.contains(&child_node)
            {
                lay_taffy_tree
                    .remove_child(old_parent_node, child_node)
                    .unwrap_or_trace(Some(old_parent), debug);
            }

            // 古い親側の Taffy 順序とレイアウトを再同期して Dirty マーク
            LayoutStore::resync_taffy_children_order(
                old_parent,
                topo_children,
                lay_taffy_tree,
                lay_taffy_nodes,
                debug,
            );
            LayoutStore::mark_layout_dirty(
                old_parent,
                topo_active_masks,
                topo_parents,
                lay_dirty_entities,
                lay_taffy_tree,
                lay_taffy_nodes,
                debug,
            );
        }

        // 新しい親へのトポロジーアタッチ
        TopologyStore::attach_to_parent(
            parent,
            child,
            topo_parents,
            topo_children,
            topo_is_structure_dirty,
            topo_is_sort_dirty,
        );

        // 新しい親の Taffy ツリーの親子関係を永続的に更新
        let parent_node = *lay_taffy_nodes.at(parent);
        let child_node = *lay_taffy_nodes.at(child);
        lay_taffy_tree
            .add_child(parent_node, child_node)
            .unwrap_or_trace(Some(parent), debug);

        LayoutStore::mark_layout_dirty(
            parent,
            topo_active_masks,
            topo_parents,
            lay_dirty_entities,
            lay_taffy_tree,
            lay_taffy_nodes,
            debug,
        );
    }

    /// 親要素の特定の古い子要素を新しい子要素へ直接差し替える
    #[inline]
    pub(crate) fn replace_child(
        parent: EntityId,
        old_child: EntityId,
        new_child: EntityId,
        window: &mut WindowStore,
        system: &mut SystemStore,
        reactive: &mut ReactiveStore,
        events: &mut EventStore,
        contents: &mut ContentStore,
        topology: &mut TopologyStore,
        states: &mut StateStore,
        layouts: &mut LayoutStore,
        renders: &mut RenderStore,
        outputs: &mut OutputStore,
        debug: &mut DebugStore,
    ) {
        // Taffy ツリー側の同期（古いノードを外し、新しいノードをアタッチ）
        let parent_node = *layouts.lay_taffy_nodes.at(parent);
        let new_node = *layouts.lay_taffy_nodes.at(new_child);
        layouts
            .lay_taffy_tree
            .add_child(parent_node, new_node)
            .unwrap_or_trace(Some(parent), debug);

        TopologyStore::replace_child_node(
            parent,
            old_child,
            new_child,
            &mut topology.topo_parents,
            &mut topology.topo_children,
            &mut topology.topo_is_structure_dirty,
            &mut topology.topo_is_sort_dirty,
        );

        // 古い子要素（およびその子孫）を完全に安全デスポーン
        TopologyStore::despawn_internal(
            old_child, window, system, reactive, events, contents, topology, states, layouts,
            renders, outputs, debug,
        );

        LayoutStore::mark_layout_dirty(
            parent,
            &mut topology.topo_active_masks,
            &topology.topo_parents,
            &mut layouts.lay_dirty_entities,
            &mut layouts.lay_taffy_tree,
            &layouts.lay_taffy_nodes,
            debug,
        );
    }

    /// 要素を安全に破棄（Despawn）。親が消えた場合子はフレーム末尾のクリーンアップフェーズで一掃
    #[inline]
    pub(crate) fn despawn_internal(
        id: EntityId,
        window: &mut WindowStore,
        system: &mut SystemStore,
        reactive: &mut ReactiveStore,
        events: &mut EventStore,
        contents: &mut ContentStore,
        topology: &mut TopologyStore,
        states: &mut StateStore,
        layouts: &mut LayoutStore,
        renders: &mut RenderStore,
        outputs: &mut OutputStore,
        debug: &mut DebugStore,
    ) {
        if !topology.topo_entities.contains_key(id) {
            return;
        }

        // キャッシュクリアの遅延処理用にキューに記録
        topology.topo_despawned_queue.push(id);

        topology.topo_is_structure_dirty = true;
        topology.topo_is_sort_dirty = true;

        // 親トポロジーおよび Taffy ツリーからのデタッチ
        // 自身がルート要素の場合親は None
        if let Some(parent_id) = *topology.topo_parents.at(id) {
            // 親も自分もレイアウトノードを持っている場合のみTaffyツリーからのデタッチ
            if let Some(&parent_node) = layouts.lay_taffy_nodes.find(parent_id) {
                let child_node = *layouts.lay_taffy_nodes.at(id); // 自分はあるはず
                let taffy_children = layouts
                    .lay_taffy_tree
                    .children(parent_node)
                    .unwrap_or_trace(Some(parent_id), debug);
                if taffy_children.contains(&child_node) {
                    layouts
                        .lay_taffy_tree
                        .remove_child(parent_node, child_node)
                        .unwrap_or_trace(Some(parent_id), debug);
                }
            }

            // 親がまだ生きていれば外す
            if let Some(parent_children) = topology.topo_children.find_mut(parent_id) {
                parent_children.retain(|x| *x != id);
            }
        }

        // Taffy ノード自体の削除
        if let Some(node) = layouts.lay_taffy_nodes.remove(id) {
            let _ = layouts.lay_taffy_tree.remove(node);
        }

        // 子要素を再帰的に削除
        if let Some(children_list) = topology.topo_children.remove(id) {
            for child_id in children_list {
                TopologyStore::despawn_internal(
                    child_id, window, system, reactive, events, contents, topology, states,
                    layouts, renders, outputs, debug,
                );
            }
        }

        // 各ストアの SoA 配列から自分自身を一掃
        topology.despawn(id);
        states.despawn(id);
        layouts.despawn(id);
        renders.despawn(id);
        outputs.despawn(id);
        contents.despawn(id);
        events.despawn(id);
        reactive.despawn(id);
        window.despawn(id);
        system.despawn(id);
    }

    /// セッションのクリーンアップを実行
    #[inline]
    pub(crate) fn end_session(
        start_marker: usize,
        window: &mut WindowStore,
        system: &mut SystemStore,
        reactive: &mut ReactiveStore,
        events: &mut EventStore,
        contents: &mut ContentStore,
        topology: &mut TopologyStore,
        states: &mut StateStore,
        layouts: &mut LayoutStore,
        renders: &mut RenderStore,
        outputs: &mut OutputStore,
        debug: &mut DebugStore,
    ) {
        // start_marker 以降に生成された要素をスキャン
        let spawned_in_session: Vec<EntityId> = topology
            .topo_session_spawned
            .drain(start_marker..)
            .collect();

        for id in spawned_in_session {
            // 親が存在しない
            let has_no_parent = topology.topo_parents.at(id).is_none();
            // ルート要素としても登録されていない
            let is_not_root = !topology.topo_session_roots.contains(&id);

            if has_no_parent && is_not_root {
                TopologyStore::despawn_internal(
                    id, window, system, reactive, events, contents, topology, states, layouts,
                    renders, outputs, debug,
                );
            }
        }
        // ルートリストをクリア
        topology.topo_session_roots.clear();
    }

    /// 親トポロジーから子要素をデタッチする
    #[inline]
    pub(crate) fn detach_from_parent(
        child: EntityId,
        topo_parents: &mut ParentsSecondary,
        topo_children: &mut ChildrenSecondary,
        topo_is_structure_dirty: &mut bool,
        topo_is_sort_dirty: &mut bool,
    ) -> Option<EntityId> {
        let parent_id = (*topo_parents.at(child))?;
        topo_children.at_mut(parent_id).retain(|x| *x != child);
        topo_parents.insert(child, None);
        *topo_is_structure_dirty = true;
        *topo_is_sort_dirty = true;
        Some(parent_id)
    }

    /// 新しい親子関係を結合する
    #[inline]
    pub(crate) fn attach_to_parent(
        parent: EntityId,
        child: EntityId,
        topo_parents: &mut ParentsSecondary,
        topo_children: &mut ChildrenSecondary,
        topo_is_structure_dirty: &mut bool,
        topo_is_sort_dirty: &mut bool,
    ) {
        topo_parents.insert(child, Some(parent));
        let children_list = topo_children.at_mut(parent);
        if !children_list.contains(&child) {
            children_list.push(child);
        }
        *topo_is_structure_dirty = true;
        *topo_is_sort_dirty = true;
    }

    /// 親要素の特定の古い子要素を、順序を維持したまま新しい子要素へ直接差し替える
    #[inline]
    pub(crate) fn replace_child_node(
        parent: EntityId,
        old_child: EntityId,
        new_child: EntityId,
        topo_parents: &mut ParentsSecondary,
        topo_children: &mut ChildrenSecondary,
        topo_is_structure_dirty: &mut bool,
        topo_is_sort_dirty: &mut bool,
    ) {
        if let Some(child) = topo_children
            .at_mut(parent)
            .iter_mut()
            .find(|x| **x == old_child)
        {
            *child = new_child;
        }
        topo_parents.insert(new_child, Some(parent));
        *topo_is_structure_dirty = true;
        *topo_is_sort_dirty = true;
    }

    /// DFS配列の高速再構築
    #[allow(unused)]
    #[inline]
    pub(crate) fn rebuild_dfs_sequence(
        root: EntityId,
        topo_flat_dfs_sequence: &mut FlatDfsSequenceVec,
        topo_is_structure_dirty: &mut bool,
        topo_children: &ChildrenSecondary,
        debug: &mut DebugStore,
    ) {
        topo_flat_dfs_sequence.clear();
        let mut stack = smallvec::SmallVec::<[EntityId; 32]>::new();
        stack.push(root);

        while let Some(id) = stack.pop() {
            topo_flat_dfs_sequence.push(id);

            let children_list = topo_children.at(id);

            let len = children_list.len();
            for i in (0..len).rev() {
                stack.push(children_list[i]);
            }
        }
        *topo_is_structure_dirty = false;

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, debug, || MichiuTrace::Dfs {
            after: Arc::from(topo_flat_dfs_sequence.0.clone()),
            add: Some("Here, the is_structure_dirty flag changes to false.")
        });
    }

    /// 子孫要素のインタラクション状態を走査
    #[inline]
    #[must_use]
    pub(crate) fn has_descendant_with_state(
        parent: EntityId,
        state_flag: u128,
        topo_entities: &EntitiesSlot,
        topo_active_masks: &ActiveMasksSecondary,
        topo_children: &ChildrenSecondary,
    ) -> bool {
        let mut stack = SmallVec::<[EntityId; 16]>::new();

        let list = topo_children.at(parent);
        stack.extend(list.iter().copied());

        while let Some(child_id) = stack.pop() {
            if topo_entities.contains_key(child_id)
                && topo_active_masks.at(child_id).has(state_flag)
            {
                return true;
            }

            let list = topo_children.at(child_id);
            stack.extend(list.iter().copied());
        }
        false
    }

    /// ウィンドウ内の最上位ルート要素の `EntityId` を自律解決して返します。
    #[inline]
    pub(crate) fn find_root_entity(
        topo_entities: &EntitiesSlot,
        topo_parents: &ParentsSecondary,
        topo_flat_dfs_sequence: &FlatDfsSequenceVec,
    ) -> Option<EntityId> {
        // すでにフラットシーケンスが構築されていればその先頭、
        // 無ければ topo_parents マップをスキャンして親が None の生存要素をフォールバック解決します
        topo_flat_dfs_sequence.first().copied().or_else(|| {
            topo_parents
                .iter()
                .find(|&(id, &parent_id)| {
                    // 親が None かつ、要素 id 自体が topo_entities に生存しているか
                    parent_id.is_none() && topo_entities.contains_key(id)
                })
                .map(|(id, _)| id)
        })
    }

    /// 直近の親要素（1世代上）が特定のインタラクション状態を持っているか検証
    #[inline]
    pub(crate) fn has_parent_with_state(
        id: EntityId,
        state_flag: u128,
        topo_entities: &EntitiesSlot,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
    ) -> bool {
        let Some(parent_id) = *topo_parents.at(id) else {
            return false;
        };
        if !topo_entities.contains_key(parent_id) {
            return false;
        }
        let mask = topo_active_masks.at(parent_id);
        mask.has(state_flag)
    }

    /// ドロップ先コンテナのフレックス方向に基づいて、
    /// マウスのドロップ座標がどの子要素の手前（インデックス）に位置するかを逆引き算出。
    #[inline]
    pub(crate) fn calculate_insert_index(
        parent: EntityId,
        logical_pos: LayoutPoint,
        topo_children: &ChildrenSecondary,
        lay_flex: &FlexLayoutsSecondary,
        out_rects: &RectsSecondary,
    ) -> usize {
        let is_row = lay_flex.flex_direction(parent).is_row();

        let mut insert_idx = 0;

        for (idx, &child) in topo_children.at(parent).iter().enumerate() {
            let rect = *out_rects.at(child);

            // 縦・横の判定
            let (mouse_pos, center_pos) = if is_row {
                (logical_pos.x, rect.x + rect.width * 0.5)
            } else {
                (logical_pos.y, rect.y + rect.height * 0.5)
            };

            if mouse_pos > center_pos {
                insert_idx = idx + 1;
            }
        }

        insert_idx
    }

    /// 指定された要素（target）が、ある親要素（parent）自身、またはその子孫であるかを判定します。
    #[inline]
    pub(crate) fn is_descendant_of(
        target: EntityId,
        parent: EntityId,
        topo_parents: &ParentsSecondary,
    ) -> bool {
        if target == parent {
            return true;
        }
        let mut curr = target;
        while let Some(p) = *topo_parents.at(curr) {
            if p == parent {
                return true;
            }
            curr = p;
        }
        false
    }

    /// 実効 `z_index` の計算と、それに基づく要素のソート
    #[inline]
    pub(crate) fn prepare_sorted_entities(
        win_last_size: Option<LayoutSize>,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_effective_z_indices: &mut EffectiveZindicesSecondary,
        topo_sorted_entities: &mut SortedEntitiesVec,
        topo_sort_cache: &mut SortCacheVec,
        topo_is_sort_dirty: &mut bool,
        topo_parents: &ParentsSecondary,
        topo_flat_dfs_sequence: &FlatDfsSequenceVec,
        rnd_visual: &VisualPropertiesSecondary,
        out_clip_rects: &mut ClipRectsSecondary,
        out_rects: &RectsSecondary,
        debug: &mut DebugStore,
    ) {
        if !*topo_is_sort_dirty {
            return;
        }

        let mut stack = smallvec::SmallVec::<[StackFrame; 32]>::new();
        let window_size = win_last_size.unwrap_or_default_trace(None, debug);
        let default_clip = LayoutRect::new(0.0, 0.0, window_size.width, window_size.height);

        topo_effective_z_indices.clear();
        // 忘れてたンゴ～
        topo_sort_cache.clear();

        // DFS順配列を使って可視性フラグを高速に伝播、および出現インデックスの記録
        for (index, &id) in topo_flat_dfs_sequence.iter().enumerate() {
            let parent_id = *topo_parents.at(id);

            // 親がスタックのトップに一致するまで遡る
            while let Some(top) = stack.last() {
                if parent_id == Some(top.id) {
                    break;
                }
                stack.pop();
            }

            // 親から累積された行列を引き継ぐ
            let parent_matrix = match stack.last() {
                Some(top) => top.matrix,
                None => IDENTITY_MATRIX,
            };

            // 実効 z_index のカスケード計算
            let self_z = rnd_visual.find(id).and_then(|v| v.z_index);
            let parent_z = parent_id.and_then(|pid| topo_effective_z_indices.find(pid).copied());
            let eff_z = self_z.or(parent_z).unwrap_or(0);
            topo_effective_z_indices.insert(id, eff_z);

            // トランスフォームのインライン累積
            let (self_transform, transform_inherit) = match rnd_visual.find(id) {
                Some(v) => (
                    v.transform.unwrap_or(IDENTITY_MATRIX),
                    v.transform_inherit.unwrap_or(false),
                ),
                None => (IDENTITY_MATRIX, false),
            };

            // 親から受け取った parent_matrix を左から掛けることで、
            // Column-Major カスケード（親の累積 * 自身）と数学的に一致
            let eff_matrix = if transform_inherit {
                OutputStore::mul_4x4(&parent_matrix, &self_transform)
            } else {
                self_transform
            };

            // クリップ矩形のインライン累積
            let rect = *out_rects.at(id);
            let eff_clip = *out_clip_rects.find_or(id, &default_clip, debug);

            // トランスフォームの適用されているブランチか伝播判定
            let is_parent_transform = parent_id.is_some_and(|p| {
                topo_active_masks
                    .at(p)
                    .has(ComponentMask::STATE_TRANSFORM_ACTIVE)
            });
            let has_self_transform = rnd_visual.find(id).is_some_and(|v| v.transform.is_some());
            let is_transform_active = is_parent_transform || has_self_transform;

            if is_transform_active {
                topo_active_masks
                    .at_mut(id)
                    .set(ComponentMask::STATE_TRANSFORM_ACTIVE);
            } else {
                topo_active_masks
                    .at_mut(id)
                    .unset(ComponentMask::STATE_TRANSFORM_ACTIVE);
            }

            // カリング判定用の AABB の取得と交差判定
            let bounding_box = if is_transform_active {
                // トランスフォームがある場合のみ、親に遡って実効トランスフォームを解決し、
                // 4頂点アフィン変換を施した正確な AABB を算出
                OutputStore::calc_aabb(rect, &eff_matrix)
            } else {
                rect // トランスフォームが無い大半の要素は元の rect をそのまま使用
            };

            // 親の可視性フラグのチェック
            let is_parent_invisible = parent_id.is_some_and(|p| {
                !topo_active_masks
                    .at(p)
                    .has(ComponentMask::STATE_RENDER_VISIBLE)
            });

            // Bounding Box と クリップの交差矩形
            let intersect = bounding_box.intersect(&eff_clip);
            let is_self_invisible = intersect.width <= 0.0 || intersect.height <= 0.0;

            let is_visible = !is_parent_invisible && !is_self_invisible;

            if is_visible {
                topo_active_masks
                    .at_mut(id)
                    .set(ComponentMask::STATE_RENDER_VISIBLE);
                topo_sort_cache.push((id, eff_z, index as u32));
            } else {
                topo_active_masks
                    .at_mut(id)
                    .unset(ComponentMask::STATE_RENDER_VISIBLE);
            }

            stack.push(StackFrame {
                id,
                matrix: eff_matrix,
            });
        }

        // 抽出された可視要素のみを z_index と出現順でソート
        topo_sort_cache.sort_unstable_by_key(|&(_, z, dfs)| (z, dfs));

        // ソート結果から ID 配列を再構成
        topo_sorted_entities.clear();
        topo_sorted_entities.extend(topo_sort_cache.iter().map(|&(id, _, _)| id));

        *topo_is_sort_dirty = false;

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, debug, || MichiuTrace::Sorted {
            after: Arc::from(topo_sorted_entities.0.clone()),
            add: Some("Here, the is_sort_dirty flag changes to false.")
        });
    }

    #[inline]
    pub(crate) fn restore_child(
        src_id: EntityId,
        holder: EntityId,
        topo_parents: &mut ParentsSecondary,
        topo_children: &mut ChildrenSecondary,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &mut TaffyNodesSecondary,
        debug: &mut DebugStore,
    ) {
        for child_id in topo_children.at(holder).clone() {
            // 子要素の親ポインタを元の要素に書き戻し
            topo_parents.insert(child_id, Some(src_id));

            // 元の要素の子要素リストへ復旧
            topo_children.at_mut(src_id).push(child_id);

            // Taffy 側の親子構造も、元の要素に繋ぎ戻し
            let src_node = *lay_taffy_nodes.at(src_id);
            let ph_node = *lay_taffy_nodes.at(holder);
            let child_node = *lay_taffy_nodes.at(child_id);

            lay_taffy_tree
                .remove_child(ph_node, child_node)
                .unwrap_or_trace(Some(child_id), debug);
            lay_taffy_tree
                .add_child(src_node, child_node)
                .unwrap_or_trace(Some(child_id), debug);
        }
    }

    #[inline]
    pub(crate) fn mark_dirty(
        id: EntityId,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        debug: &mut DebugStore,
    ) {
        LayoutStore::mark_layout_dirty(
            id,
            topo_active_masks,
            topo_parents,
            lay_dirty_entities,
            lay_taffy_tree,
            lay_taffy_nodes,
            debug,
        );
        RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);
    }

    /// マウス座標などが、要素の描画領域かつ表示枠内に収まっているかを判定。
    /// 階層的な早期枝刈りヒットテスト
    #[inline]
    pub(crate) fn hit_test(
        point: LayoutPoint,
        win_last_size: Option<LayoutSize>,
        evt_interaction_states: &ActiveInteractionStates,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_effective_z_indices: &mut EffectiveZindicesSecondary,
        topo_sorted_entities: &mut SortedEntitiesVec,
        topo_sort_cache: &mut SortCacheVec,
        topo_is_sort_dirty: &mut bool,
        topo_parents: &ParentsSecondary,
        topo_flat_dfs_sequence: &FlatDfsSequenceVec,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        out_clip_rects: &mut ClipRectsSecondary,
        out_rects: &RectsSecondary,
        debug: &mut DebugStore,
    ) -> Option<EntityId> {
        TopologyStore::prepare_sorted_entities(
            win_last_size,
            topo_active_masks,
            topo_effective_z_indices,
            topo_sorted_entities,
            topo_sort_cache,
            topo_is_sort_dirty,
            topo_parents,
            topo_flat_dfs_sequence,
            rnd_visual,
            out_clip_rects,
            out_rects,
            debug,
        );
        for &id in topo_sorted_entities.iter().rev() {
            let is_drag_over = topo_active_masks
                .at(id)
                .has(ComponentMask::STATE_DND_DRAG_OVER);

            // ドラッグ中かつゴースト化した元の実体要素、およびプレースホルダー要素はヒットテストを強制スルーさせる
            if Some(id) == evt_interaction_states.dragged || is_drag_over {
                continue;
            }

            // 物理範囲に含まれているか
            if !out_rects.at(id).contains(point) {
                continue;
            }

            // 親などの overflow 等でクリップされている表示範囲外ならスキップ
            if !out_clip_rects.at(id).contains(point) {
                continue;
            }

            // pointer-events 設定の解決
            let pointer_events = rnd_visual.pointer_events(id, rnd_base_visual);
            if pointer_events == PointerEvents::None {
                continue; // 透過設定
            }

            #[cfg(feature = "trace-lifecycle")]
            trace_lifecycle!(None, debug, || MichiuTrace::HitTest {
                found: Some(id),
                hit_x: point.x,
                hit_y: point.y,
                add: None
            });

            return Some(id);
        }

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, debug, || MichiuTrace::HitTest {
            found: None,
            hit_x: point.x,
            hit_y: point.y,
            add: None
        });

        None
    }

    #[inline]
    pub fn active_entities_mut(&mut self) -> &mut ActiveEntitiesVec {
        &mut self.topo_active_entities
    }

    #[inline]
    pub fn active_masks_mut(&mut self) -> &mut ActiveMasksSecondary {
        &mut self.topo_active_masks
    }

    #[inline]
    pub fn children_mut(&mut self) -> &mut ChildrenSecondary {
        &mut self.topo_children
    }

    #[inline]
    pub fn despawned_queue_mut(&mut self) -> &mut DespawnedQueueVec {
        &mut self.topo_despawned_queue
    }

    #[inline]
    pub fn effective_z_indices_mut(&mut self) -> &mut EffectiveZindicesSecondary {
        &mut self.topo_effective_z_indices
    }

    #[inline]
    pub fn entities_mut(&mut self) -> &mut EntitiesSlot {
        &mut self.topo_entities
    }

    #[inline]
    pub fn flat_dfs_sequence_mut(&mut self) -> &mut FlatDfsSequenceVec {
        &mut self.topo_flat_dfs_sequence
    }

    #[inline]
    pub fn is_sort_dirty_mut(&mut self) -> &mut bool {
        &mut self.topo_is_sort_dirty
    }

    #[inline]
    pub fn is_structure_dirty_mut(&mut self) -> &mut bool {
        &mut self.topo_is_structure_dirty
    }

    #[inline]
    pub fn parents_mut(&mut self) -> &mut ParentsSecondary {
        &mut self.topo_parents
    }

    #[inline]
    pub fn session_roots_mut(&mut self) -> &mut SessionRootsVec {
        &mut self.topo_session_roots
    }

    #[inline]
    pub fn session_spawned_mut(&mut self) -> &mut SessionSpawnedVec {
        &mut self.topo_session_spawned
    }

    #[inline]
    pub fn sort_cache_mut(&mut self) -> &mut SortCacheVec {
        &mut self.topo_sort_cache
    }

    #[inline]
    pub fn sorted_entities_mut(&mut self) -> &mut SortedEntitiesVec {
        &mut self.topo_sorted_entities
    }

    #[inline]
    pub fn tag_registry_mut(&mut self) -> &mut MichiuTagRegistry {
        &mut self.topo_tag_registry
    }

    #[inline]
    pub fn webview_entities_mut(&mut self) -> &mut WebviewEntitiesVec {
        &mut self.topo_webview_entities
    }
}

impl ActiveMasksSecondary {}

impl Context {
    /// 要素を新規に生成
    #[inline]
    pub(crate) fn spawn(&mut self, parent_id: Option<EntityId>) -> EntityId {
        TopologyStore::spawn(
            parent_id,
            &mut self.topology.topo_entities,
            &mut self.topology.topo_active_entities,
            &mut self.topology.topo_active_masks,
            &mut self.topology.topo_parents,
            &mut self.topology.topo_children,
            &mut self.topology.topo_session_spawned,
            &mut self.topology.topo_is_structure_dirty,
            &mut self.topology.topo_is_sort_dirty,
            &mut self.layouts.lay_taffy_tree,
            &mut self.layouts.lay_taffy_nodes,
            &mut self.renders.rnd_dirty_entities,
            &mut self.debug,
        )
    }

    /// 親子関係の追加
    #[inline]
    pub(crate) fn add_child(&mut self, parent: EntityId, child: EntityId) {
        TopologyStore::add_child(
            parent,
            child,
            &mut self.topology.topo_active_masks,
            &mut self.topology.topo_parents,
            &mut self.topology.topo_children,
            &mut self.topology.topo_is_structure_dirty,
            &mut self.topology.topo_is_sort_dirty,
            &mut self.layouts.lay_dirty_entities,
            &mut self.layouts.lay_taffy_tree,
            &mut self.layouts.lay_taffy_nodes,
            &mut self.debug,
        );
    }

    /// 親要素の特定の古い子要素を新しい子要素へ直接差し替える
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
            &mut self.debug,
        );
    }

    // セッションの開始マーカーを取得
    #[inline]
    pub(crate) fn start_session(&mut self) -> usize {
        self.topology.topo_session_spawned.len()
    }

    // ルート要素として保護するIDを登録
    #[inline]
    pub(crate) fn register_root(&mut self, id: EntityId) {
        self.topology.topo_session_roots.push(id);
        self.debug.dbg_root = Some(id);
    }

    // セッションのクリーンアップを実行
    #[inline]
    pub(crate) fn end_session(&mut self, start_marker: usize) {
        TopologyStore::end_session(
            start_marker,
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
            &mut self.debug,
        );
    }

    /// 要素を安全に破棄（Despawn）。親が消えた場合子はフレーム末尾のクリーンアップフェーズで自動修復・一掃
    #[inline]
    pub(crate) fn despawn_internal(&mut self, id: EntityId) {
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
            &mut self.debug,
        );
    }

    /// ウィンドウ内の最上位ルート要素の `EntityId` を自律解決して返します。
    #[inline]
    pub(crate) fn find_root_entity(&self) -> Option<EntityId> {
        TopologyStore::find_root_entity(
            &self.topology.topo_entities,
            &self.topology.topo_parents,
            &self.topology.topo_flat_dfs_sequence,
        )
    }
}

#[cfg(test)]
mod tests;
