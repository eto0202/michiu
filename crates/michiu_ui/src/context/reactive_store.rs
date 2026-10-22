use crate::{
    CapacityConfig, Context, EffectId, EntitiesSlot, Entity, FlatDfsSequenceVec, MichiuSoA,
    ParentsSecondary, ReadSignal, SignalId, TopologyStore, WriteSignal, define_secondary,
    define_slotmap, define_sparse_secondary, define_vec, execute_effect,
};
use rustc_hash::FxHashMap;
use slotmap::{SecondaryMap, SlotMap, SparseSecondaryMap};
use smallvec::SmallVec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectCategory {
    None,
    Style,
    Text,
    Input,
    Image,
    Movie,
    ExternalTexture,
    WebView2,
    Contents,
    Accessibility,
    ActiveState,
    SelectState,
    DisableState,
    FocusState,
    FocusableState,
}

pub struct Effects(pub Box<dyn FnMut(&mut Context)>);
impl std::fmt::Debug for Effects {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Effects(<closure>)")
    }
}

define_slotmap!(pub struct SignalsSlot(SignalId, Box<dyn std::any::Any>));
define_slotmap!(pub struct EffectsSlot(EffectId, Effects));

define_secondary!(pub struct SubscribersSecondary(SignalId, SmallVec<[EffectId; 8]>));
define_secondary!(pub struct EntityEffectsSecondary(SmallVec<[(EffectCategory, EffectId); 8]>));
define_secondary!(pub struct EffectToEntitySecondary(EffectId, Entity));

define_sparse_secondary!(pub struct ProvidersSparseSecondary(FxHashMap<std::any::TypeId, SignalId>));

define_vec!(pub struct PendingEntityEffectsVec(EffectId));

pub struct ReactiveStore {
    pub(crate) react_signals: SignalsSlot,
    pub(crate) react_effects: EffectsSlot,
    pub(crate) react_subscribers: SubscribersSecondary,
    pub(crate) react_entity_effects: EntityEffectsSecondary,
    pub(crate) react_effect_to_entity: EffectToEntitySecondary,
    pub(crate) react_pending_entity_effects: PendingEntityEffectsVec,
    pub(crate) react_providers: ProvidersSparseSecondary,
}

impl Default for ReactiveStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ReactiveStore {
    #[inline]
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            react_signals: SignalsSlot(SlotMap::with_key()),
            react_effects: EffectsSlot(SlotMap::with_key()),
            react_subscribers: SubscribersSecondary(SecondaryMap::new()),
            react_entity_effects: EntityEffectsSecondary(SecondaryMap::new()),
            react_effect_to_entity: EffectToEntitySecondary(SecondaryMap::new()),
            react_pending_entity_effects: PendingEntityEffectsVec(Vec::new()),
            react_providers: ProvidersSparseSecondary(SparseSecondaryMap::new()),
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn with_capacity(c: &CapacityConfig) -> Self {
        Self {
            react_signals: SignalsSlot(SlotMap::with_capacity_and_key(c.react_signals)),
            react_effects: EffectsSlot(SlotMap::with_capacity_and_key(c.react_effects)),
            react_subscribers: SubscribersSecondary(SecondaryMap::with_capacity(
                c.react_subscribers,
            )),
            react_entity_effects: EntityEffectsSecondary(SecondaryMap::with_capacity(
                c.react_entity_effects,
            )),
            react_effect_to_entity: EffectToEntitySecondary(SecondaryMap::with_capacity(
                c.react_effect_to_entity,
            )),
            react_pending_entity_effects: PendingEntityEffectsVec(Vec::with_capacity(
                c.react_pending_entity_effects,
            )),
            react_providers: ProvidersSparseSecondary(SparseSecondaryMap::with_capacity(
                c.react_providers,
            )),
        }
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.react_signals.clear();
        self.react_effects.clear();
        self.react_subscribers.clear();
        self.react_entity_effects.clear();
        self.react_effect_to_entity.clear();
        self.react_pending_entity_effects.clear();
        self.react_providers.clear();
    }

    #[inline]
    pub(crate) fn despawn(&mut self, id: Entity) {
        if let Some(react_effects) = self.react_entity_effects.remove(id) {
            for (_, effect_id) in react_effects {
                self.react_effects.remove(effect_id);
                self.react_effect_to_entity.remove(effect_id);
                self.react_pending_entity_effects
                    .retain(|&x| x != effect_id);
            }
        }
        self.react_providers.remove(id);
    }
}

impl ReactiveStore {
    /// 要素の階層トポロジーを親に向かって遡り、最初に見つかった型 T の `ReadSignal` を解決して返す
    #[track_caller]
    #[inline]
    pub(crate) fn use_provided_from<T: Clone + 'static>(
        id: Entity,
        react_providers: &ProvidersSparseSecondary,
        topo_parents: &ParentsSecondary,
    ) -> Option<ReadSignal<T>> {
        let type_id = std::any::TypeId::of::<T>();

        // 親要素へ遡るイテレータを生成
        std::iter::successors(Some(id), |&curr_id| *topo_parents.at(curr_id)).find_map(|curr_id| {
            react_providers
                .find(curr_id)
                .and_then(|map| map.get(&type_id))
                .map(|&signal_id| ReadSignal::new(signal_id))
        })
    }

    pub(crate) fn resolve_entity_effect(
        react_effect_to_entity: &EffectToEntitySecondary,
    ) -> Option<Entity> {
        // ACTIVE_EFFECT（エフェクト実行中）から解決
        if let Some(effect_id) = crate::ACTIVE_EFFECT.with(std::cell::Cell::get) {
            return react_effect_to_entity.get(effect_id).copied();
        }

        // ACTIVE_EFFECT が None であれば、ACTIVE_ENTITY にフォールバック
        if let Some(entity) = crate::ACTIVE_ENTITY.with(std::cell::Cell::get) {
            return Some(entity);
        }

        None
    }

    /// 親ツリーを遡り、最初に見つかった型 T の `WriteSignal` を解決して返す
    #[track_caller]
    #[inline]
    pub(crate) fn use_provided_setter_from<T: Send + 'static>(
        id: Entity,
        react_providers: &ProvidersSparseSecondary,
        topo_parents: &ParentsSecondary,
    ) -> Option<WriteSignal<T>> {
        let type_id = std::any::TypeId::of::<T>();

        // 親要素へ遡るイテレータを生成
        std::iter::successors(Some(id), |&curr_id| *topo_parents.at(curr_id)).find_map(|curr_id| {
            react_providers
                .find(curr_id)
                .and_then(|map| map.get(&type_id))
                .map(|&signal_id| WriteSignal::new(signal_id))
        })
    }

    /// 要素にエフェクトをカテゴリ指定付きで紐づけて登録。
    /// 同一カテゴリのエフェクトが既に存在する場合、自動的に古いエフェクトを破棄してから上書き。
    #[inline]
    pub(crate) fn register_entity_effect(
        entity: Entity,
        category: EffectCategory,
        effect_id: EffectId,
        react_effects: &mut EffectsSlot,
        react_entity_effects: &mut EntityEffectsSecondary,
        react_effect_to_entity: &mut EffectToEntitySecondary,
        react_pending_entity_effects: &mut PendingEntityEffectsVec,
    ) {
        // 既に登録済みの場合は、更新処理を行って早期リターン
        if let Some(e) = react_entity_effects.find_mut(entity) {
            if let Some(pos) = e.iter().position(|(cat, _)| *cat == category) {
                let (_, old_id) = e.remove(pos);
                react_effects.remove(old_id); // エフェクト実体を削除
                react_effect_to_entity.remove(old_id); // 要素との紐付けを解除
                react_pending_entity_effects.retain(|&x| x != old_id); // 実行待ちキューから排除
            }
            e.push((category, effect_id));
            return;
        }
        // 未登録の場合
        react_entity_effects.insert(entity, smallvec::smallvec![(category, effect_id)]);
    }

    /// 要素に動的エフェクトを登録し初期評価を実行
    pub(crate) fn create_entity_effect<F>(
        entity: Entity,
        category: EffectCategory,
        react_effects: &mut EffectsSlot,
        react_entity_effects: &mut EntityEffectsSecondary,
        react_effect_to_entity: &mut EffectToEntitySecondary,
        react_pending_entity_effects: &mut PendingEntityEffectsVec,
        f: F,
    ) -> EffectId
    where
        F: FnMut(&mut Context) + 'static,
    {
        let effect_id = react_effects.insert(Effects(Box::new(f)));

        // 初回評価が走る前に要素との紐付けを登録
        react_effect_to_entity.insert(effect_id, entity);

        // 要素のエフェクトリストに登録し、古い同じカテゴリのエフェクトがあれば破棄
        ReactiveStore::register_entity_effect(
            entity,
            category,
            effect_id,
            react_effects,
            react_entity_effects,
            react_effect_to_entity,
            react_pending_entity_effects,
        );

        // 即時実行を廃止。トポロジーが整うまで初回評価を一時保留
        react_pending_entity_effects.push(effect_id);

        effect_id
    }

    /// 指定された要素もしくはルート要素に対してシグナルコンテキストを提供
    #[inline]
    pub(crate) fn provide<T: Send + 'static>(
        id: Option<Entity>,
        read_signal: ReadSignal<T>,
        react_providers: &mut ProvidersSparseSecondary,
        topo_entities: &EntitiesSlot,
        topo_parents: &ParentsSecondary,
        topo_flat_dfs_sequence: &FlatDfsSequenceVec,
    ) {
        let id = if let Some(i) = id {
            i
        } else {
            let Some(i) =
                TopologyStore::find_root(topo_entities, topo_parents, topo_flat_dfs_sequence)
            else {
                return;
            };
            i
        };
        let Some(entry) = react_providers.entry(id) else {
            return;
        };

        let map = entry.or_default();
        map.insert(std::any::TypeId::of::<T>(), read_signal.id);
    }

    /// Context インスタンスから直接シグナルを生成。
    /// これにより `build_ui` の外側（メインスレッド上）でもシグナルを定義できる。
    #[inline]
    pub(crate) fn create_signal<T: Send + 'static>(
        initial_value: T,
        react_signals: &mut SignalsSlot,
        react_subscribers: &mut SubscribersSecondary,
    ) -> (ReadSignal<T>, WriteSignal<T>) {
        let id = react_signals.insert(Box::new(initial_value));

        react_subscribers.insert(id, SmallVec::new());

        (ReadSignal::new(id), WriteSignal::new(id))
    }

    #[inline]
    pub(crate) fn evaluate_pending_entity_effects(
        react_pending_entity_effects: &mut PendingEntityEffectsVec,
        react_effects: &EffectsSlot,
    ) {
        // レイアウトが再計算される前に、溜まっているすべてのエフェクトを評価完了させる
        if react_pending_entity_effects.is_empty() {
            return;
        }

        let pending: Vec<EffectId> = std::mem::take(react_pending_entity_effects);
        for effect_id in pending
            .into_iter()
            .filter(|&id| react_effects.contains_key(id))
        {
            execute_effect(effect_id);
        }
    }

    #[inline]
    pub fn effect_to_entity_mut(&mut self) -> &mut EffectToEntitySecondary {
        &mut self.react_effect_to_entity
    }

    #[inline]
    pub fn effects_mut(&mut self) -> &mut EffectsSlot {
        &mut self.react_effects
    }

    #[inline]
    pub fn entity_effects_mut(&mut self) -> &mut EntityEffectsSecondary {
        &mut self.react_entity_effects
    }

    #[inline]
    pub fn pending_entity_effects_mut(&mut self) -> &mut PendingEntityEffectsVec {
        &mut self.react_pending_entity_effects
    }

    #[inline]
    pub fn providers_mut(&mut self) -> &mut ProvidersSparseSecondary {
        &mut self.react_providers
    }

    #[inline]
    pub fn signals_mut(&mut self) -> &mut SignalsSlot {
        &mut self.react_signals
    }

    #[inline]
    pub fn subscribers_mut(&mut self) -> &mut SubscribersSecondary {
        &mut self.react_subscribers
    }
}

impl Context {
    /// 要素に動的エフェクト（Style、Text等のリアクティブクロージャ）を安全に登録し、初期評価を実行します。
    #[inline]
    pub(crate) fn create_entity_effect<F>(
        &mut self,
        entity: Entity,
        category: EffectCategory,
        f: F,
    ) -> EffectId
    where
        F: FnMut(&mut Context) + 'static,
    {
        ReactiveStore::create_entity_effect(
            entity,
            category,
            &mut self.reactive.react_effects,
            &mut self.reactive.react_entity_effects,
            &mut self.reactive.react_effect_to_entity,
            &mut self.reactive.react_pending_entity_effects,
            f,
        )
    }

    #[inline]
    pub(crate) fn evaluate_pending_entity_effects(&mut self) {
        ReactiveStore::evaluate_pending_entity_effects(
            &mut self.reactive.react_pending_entity_effects,
            &self.reactive.react_effects,
        );
    }
}

#[cfg(test)]
mod tests;
