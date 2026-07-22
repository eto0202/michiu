use crate::*;
use slotmap::{SecondaryMap, SlotMap, SparseSecondaryMap};
use smallvec::SmallVec;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EffectCategory {
    None,
    Style,
    Text,
    Input,
    Image,
    Movie,
    WebView2,
    Contents,
    UiaName,
    UiaAutomationId,
    ActiveState,
    SelectState,
    DisableState,
    FocusState,
    FocusableState,
}

pub struct ReactiveStore {
    pub(crate) signals: SlotMap<SignalId, Box<dyn std::any::Any>>,
    pub(crate) effects: SlotMap<EffectId, Effects>,
    pub(crate) subscribers: SecondaryMap<SignalId, SmallVec<[EffectId; 4]>>,
    pub(crate) element_effects: SecondaryMap<EntityId, SmallVec<[(EffectCategory, EffectId); 4]>>,
    pub(crate) effect_to_element: SecondaryMap<EffectId, EntityId>,
    pub(crate) pending_element_effects: Vec<EffectId>,
    pub(crate) providers: SparseSecondaryMap<EntityId, HashMap<std::any::TypeId, SignalId>>,
}

pub(crate) type Effects = Box<dyn FnMut(&mut Context)>;

impl Default for ReactiveStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ReactiveStore {
    #[inline]
    pub fn new() -> Self {
        Self {
            signals: SlotMap::with_key(),
            effects: SlotMap::with_key(),
            subscribers: SecondaryMap::new(),
            element_effects: SecondaryMap::new(),
            effect_to_element: SecondaryMap::new(),
            pending_element_effects: Vec::new(),
            providers: SparseSecondaryMap::new(),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.signals.clear();
        self.effects.clear();
        self.subscribers.clear();
        self.element_effects.clear();
        self.effect_to_element.clear();
        self.pending_element_effects.clear();
        self.providers.clear();
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        if let Some(effects) = self.element_effects.remove(id) {
            for (_, effect_id) in effects {
                self.effects.remove(effect_id);
                self.effect_to_element.remove(effect_id);
                self.pending_element_effects.retain(|&x| x != effect_id);
            }
        }
        self.providers.remove(id);
    }
}

impl ReactiveStore {
    /// 要素にエフェクトをカテゴリ指定付きで紐づけて登録します。
    /// 同一カテゴリのエフェクトが既に存在する場合、自動的に古いエフェクトを破棄してから上書きします。
    pub(crate) fn register_element_effect(
        &mut self,
        element_id: EntityId,
        category: EffectCategory,
        effect_id: EffectId,
    ) {
        if let Some(effects) = self.element_effects.get_mut(element_id) {
            // 同一カテゴリのエフェクトが既に登録されていれば、古いものを破棄
            if let Some(pos) = effects.iter().position(|(cat, _)| *cat == category) {
                let (_, old_effect_id) = effects.remove(pos);
                self.effects.remove(old_effect_id); // SoA から古いエフェクト実体を削除
            }
            effects.push((category, effect_id));
        } else {
            self.element_effects
                .insert(element_id, smallvec::smallvec![(category, effect_id)]);
        }
    }

    /// 要素に動的エフェクト（Style、Text等のリアクティブクロージャ）を安全に登録し、初期評価を実行します。
    pub(crate) fn create_element_effect<F>(
        &mut self,
        element_id: EntityId,
        category: EffectCategory,
        f: F,
    ) -> EffectId
    where
        F: FnMut(&mut Context) + 'static,
    {
        let effect_id = self.effects.insert(Box::new(f));

        // 初回評価が走る前に要素との紐付けを確実に登録
        self.effect_to_element.insert(effect_id, element_id);

        // 要素のエフェクトリストに登録し、既存の同じカテゴリの古いエフェクトは自動破棄
        if !self.element_effects.contains_key(element_id) {
            self.element_effects
                .insert(element_id, smallvec::smallvec![]);
        }
        let list = self.element_effects.get_mut(element_id).unwrap();
        if let Some(pos) = list.iter().position(|(cat, _)| *cat == category) {
            let (_, old_id) = list.remove(pos);
            self.effects.remove(old_id);
            self.effect_to_element.remove(old_id);
            self.pending_element_effects.retain(|&x| x != old_id); // キューから古いものを排除
        }
        list.push((category, effect_id));

        // 即時実行を廃止。トポロジーが整うまで初回評価を一時保留
        self.pending_element_effects.push(effect_id);

        effect_id
    }

    /// トポロジーが完全に完成したビルド完了後、または同期直前に、溜めてある初回評価を一挙に安全実行します
    #[inline]
    pub(crate) fn evaluate_pending_element_effects(&mut self) {
        if self.pending_element_effects.is_empty() {
            return;
        }

        // 評価中に別のネストしたエフェクトが追加されるケースを許容するため、drain で一度排出して処理
        let pending: Vec<EffectId> = self.pending_element_effects.drain(..).collect();
        for effect_id in pending {
            if self.effects.contains_key(effect_id) {
                crate::execute_effect(effect_id);
            }
        }
    }

    /// 指定された要素に対してシグナルコンテキストを提供します
    #[inline]
    pub(crate) fn provide_context<T: Send + 'static>(&mut self, id: EntityId, signal_id: SignalId) {
        if !self.providers.contains_key(id) {
            self.providers.insert(id, std::collections::HashMap::new());
        }
        let map = self.providers.get_mut(id).unwrap();
        map.insert(std::any::TypeId::of::<T>(), signal_id);
    }
}
