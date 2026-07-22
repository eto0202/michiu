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
}
