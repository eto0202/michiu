#![expect(clippy::unwrap_used)]

use super::*;
use slotmap::SlotMap;
use smallvec::SmallVec;
use taffy::{Style, TraversePartialTree};

struct TestFixture {
    entities: SlotMap<Entity, ()>,
    taffy_tree: TaffyTreeEntity,
    taffy_nodes: TaffyNodesSecondary,
    active_masks: ActiveMasksSecondary,
    parents: ParentsSecondary,
    children: ChildrenSecondary,
    dirty_entities: DirtyLayoutEntitiesVec,
    debug: DebugStore,
}

impl TestFixture {
    fn new() -> Self {
        Self {
            entities: SlotMap::with_key(),
            taffy_tree: TaffyTreeEntity::new(),
            taffy_nodes: TaffyNodesSecondary::default(),
            active_masks: ActiveMasksSecondary::default(),
            parents: ParentsSecondary::default(),
            children: ChildrenSecondary::default(),
            dirty_entities: DirtyLayoutEntitiesVec::default(),
            debug: DebugStore::new(),
        }
    }

    fn spawn_node(&mut self, parent: Option<Entity>) -> Entity {
        let entity = self.entities.insert(());
        let node = self
            .taffy_tree
            .new_leaf_with_context(Style::default(), entity)
            .unwrap();

        self.taffy_nodes.insert(entity, node);
        self.active_masks.insert(entity, ComponentMask::default());
        self.parents.insert(entity, parent);
        self.children.insert(entity, SmallVec::default());

        if let Some(p) = parent {
            self.children.at_mut(p).push(entity);
            let p_node = *self.taffy_nodes.at(p);
            self.taffy_tree.add_child(p_node, node).unwrap();
        }

        entity
    }
}

// `LayoutStore::resync_taffy_children_order` の検証
//
// `test_resync_taffy_children_order_reorder`
//  - 親ノードと複数の子ノード（A, B, C）を作成し、Taffy に初期登録。
//  - `ChildrenSecondary` 側の順序を (C, A, B) に変更して `resync_taffy_children_order` を実行。
//  - Taffy 側の `lay_taffy_tree.children(parent_node)` の順序が (C, A, B) に正確に同期されることを確認。
//
// test_resync_taffy_children_order_empty`
//  - 子ノードが存在する状態から、`ChildrenSecondary` を空リストに変更して実行。
//  - Taffy 側の子ノードがすべてデタッチされ、0件になることを確認。

#[test]
fn test_resync_taffy_children_order_reorder() {
    let mut f = TestFixture::new();
    let parent = f.spawn_node(None);
    let c1 = f.spawn_node(Some(parent));
    let c2 = f.spawn_node(Some(parent));
    let c3 = f.spawn_node(Some(parent));

    let p_node = *f.taffy_nodes.at(parent);
    let n1 = *f.taffy_nodes.at(c1);
    let n2 = *f.taffy_nodes.at(c2);
    let n3 = *f.taffy_nodes.at(c3);

    // 順序を (c3, c1, c2) に変更
    f.children.at_mut(parent).clear();
    f.children.at_mut(parent).extend([c3, c1, c2]);

    LayoutStore::resync_taffy_children_order(
        parent,
        &f.children,
        &mut f.taffy_tree,
        &f.taffy_nodes,
        &mut f.debug,
    );

    let taffy_children = f.taffy_tree.children(p_node).unwrap();
    assert_eq!(taffy_children, vec![n3, n1, n2]);
}

#[test]
fn test_resync_taffy_children_order_empty() {
    let mut f = TestFixture::new();
    let parent = f.spawn_node(None);
    let _c1 = f.spawn_node(Some(parent));

    let p_node = *f.taffy_nodes.at(parent);
    assert_eq!(f.taffy_tree.child_count(p_node), 1);

    // 子を空にする
    f.children.at_mut(parent).clear();

    LayoutStore::resync_taffy_children_order(
        parent,
        &f.children,
        &mut f.taffy_tree,
        &f.taffy_nodes,
        &mut f.debug,
    );

    assert_eq!(f.taffy_tree.child_count(p_node), 0);
}

// `LayoutStore::mark_layout_dirty` & `clear_layout_dirty` の検証
//
// `test_mark_layout_dirty_propagation_to_ancestors`
//  - 親 $\to$ 子 $\to$ 孫 のツリー構造を構築。
//  - 孫に対して `mark_layout_dirty` を呼び出す。
//    1. 孫の Taffy ノードが `dirty` 状態になっていること。
//    2. 孫、子、親のすべてに `ComponentMask::STATE_QUEUED_LAYOUT` がセットされること。
//    3. `lay_dirty_entities` に `[孫, 子, 親]` の順で追加されること。
// 
// `test_mark_layout_dirty_early_break_on_already_dirty`
//  - 親にあらかじめ `STATE_QUEUED_LAYOUT` をセットしておく。
//  - 孫に対して `mark_layout_dirty` を呼び出す。
//  - 親に到達した時点で探索が打ち切られ、親より上の先祖や重複登録が発生しないことを確認。
// 
// `test_clear_layout_dirty`
//  - `STATE_QUEUED_LAYOUT` が付与された複数エンティティをキューに投入。
//  - `clear_layout_dirty` を呼び出し後、全エンティティのマスクが解除され、`lay_dirty_entities` が空になることを確認。

#[test]
fn test_mark_layout_dirty_propagation_to_ancestors() {
    let mut f = TestFixture::new();
    let root = f.spawn_node(None);
    let child = f.spawn_node(Some(root));
    let grandchild = f.spawn_node(Some(child));

    LayoutStore::mark_layout_dirty(
        grandchild,
        &mut f.active_masks,
        &f.parents,
        &mut f.dirty_entities,
        &mut f.taffy_tree,
        &f.taffy_nodes,
        &mut f.debug,
    );

    // 孫の Taffy ノードが dirty 化されているか
    let gc_node = *f.taffy_nodes.at(grandchild);
    assert!(f.taffy_tree.dirty(gc_node).unwrap());

    // 孫 -> 子 -> 親 の順でキューに追加されたか
    assert_eq!(f.dirty_entities.as_slice(), &[grandchild, child, root]);

    // 全祖先にマスクが設定されたか
    assert!(
        f.active_masks
            .at(grandchild)
            .has(ComponentMask::STATE_QUEUED_LAYOUT)
    );
    assert!(
        f.active_masks
            .at(child)
            .has(ComponentMask::STATE_QUEUED_LAYOUT)
    );
    assert!(
        f.active_masks
            .at(root)
            .has(ComponentMask::STATE_QUEUED_LAYOUT)
    );
}

#[test]
fn test_mark_layout_dirty_early_break_on_already_dirty() {
    let mut f = TestFixture::new();
    let root = f.spawn_node(None);
    let child = f.spawn_node(Some(root));
    let grandchild = f.spawn_node(Some(child));

    // あらかじめ child にフラグをセット
    f.active_masks
        .at_mut(child)
        .set(ComponentMask::STATE_QUEUED_LAYOUT);

    LayoutStore::mark_layout_dirty(
        grandchild,
        &mut f.active_masks,
        &f.parents,
        &mut f.dirty_entities,
        &mut f.taffy_tree,
        &f.taffy_nodes,
        &mut f.debug,
    );

    // child で探索が停止するため、キューには grandchild のみ入り、root は追加されない
    assert_eq!(f.dirty_entities.as_slice(), &[grandchild]);
    assert!(
        !f.active_masks
            .at(root)
            .has(ComponentMask::STATE_QUEUED_LAYOUT)
    );
}

#[test]
fn test_clear_layout_dirty() {
    let mut f = TestFixture::new();
    let e1 = f.spawn_node(None);
    let e2 = f.spawn_node(None);

    f.active_masks
        .at_mut(e1)
        .set(ComponentMask::STATE_QUEUED_LAYOUT);
    f.active_masks
        .at_mut(e2)
        .set(ComponentMask::STATE_QUEUED_LAYOUT);
    f.dirty_entities.push(e1);
    f.dirty_entities.push(e2);

    LayoutStore::clear_layout_dirty(&mut f.active_masks, &mut f.dirty_entities);

    assert!(f.dirty_entities.is_empty());
    assert!(
        !f.active_masks
            .at(e1)
            .has(ComponentMask::STATE_QUEUED_LAYOUT)
    );
    assert!(
        !f.active_masks
            .at(e2)
            .has(ComponentMask::STATE_QUEUED_LAYOUT)
    );
}
