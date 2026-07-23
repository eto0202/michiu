use crate::*;
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
    /// 要素の階層トポロジーを親（Ancestor）に向かって遡り、最初に見つかった型 T の ReadSignal を解決して返します
    pub(crate) fn use_provided_from<T: Clone + 'static>(
        id: EntityId,
        reactive: &ReactiveStore,
        topology: &TopologyStore,
    ) -> Option<ReadSignal<T>> {
        let mut curr = Some(id);
        let type_id = std::any::TypeId::of::<T>();

        while let Some(curr_id) = curr {
            if let Some(map) = reactive.providers.get(curr_id)
                && let Some(&signal_id) = map.get(&type_id)
            {
                return Some(ReadSignal::new(signal_id));
            }
            // トポロジー親を安全に探索
            curr = topology.parents.get(curr_id).copied().flatten();
        }
        None
    }

    pub(crate) fn resolve_element_effect(reactive: &ReactiveStore) -> EntityId {
        // 1. ACTIVE_EFFECT（エフェクト実行中）から解決を試みる
        if let Some(active_effect_id) = crate::signal::ACTIVE_EFFECT.with(|cell| cell.get()) {
            reactive
                .effect_to_element
                .get(active_effect_id)
                .copied()
                .expect("use_provided failed: active effect is not associated with any UI Element")
        } else if let Some(active_element_id) =
            crate::signal::ACTIVE_ELEMENT.with(|cell| cell.get())
        {
            // 2. ACTIVE_EFFECTがNoneであれば、ACTIVE_ELEMENT（イベントハンドラ実行中）にフォールバック
            active_element_id
        } else {
            panic!(
                "use_provided must be called inside a dynamic style, text, content closure, or an active event handler context"
            );
        }
    }

    /// 現在のスレッドローカルコンテキストから、
    /// 親ツリーを自動的に遡って解決した型 T のシグナルに対する同期書き込み用端（WriteSignal）を取得します。
    pub fn use_provided_setter<T: Send + 'static>(
        reactive: &ReactiveStore,
        topology: &TopologyStore,
    ) -> WriteSignal<T> {
        let element_id = if let Some(active_effect_id) =
            crate::signal::ACTIVE_EFFECT.with(|cell| cell.get())
        {
            reactive
                .effect_to_element
                .get(active_effect_id)
                .copied()
                .expect("use_provided_setter failed: active effect not associated with an Element")
        } else if let Some(active_element_id) =
            crate::signal::ACTIVE_ELEMENT.with(|cell| cell.get())
        {
            active_element_id
        } else {
            panic!(
                "use_provided_setter must be called inside a dynamic reactive context or an active event handler context"
            );
        };

        let mut curr = Some(element_id);
        let type_id = std::any::TypeId::of::<T>();

        while let Some(curr_id) = curr {
            if let Some(map) = reactive.providers.get(curr_id)
                && let Some(&signal_id) = map.get(&type_id)
            {
                return WriteSignal {
                    id: signal_id,
                    _marker: std::marker::PhantomData,
                };
            }
            curr = topology.parents.get(curr_id).copied().flatten();
        }
        panic!(
            "Dependency resolution failed: No Provider Setter found in ancestor sub-tree for type: '{}'",
            std::any::type_name::<T>()
        )
    }

    /// 要素にエフェクトをカテゴリ指定付きで紐づけて登録します。
    /// 同一カテゴリのエフェクトが既に存在する場合、自動的に古いエフェクトを破棄してから上書きします。
    pub(crate) fn register_element_effect(
        element_id: EntityId,
        reactive: &mut ReactiveStore,
        category: EffectCategory,
        effect_id: EffectId,
    ) {
        if let Some(effects) = reactive.element_effects.get_mut(element_id) {
            // 同一カテゴリのエフェクトが既に登録されていれば、古いものを破棄
            if let Some(pos) = effects.iter().position(|(cat, _)| *cat == category) {
                let (_, old_effect_id) = effects.remove(pos);
                reactive.effects.remove(old_effect_id); // SoA から古いエフェクト実体を削除
            }
            effects.push((category, effect_id));
        } else {
            reactive
                .element_effects
                .insert(element_id, smallvec::smallvec![(category, effect_id)]);
        }
    }

    /// 要素に動的エフェクト（Style、Text等のリアクティブクロージャ）を安全に登録し、初期評価を実行します。
    pub(crate) fn create_element_effect<F>(
        element_id: EntityId,
        reactive: &mut ReactiveStore,
        category: EffectCategory,
        f: F,
    ) -> EffectId
    where
        F: FnMut(&mut Context) + 'static,
    {
        let effect_id = reactive.effects.insert(Box::new(f));

        // 初回評価が走る前に要素との紐付けを確実に登録
        reactive.effect_to_element.insert(effect_id, element_id);

        // 要素のエフェクトリストに登録し、既存の同じカテゴリの古いエフェクトは自動破棄
        if !reactive.element_effects.contains_key(element_id) {
            reactive
                .element_effects
                .insert(element_id, smallvec::smallvec![]);
        }
        let list = reactive.element_effects.get_mut(element_id).unwrap();
        if let Some(pos) = list.iter().position(|(cat, _)| *cat == category) {
            let (_, old_id) = list.remove(pos);
            reactive.effects.remove(old_id);
            reactive.effect_to_element.remove(old_id);
            reactive.pending_element_effects.retain(|&x| x != old_id); // キューから古いものを排除
        }
        list.push((category, effect_id));

        // 即時実行を廃止。トポロジーが整うまで初回評価を一時保留
        reactive.pending_element_effects.push(effect_id);

        effect_id
    }

    /// トポロジーが完全に完成したビルド完了後、または同期直前に、溜めてある初回評価を一挙に安全実行します
    #[inline]
    pub(crate) fn evaluate_pending_element_effects(reactive: &mut ReactiveStore) {
        if reactive.pending_element_effects.is_empty() {
            return;
        }

        // 評価中に別のネストしたエフェクトが追加されるケースを許容するため、drain で一度排出して処理
        let pending: Vec<EffectId> = reactive.pending_element_effects.drain(..).collect();
        for effect_id in pending {
            if reactive.effects.contains_key(effect_id) {
                crate::execute_effect(effect_id);
            }
        }
    }

    /// 指定された要素に対してシグナルコンテキストを提供します
    #[inline]
    pub(crate) fn provide_context<T: Send + 'static>(
        id: EntityId,
        reactive: &mut ReactiveStore,
        signal_id: SignalId,
    ) {
        if !reactive.providers.contains_key(id) {
            reactive
                .providers
                .insert(id, std::collections::HashMap::new());
        }
        let map = reactive.providers.get_mut(id).unwrap();
        map.insert(std::any::TypeId::of::<T>(), signal_id);
    }

    /// Context インスタンスから直接シグナルを生成します。
    /// これにより build_ui の外側（メインスレッド上）でもシグナルを定義できます。
    #[inline]
    pub fn create_signal<T: Send + 'static>(
        initial_value: T,
        reactive: &mut ReactiveStore,
    ) -> (ReadSignal<T>, WriteSignal<T>) {
        let id = reactive.signals.insert(Box::new(initial_value));
        reactive.subscribers.insert(id, SmallVec::new());
        (
            ReadSignal {
                id,
                _marker: PhantomData,
            },
            WriteSignal {
                id,
                _marker: PhantomData,
            },
        )
    }
}

impl Context {
    /// Context インスタンスから直接シグナルを生成します。
    /// これにより build_ui の外側（メインスレッド上）でもシグナルを定義できます。
    #[inline]
    pub fn create_signal<T: Send + 'static>(
        &mut self,
        initial_value: T,
    ) -> (ReadSignal<T>, WriteSignal<T>) {
        ReactiveStore::create_signal(initial_value, &mut self.reactive)
    }

    /// 要素の階層トポロジーを親（Ancestor）に向かって遡り、最初に見つかった型 T の ReadSignal を解決して返します
    #[inline]
    pub(crate) fn use_provided_from<T: Clone + 'static>(
        &self,
        id: EntityId,
    ) -> Option<ReadSignal<T>> {
        ReactiveStore::use_provided_from(id, &self.reactive, &self.topology)
    }

    /// 現在のスレッドローカルコンテキスト（アクティブなエフェクト、またはイベントハンドラ）から、
    /// 自動的に対象の要素を特定し、親ツリーを遡って型 T の ReadSignal を解決します。
    #[inline]
    pub fn use_provided<T: Clone + 'static>(&self) -> ReadSignal<T> {
        // ACTIVE_EFFECT（エフェクト実行中）から解決を試みる
        let element_id = ReactiveStore::resolve_element_effect(&self.reactive);

        // 親ツリーを遡って解決
        ReactiveStore::use_provided_from::<T>(element_id, &self.reactive, &self.topology)
                .unwrap_or_else(|| {
                    panic!(
                        "Dependency resolution failed: No Provider found in ancestor sub-tree for type: '{}'",
                        std::any::type_name::<T>()
                    )
                })
    }

    /// 現在のスレッドローカルコンテキストから、
    /// 親ツリーを自動的に遡って解決した型 T のシグナルに対する同期書き込み用端（WriteSignal）を取得します。
    #[inline]
    pub fn use_provided_setter<T: Send + 'static>(&self) -> WriteSignal<T> {
        ReactiveStore::use_provided_setter(&self.reactive, &self.topology)
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
        ReactiveStore::register_element_effect(element_id, &mut self.reactive, category, effect_id);
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
        ReactiveStore::create_element_effect(element_id, &mut self.reactive, category, f)
    }

    /// トポロジーが完全に完成したビルド完了後、または同期直前に、溜めてある初回評価を一挙に安全実行します
    #[inline]
    pub(crate) fn evaluate_pending_element_effects(&mut self) {
        ReactiveStore::evaluate_pending_element_effects(&mut self.reactive);
    }

    /// 指定された要素に対してシグナルコンテキストを提供します
    #[inline]
    pub(crate) fn provide_context<T: Send + 'static>(&mut self, id: EntityId, signal_id: SignalId) {
        ReactiveStore::provide_context::<T>(id, &mut self.reactive, signal_id);
    }
}
