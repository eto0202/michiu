use crate::*;
use slotmap::{SecondaryMap, SlotMap};
use smallvec::SmallVec;

pub struct TopologyStore {
    /// 全要素の生存期間を管理するプライマリマップ
    pub(crate) entities: SlotMap<EntityId, ()>,
    /// 単方向の親ID参照。親子ポインタを排除した木構造の表現
    pub(crate) parents: SecondaryMap<EntityId, Option<EntityId>>,
    /// 子要素のIDリスト。ヒープ割り当てを防ぐため SmallVec を採用
    pub(crate) children: SecondaryMap<EntityId, SmallVec<[EntityId; 4]>>,
    /// 各要素がどのSoAプロパティ（コンポーネント）を有効化しているかを示すビットマスク
    pub(crate) active_masks: SecondaryMap<EntityId, ComponentMask>,
    /// 画面に表示されているアクティブな全要素のIDを詰め込んだ1次元配列。
    /// 描画やイベント走査はこの1つの配列のみを回す。
    pub(crate) active_entities: Vec<EntityId>,
    /// 現在のビルドセッションで新しく生成（Spawn）された要素のリスト
    pub(crate) session_spawned: Vec<EntityId>,
    /// セッション終了時に、親がいなくても破棄してはならないルート要素のリスト
    pub(crate) session_roots: Vec<EntityId>,
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
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.entities.clear();
        self.parents.clear();
        self.children.clear();
        self.active_masks.clear();
        self.active_entities.clear();
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
    }
}

impl TopologyStore {
    /// 要素を新規に生成（Spawn）
    pub(crate) fn spawn(
        parent_id: Option<EntityId>,
        topology: &mut TopologyStore,
        layouts: &mut LayoutStore,
        renders: &mut RenderStore,
    ) -> EntityId {
        let id = topology.entities.insert(());

        topology.parents.insert(id, parent_id);
        topology.children.insert(id, SmallVec::new());
        topology.active_masks.insert(id, ComponentMask::new(0)); // 初期状態はどのプロパティも無効

        // Leaf ノード作成時に、Context として自分自身の ID を登録する
        let node = layouts
            .taffy
            .new_leaf_with_context(taffy::Style::default(), id)
            .unwrap();
        layouts.taffy_nodes.insert(id, node);

        topology.active_entities.push(id);

        // 新規作成された要素は、当然レイアウトと描画の対象となる
        // カスタムスタイルが当てられるまではデフォルト（Style::default）を再利用するため
        // mark_layout_dirty(id) の呼び出しを完全にスキップして、Taffyへの無駄な伝播をカット
        TopologyStore::mark_render_dirty(id, topology, renders);
        layouts.is_structure_dirty = true; // 構造変化をマーク

        topology.session_spawned.push(id);

        id
    }
    /// 親子関係の追加と、永続Taffy構造のリアルタイム同期。
    /// 子がすでに別の親に属している場合は古い親からデタッチします。
    pub(crate) fn add_child(
        parent: EntityId,
        child: EntityId,
        topology: &mut TopologyStore,
        layouts: &mut LayoutStore,
    ) {
        // 子がすでに別の親に属しているか検証
        if let Some(Some(old_parent)) = topology.parents.get(child).copied()
            && old_parent != parent
        {
            // 1. 古い親の children SoA リストから自分自身を安全に削除
            if let Some(old_children) = topology.children.get_mut(old_parent) {
                old_children.retain(|x| *x != child);
            }

            // 2. 古い親の Taffy ノードから安全にデタッチ
            if let Some(&old_parent_node) = layouts.taffy_nodes.get(old_parent)
                && let Some(&child_node) = layouts.taffy_nodes.get(child)
                && let Ok(taffy_children) = layouts.taffy.children(old_parent_node)
                && taffy_children.contains(&child_node)
            {
                let _ = layouts.taffy.remove_child(old_parent_node, child_node);
            }

            // 3. 古い親側の Taffy 順序とレイアウトを再同期して Dirty マーク
            LayoutStore::resync_taffy_children_order(old_parent, layouts, topology);
            TopologyStore::mark_layout_dirty(old_parent, topology, layouts);
        }

        // 新しい親の親子関係を更新
        topology.parents.insert(child, Some(parent));
        if let Some(children_list) = topology.children.get_mut(parent)
            && !children_list.contains(&child)
        {
            children_list.push(child);
        }

        // 新しい親の Taffy ツリーの親子関係を永続的に更新
        if let Some(&parent_node) = layouts.taffy_nodes.get(parent)
            && let Some(&child_node) = layouts.taffy_nodes.get(child)
        {
            let _ = layouts.taffy.add_child(parent_node, child_node);
        }

        TopologyStore::mark_layout_dirty(parent, topology, layouts);
        layouts.is_structure_dirty = true;
    }

    /// レイアウト変更フラグを立てる（Taffy同期要求）
    pub(crate) fn mark_layout_dirty(
        id: EntityId,
        topology: &mut TopologyStore,
        layouts: &mut LayoutStore,
    ) {
        let mut curr = id;
        // Taffy 側の該当ノードのレイアウトキャッシュを無効化
        if let Some(&taffy_node) = layouts.taffy_nodes.get(curr) {
            let _ = layouts.taffy.mark_dirty(taffy_node);
        }

        loop {
            if let Some(mask) = topology.active_masks.get_mut(curr) {
                // すでにレイアウトキューに登録済み（STATE_QUEUED_LAYOUT がオン）なら
                // 多重登録を防ぎつつ、それより上の親はすでに Dirty 化されているため探索を早期ブレイク
                if !mask.has(STATE_QUEUED_LAYOUT) {
                    mask.set(STATE_QUEUED_LAYOUT); // 自身を Dirty マーク
                    layouts.dirty_layout_entities.push(curr);
                } else {
                    break;
                }
            }

            // 親要素（先祖）をルートまで辿って Dirty フラグを連鎖伝播させる
            if let Some(Some(parent_id)) = topology.parents.get(curr).copied() {
                curr = parent_id;
            } else {
                break;
            }
        }
    }

    /// 描画変更フラグを立てる（wgpu転送要求）
    pub(crate) fn mark_render_dirty(
        id: EntityId,
        topology: &mut TopologyStore,
        renders: &mut RenderStore,
    ) {
        if let Some(mask) = topology.active_masks.get_mut(id) {
            // すでにレンダーキューに登録済み（STATE_QUEUED_RENDER がオン）なら早期リターン
            if !mask.has(STATE_QUEUED_RENDER) {
                mask.set(STATE_QUEUED_RENDER); // フラグをオンにして多重登録を防ぐ
                renders.dirty_render_entities.push(id);
            }
        }
    }

    /// 要素を安全に破棄（Despawn）。親が消えた場合子はフレーム末尾のクリーンアップフェーズで自動修復・一掃
    pub(crate) fn despawn_internal(id: EntityId, cx: &mut Context) {
        if cx.topology.entities.contains_key(id) {
            // トポロジーと Taffy ツリーのデタッチ処理
            if let Some(Some(parent_id)) = cx.topology.parents.get(id) {
                // Taffy からノードをデタッチ
                if let Some(&parent_node) = cx.layouts.taffy_nodes.get(*parent_id)
                    && let Some(&child_node) = cx.layouts.taffy_nodes.get(id)
                    && let Ok(taffy_children) = cx.layouts.taffy.children(parent_node)
                    && taffy_children.contains(&child_node)
                {
                    let _ = cx.layouts.taffy.remove_child(parent_node, child_node);
                }

                // 親の children リストから自身を除外
                if let Some(parent_children) = cx.topology.children.get_mut(*parent_id) {
                    parent_children.retain(|x| *x != id);
                }
            }

            // Taffy ノード自体の削除
            if let Some(node) = cx.layouts.taffy_nodes.remove(id) {
                let _ = cx.layouts.taffy.remove(node);
            }

            // 子要素を再帰的に despawn
            if let Some(children_list) = cx.topology.children.remove(id) {
                for child_id in children_list {
                    TopologyStore::despawn_internal(child_id, cx);
                }
            }

            cx.topology.despawn(id);
            cx.layouts.despawn(id);
            cx.renders.despawn(id);
            cx.outputs.despawn(id);
            cx.contents.despawn(id);
            cx.events.despawn(id);
            cx.reactive.despawn(id);
            cx.window.despawn(id);
            cx.system.despawn(id);
        }
    }

    /// 親要素の特定の古い子要素を、順序（インデックス）を維持したまま新しい子要素へ直接差し替えます。
    pub(crate) fn replace_child(
        parent: EntityId,
        old_child: EntityId,
        new_child: EntityId,
        layouts: &mut LayoutStore,
        topology: &mut TopologyStore,
    ) {
        // Taffy ツリー側の同期（古いノードを外し、新しいノードをアタッチ）
        if let Some(&parent_node) = layouts.taffy_nodes.get(parent)
            && let Some(&new_node) = layouts.taffy_nodes.get(new_child)
        {
            let _ = layouts.taffy.add_child(parent_node, new_node);
        }

        // children リスト内のインデックス位置を特定して直接置換
        if let Some(children_list) = topology.children.get_mut(parent)
            && let Some(pos) = children_list.iter().position(|&x| x == old_child)
        {
            children_list[pos] = new_child;
        }
    }

    /// デスポーン済みの無効な EntityId を各走査・Dirty配列から一括して排除。
    pub(crate) fn gc_inactive_entities(
        topology: &mut TopologyStore,
        layouts: &mut LayoutStore,
        renders: &mut RenderStore,
    ) {
        // SlotMap (entities) にキーが存在するもの（生存している要素）だけを保持する
        topology
            .active_entities
            .retain(|&id| topology.entities.contains_key(id));
        layouts
            .dirty_layout_entities
            .retain(|&id| topology.entities.contains_key(id));
        renders
            .dirty_render_entities
            .retain(|&id| topology.entities.contains_key(id));
    }

    // セッションのクリーンアップを実行
    pub(crate) fn no_root_no_parent(id: EntityId, topology: &TopologyStore) -> bool {
        // 親が存在しない
        let has_no_parent = topology.parents.get(id).copied().flatten().is_none();
        // ルート要素としても登録されていない
        let is_not_root = !topology.session_roots.contains(&id);

        has_no_parent && is_not_root
    }

    /// 子孫要素のインタラクション状態（state_flag）を走査します
    pub(crate) fn has_descendant_with_state(
        parent: EntityId,
        topology: &TopologyStore,
        state_flag: u128,
    ) -> bool {
        // ヒープアロケーションを防ぐため、スタック領域に16要素まで確保可能な SmallVec を用意
        let mut stack = SmallVec::<[EntityId; 16]>::new();

        if let Some(children) = topology.children.get(parent) {
            for &child_id in children {
                stack.push(child_id);
            }
        }

        while let Some(child_id) = stack.pop() {
            if topology.entities.contains_key(child_id)
                && let Some(mask) = topology.active_masks.get(child_id)
                && mask.has(state_flag)
            {
                return true; // 状態が見つかれば、関数呼び出しを重ねることなく即時早期リターン
            }

            // 子要素があれば、非再帰スタックにプッシュして探索を継続
            if let Some(children) = topology.children.get(child_id) {
                for &next_child in children {
                    stack.push(next_child);
                }
            }
        }

        false
    }

    /// いずれか一つのアクティブなユーザーインタラクションが子孫要素でONになっているか非再帰で走査します
    pub(crate) fn has_descendant_with_any_active_state(
        parent: EntityId,
        topology: &TopologyStore,
    ) -> bool {
        let mut stack = SmallVec::<[EntityId; 16]>::new();

        if let Some(children) = topology.children.get(parent) {
            for &child_id in children {
                stack.push(child_id);
            }
        }

        while let Some(child_id) = stack.pop() {
            if topology.entities.contains_key(child_id)
                && let Some(mask) = topology.active_masks.get(child_id)
                && mask.has_active_interaction_property()
            {
                return true;
            }

            if let Some(children) = topology.children.get(child_id) {
                for &next_child in children {
                    stack.push(next_child);
                }
            }
        }

        false
    }

    /// ドロップ先コンテナのフレックス方向（Row / Column）に基づいて、
    /// マウスのドロップ座標がどの子要素の手前（インデックス）に位置するかを逆引き算出します。
    pub(crate) fn calculate_insert_index(
        parent_id: EntityId,
        logical_pos: LayoutPoint,
        topology: &TopologyStore,
        layouts: &LayoutStore,
        outputs: &OutputStore,
    ) -> usize {
        let mut insert_idx = 0;

        if let Some(children) = topology.children.get(parent_id) {
            let parent_flex = layouts
                .flex_layouts
                .get(parent_id)
                .copied()
                .unwrap_or_default();
            let is_row = parent_flex.flex_direction == FlexDirection::Row
                || parent_flex.flex_direction == FlexDirection::RowReverse;

            for (idx, &child_id) in children.iter().enumerate() {
                if let Some(rect) = outputs.rects.get(child_id) {
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
        for &id in &layouts.flat_dfs_sequence {
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
    pub(crate) fn calculate_insert_index(
        &self,
        parent_id: EntityId,
        logical_pos: LayoutPoint,
    ) -> usize {
        TopologyStore::calculate_insert_index(
            parent_id,
            logical_pos,
            &self.topology,
            &self.layouts,
            &self.outputs,
        )
    }

    #[inline]
    pub(crate) fn compute_effective_z_indices(&self) -> SecondaryMap<EntityId, i32> {
        TopologyStore::compute_effective_z_indices(&self.topology, &self.layouts, &self.renders)
    }

    /// 指定された要素（target）が、ある親要素（parent）自身、またはその子孫であるかを判定します。
    #[inline]
    pub(crate) fn is_descendant_of(&self, target: EntityId, parent: EntityId) -> bool {
        TopologyStore::is_descendant_of(target, parent, &self.topology)
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
}
