use crate::{
    Entity, FlatDfsSequenceVec, ParentsSecondary,
    a11y::{A11yInferenceTag, InferenceFn},
};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use std::any::TypeId;

/// Registry for Managing Tags
#[derive(Default, Debug, Clone)]
pub struct MichiuTagRegistry {
    pub type_to_entities: FxHashMap<TypeId, SmallVec<[Entity; 1]>>,
    pub entity_to_types: FxHashMap<Entity, SmallVec<[TypeId; 4]>>,
    pub inference_handlers: FxHashMap<TypeId, InferenceFn>,
}

impl MichiuTagRegistry {
    #[inline]
    pub(crate) fn new() -> Self {
        let mut registry = Self::default();
        registry.register_builtin_inferences();
        registry
    }

    /// 特定の型 `T` に紐づく `Entity` を追加する
    #[inline]
    pub(crate) fn register_entity<T: 'static>(&mut self, id: Entity) {
        let type_id = TypeId::of::<T>();
        self.type_to_entities.entry(type_id).or_default().push(id);
        self.entity_to_types.entry(id).or_default().push(type_id);
    }

    /// 推論ルールを持つタグを登録する関数
    #[inline]
    pub(crate) fn register_a11y_inference<T: A11yInferenceTag + 'static>(&mut self) {
        self.inference_handlers
            .entry(TypeId::of::<T>())
            .or_insert(T::inference);
    }

    /// 削除
    #[inline]
    pub(crate) fn unregister_entity(&mut self, id: Entity) {
        // そのエンティティがどの型に登録されていたかを取得してインデックスから削除
        if let Some(associated_types) = self.entity_to_types.remove(&id) {
            for type_id in associated_types {
                // 紐づいていた型の SmallVec から、該当の Entity を削除
                if let Some(entities) = self.type_to_entities.get_mut(&type_id) {
                    entities.retain(|i| *i != id);
                }
            }
        }
    }

    /// 検索
    #[inline]
    pub(crate) fn get_entities<T: 'static>(&self) -> Option<&SmallVec<[Entity; 1]>> {
        self.type_to_entities.get(&TypeId::of::<T>())
    }

    /// 特定の親エンティティの子孫の中から、型 T を持つエンティティを検索する
    #[inline]
    pub(crate) fn query_descendants_of_type<'a, T: 'static>(
        &'a self,
        parent: Entity,
        topo_parents: &'a ParentsSecondary,
        topo_flat_dfs_sequence: &'a FlatDfsSequenceVec,
    ) -> Box<dyn Iterator<Item = Entity> + 'a> {
        let type_id = TypeId::of::<T>();

        // 親エンティティのインデックスを探す
        let Some(start_idx) = topo_flat_dfs_sequence.iter().position(|&id| id == parent) else {
            // 見つからなければ空のイテレータを即座に返す
            return Box::new(std::iter::empty());
        };

        let scan_start = start_idx + 1;

        // 子孫の終端インデックスを特定
        let mut end_idx = topo_flat_dfs_sequence.len();
        for (i, &id) in topo_flat_dfs_sequence.iter().enumerate().skip(scan_start) {
            if !Self::is_descendant_of(id, parent, topo_parents) {
                end_idx = i;
                break;
            }
        }

        // 子孫スライスをフィルターして Box に包んで返す
        let iter = topo_flat_dfs_sequence[scan_start..end_idx]
            .iter()
            .copied()
            .filter(move |&id| {
                self.entity_to_types
                    .get(&id)
                    .is_some_and(|types| types.contains(&type_id))
            });

        Box::new(iter)
    }

    /// 自分自身の子孫の中で、最初に見つかった型 T の `Entity` を返す
    #[inline]
    pub(crate) fn query_first_descendant_of_type<T: 'static>(
        &self,
        parent: Entity,
        topo_parents: &ParentsSecondary,
        topo_flat_dfs_sequence: &FlatDfsSequenceVec,
    ) -> Option<Entity> {
        let type_id = TypeId::of::<T>();

        // 親の位置
        let start_idx = topo_flat_dfs_sequence.iter().position(|&id| id == parent)?;
        let scan_start = start_idx + 1;

        // 最初に見つかったものを即座に返す
        for &id in topo_flat_dfs_sequence.iter().skip(scan_start) {
            // 子孫スコープから外れた瞬間に探索を打ち切り
            if !Self::is_descendant_of(id, parent, topo_parents) {
                break;
            }

            // 型のチェック
            let has_type = self
                .entity_to_types
                .get(&id)
                .is_some_and(|types| types.contains(&type_id));

            if has_type {
                return Some(id);
            }
        }

        None
    }

    #[inline]
    fn is_descendant_of(
        mut id: Entity,
        target_parent: Entity,
        topo_parents: &ParentsSecondary,
    ) -> bool {
        while let Some(&Some(parent)) = topo_parents.get(id) {
            if target_parent == parent {
                return true;
            }
            id = parent;
        }
        false
    }

    #[inline]
    pub(crate) fn collect_a11y_inferences(&self, entity_id: Entity) -> SmallVec<[InferenceFn; 1]> {
        let mut fns = SmallVec::new();
        if let Some(type_ids) = self.entity_to_types.get(&entity_id) {
            for type_id in type_ids {
                // 推論ハンドラがあるタグだけ
                if let Some(&handler) = self.inference_handlers.get(type_id) {
                    fns.push(handler);
                }
            }
        }
        fns
    }
}

#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    use super::*;
    use slotmap::SlotMap;

    struct TagA;
    struct TagB;
    struct TagC;

    fn setup_entities(count: usize) -> (SlotMap<Entity, ()>, Vec<Entity>) {
        let mut sm = SlotMap::<Entity, ()>::with_key();
        let entities = (0..count).map(|_| sm.insert(())).collect();
        (sm, entities)
    }

    // `test_register_and_get_entities`
    //  - 単一エンティティに複数のタグ型（例: `TagA`, `TagB`）を登録。
    //  - 単一タグ型に複数のエンティティを登録。
    //  - `get_entities::<T>()` で対応するエンティティ一覧が取得できること。
    //  - 未登録の型に対しては `None` が返ること。
    //
    // `test_unregister_entity`
    //  - エンティティに複数タグが紐づいた状態で `unregister_entity` を実行。
    //  - `entity_to_types` から完全に削除されること。
    //  - 各型の `type_to_entities` リストからも対象エンティティのみが正しく削除され、同タグを持つ他のエンティティは保持されること。
    //  - 存在しないエンティティの登録解除を呼び出しても安全に処理されること。
    //
    // `test_is_descendant_of`
    //  - 親 $\to$ 子 $\to$ 孫 のツリーを構築。
    //  - 子および孫が親の「子孫」と判定されること（`true`）。
    //  - 兄弟ノード、親自身、別サブツリーのノードは「子孫ではない」と判定されること（`false`）。
    //
    // `test_query_descendants_of_type`
    //  - 木構造（ルート、親A、子A1、子A2、親B、子B1）と対応する `FlatDfsSequenceVec` を用意。
    //  - 子A1 と 子B1 に同一タグ `TagA` を登録。
    //  - 親A を対象に `query_descendants_of_type::<TagA>` を実行。
    //  - 親A の子孫である 子A1 のみが抽出され、親B の子孫（子B1）や無関係なノードが混入しないことを確認。
    //  - DFS順序通りに走査・返却されること。
    //
    // `test_query_descendants_of_type_edge_cases`
    //  - 親ノードが `FlatDfsSequenceVec` 内に存在しない場合、空イテレータが返ること。
    //  - 対象タグを持つ子孫が存在しない場合、空イテレータが返ること。
    //
    // `test_query_first_descendant_of_type`
    //  - 親A の子孫に同一タグを持つエンティティが複数存在する場合（子A1、孫A1_1）、DFS順で最初に出現する要素（子A1）のみが `Some` で返ること。
    //  - 子孫範囲外を跨ぐ前に正しく探索が打ち切られること。
    //
    // `test_collect_a11y_inferences`
    //  - モックタグに対して `register_a11y_inference` を実行。
    //  - タグが付与されたエンティティから、該当する推論関数ポインタ（`InferenceFn`）が過不足なく収集されること。
    //  - 推論ハンドラが未登録のタグのみを持つエンティティでは空の `SmallVec` が返ること。

    #[test]
    fn test_register_and_get_entities() {
        let (_sm, e) = setup_entities(2);
        let mut registry = MichiuTagRegistry::default();

        registry.register_entity::<TagA>(e[0]);
        registry.register_entity::<TagB>(e[0]);
        registry.register_entity::<TagA>(e[1]);

        // TagA には e[0], e[1] が登録されている
        let tag_a_entities = registry.get_entities::<TagA>().expect("TagA should exist");
        assert_eq!(tag_a_entities.as_slice(), &[e[0], e[1]]);

        // TagB には e[0] のみが登録されている
        let tag_b_entities = registry.get_entities::<TagB>().expect("TagB should exist");
        assert_eq!(tag_b_entities.as_slice(), &[e[0]]);

        // 未登録の TagC は None
        assert!(registry.get_entities::<TagC>().is_none());
    }

    #[test]
    fn test_unregister_entity() {
        let (_sm, e) = setup_entities(2);
        let mut registry = MichiuTagRegistry::default();

        registry.register_entity::<TagA>(e[0]);
        registry.register_entity::<TagB>(e[0]);
        registry.register_entity::<TagA>(e[1]);

        registry.unregister_entity(e[0]);

        // e[0] の逆引きインデックスが削除されていること
        assert!(!registry.entity_to_types.contains_key(&e[0]));

        // TagA から e[0] が消え、e[1] だけが残っていること
        let tag_a_entities = registry.get_entities::<TagA>().expect("TagA should exist");
        assert_eq!(tag_a_entities.as_slice(), &[e[1]]);

        // TagB から e[0] が消え、空になっていること
        let tag_b_entities = registry.get_entities::<TagB>().expect("TagB should exist");
        assert!(tag_b_entities.is_empty());

        // 存在しないエンティティの解除呼び出しが安全であること
        registry.unregister_entity(e[0]);
    }

    #[test]
    fn test_is_descendant_of() {
        let (_sm, e) = setup_entities(4);
        let root = e[0];
        let child = e[1];
        let grandchild = e[2];
        let sibling = e[3];

        let mut parents = ParentsSecondary::default();
        parents.insert(root, None);
        parents.insert(child, Some(root));
        parents.insert(grandchild, Some(child));
        parents.insert(sibling, Some(root));

        // 深さ1
        assert!(MichiuTagRegistry::is_descendant_of(child, root, &parents));
        // 深さ2
        assert!(MichiuTagRegistry::is_descendant_of(
            grandchild, root, &parents
        ));
        assert!(MichiuTagRegistry::is_descendant_of(
            grandchild, child, &parents
        ));

        // 自分自身、兄弟、逆向きは false
        assert!(!MichiuTagRegistry::is_descendant_of(root, root, &parents));
        assert!(!MichiuTagRegistry::is_descendant_of(
            sibling, child, &parents
        ));
        assert!(!MichiuTagRegistry::is_descendant_of(root, child, &parents));
    }

    #[test]
    fn test_query_descendants_of_type() {
        let (_sm, e) = setup_entities(6);
        // ツリー構造:
        // root (e[0])
        //   ├─ parent_a (e[1])
        //   │    ├─ child_a1 (e[2])  [TagA]
        //   │    └─ child_a2 (e[3])  [TagB]
        //   └─ parent_b (e[4])
        //        └─ child_b1 (e[5])  [TagA]
        let root = e[0];
        let parent_a = e[1];
        let child_a1 = e[2];
        let child_a2 = e[3];
        let parent_b = e[4];
        let child_b1 = e[5];

        let mut parents = ParentsSecondary::default();
        parents.insert(root, None);
        parents.insert(parent_a, Some(root));
        parents.insert(child_a1, Some(parent_a));
        parents.insert(child_a2, Some(parent_a));
        parents.insert(parent_b, Some(root));
        parents.insert(child_b1, Some(parent_b));

        let mut dfs_seq = FlatDfsSequenceVec::default();
        dfs_seq.extend([root, parent_a, child_a1, child_a2, parent_b, child_b1]);

        let mut registry = MichiuTagRegistry::default();
        registry.register_entity::<TagA>(child_a1);
        registry.register_entity::<TagB>(child_a2);
        registry.register_entity::<TagA>(child_b1);

        // parent_a の子孫で TagA を持つものをクエリ
        let result: Vec<Entity> = registry
            .query_descendants_of_type::<TagA>(parent_a, &parents, &dfs_seq)
            .collect();

        // child_b1 は parent_b の配下なので含まれず、child_a1 のみ返る
        assert_eq!(result, vec![child_a1]);
    }

    #[test]
    fn test_query_descendants_of_type_edge_cases() {
        let (_sm, e) = setup_entities(3);
        let root = e[0];
        let child = e[1];
        let outside = e[2];

        let mut parents = ParentsSecondary::default();
        parents.insert(root, None);
        parents.insert(child, Some(root));

        let mut dfs_seq = FlatDfsSequenceVec::default();
        dfs_seq.extend([root, child]);

        let mut registry = MichiuTagRegistry::default();
        registry.register_entity::<TagA>(child);

        // DFS シーケンスに含まれないエンティティを指定した場合は空イテレータ
        let result: Vec<Entity> = registry
            .query_descendants_of_type::<TagA>(outside, &parents, &dfs_seq)
            .collect();
        assert!(result.is_empty());

        // 一致するタグを持つ子孫が存在しない場合は空イテレータ
        let result_no_tag: Vec<Entity> = registry
            .query_descendants_of_type::<TagC>(root, &parents, &dfs_seq)
            .collect();
        assert!(result_no_tag.is_empty());
    }

    #[test]
    fn test_query_first_descendant_of_type() {
        let (_sm, e) = setup_entities(4);
        let root = e[0];
        let child1 = e[1];
        let grandchild = e[2];
        let child2 = e[3];

        let mut parents = ParentsSecondary::default();
        parents.insert(root, None);
        parents.insert(child1, Some(root));
        parents.insert(grandchild, Some(child1));
        parents.insert(child2, Some(root));

        let mut dfs_seq = FlatDfsSequenceVec::default();
        dfs_seq.extend([root, child1, grandchild, child2]);

        let mut registry = MichiuTagRegistry::default();
        registry.register_entity::<TagA>(grandchild);
        registry.register_entity::<TagA>(child2);

        // root 配下で最初に見つかる TagA は DFS順で grandchild
        let found = registry.query_first_descendant_of_type::<TagA>(root, &parents, &dfs_seq);
        assert_eq!(found, Some(grandchild));

        // child1 の配下で探す場合も grandchild
        let found_in_child1 =
            registry.query_first_descendant_of_type::<TagA>(child1, &parents, &dfs_seq);
        assert_eq!(found_in_child1, Some(grandchild));

        // 該当タグがない場合は None
        let not_found = registry.query_first_descendant_of_type::<TagC>(root, &parents, &dfs_seq);
        assert_eq!(not_found, None);
    }

    #[test]
    fn test_collect_a11y_inferences() {
        let (_sm, e) = setup_entities(1);
        let entity = e[0];

        let registry = MichiuTagRegistry::new();

        // 登録されているビルトインのハンドラが存在する場合、その型をエンティティに付与して検証
        if let Some((&registered_type, &expected_fn)) = registry.inference_handlers.iter().next() {
            let mut reg = registry.clone();
            reg.entity_to_types
                .entry(entity)
                .or_default()
                .push(registered_type);

            let collected = reg.collect_a11y_inferences(entity);
            assert_eq!(collected.len(), 1);
            assert_eq!(collected[0] as usize, expected_fn as usize);
        } else {
            // ハンドラが空の環境でのフォールバック検証
            let collected = registry.collect_a11y_inferences(entity);
            assert!(collected.is_empty());
        }
    }
}
