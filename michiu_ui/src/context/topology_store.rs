use crate::*;
use slotmap::{SecondaryMap, SlotMap};
use smallvec::SmallVec;

pub(crate) type EntitiesSlotMap = SlotMap<EntityId, ()>;
pub(crate) type ParentsSecondaryMap = SecondaryMap<EntityId, Option<EntityId>>;
pub(crate) type ChildrenSecondaryMap = SecondaryMap<EntityId, SmallVec<[EntityId; 4]>>;
pub(crate) type ActiveMasksSecondaryMap = SecondaryMap<EntityId, ComponentMask>;
pub(crate) type ActiveEntitiesVec = Vec<EntityId>;
pub(crate) type SessionSpawnedVec = Vec<EntityId>;
pub(crate) type SessionRootsVec = Vec<EntityId>;
pub(crate) type FlatDfsSequenceVec = Vec<EntityId>;

pub struct TopologyStore {
    /// 全要素の生存期間を管理するプライマリマップ
    pub(crate) entities: EntitiesSlotMap,
    /// 単方向の親ID参照。親子ポインタを排除した木構造の表現
    pub(crate) parents: ParentsSecondaryMap,
    /// 子要素のIDリスト。ヒープ割り当てを防ぐため SmallVec を採用
    pub(crate) children: ChildrenSecondaryMap,
    /// 各要素がどのSoAプロパティ（コンポーネント）を有効化しているかを示すビットマスク
    pub(crate) active_masks: ActiveMasksSecondaryMap,
    /// 画面に表示されているアクティブな全要素のIDを詰め込んだ1次元配列。
    /// 描画やイベント走査はこの1つの配列のみを回す。
    pub(crate) active_entities: ActiveEntitiesVec,
    /// 現在のビルドセッションで新しく生成（Spawn）された要素のリスト
    pub(crate) session_spawned: SessionSpawnedVec,
    /// セッション終了時に、親がいなくても破棄してはならないルート要素のリスト
    pub(crate) session_roots: SessionRootsVec,
    pub(crate) flat_dfs_sequence: FlatDfsSequenceVec,
    pub(crate) is_structure_dirty: bool,
}

impl Default for TopologyStore {
    fn default() -> Self {
        TopologyStore::new()
    }
}

impl TopologyStore {
    #[inline]
    pub fn new() -> Self {
        Self {
            entities: SlotMap::with_key(),
            parents: SecondaryMap::new(),
            children: SecondaryMap::new(),
            active_masks: SecondaryMap::new(),
            active_entities: Vec::new(),
            session_spawned: Vec::new(),
            session_roots: Vec::new(),
            flat_dfs_sequence: Vec::new(),
            is_structure_dirty: true,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.entities.clear();
        self.parents.clear();
        self.children.clear();
        self.active_masks.clear();
        self.active_entities.clear();
        self.flat_dfs_sequence.clear();
        self.is_structure_dirty = true;
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.entities.remove(id);
        self.parents.remove(id);
        self.active_masks.remove(id);
        // ダーティキュー、DFSシーケンス、アクティブ走査用の一時配列から
        // デスポーンされた無効な ID をその場で即時に抹消クリーンアップします。
        self.active_entities.retain(|&x| x != id);
        self.session_spawned.retain(|&x| x != id);
        self.session_roots.retain(|&x| x != id);
        self.flat_dfs_sequence.retain(|&x| x != id);
    }
}

impl TopologyStore {
    /// 親トポロジーから子要素をデタッチする
    #[inline]
    pub fn detach_from_parent(
        parents: &mut ParentsSecondaryMap,
        children: &mut ChildrenSecondaryMap,
        is_structure_dirty: &mut bool,
        child: EntityId,
    ) -> Option<EntityId> {
        if let Some(Some(parent_id)) = parents.get(child).copied() {
            if let Some(children_list) = children.get_mut(parent_id) {
                children_list.retain(|x| *x != child);
            }
            parents.insert(child, None);
            *is_structure_dirty = true;
            return Some(parent_id);
        }
        None
    }

    /// 新しい親子関係を結合する
    #[inline]
    pub fn attach_to_parent(
        parents: &mut ParentsSecondaryMap,
        children: &mut ChildrenSecondaryMap,
        is_structure_dirty: &mut bool,
        parent: EntityId,
        child: EntityId,
    ) {
        parents.insert(child, Some(parent));
        if let Some(children_list) = children.get_mut(parent)
            && !children_list.contains(&child)
        {
            children_list.push(child);
        }
        *is_structure_dirty = true;
    }

    /// 親要素の特定の古い子要素を、順序を維持したまま新しい子要素へ直接差し替える
    #[inline]
    pub fn replace_child_node(
        parents: &mut ParentsSecondaryMap,
        children: &mut ChildrenSecondaryMap,
        is_structure_dirty: &mut bool,
        parent: EntityId,
        old_child: EntityId,
        new_child: EntityId,
    ) {
        if let Some(children_list) = children.get_mut(parent)
            && let Some(pos) = children_list.iter().position(|&x| x == old_child)
        {
            children_list[pos] = new_child;
        }
        parents.insert(new_child, Some(parent));
        *is_structure_dirty = true;
    }

    /// DFS配列の高速再構築
    pub fn rebuild_dfs_sequence(
        children: &ChildrenSecondaryMap,
        flat_dfs_sequence: &mut FlatDfsSequenceVec,
        is_structure_dirty: &mut bool,
        root: EntityId,
    ) {
        flat_dfs_sequence.clear();
        let mut stack = Vec::with_capacity(32);
        stack.push(root);

        while let Some(id) = stack.pop() {
            flat_dfs_sequence.push(id);
            if let Some(children_list) = children.get(id) {
                let len = children_list.len();
                for i in (0..len).rev() {
                    stack.push(children_list[i]);
                }
            }
        }
        *is_structure_dirty = false;
    }

    /// 子孫要素のインタラクション状態（state_flag）を走査する純粋関連関数
    pub fn has_descendant_with_state(
        entities: &EntitiesSlotMap,
        children: &ChildrenSecondaryMap,
        active_masks: &ActiveMasksSecondaryMap,
        parent: EntityId,
        state_flag: u128,
    ) -> bool {
        let mut stack = SmallVec::<[EntityId; 16]>::new();

        if let Some(children_list) = children.get(parent) {
            for &child_id in children_list {
                stack.push(child_id);
            }
        }

        while let Some(child_id) = stack.pop() {
            if entities.contains_key(child_id)
                && let Some(mask) = active_masks.get(child_id)
                && mask.has(state_flag)
            {
                return true;
            }

            if let Some(children_list) = children.get(child_id) {
                for &next_child in children_list {
                    stack.push(next_child);
                }
            }
        }
        false
    }

    /// ウィンドウ内の最上位ルート要素の EntityId を自律解決して返します。
    #[inline]
    pub(crate) fn find_root_entity(
        entities: &EntitiesSlotMap,
        parents: &ParentsSecondaryMap,
        flat_dfs_sequence: &FlatDfsSequenceVec,
    ) -> Option<EntityId> {
        // すでにフラットシーケンスが構築されていればその先頭、
        // 無ければ parents マップをスキャンして親が None の生存要素をフォールバック解決します
        flat_dfs_sequence.first().copied().or_else(|| {
            parents
                .iter()
                .find(|&(id, &parent_id_opt)| {
                    // 親が None かつ、要素 id 自体が slotmap (entities) に生存しているか
                    parent_id_opt.is_none() && entities.contains_key(id)
                })
                .map(|(id, _)| id)
        })
    }

    /// 直近の親要素（1世代上）が特定のインタラクション状態を持っているか検証
    #[inline]
    pub(crate) fn has_parent_with_state(
        id: EntityId,
        parents: &ParentsSecondaryMap,
        entities: &EntitiesSlotMap,
        active_masks: &ActiveMasksSecondaryMap,
        state_flag: u128,
    ) -> bool {
        if let Some(Some(parent_id)) = parents.get(id).copied()
            && entities.contains_key(parent_id)
            && let Some(mask) = active_masks.get(parent_id)
            && mask.has(state_flag)
        {
            return true;
        }
        false
    }

    /// ドロップ先コンテナのフレックス方向に基づいて、
    /// マウスのドロップ座標がどの子要素の手前（インデックス）に位置するかを逆引き算出。
    pub(crate) fn calculate_insert_index(
        parent: EntityId,
        logical_pos: LayoutPoint,
        children: &ChildrenSecondaryMap,
        flex_layouts: &FlexLayoutsSecondaryMap,
        rects: &RectsSecondaryMap,
    ) -> usize {
        let mut insert_idx = 0;

        if let Some(children) = children.get(parent) {
            let parent_flex = flex_layouts.get(parent).copied().unwrap_or_default();
            let is_row = parent_flex.flex_direction == FlexDirection::Row
                || parent_flex.flex_direction == FlexDirection::RowReverse;

            for (idx, &child) in children.iter().enumerate() {
                if let Some(rect) = rects.get(child) {
                    if is_row {
                        let center_x = rect.x + rect.width * 0.5;
                        if logical_pos.x > center_x {
                            insert_idx = idx + 1;
                        }
                    } else {
                        let center_y = rect.y + rect.height * 0.5;
                        if logical_pos.y > center_y {
                            insert_idx = idx + 1;
                        }
                    }
                }
            }
        }

        insert_idx
    }

    /// 指定された要素（target）が、ある親要素（parent）自身、またはその子孫であるかを判定します。
    pub(crate) fn is_descendant_of(
        target: EntityId,
        parent: EntityId,
        topology: &TopologyStore,
    ) -> bool {
        if target == parent {
            return true;
        }
        let mut curr = target;
        while let Some(Some(p)) = topology.parents.get(curr) {
            if *p == parent {
                return true;
            }
            curr = *p;
        }
        false
    }

    pub(crate) fn compute_effective_z_indices(
        topology: &TopologyStore,
        layouts: &LayoutStore,
        renders: &RenderStore,
    ) -> SecondaryMap<EntityId, i32> {
        // 各要素の実効 z_index を親から子へカスケード（伝播）して計算
        let mut effective_z_indices = SecondaryMap::with_capacity(topology.active_entities.len());

        // flat_dfs_sequence は必ず親から子への順でフラットに並んでいるため、前方1方向の走査で完結
        for &id in &topology.flat_dfs_sequence {
            let self_z = renders.visual_properties.get(id).and_then(|v| v.z_index);

            let parent_z = topology
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

        effective_z_indices
    }
}

impl Context {
    /// 要素を新規に生成
    #[inline]
    pub(crate) fn spawn(&mut self, parent_id: Option<EntityId>) -> EntityId {
        let Context {
            topology, layouts, ..
        } = self;

        let id = topology.entities.insert(());
        topology.parents.insert(id, parent_id);
        topology.children.insert(id, SmallVec::new());
        topology.active_masks.insert(id, ComponentMask::new(0));
        topology.active_entities.push(id);
        topology.session_spawned.push(id);
        topology.is_structure_dirty = true;

        // Taffyノードとの同期
        let node = layouts
            .taffy
            .new_leaf_with_context(taffy::Style::default(), id)
            .unwrap();
        layouts.taffy_nodes.insert(id, node);

        self.mark_render_dirty(id);
        id
    }

    /// 親子関係の追加
    #[inline]
    pub(crate) fn add_child(&mut self, parent: EntityId, child: EntityId) {
        let Context {
            topology, layouts, ..
        } = self;

        // 子がすでに別の親に属している場合は、古い親からデタッチ
        if let Some(old_parent) = topology.parents.get(child).copied().flatten()
            && old_parent != parent
        {
            TopologyStore::detach_from_parent(
                &mut topology.parents,
                &mut topology.children,
                &mut topology.is_structure_dirty,
                child,
            );

            // 古い親の Taffy ノードから安全にデタッチ
            if let Some(&old_parent_node) = layouts.taffy_nodes.get(old_parent)
                && let Some(&child_node) = layouts.taffy_nodes.get(child)
                && let Ok(taffy_children) = layouts.taffy.children(old_parent_node)
                && taffy_children.contains(&child_node)
            {
                let _ = layouts.taffy.remove_child(old_parent_node, child_node);
            }

            // 古い親側の Taffy 順序とレイアウトを再同期して Dirty マーク
            LayoutStore::resync_taffy_children_order(old_parent, layouts, topology);
            LayoutStore::mark_layout_dirty(layouts, topology, old_parent);
        }

        // 新しい親へのトポロジーアタッチ
        TopologyStore::attach_to_parent(
            &mut topology.parents,
            &mut topology.children,
            &mut topology.is_structure_dirty,
            parent,
            child,
        );

        // 新しい親の Taffy ツリーの親子関係を永続的に更新
        if let Some(&parent_node) = layouts.taffy_nodes.get(parent)
            && let Some(&child_node) = layouts.taffy_nodes.get(child)
        {
            let _ = layouts.taffy.add_child(parent_node, child_node);
        }

        LayoutStore::mark_layout_dirty(layouts, topology, parent);
    }

    /// 3. 親要素の特定の古い子要素を新しい子要素へ直接差し替える
    #[inline]
    pub(crate) fn replace_child(
        &mut self,
        parent: EntityId,
        old_child: EntityId,
        new_child: EntityId,
    ) {
        let Context {
            topology, layouts, ..
        } = self;

        // Taffy ツリー側の同期（古いノードを外し、新しいノードをアタッチ）
        if let Some(&parent_node) = layouts.taffy_nodes.get(parent)
            && let Some(&new_node) = layouts.taffy_nodes.get(new_child)
        {
            let _ = layouts.taffy.add_child(parent_node, new_node);
        }

        TopologyStore::replace_child_node(
            &mut topology.parents,
            &mut topology.children,
            &mut topology.is_structure_dirty,
            parent,
            old_child,
            new_child,
        );

        // 古い子要素（およびその子孫）を完全に安全デスポーン
        self.despawn_internal(old_child);

        self.mark_layout_dirty(parent);
    }

    /// 非再帰スタックによるフラットDFS配列の構築
    #[inline]
    pub(crate) fn rebuild_flat_dfs_sequence(&mut self, root: EntityId) {
        let Context { topology, .. } = self;
        TopologyStore::rebuild_dfs_sequence(
            &topology.children,
            &mut topology.flat_dfs_sequence,
            &mut topology.is_structure_dirty,
            root,
        );
    }

    /// 子孫のインタラクション状態走査
    #[inline]
    pub(crate) fn has_descendant_with_state(&self, parent: EntityId, state_flag: u128) -> bool {
        let Context { topology, .. } = self;
        TopologyStore::has_descendant_with_state(
            &topology.entities,
            &topology.children,
            &topology.active_masks,
            parent,
            state_flag,
        )
    }

    /// いずれか一つのアクティブなユーザーインタラクションが子孫要素でONになっているか
    #[inline]
    pub(crate) fn has_descendant_with_any_active_state(&self, parent: EntityId) -> bool {
        self.has_descendant_with_state(parent, STYLE_ACTIVE_INTERACTION_PROPERTY)
    }

    /// 直近の親要素（1世代上）が特定のインタラクション状態を持っているか
    #[inline]
    pub(crate) fn has_parent_with_state(&self, id: EntityId, state_flag: u128) -> bool {
        let Context { topology, .. } = self;
        let TopologyStore {
            entities,
            parents,
            children,
            active_masks,
            ..
        } = topology;
        TopologyStore::has_parent_with_state(id, parents, entities, active_masks, state_flag)
    }

    /// 直近の親要素（1世代上）がいずれか一つのアクティブなユーザーインタラクション状態を満たしているか
    #[inline]
    pub(crate) fn has_parent_with_any_active_state(&self, id: EntityId) -> bool {
        self.has_parent_with_state(id, STYLE_ACTIVE_INTERACTION_PROPERTY)
    }

    /// ドロップ先コンテナのフレックス方向に基づいて、
    /// マウスのドロップ座標がどの子要素の手前（インデックス）に位置するかを逆引き算出。
    pub(crate) fn mouse_drop_insert_element_index(
        &self,
        parent: EntityId,
        logical_pos: LayoutPoint,
    ) -> usize {
        let Context {
            topology,
            layouts,
            outputs,
            ..
        } = self;

        TopologyStore::calculate_insert_index(
            parent,
            logical_pos,
            &self.topology.children,
            &self.layouts.flex_layouts,
            &self.outputs.rects,
        )
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
            // 親が存在しない
            let has_no_parent = self.topology.parents.get(id).copied().flatten().is_none();
            // ルート要素としても登録されていない
            let is_not_root = !self.topology.session_roots.contains(&id);

            if has_no_parent && is_not_root {
                self.despawn_internal(id);
            }
        }
        // ルートリストをクリア
        self.topology.session_roots.clear();
    }

    /// 要素を安全に破棄（Despawn）。親が消えた場合子はフレーム末尾のクリーンアップフェーズで自動修復・一掃
    #[inline]
    pub(crate) fn despawn_internal(&mut self, id: EntityId) {
        if !self.topology.entities.contains_key(id) {
            return;
        }

        // 親トポロジーおよび Taffy ツリーからのデタッチ
        if let Some(Some(parent_id)) = self.topology.parents.get(id) {
            if let Some(&parent_node) = self.layouts.taffy_nodes.get(*parent_id)
                && let Some(&child_node) = self.layouts.taffy_nodes.get(id)
                && let Ok(taffy_children) = self.layouts.taffy.children(parent_node)
                && taffy_children.contains(&child_node)
            {
                let _ = self.layouts.taffy.remove_child(parent_node, child_node);
            }

            if let Some(parent_children) = self.topology.children.get_mut(*parent_id) {
                parent_children.retain(|x| *x != id);
            }
        }

        // Taffy ノード自体の削除
        if let Some(node) = self.layouts.taffy_nodes.remove(id) {
            let _ = self.layouts.taffy.remove(node);
        }

        // 子要素を再帰的に削除
        if let Some(children_list) = self.topology.children.remove(id) {
            for child_id in children_list {
                self.despawn_internal(child_id);
            }
        }

        // 各ストアの SoA 配列から自分自身を一掃
        self.topology.despawn(id);
        self.layouts.despawn(id);
        self.renders.despawn(id);
        self.outputs.despawn(id);
        self.contents.despawn(id);
        self.events.despawn(id);
        self.reactive.despawn(id);
        self.window.despawn(id);
        self.system.despawn(id);
    }

    /// ウィンドウ内の最上位ルート要素の EntityId を自律解決して返します。
    #[inline]
    pub(crate) fn find_root_entity(&self) -> Option<EntityId> {
        let Context { topology, .. } = self;
        let TopologyStore {
            entities,
            parents,
            flat_dfs_sequence,
            ..
        } = topology;

        TopologyStore::find_root_entity(entities, parents, flat_dfs_sequence)
    }
}
