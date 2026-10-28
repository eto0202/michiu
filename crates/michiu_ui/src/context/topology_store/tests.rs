#![expect(clippy::unwrap_used)]

use super::*;
use crate::VisualProperty;

struct TopologyFixture {
    topo_entities: EntitiesSlot,
    topo_active_entities: ActiveEntitiesVec,
    topo_active_masks: ActiveMasksSecondary,
    topo_parents: ParentsSecondary,
    topo_children: ChildrenSecondary,
    topo_session_spawned: SessionSpawnedVec,
    topo_is_structure_dirty: bool,
    topo_is_sort_dirty: bool,
    lay_dirty_entities: DirtyLayoutEntitiesVec,
    lay_taffy_tree: TaffyTreeEntity,
    lay_taffy_nodes: TaffyNodesSecondary,
    rnd_dirty_entities: DirtyRenderEntitiesVec,
    debug: DebugStore,
}

impl TopologyFixture {
    fn new() -> Self {
        Self {
            topo_entities: EntitiesSlot::default(),
            topo_active_entities: ActiveEntitiesVec::default(),
            topo_active_masks: ActiveMasksSecondary::default(),
            topo_parents: ParentsSecondary::default(),
            topo_children: ChildrenSecondary::default(),
            topo_session_spawned: SessionSpawnedVec::default(),
            topo_is_structure_dirty: false,
            topo_is_sort_dirty: false,
            lay_dirty_entities: DirtyLayoutEntitiesVec::default(),
            lay_taffy_tree: TaffyTreeEntity::new(),
            lay_taffy_nodes: TaffyNodesSecondary::default(),
            rnd_dirty_entities: DirtyRenderEntitiesVec::default(),
            debug: DebugStore::new(),
        }
    }

    fn spawn(&mut self, parent_id: Option<Entity>) -> Entity {
        TopologyStore::spawn(
            parent_id,
            &mut self.topo_entities,
            &mut self.topo_active_entities,
            &mut self.topo_active_masks,
            &mut self.topo_parents,
            &mut self.topo_children,
            &mut self.topo_session_spawned,
            &mut self.topo_is_structure_dirty,
            &mut self.topo_is_sort_dirty,
            &mut self.lay_taffy_tree,
            &mut self.lay_taffy_nodes,
            &mut self.rnd_dirty_entities,
            &mut self.debug,
        )
    }
}

// グループ1:
// `test_topology_spawn`
//  - `TopologyStore::spawn` を呼び出す。
//  - `topo_entities`, `topo_active_entities`, `topo_session_spawned` に登録され、Taffy ノードが作成されること。
//  - 各種 dirty フラグ（structure, sort, render）が `true` に設定されること。
//
// `test_attach_and_detach_from_parent`
//  - `attach_to_parent`: 親の `topo_children` に子が追加され、子の `topo_parents` が `Some(parent)` になること。
//  - `detach_from_parent`: 親の `topo_children` から子が除外され、子の `topo_parents` が `None` になること。戻り値として `Some(parent)` が返ること。
//
// `test_add_child_basic_and_reparenting`
//  - 未所属ノードの追加：親の `topo_children` および Taffy ツリーの子リストに追加され、親がレイアウトダーティになること。
//  - 付け替え（親A $\to$ 親B）：古い親Aから安全にデタッチ（Taffyツリー・子供順序・レイアウトダーティ反映）され、親Bにアタッチされること。
//
// `test_replace_child_node`
//  - 子リスト内の `old_child` の位置（インデックス）が `new_child` で正確に置換され、`new_child` の親ポインタが設定されること。
//
// `test_restore_child`
//  - 仮の親（`holder`）に退避されていた複数の子ノードが、元の親（`src_id`）に親子関係および Taffy ツリーの両面で復旧されること。
//
// `test_mark_dirty`
// - 指定要素に対してレイアウトダーティとレンダーダーティの両方が同時にマークされること。

#[test]
fn test_topology_spawn() {
    let mut f = TopologyFixture::new();
    let entity = f.spawn(None);

    assert!(f.topo_entities.contains_key(entity));
    assert_eq!(*f.topo_parents.at(entity), None);
    assert!(f.topo_children.at(entity).is_empty());
    assert_eq!(f.topo_active_entities.as_slice(), &[entity]);
    assert_eq!(f.topo_session_spawned.as_slice(), &[entity]);
    assert!(f.lay_taffy_nodes.contains(entity));

    assert!(f.topo_is_structure_dirty);
    assert!(f.topo_is_sort_dirty);

    let mask = f.topo_active_masks.at(entity);
    assert!(mask.has(ComponentMask::STATE_QUEUED_RENDER));
    assert_eq!(f.rnd_dirty_entities.as_slice(), &[entity]);
}

#[test]
fn test_attach_and_detach_from_parent() {
    let mut f = TopologyFixture::new();
    let parent = f.spawn(None);
    let child = f.spawn(None);

    f.topo_is_structure_dirty = false;
    f.topo_is_sort_dirty = false;

    // attach_to_parent
    TopologyStore::attach_to_parent(
        parent,
        child,
        &mut f.topo_parents,
        &mut f.topo_children,
        &mut f.topo_is_structure_dirty,
        &mut f.topo_is_sort_dirty,
    );

    assert_eq!(*f.topo_parents.at(child), Some(parent));
    assert_eq!(f.topo_children.at(parent).as_slice(), &[child]);
    assert!(f.topo_is_structure_dirty);
    assert!(f.topo_is_sort_dirty);

    // 重複 attach しても二重追加されない
    TopologyStore::attach_to_parent(
        parent,
        child,
        &mut f.topo_parents,
        &mut f.topo_children,
        &mut f.topo_is_structure_dirty,
        &mut f.topo_is_sort_dirty,
    );
    assert_eq!(f.topo_children.at(parent).len(), 1);

    f.topo_is_structure_dirty = false;
    f.topo_is_sort_dirty = false;

    // detach_from_parent
    let old_p = TopologyStore::detach_from_parent(
        child,
        &mut f.topo_parents,
        &mut f.topo_children,
        &mut f.topo_is_structure_dirty,
        &mut f.topo_is_sort_dirty,
    );

    assert_eq!(old_p, Some(parent));
    assert_eq!(*f.topo_parents.at(child), None);
    assert!(f.topo_children.at(parent).is_empty());
    assert!(f.topo_is_structure_dirty);
    assert!(f.topo_is_sort_dirty);
}

#[test]
fn test_add_child_basic_and_reparenting() {
    let mut f = TopologyFixture::new();
    let parent1 = f.spawn(None);
    let parent2 = f.spawn(None);
    let child = f.spawn(None);

    // 初回 add_child (parent1 へ追加)
    TopologyStore::add_child(
        parent1,
        child,
        &mut f.topo_active_masks,
        &mut f.topo_parents,
        &mut f.topo_children,
        &mut f.topo_is_structure_dirty,
        &mut f.topo_is_sort_dirty,
        &mut f.lay_dirty_entities,
        &mut f.lay_taffy_tree,
        &mut f.lay_taffy_nodes,
        &mut f.debug,
    );

    let p1_node = *f.lay_taffy_nodes.at(parent1);
    let c_node = *f.lay_taffy_nodes.at(child);

    assert_eq!(*f.topo_parents.at(child), Some(parent1));
    assert_eq!(f.topo_children.at(parent1).as_slice(), &[child]);
    assert_eq!(f.lay_taffy_tree.children(p1_node).unwrap(), vec![c_node]);

    // 付け替え (parent1 から parent2 へ)
    f.lay_dirty_entities.clear();
    TopologyStore::add_child(
        parent2,
        child,
        &mut f.topo_active_masks,
        &mut f.topo_parents,
        &mut f.topo_children,
        &mut f.topo_is_structure_dirty,
        &mut f.topo_is_sort_dirty,
        &mut f.lay_dirty_entities,
        &mut f.lay_taffy_tree,
        &mut f.lay_taffy_nodes,
        &mut f.debug,
    );

    let p2_node = *f.lay_taffy_nodes.at(parent2);

    // 古い親からデタッチされ、新しい親にアタッチされていること
    assert_eq!(*f.topo_parents.at(child), Some(parent2));
    assert!(f.topo_children.at(parent1).is_empty());
    assert_eq!(f.lay_taffy_tree.children(p1_node).unwrap(), vec![]);
    assert_eq!(f.topo_children.at(parent2).as_slice(), &[child]);
    assert_eq!(f.lay_taffy_tree.children(p2_node).unwrap(), vec![c_node]);
}

#[test]
fn test_replace_child_node() {
    let mut f = TopologyFixture::new();
    let parent = f.spawn(None);
    let c1 = f.spawn(None);
    let c2 = f.spawn(None);
    let c3 = f.spawn(None);
    let new_c = f.spawn(None);

    f.topo_children.at_mut(parent).extend([c1, c2, c3]);
    f.topo_parents.insert(c1, Some(parent));
    f.topo_parents.insert(c2, Some(parent));
    f.topo_parents.insert(c3, Some(parent));

    f.topo_is_structure_dirty = false;
    f.topo_is_sort_dirty = false;

    TopologyStore::replace_child_node(
        parent,
        c2,
        new_c,
        &mut f.topo_parents,
        &mut f.topo_children,
        &mut f.topo_is_structure_dirty,
        &mut f.topo_is_sort_dirty,
    );

    // c2 の位置が new_c に置換され、順序が維持されていること
    assert_eq!(f.topo_children.at(parent).as_slice(), &[c1, new_c, c3]);
    assert_eq!(*f.topo_parents.at(new_c), Some(parent));
    assert!(f.topo_is_structure_dirty);
    assert!(f.topo_is_sort_dirty);
}

#[test]
fn test_restore_child() {
    let mut f = TopologyFixture::new();
    let src = f.spawn(None);
    let holder = f.spawn(None);
    let c1 = f.spawn(None);
    let c2 = f.spawn(None);

    // holder に c1, c2 を所属させる
    TopologyStore::add_child(
        holder,
        c1,
        &mut f.topo_active_masks,
        &mut f.topo_parents,
        &mut f.topo_children,
        &mut f.topo_is_structure_dirty,
        &mut f.topo_is_sort_dirty,
        &mut f.lay_dirty_entities,
        &mut f.lay_taffy_tree,
        &mut f.lay_taffy_nodes,
        &mut f.debug,
    );
    TopologyStore::add_child(
        holder,
        c2,
        &mut f.topo_active_masks,
        &mut f.topo_parents,
        &mut f.topo_children,
        &mut f.topo_is_structure_dirty,
        &mut f.topo_is_sort_dirty,
        &mut f.lay_dirty_entities,
        &mut f.lay_taffy_tree,
        &mut f.lay_taffy_nodes,
        &mut f.debug,
    );

    // holder から src へ復旧
    TopologyStore::restore_child(
        src,
        holder,
        &mut f.topo_parents,
        &mut f.topo_children,
        &mut f.lay_taffy_tree,
        &mut f.lay_taffy_nodes,
        &mut f.debug,
    );

    assert_eq!(*f.topo_parents.at(c1), Some(src));
    assert_eq!(*f.topo_parents.at(c2), Some(src));
    assert_eq!(f.topo_children.at(src).as_slice(), &[c1, c2]);

    let src_node = *f.lay_taffy_nodes.at(src);
    let c1_node = *f.lay_taffy_nodes.at(c1);
    let c2_node = *f.lay_taffy_nodes.at(c2);
    assert_eq!(
        f.lay_taffy_tree.children(src_node).unwrap(),
        vec![c1_node, c2_node]
    );
}

#[test]
fn test_mark_dirty() {
    let mut f = TopologyFixture::new();
    let entity = f.spawn(None);

    // 初期ダーティフラグをクリア
    f.lay_dirty_entities.clear();
    f.rnd_dirty_entities.clear();
    f.topo_active_masks
        .at_mut(entity)
        .unset(ComponentMask::STATE_QUEUED_LAYOUT);
    f.topo_active_masks
        .at_mut(entity)
        .unset(ComponentMask::STATE_QUEUED_RENDER);

    TopologyStore::mark_dirty(
        entity,
        &mut f.topo_active_masks,
        &f.topo_parents,
        &mut f.lay_dirty_entities,
        &mut f.lay_taffy_tree,
        &f.lay_taffy_nodes,
        &mut f.rnd_dirty_entities,
        &mut f.debug,
    );

    assert_eq!(f.lay_dirty_entities.as_slice(), &[entity]);
    assert_eq!(f.rnd_dirty_entities.as_slice(), &[entity]);

    let mask = f.topo_active_masks.at(entity);
    assert!(mask.has(ComponentMask::STATE_QUEUED_LAYOUT));
    assert!(mask.has(ComponentMask::STATE_QUEUED_RENDER));
}

// グループ2:
// `test_rebuild_dfs_sequence`
//  - 多階層ツリー（ルート, 子1(孫1, 孫2), 子2）において、先行順（Pre-order / DFS）でフラット配列が正しく生成されること。
//  - 完了後に `topo_is_structure_dirty` が `false` にリセットされること。
//
// `test_is_descendant_of`
//  - 自身、直下の子、孫すべてに対して `true` を返し、兄弟や無関係なノードには `false` を返すこと。
//
// `test_find_root`
//  - `topo_flat_dfs_sequence` がある場合はその先頭、空の場合は `topo_parents` から親が `None` の生存要素を正しく特定すること。
//
// `test_has_parent_with_state`
//  - 親が指定マスクを持っている場合のみ `true`、親が `None` またはマスクを持たない場合は `false` を返すこと。
//
// `test_has_descendant_with_state`
//  - 子孫ツリー（孫・ひ孫含む）のいずれかが指定マスクを持っている場合に `true`、誰も持っていない場合に `false` を返すこと。

#[test]
fn test_rebuild_dfs_sequence() {
    let mut f = TopologyFixture::new();
    // ツリー構造:
    // root
    //   ├─ c1
    //   │   ├─ gc1
    //   │   └─ gc2
    //   └─ c2
    let root = f.spawn(None);
    let c1 = f.spawn(Some(root));
    let c2 = f.spawn(Some(root));
    let gc1 = f.spawn(Some(c1));
    let gc2 = f.spawn(Some(c1));

    f.topo_children.at_mut(root).extend([c1, c2]);
    f.topo_children.at_mut(c1).extend([gc1, gc2]);

    let mut dfs_seq = FlatDfsSequenceVec::default();
    f.topo_is_structure_dirty = true;

    TopologyStore::rebuild_dfs_sequence(
        root,
        &mut dfs_seq,
        &mut f.topo_is_structure_dirty,
        &f.topo_children,
        &mut f.debug,
    );

    // Pre-order (先行順) で並んでいること
    assert_eq!(dfs_seq.as_slice(), &[root, c1, gc1, gc2, c2]);
    assert!(!f.topo_is_structure_dirty);
}

#[test]
fn test_is_descendant_of() {
    let mut f = TopologyFixture::new();
    let root = f.spawn(None);
    let child = f.spawn(Some(root));
    let grandchild = f.spawn(Some(child));
    let other = f.spawn(None);

    f.topo_parents.insert(child, Some(root));
    f.topo_parents.insert(grandchild, Some(child));

    // 自身
    assert!(TopologyStore::is_descendant_of(root, root, &f.topo_parents));
    // 直下の子
    assert!(TopologyStore::is_descendant_of(
        child,
        root,
        &f.topo_parents
    ));
    // 孫
    assert!(TopologyStore::is_descendant_of(
        grandchild,
        root,
        &f.topo_parents
    ));
    assert!(TopologyStore::is_descendant_of(
        grandchild,
        child,
        &f.topo_parents
    ));

    // 逆方向・無関係
    assert!(!TopologyStore::is_descendant_of(
        root,
        child,
        &f.topo_parents
    ));
    assert!(!TopologyStore::is_descendant_of(
        other,
        root,
        &f.topo_parents
    ));
}

#[test]
fn test_find_root() {
    let mut f = TopologyFixture::new();
    let root = f.spawn(None);
    let child = f.spawn(Some(root));
    f.topo_parents.insert(child, Some(root));

    let mut dfs_seq = FlatDfsSequenceVec::default();

    // DFS シーケンスが空のとき（topo_parents からのフォールバック解決）
    let found = TopologyStore::find_root(&f.topo_entities, &f.topo_parents, &dfs_seq);
    assert_eq!(found, Some(root));

    // DFS シーケンスが存在するとき（先頭要素を優先）
    dfs_seq.push(root);
    dfs_seq.push(child);
    let found_from_dfs = TopologyStore::find_root(&f.topo_entities, &f.topo_parents, &dfs_seq);
    assert_eq!(found_from_dfs, Some(root));

    // 生存ノードが存在しないとき
    let empty_entities = EntitiesSlot::default();
    let empty_parents = ParentsSecondary::default();
    let empty_dfs = FlatDfsSequenceVec::default();
    assert_eq!(
        TopologyStore::find_root(&empty_entities, &empty_parents, &empty_dfs),
        None
    );
}

#[test]
fn test_has_parent_with_state() {
    let mut f = TopologyFixture::new();
    let root = f.spawn(None);
    let child = f.spawn(Some(root));
    f.topo_parents.insert(child, Some(root));

    let test_flag = ComponentMask::STATE_HOVERED;

    // 親がフラグを持っていない場合
    assert!(!TopologyStore::has_parent_with_state(
        child,
        test_flag,
        &f.topo_entities,
        &f.topo_active_masks,
        &f.topo_parents,
    ));

    // 親がフラグを持っている場合
    f.topo_active_masks.at_mut(root).set(test_flag);
    assert!(TopologyStore::has_parent_with_state(
        child,
        test_flag,
        &f.topo_entities,
        &f.topo_active_masks,
        &f.topo_parents,
    ));

    // ルート要素（親が存在しない場合）
    assert!(!TopologyStore::has_parent_with_state(
        root,
        test_flag,
        &f.topo_entities,
        &f.topo_active_masks,
        &f.topo_parents,
    ));
}

#[test]
fn test_has_descendant_with_state() {
    let mut f = TopologyFixture::new();
    let root = f.spawn(None);
    let child = f.spawn(Some(root));
    let grandchild = f.spawn(Some(child));

    f.topo_children.at_mut(root).push(child);
    f.topo_children.at_mut(child).push(grandchild);

    let test_flag = ComponentMask::STATE_ACTIVED;

    // 誰にもフラグがない状態
    assert!(!TopologyStore::has_descendant_with_state(
        root,
        test_flag,
        &f.topo_entities,
        &f.topo_active_masks,
        &f.topo_children,
    ));

    // 親自身にのみフラグがある場合（子孫ではないため false）
    f.topo_active_masks.at_mut(root).set(test_flag);
    assert!(!TopologyStore::has_descendant_with_state(
        root,
        test_flag,
        &f.topo_entities,
        &f.topo_active_masks,
        &f.topo_children,
    ));

    // 孫にフラグが付与された場合
    f.topo_active_masks.at_mut(grandchild).set(test_flag);
    assert!(TopologyStore::has_descendant_with_state(
        root,
        test_flag,
        &f.topo_entities,
        &f.topo_active_masks,
        &f.topo_children,
    ));
}

// グループ3: セッション・安全破棄（Despawn）
// `test_despawn_internal_recursive`
//  - 親, 子, 孫 のツリーにおいて、親を `despawn_internal` すると、子および孫も再帰的に全ストア（SoA）から一掃されること。
//  - 親の親（祖父）が存在する場合、祖父の子供リストおよび Taffy ツリーから親が適切にデタッチされること。
//
// `test_replace_child`
//  - 古い子要素が `despawn_internal` され、新しい子要素が Taffy ツリーおよび子供リストの同位置に配置されること。
//
// `test_end_session`
//  - セッション中に生成された孤立ノード（親なし）のうち、`topo_session_roots` に含まれていないものは自動破棄され、ルート登録されたものや親を持つものは生存すること。

struct FullStoreFixture {
    window: WindowStore,
    system: SystemStore,
    reactive: ReactiveStore,
    events: EventStore,
    contents: ContentStore,
    topology: TopologyStore,
    states: StateStore,
    layouts: LayoutStore,
    renders: RenderStore,
    outputs: OutputStore,
    acce: AccessibilityStore,
    debug: DebugStore,
}

impl FullStoreFixture {
    fn new() -> Self {
        Self {
            window: WindowStore::default(),
            system: SystemStore::default(),
            reactive: ReactiveStore::default(),
            events: EventStore::default(),
            contents: ContentStore::default(),
            topology: TopologyStore::new(),
            states: StateStore::default(),
            layouts: LayoutStore::default(),
            renders: RenderStore::default(),
            outputs: OutputStore::default(),
            acce: AccessibilityStore::default(),
            debug: DebugStore::new(),
        }
    }

    fn spawn(&mut self, parent: Option<Entity>) -> Entity {
        let id = TopologyStore::spawn(
            parent,
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
        );

        if let Some(p) = parent {
            TopologyStore::add_child(
                p,
                id,
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

        id
    }
}

#[test]
fn test_despawn_internal_recursive() {
    let mut f = FullStoreFixture::new();
    let root = f.spawn(None);
    let child = f.spawn(Some(root));
    let grandchild = f.spawn(Some(child));

    let root_node = *f.layouts.lay_taffy_nodes.at(root);
    let _child_node = *f.layouts.lay_taffy_nodes.at(child);
    let _gc_node = *f.layouts.lay_taffy_nodes.at(grandchild);

    // child を破棄（grandchild も再帰的に破棄される）
    TopologyStore::despawn_internal(
        child,
        &mut f.window,
        &mut f.system,
        &mut f.reactive,
        &mut f.events,
        &mut f.contents,
        &mut f.topology,
        &mut f.states,
        &mut f.layouts,
        &mut f.renders,
        &mut f.outputs,
        &mut f.acce,
        &mut f.debug,
    );

    // root は生存、child と grandchild は破棄されていること
    assert!(f.topology.topo_entities.contains_key(root));
    assert!(!f.topology.topo_entities.contains_key(child));
    assert!(!f.topology.topo_entities.contains_key(grandchild));

    // 親（root）の子リストから除外されていること
    assert!(f.topology.topo_children.at(root).is_empty());

    // Taffy ツリーから child ノードがデタッチされ、各ノード自体も破棄されていること
    assert_eq!(
        f.layouts.lay_taffy_tree.children(root_node).unwrap(),
        vec![]
    );
    assert!(!f.layouts.lay_taffy_nodes.contains(child));
    assert!(!f.layouts.lay_taffy_nodes.contains(grandchild));
}

#[test]
fn test_replace_child() {
    let mut f = FullStoreFixture::new();
    let parent = f.spawn(None);
    let old_child = f.spawn(Some(parent));
    let new_child = f.spawn(None);

    let parent_node = *f.layouts.lay_taffy_nodes.at(parent);
    let new_node = *f.layouts.lay_taffy_nodes.at(new_child);

    TopologyStore::replace_child(
        parent,
        old_child,
        new_child,
        &mut f.window,
        &mut f.system,
        &mut f.reactive,
        &mut f.events,
        &mut f.contents,
        &mut f.topology,
        &mut f.states,
        &mut f.layouts,
        &mut f.renders,
        &mut f.outputs,
        &mut f.debug,
        &mut f.acce,
    );

    // old_child は破棄され、new_child は生存
    assert!(!f.topology.topo_entities.contains_key(old_child));
    assert!(f.topology.topo_entities.contains_key(new_child));

    // parent の子リストが new_child に置換されていること
    assert_eq!(f.topology.topo_children.at(parent).as_slice(), &[new_child]);
    assert_eq!(*f.topology.topo_parents.at(new_child), Some(parent));

    // Taffy 側に new_child ノードが登録されていること
    let taffy_children = f.layouts.lay_taffy_tree.children(parent_node).unwrap();
    assert!(taffy_children.contains(&new_node));

    // 親にレイアウトダーティがマークされていること
    assert!(
        f.topology
            .topo_active_masks
            .at(parent)
            .has(ComponentMask::STATE_QUEUED_LAYOUT)
    );
}

#[test]
fn test_end_session() {
    let mut f = FullStoreFixture::new();
    let start_marker = f.topology.topo_session_spawned.len();

    // 1. ルート要素（親なし・session_roots 登録済み）
    let root = f.spawn(None);
    f.topology.topo_session_roots.push(root);

    // 2. 子要素（親あり）
    let child = f.spawn(Some(root));

    // 3. 孤立要素（親なし・session_roots 未登録）
    let orphan = f.spawn(None);

    TopologyStore::end_session(
        start_marker,
        &mut f.window,
        &mut f.system,
        &mut f.reactive,
        &mut f.events,
        &mut f.contents,
        &mut f.topology,
        &mut f.states,
        &mut f.layouts,
        &mut f.renders,
        &mut f.outputs,
        &mut f.debug,
        &mut f.acce,
    );

    // 孤立要素のみ自動デスポーンされ、root と child は生存すること
    assert!(f.topology.topo_entities.contains_key(root));
    assert!(f.topology.topo_entities.contains_key(child));
    assert!(!f.topology.topo_entities.contains_key(orphan));

    // session_roots はクリアされること
    assert!(f.topology.topo_session_roots.is_empty());
}

// グループ4:
// `test_calculate_insert_index`
//  - Row（横並び）および Column（縦並び）のフレックスコンテナにおいて、各子要素の中心座標との比較により、マウス座標に応じた適切な挿入インデックス（先頭、中間、末尾）が返ること。
//
// `test_prepare_sorted_entities`
//  - 実効 `z_index`（親からの継承）と DFS 出現順に基づき、描画順が正しく昇順ソートされること。
//  - クリップ範囲外または親が非表示（カリング）の要素がソート結果から除外され、`STATE_RENDER_VISIBLE` フラグが適切に反映されること。
//  - 完了後に `topo_is_sort_dirty` が `false` になること。
//
// `test_hit_test`
//  - 最前面（ソート結果の末尾）から逆順走査し、物理矩形およびクリップ矩形内に含まれる最前面の要素がヒットすること。
//  - ドラッグ中要素（`dragged`）、`STATE_DND_DRAG_OVER` 要素、および `PointerEvents::None` が付与された要素はスキップすること。

#[test]
fn test_calculate_insert_index() {
    let mut f = TopologyFixture::new();
    let parent = f.spawn(None);
    let c1 = f.spawn(Some(parent));
    let c2 = f.spawn(Some(parent));
    let c3 = f.spawn(Some(parent));

    f.topo_children.at_mut(parent).extend([c1, c2, c3]);

    let mut out_rects = RectsSecondary::default();
    // 横並び（Row）: 幅100の要素を3つ並べる
    // c1: 0..100 (中心 50)
    // c2: 100..200 (中心 150)
    // c3: 200..300 (中心 250)
    out_rects.insert(c1, LayoutRect::new(0.0, 0.0, 100.0, 50.0));
    out_rects.insert(c2, LayoutRect::new(100.0, 0.0, 100.0, 50.0));
    out_rects.insert(c3, LayoutRect::new(200.0, 0.0, 100.0, 50.0));

    let lay_flex = FlexLayoutsSecondary::default();

    // 1. c1 の中心(50)より手前 -> インデックス 0
    let p0 = LayoutPoint { x: 30.0, y: 20.0 };
    assert_eq!(
        TopologyStore::calculate_insert_index(parent, p0, &f.topo_children, &lay_flex, &out_rects,),
        0
    );

    // 2. c1 と c2 の中心の間(100) -> インデックス 1
    let p1 = LayoutPoint { x: 100.0, y: 20.0 };
    assert_eq!(
        TopologyStore::calculate_insert_index(parent, p1, &f.topo_children, &lay_flex, &out_rects,),
        1
    );

    // 3. c3 の中心(250)より後ろ -> インデックス 3（末尾）
    let p3 = LayoutPoint { x: 300.0, y: 20.0 };
    assert_eq!(
        TopologyStore::calculate_insert_index(parent, p3, &f.topo_children, &lay_flex, &out_rects,),
        3
    );
}

#[test]
fn test_prepare_sorted_entities() {
    let mut f = TopologyFixture::new();
    let root = f.spawn(None);
    let c1 = f.spawn(Some(root));
    let c2 = f.spawn(Some(root));

    f.topo_parents.insert(c1, Some(root));
    f.topo_parents.insert(c2, Some(root));

    let mut dfs_seq = FlatDfsSequenceVec::default();
    dfs_seq.extend([root, c1, c2]);

    let mut effective_z = EffectiveZindicesSecondary::default();
    let mut sorted = SortedEntitiesVec::default();
    let mut sort_cache = SortCacheVec::default();
    let mut is_sort_dirty = true;

    let mut rnd_visual = VisualPropertiesSecondary::default();
    // c1 に z_index = 10、c2 に z_index = 1 を付与
    if let Some(v1) = rnd_visual.find_mut(c1) {
        v1.z_index = Some(10);
    } else {
        let v = VisualProperty {
            z_index: Some(10),
            ..Default::default()
        };
        rnd_visual.insert(c1, v);
    }
    if let Some(v2) = rnd_visual.find_mut(c2) {
        v2.z_index = Some(1);
    } else {
        let v = VisualProperty {
            z_index: Some(1),
            ..Default::default()
        };
        rnd_visual.insert(c2, v);
    }

    let mut out_rects = RectsSecondary::default();
    let mut out_clip_rects = ClipRectsSecondary::default();
    let win_size = Some(LayoutSize {
        width: 800.0,
        height: 600.0,
    });

    // 画面内に配置
    out_rects.insert(root, LayoutRect::new(0.0, 0.0, 800.0, 600.0));
    out_rects.insert(c1, LayoutRect::new(10.0, 10.0, 100.0, 100.0));
    out_rects.insert(c2, LayoutRect::new(20.0, 20.0, 100.0, 100.0));

    out_clip_rects.insert(root, LayoutRect::new(0.0, 0.0, 800.0, 600.0));
    out_clip_rects.insert(c1, LayoutRect::new(0.0, 0.0, 800.0, 600.0));
    out_clip_rects.insert(c2, LayoutRect::new(0.0, 0.0, 800.0, 600.0));

    TopologyStore::prepare_sorted_entities(
        win_size,
        &mut f.topo_active_masks,
        &mut effective_z,
        &mut sorted,
        &mut sort_cache,
        &mut is_sort_dirty,
        &f.topo_parents,
        &dfs_seq,
        &rnd_visual,
        &mut out_clip_rects,
        &out_rects,
        &mut f.debug,
    );

    // z_index 昇順（root:0, c2:1, c1:10）でソートされていること
    assert_eq!(sorted.as_slice(), &[root, c2, c1]);
    assert!(!is_sort_dirty);
    assert!(
        f.topo_active_masks
            .at(c1)
            .has(ComponentMask::STATE_RENDER_VISIBLE)
    );
}

#[test]
fn test_hit_test() {
    let mut f = TopologyFixture::new();
    let root = f.spawn(None);
    let e1 = f.spawn(Some(root));
    let e2 = f.spawn(Some(root));

    f.topo_parents.insert(e1, Some(root));
    f.topo_parents.insert(e2, Some(root));

    let mut dfs_seq = FlatDfsSequenceVec::default();
    dfs_seq.extend([root, e1, e2]);

    let mut effective_z = EffectiveZindicesSecondary::default();
    let mut sorted = SortedEntitiesVec::default();
    let mut sort_cache = SortCacheVec::default();
    let mut is_sort_dirty = true;

    let rnd_visual = VisualPropertiesSecondary::default();
    let rnd_base_visual = BaseVisualPropertiesSecondary::default();

    let mut out_rects = RectsSecondary::default();
    let mut out_clip_rects = ClipRectsSecondary::default();
    let win_size = Some(LayoutSize {
        width: 800.0,
        height: 600.0,
    });

    // root は全体、e1 と e2 は重なり領域を持つ
    // e2 は DFS 上後にあるため通常前面に配置される
    out_rects.insert(root, LayoutRect::new(0.0, 0.0, 800.0, 600.0));
    out_rects.insert(e1, LayoutRect::new(0.0, 0.0, 100.0, 100.0));
    out_rects.insert(e2, LayoutRect::new(50.0, 50.0, 100.0, 100.0));

    out_clip_rects.insert(root, LayoutRect::new(0.0, 0.0, 800.0, 600.0));
    out_clip_rects.insert(e1, LayoutRect::new(0.0, 0.0, 800.0, 600.0));
    out_clip_rects.insert(e2, LayoutRect::new(0.0, 0.0, 800.0, 600.0));

    let mut interaction_states = ActiveInteractionStates::default();

    // 1. 重なり領域 (75, 75) のテスト -> 前面にある e2 がヒット
    let p_overlap = LayoutPoint { x: 75.0, y: 75.0 };
    let hit = TopologyStore::hit_test(
        p_overlap,
        win_size,
        &interaction_states,
        &mut f.topo_active_masks,
        &mut effective_z,
        &mut sorted,
        &mut sort_cache,
        &mut is_sort_dirty,
        &f.topo_parents,
        &dfs_seq,
        &rnd_visual,
        &rnd_base_visual,
        &mut out_clip_rects,
        &out_rects,
        &mut f.debug,
    );
    assert_eq!(hit, Some(e2));

    // 2. e2 がドラッグ中の場合 -> スルーされて背面の e1 がヒット
    is_sort_dirty = true;
    interaction_states.dragged = Some(e2);
    let hit_dragged = TopologyStore::hit_test(
        p_overlap,
        win_size,
        &interaction_states,
        &mut f.topo_active_masks,
        &mut effective_z,
        &mut sorted,
        &mut sort_cache,
        &mut is_sort_dirty,
        &f.topo_parents,
        &dfs_seq,
        &rnd_visual,
        &rnd_base_visual,
        &mut out_clip_rects,
        &out_rects,
        &mut f.debug,
    );
    assert_eq!(hit_dragged, Some(e1));

    // 3. 領域外 (900, 900) -> None
    is_sort_dirty = true;
    interaction_states.dragged = None;
    let p_out = LayoutPoint { x: 900.0, y: 900.0 };
    let hit_none = TopologyStore::hit_test(
        p_out,
        win_size,
        &interaction_states,
        &mut f.topo_active_masks,
        &mut effective_z,
        &mut sorted,
        &mut sort_cache,
        &mut is_sort_dirty,
        &f.topo_parents,
        &dfs_seq,
        &rnd_visual,
        &rnd_base_visual,
        &mut out_clip_rects,
        &out_rects,
        &mut f.debug,
    );
    assert_eq!(hit_none, None);
}

// グループ5:
// `test_topology_tag_and_query_global` / `test_topology_query_descendants`
//  - `tag`, `try_query_first`, `query_all`, `try_query_descendant`, `query_descendants` の連携。
//  - `query_first` / `query_descendant` で存在しないタグを指定した際にパニック（`unwrap_or_trace`）すること。

struct TestTag1;
struct TestTag2;
struct TestTagUnregistered;

#[test]
fn test_topology_tag_and_query_global() {
    let mut f = TopologyFixture::new();
    let e1 = f.spawn(None);
    let e2 = f.spawn(None);
    let mut registry = MichiuTagRegistry::default();

    TopologyStore::tag::<TestTag1>(e1, &mut registry);
    TopologyStore::tag::<TestTag2>(e1, &mut registry);
    TopologyStore::tag::<TestTag1>(e2, &mut registry);

    // try_query_first
    assert_eq!(
        TopologyStore::try_query_first::<TestTag1>(&registry),
        Some(e1)
    );
    assert_eq!(
        TopologyStore::try_query_first::<TestTagUnregistered>(&registry),
        None
    );

    // query_first (正常系)
    assert_eq!(
        TopologyStore::query_first::<TestTag1>(&mut registry, &mut f.debug),
        e1
    );

    // query_first (異常系: 未登録タグでパニック)
    let panic_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut reg = registry.clone();
        let mut dbg = DebugStore::new();
        TopologyStore::query_first::<TestTagUnregistered>(&mut reg, &mut dbg);
    }));
    assert!(panic_res.is_err());

    // query_all
    let all_tag1: Vec<Entity> = TopologyStore::query_all::<TestTag1>(&registry).collect();
    assert_eq!(all_tag1, vec![e1, e2]);

    let all_unregistered: Vec<Entity> =
        TopologyStore::query_all::<TestTagUnregistered>(&registry).collect();
    assert!(all_unregistered.is_empty());
}

#[test]
fn test_topology_query_descendants() {
    let mut f = TopologyFixture::new();
    let root = f.spawn(None);
    let c1 = f.spawn(Some(root));
    let c2 = f.spawn(Some(root));
    let gc1 = f.spawn(Some(c1));

    f.topo_parents.insert(c1, Some(root));
    f.topo_parents.insert(c2, Some(root));
    f.topo_parents.insert(gc1, Some(c1));

    let mut dfs_seq = FlatDfsSequenceVec::default();
    dfs_seq.extend([root, c1, gc1, c2]);

    let mut registry = MichiuTagRegistry::default();
    // gc1（c1の子孫）と c2（rootの子孫だがc1の子孫ではない）にタグを付与
    TopologyStore::tag::<TestTag1>(gc1, &mut registry);
    TopologyStore::tag::<TestTag1>(c2, &mut registry);

    // 1. try_query_descendant
    assert_eq!(
        TopologyStore::try_query_descendant::<TestTag1>(c1, &registry, &f.topo_parents, &dfs_seq),
        Some(gc1)
    );
    assert_eq!(
        TopologyStore::try_query_descendant::<TestTag1>(c2, &registry, &f.topo_parents, &dfs_seq),
        None
    );

    // 2. query_descendant (正常系)
    assert_eq!(
        TopologyStore::query_descendant::<TestTag1>(
            c1,
            &registry,
            &f.topo_parents,
            &dfs_seq,
            &mut f.debug
        ),
        gc1
    );

    // 3. query_descendant (異常系: 見つからない場合パニック)
    let panic_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut dbg = DebugStore::new();
        TopologyStore::query_descendant::<TestTagUnregistered>(
            root,
            &registry,
            &f.topo_parents,
            &dfs_seq,
            &mut dbg,
        );
    }));
    assert!(panic_res.is_err());

    // 4. query_descendants (複数ヒットの走査)
    let root_descendants: Vec<Entity> =
        TopologyStore::query_descendants::<TestTag1>(root, &registry, &f.topo_parents, &dfs_seq)
            .collect();
    assert_eq!(root_descendants, vec![gc1, c2]);

    let c1_descendants: Vec<Entity> =
        TopologyStore::query_descendants::<TestTag1>(c1, &registry, &f.topo_parents, &dfs_seq)
            .collect();
    assert_eq!(c1_descendants, vec![gc1]);
}
