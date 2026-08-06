use crate::{
    Context, EffectId, EntityId, ParentsSecondary, ReadSignal, SignalId, TopologyStore, WriteSignal,
};
use slotmap::{SecondaryMap, SlotMap, SparseSecondaryMap};
use smallvec::SmallVec;
use std::{collections::HashMap, marker::PhantomData};

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

pub(crate) type SignalsSlotMap = SlotMap<SignalId, Box<dyn std::any::Any>>;
pub(crate) type EffectsSlotMap = SlotMap<EffectId, Effects>;
pub(crate) type SubscribersSecondary = SecondaryMap<SignalId, SmallVec<[EffectId; 4]>>;
pub(crate) type ElementEffectsSecondary =
    SecondaryMap<EntityId, SmallVec<[(EffectCategory, EffectId); 4]>>;
pub(crate) type EffectToElementSecondary = SecondaryMap<EffectId, EntityId>;
pub(crate) type PendingElementEffectsVec = Vec<EffectId>;
pub(crate) type ProvidersSparseSecondary =
    SparseSecondaryMap<EntityId, HashMap<std::any::TypeId, SignalId>>;

pub struct ReactiveStore {
    pub(crate) signals: SignalsSlotMap,
    pub(crate) effects: EffectsSlotMap,
    pub(crate) subscribers: SubscribersSecondary,
    pub(crate) element_effects: ElementEffectsSecondary,
    pub(crate) effect_to_element: EffectToElementSecondary,
    pub(crate) pending_element_effects: PendingElementEffectsVec,
    pub(crate) providers: ProvidersSparseSecondary,
}

pub(crate) type Effects = Box<dyn FnMut(&mut Context)>;

impl Default for ReactiveStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ReactiveStore {
    #[inline]
    #[must_use]
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
    /// 要素の階層トポロジーを親に向かって遡り、最初に見つかった型 T の `ReadSignal` を解決して返す
    pub(crate) fn use_provided_from<T: Clone + 'static>(
        id: EntityId,
        providers: &ProvidersSparseSecondary,
        parents: &ParentsSecondary,
    ) -> Option<ReadSignal<T>> {
        let type_id = std::any::TypeId::of::<T>();

        // 親要素へ遡るイテレータを生成
        std::iter::successors(Some(id), |&curr_id| parents.get(curr_id).copied().flatten())
            .find_map(|curr_id| {
                providers
                    .get(curr_id)
                    .and_then(|map| map.get(&type_id))
                    .map(|&signal_id| ReadSignal::new(signal_id))
            })
    }

    pub(crate) fn resolve_element_effect(
        effect_to_element: &EffectToElementSecondary,
    ) -> Option<EntityId> {
        // ACTIVE_EFFECT（エフェクト実行中）から解決
        if let Some(effect_id) = crate::ACTIVE_EFFECT.with(std::cell::Cell::get) {
            return Some(effect_to_element.get(effect_id).copied().expect(
                "use_provided failed: active effect is not associated with any UI Element",
            ));
        }

        // ACTIVE_EFFECT が None であれば、ACTIVE_ELEMENT にフォールバック
        if let Some(element_id) = crate::ACTIVE_ELEMENT.with(std::cell::Cell::get) {
            return Some(element_id);
        }

        None
    }

    /// 親ツリーを遡り、最初に見つかった型 T の `WriteSignal` を解決して返す
    pub(crate) fn use_provided_setter_from<T: Send + 'static>(
        id: EntityId,
        providers: &ProvidersSparseSecondary,
        parents: &ParentsSecondary,
    ) -> Option<WriteSignal<T>> {
        let type_id = std::any::TypeId::of::<T>();

        // 親要素へ遡るイテレータを生成
        std::iter::successors(Some(id), |&curr_id| parents.get(curr_id).copied().flatten())
            .find_map(|curr_id| {
                providers
                    .get(curr_id)
                    .and_then(|map| map.get(&type_id))
                    .map(|&signal_id| WriteSignal::new(signal_id))
            })
    }

    /// 要素にエフェクトをカテゴリ指定付きで紐づけて登録。
    /// 同一カテゴリのエフェクトが既に存在する場合、自動的に古いエフェクトを破棄してから上書き。
    #[inline]
    pub(crate) fn register_element_effect(
        element_id: EntityId,
        category: EffectCategory,
        effect_id: EffectId,
        effects: &mut EffectsSlotMap,
        effect_to_element: &mut EffectToElementSecondary,
        pending_element_effects: &mut PendingElementEffectsVec,
        element_effects: &mut ElementEffectsSecondary,
    ) {
        // 既に登録済みの場合は、更新処理を行って早期リターン
        if let Some(e) = element_effects.get_mut(element_id) {
            if let Some(pos) = e.iter().position(|(cat, _)| *cat == category) {
                let (_, old_id) = e.remove(pos);
                effects.remove(old_id); // エフェクト実体を削除
                effect_to_element.remove(old_id); // 要素との紐付けを解除
                pending_element_effects.retain(|&x| x != old_id); // 実行待ちキューから排除
            }
            e.push((category, effect_id));
            return;
        }
        // 未登録の場合
        element_effects.insert(element_id, smallvec::smallvec![(category, effect_id)]);
    }

    /// 要素に動的エフェクトを登録し初期評価を実行
    pub(crate) fn create_element_effect<F>(
        element_id: EntityId,
        category: EffectCategory,
        effects: &mut EffectsSlotMap,
        effect_to_element: &mut EffectToElementSecondary,
        element_effects: &mut ElementEffectsSecondary,
        pending_element_effects: &mut PendingElementEffectsVec,
        f: F,
    ) -> EffectId
    where
        F: FnMut(&mut Context) + 'static,
    {
        let effect_id = effects.insert(Box::new(f));

        // 初回評価が走る前に要素との紐付けを登録
        effect_to_element.insert(effect_id, element_id);

        // 要素のエフェクトリストに登録し、古い同じカテゴリのエフェクトがあれば破棄
        ReactiveStore::register_element_effect(
            element_id,
            category,
            effect_id,
            effects,
            effect_to_element,
            pending_element_effects,
            element_effects,
        );

        // 即時実行を廃止。トポロジーが整うまで初回評価を一時保留
        pending_element_effects.push(effect_id);

        effect_id
    }

    /// ビルド完了後、または同期直前に、溜めてある初回評価を実行
    #[inline]
    pub(crate) fn evaluate_pending_element_effects(
        pending_element_effects: &mut PendingElementEffectsVec,
        effects: &mut EffectsSlotMap,
    ) {
        if pending_element_effects.is_empty() {
            return;
        }

        // 評価中に別のネストしたエフェクトが追加されるケースを許容するため、drain で一度排出して処理
        let pending: Vec<EffectId> = std::mem::take(pending_element_effects);

        for effect_id in pending.into_iter().filter(|&id| effects.contains_key(id)) {
            crate::execute_effect(effect_id);
        }
    }

    /// 指定された要素に対してシグナルコンテキストを提供
    #[inline]
    pub(crate) fn provide_context<T: Send + 'static>(
        id: EntityId,
        signal_id: SignalId,
        providers: &mut ProvidersSparseSecondary,
    ) {
        let Some(entry) = providers.entry(id) else {
            return;
        };

        let map = entry.or_default();
        map.insert(std::any::TypeId::of::<T>(), signal_id);
    }

    /// Context インスタンスから直接シグナルを生成。
    /// これにより `build_ui` の外側（メインスレッド上）でもシグナルを定義できる。
    #[inline]
    pub(crate) fn create_signal<T: Send + 'static>(
        initial_value: T,
        signals: &mut SignalsSlotMap,
        subscribers: &mut SubscribersSecondary,
    ) -> (ReadSignal<T>, WriteSignal<T>) {
        let id = signals.insert(Box::new(initial_value));

        subscribers.insert(id, SmallVec::new());

        (ReadSignal::new(id), WriteSignal::new(id))
    }
}

impl Context {
    /// 要素の階層トポロジーを親（Ancestor）に向かって遡り、最初に見つかった型 T の `ReadSignal` を解決して返します
    #[inline]
    pub(crate) fn use_provided_from<T: Clone + 'static>(
        &self,
        id: EntityId,
    ) -> Option<ReadSignal<T>> {
        let ReactiveStore { providers, .. } = &self.reactive;
        let TopologyStore { parents, .. } = &self.topology;

        ReactiveStore::use_provided_from(id, providers, parents)
    }

    /// 要素にエフェクトをカテゴリ指定付きで紐づけて登録します。
    /// 同一カテゴリのエフェクトが既に存在する場合、自動的に古いエフェクトを破棄してから上書きします。
    #[inline]
    pub(crate) fn register_element_effect(
        &mut self,
        element_id: EntityId,
        category: EffectCategory,
        effect_id: EffectId,
    ) {
        let ReactiveStore {
            effects,
            element_effects,
            effect_to_element,
            pending_element_effects,
            ..
        } = &mut self.reactive;

        ReactiveStore::register_element_effect(
            element_id,
            category,
            effect_id,
            effects,
            effect_to_element,
            pending_element_effects,
            element_effects,
        );
    }

    /// 要素に動的エフェクト（Style、Text等のリアクティブクロージャ）を安全に登録し、初期評価を実行します。
    #[inline]
    pub(crate) fn create_element_effect<F>(
        &mut self,
        element_id: EntityId,
        category: EffectCategory,
        f: F,
    ) -> EffectId
    where
        F: FnMut(&mut Context) + 'static,
    {
        let ReactiveStore {
            effects,
            effect_to_element,
            element_effects,
            pending_element_effects,
            ..
        } = &mut self.reactive;

        ReactiveStore::create_element_effect(
            element_id,
            category,
            effects,
            effect_to_element,
            element_effects,
            pending_element_effects,
            f,
        )
    }

    /// トポロジーが完全に完成したビルド完了後、または同期直前に、溜めてある初回評価を一挙に安全実行します
    #[inline]
    pub(crate) fn evaluate_pending_element_effects(&mut self) {
        let ReactiveStore {
            pending_element_effects,
            effects,
            ..
        } = &mut self.reactive;

        ReactiveStore::evaluate_pending_element_effects(pending_element_effects, effects);
    }

    /// 指定された要素に対してシグナルコンテキストを提供します
    #[inline]
    pub(crate) fn provide_context<T: Send + 'static>(&mut self, id: EntityId, signal_id: SignalId) {
        let ReactiveStore { providers, .. } = &mut self.reactive;

        ReactiveStore::provide_context::<T>(id, signal_id, providers);
    }
}
