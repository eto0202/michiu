use std::any::TypeId;

use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::{EntityId, FlatDfsSequenceVec, ParentsSecondary};

#[derive(Default, Debug, Clone)]
pub struct MichiuTagRegistry {
    pub type_to_entities: FxHashMap<TypeId, SmallVec<[EntityId; 1]>>,
    pub entity_to_types: FxHashMap<EntityId, SmallVec<[TypeId; 4]>>,
}

impl MichiuTagRegistry {
    #[inline]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 特定の型 `T` に紐づく `EntityId` を追加する
    #[inline]
    pub(crate) fn register_entity<T: 'static>(&mut self, id: EntityId) {
        let type_id = TypeId::of::<T>();
        self.type_to_entities.entry(type_id).or_default().push(id);

        self.entity_to_types.entry(id).or_default().push(type_id);
    }

    /// 削除
    #[inline]
    pub(crate) fn unregister_entity(&mut self, id: EntityId) {
        // そのエンティティがどの型に登録されていたかを取得してインデックスから削除
        if let Some(associated_types) = self.entity_to_types.remove(&id) {
            for type_id in associated_types {
                // 紐づいていた型の SmallVec から、該当の EntityId を削除
                if let Some(entities) = self.type_to_entities.get_mut(&type_id) {
                    entities.retain(|i| *i != id);
                }
            }
        }
    }

    /// 検索
    #[inline]
    pub(crate) fn get_entities<T: 'static>(&self) -> Option<&SmallVec<[EntityId; 1]>> {
        self.type_to_entities.get(&TypeId::of::<T>())
    }

    /// 特定の親エンティティの子孫の中から、型 T を持つエンティティを検索する
    #[inline]
    pub(crate) fn query_descendants_of_type<'a, T: 'static>(
        &'a self,
        parent: EntityId,
        topo_flat_dfs_sequence: &'a FlatDfsSequenceVec,
        topo_parents: &'a ParentsSecondary,
    ) -> Box<dyn Iterator<Item = EntityId> + 'a> {
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

    /// 自分自身の子孫の中で、最初に見つかった型 T の `EntityId` を返す
    #[inline]
    pub(crate) fn query_first_descendant_of_type<T: 'static>(
        &self,
        parent: EntityId,
        topo_flat_dfs_sequence: &FlatDfsSequenceVec,
        topo_parents: &ParentsSecondary,
    ) -> Option<EntityId> {
        let type_id = TypeId::of::<T>();

        // 親の位置を探す
        let start_idx = topo_flat_dfs_sequence.iter().position(|&id| id == parent)?;
        let scan_start = start_idx + 1;

        // 最初に見つかったものを即座に返す
        for &id in topo_flat_dfs_sequence.iter().skip(scan_start) {
            // 子孫スコープから外れた瞬間に探索を打ち切り（高速化の肝）
            if !Self::is_descendant_of(id, parent, topo_parents) {
                break;
            }

            // 型のチェック
            let has_type = self
                .entity_to_types
                .get(&id)
                .is_some_and(|types| types.contains(&type_id));

            if has_type {
                return Some(id); // 見つかったので即座に返す！
            }
        }

        None
    }

    #[allow(unused)]
    #[inline]
    fn is_descendant_of(
        mut id: EntityId,
        target_parent: EntityId,
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
}
