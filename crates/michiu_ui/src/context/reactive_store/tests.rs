use super::*;
use crate::{ActiveEntityGuard, Context, bind_context, with_context};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// `test_reactive_create_signal`
//  - `ReactiveStore::create_signal(42, ...)` を呼び出す。
//  - `react_signals` に値が格納され、`react_subscribers` に空のサブスクライバが初期化されること。
//  - 返された `ReadSignal` / `WriteSignal` が同一の `SignalId` を持つこと。
//
// `test_provide_and_use_provided_hierarchical`
//  - 親 $\to$ 子 $\to$ 孫 のツリー構造を構築。
//  - 親に `provide` で型 `T` のシグナルを登録。
//  - 孫および子の位置から `use_provided_from::<T>` および `use_provided_setter_from::<T>` を呼び出し、親のシグナルが取得できること。
//  - 未登録の型に対しては `None` が返ること。
//
// `test_provide_shadowing`
//  - 親と子の両方で同一型 `T` の異なるシグナルを `provide`。
//  - 孫から検索した場合、より近い祖先（子）のシグナルが優先して取得されること。
//
// `test_provide_to_root_fallback`
//  - `provide` の引数 `id: None` を指定。
//  - `TopologyStore::find_root` で解決されたルート要素にプロバイダーが登録され、子孫から解決できること。
//
// `test_create_entity_effect_queuing`
//  - `create_entity_effect` で特定カテゴリ（例: `EffectCategory::Style`）のエフェクトを登録。
//  - エフェクト実体と `react_effect_to_entity` の逆引きが登録されること。
//  - 即時実行されず、`react_pending_entity_effects` にIDが積まれること。
//
// `test_register_entity_effect_replace_same_category`
//  - 同一エンティティに対して、同一カテゴリ（`EffectCategory::Style`）のエフェクトを続けて登録。
//  - 古いエフェクトが `react_effects`, `react_effect_to_entity`, `react_pending_entity_effects` から完全に除去され、新しいエフェクトに置き換わること。
//  - 異なるカテゴリ（例: `EffectCategory::Text`）であれば両方保持されること。
//
// `test_resolve_entity_effect_priority`
//  - `ACTIVE_EFFECT` が設定されている場合：該当エフェクトに紐づくエンティティが返ること。
//  - `ACTIVE_EFFECT` が `None` で `ACTIVE_ENTITY` が設定されている場合：フォールバックでそのエンティティが返ること。
//  - 双方とも `None` の場合：`None` が返ること。
//
// `test_evaluate_pending_entity_effects`
//  - 複数のエフェクトを `create_entity_effect` でキューイング。
//  - `evaluate_pending_entity_effects` を呼び出す。
//  - キュー内の全エフェクトが実行され、キューが空になること。
//  - 事前に破棄されたエフェクトがキューに残っていても、パニックせず安全にスキップされること。
//
// `test_reactive_despawn`
//  - エンティティに複数カテゴリのエフェクトおよびプロバイダーを登録。
//  - `store.despawn(entity)` を実行。
//  - 紐づいていた全エフェクトが `react_effects`, `react_effect_to_entity`, `react_pending_entity_effects` から除去されること。
//  - `react_providers` から対象エンティティのエントリが削除されること。

fn setup_entities(count: usize) -> (EntitiesSlot, Vec<Entity>) {
    let mut es = EntitiesSlot::default();
    let entities = (0..count).map(|_| es.insert(())).collect();
    (es, entities)
}

#[test]
fn test_reactive_create_signal() {
    let mut store = ReactiveStore::new();
    let (read_sig, write_sig) = ReactiveStore::create_signal(
        123_i32,
        &mut store.react_signals,
        &mut store.react_subscribers,
    );

    assert_eq!(read_sig.id(), write_sig.id());
    assert!(store.react_signals.contains_key(read_sig.id()));
    assert!(store.react_subscribers.contains_key(read_sig.id()));
}

#[test]
fn test_provide_and_use_provided_hierarchical() {
    let (_es, e) = setup_entities(3);
    let parent = e[0];
    let child = e[1];
    let grandchild = e[2];

    let mut parents = ParentsSecondary::default();
    parents.insert(parent, None);
    parents.insert(child, Some(parent));
    parents.insert(grandchild, Some(child));

    let mut store = ReactiveStore::new();
    let (read_sig, _write_sig) = ReactiveStore::create_signal(
        "provided_text".to_string(),
        &mut store.react_signals,
        &mut store.react_subscribers,
    );

    let topo_entities = EntitiesSlot::default();
    let dfs_seq = FlatDfsSequenceVec::default();

    // 親に provide
    ReactiveStore::provide(
        Some(parent),
        read_sig,
        &mut store.react_providers,
        &topo_entities,
        &parents,
        &dfs_seq,
    );

    // 孫および子から解決できること
    let resolved_read =
        ReactiveStore::use_provided_from::<String>(grandchild, &store.react_providers, &parents);
    assert_eq!(resolved_read.map(|r| r.id()), Some(read_sig.id()));

    let resolved_write =
        ReactiveStore::use_provided_setter_from::<String>(child, &store.react_providers, &parents);
    assert_eq!(resolved_write.map(|w| w.id()), Some(read_sig.id()));

    // 未登録の型は None
    assert!(
        ReactiveStore::use_provided_from::<i32>(grandchild, &store.react_providers, &parents)
            .is_none()
    );
}

#[test]
fn test_provide_shadowing() {
    let (_es, e) = setup_entities(3);
    let parent = e[0];
    let child = e[1];
    let grandchild = e[2];

    let mut parents = ParentsSecondary::default();
    parents.insert(parent, None);
    parents.insert(child, Some(parent));
    parents.insert(grandchild, Some(child));

    let mut store = ReactiveStore::new();
    let (sig_parent, _) = ReactiveStore::create_signal(
        10_i32,
        &mut store.react_signals,
        &mut store.react_subscribers,
    );
    let (sig_child, _) = ReactiveStore::create_signal(
        20_i32,
        &mut store.react_signals,
        &mut store.react_subscribers,
    );

    let topo_entities = EntitiesSlot::default();
    let dfs_seq = FlatDfsSequenceVec::default();

    ReactiveStore::provide(
        Some(parent),
        sig_parent,
        &mut store.react_providers,
        &topo_entities,
        &parents,
        &dfs_seq,
    );
    ReactiveStore::provide(
        Some(child),
        sig_child,
        &mut store.react_providers,
        &topo_entities,
        &parents,
        &dfs_seq,
    );

    // 孫から解決した際、近い祖先（子）のシグナルが優先される
    let resolved =
        ReactiveStore::use_provided_from::<i32>(grandchild, &store.react_providers, &parents);
    assert_eq!(resolved.map(|r| r.id()), Some(sig_child.id()));
}

#[test]
fn test_provide_to_root_fallback() {
    let (es, e) = setup_entities(2);
    let root = e[0];
    let child = e[1];

    let mut parents = ParentsSecondary::default();
    parents.insert(root, None);
    parents.insert(child, Some(root));

    let mut dfs_seq = FlatDfsSequenceVec::default();
    dfs_seq.extend([root, child]);

    let mut store = ReactiveStore::new();
    let (read_sig, _) = ReactiveStore::create_signal(
        999_i32,
        &mut store.react_signals,
        &mut store.react_subscribers,
    );

    // id: None で provide -> ルートが特定されて登録される
    ReactiveStore::provide(
        None,
        read_sig,
        &mut store.react_providers,
        &es,
        &parents,
        &dfs_seq,
    );

    let resolved = ReactiveStore::use_provided_from::<i32>(child, &store.react_providers, &parents);
    assert_eq!(resolved.map(|r| r.id()), Some(read_sig.id()));
}

#[test]
fn test_create_entity_effect_queuing() {
    let (_es, e) = setup_entities(1);
    let entity = e[0];

    let mut store = ReactiveStore::new();

    let effect_id = ReactiveStore::create_entity_effect(
        entity,
        EffectCategory::Style,
        &mut store.react_effects,
        &mut store.react_entity_effects,
        &mut store.react_effect_to_entity,
        &mut store.react_pending_entity_effects,
        |_| {},
    );

    // 即時実行されず、保留キューに積まれる
    assert_eq!(store.react_pending_entity_effects.as_slice(), &[effect_id]);
    assert!(store.react_effects.contains_key(effect_id));
    assert_eq!(store.react_effect_to_entity.get(effect_id), Some(&entity));
}

#[test]
fn test_register_entity_effect_replace_same_category() {
    let (_es, e) = setup_entities(1);
    let entity = e[0];

    let mut store = ReactiveStore::new();

    let eff1 = ReactiveStore::create_entity_effect(
        entity,
        EffectCategory::Style,
        &mut store.react_effects,
        &mut store.react_entity_effects,
        &mut store.react_effect_to_entity,
        &mut store.react_pending_entity_effects,
        |_| {},
    );

    // 同一カテゴリで2つ目を登録
    let eff2 = ReactiveStore::create_entity_effect(
        entity,
        EffectCategory::Style,
        &mut store.react_effects,
        &mut store.react_entity_effects,
        &mut store.react_effect_to_entity,
        &mut store.react_pending_entity_effects,
        |_| {},
    );

    // 古いエフェクト (eff1) が除去され、eff2 のみ残る
    assert!(!store.react_effects.contains_key(eff1));
    assert!(!store.react_effect_to_entity.contains_key(eff1));
    assert!(!store.react_pending_entity_effects.contains(&eff1));

    assert!(store.react_effects.contains_key(eff2));
    assert_eq!(store.react_effect_to_entity.get(eff2), Some(&entity));
    assert_eq!(store.react_pending_entity_effects.as_slice(), &[eff2]);

    // 別カテゴリで登録
    let eff3 = ReactiveStore::create_entity_effect(
        entity,
        EffectCategory::Text,
        &mut store.react_effects,
        &mut store.react_entity_effects,
        &mut store.react_effect_to_entity,
        &mut store.react_pending_entity_effects,
        |_| {},
    );

    // eff2, eff3 の両方が保持される
    assert!(store.react_effects.contains_key(eff2));
    assert!(store.react_effects.contains_key(eff3));
    let effects = store.react_entity_effects.at(entity);
    assert_eq!(effects.len(), 2);
}

#[test]
fn test_resolve_entity_effect_priority() {
    let (_es, e) = setup_entities(2);
    let entity_from_effect = e[0];
    let entity_from_active = e[1];

    let mut store = ReactiveStore::new();
    let eff_id = store.react_effects.insert(Effects(Box::new(|_| {})));
    store
        .react_effect_to_entity
        .insert(eff_id, entity_from_effect);

    // ACTIVE_EFFECT 優先
    crate::ACTIVE_EFFECT.with(|c| c.set(Some(eff_id)));
    let g = ActiveEntityGuard::new(entity_from_active);
    assert_eq!(
        ReactiveStore::resolve_entity_effect(&store.react_effect_to_entity),
        Some(entity_from_effect)
    );

    // ACTIVE_EFFECT なし -> ACTIVE_ENTITY にフォールバック
    crate::ACTIVE_EFFECT.with(|c| c.set(None));
    assert_eq!(
        ReactiveStore::resolve_entity_effect(&store.react_effect_to_entity),
        Some(entity_from_active)
    );
    drop(g);

    // 双方なし -> None
    assert_eq!(
        ReactiveStore::resolve_entity_effect(&store.react_effect_to_entity),
        None
    );
}

#[test]
fn test_evaluate_pending_entity_effects() {
    let mut cx = Context::default();
    let _guard = bind_context(&mut cx);

    let (_es, e) = setup_entities(1);
    let entity = e[0];

    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_clone = Arc::clone(&call_count);

    with_context(|c| {
        ReactiveStore::create_entity_effect(
            entity,
            EffectCategory::Style,
            &mut c.reactive.react_effects,
            &mut c.reactive.react_entity_effects,
            &mut c.reactive.react_effect_to_entity,
            &mut c.reactive.react_pending_entity_effects,
            move |_| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
            },
        );
    });

    // 評価前は 0 回
    assert_eq!(call_count.load(Ordering::SeqCst), 0);

    with_context(|c| {
        ReactiveStore::evaluate_pending_entity_effects(
            &mut c.reactive.react_pending_entity_effects,
            &c.reactive.react_effects,
        );
    });

    // 評価されて 1 回になり、キューが空になる
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
    with_context(|c| {
        assert!(c.reactive.react_pending_entity_effects.is_empty());
    });
}

#[test]
fn test_reactive_despawn() {
    let (_es, e) = setup_entities(1);
    let entity = e[0];

    let mut store = ReactiveStore::new();

    let eff1 = ReactiveStore::create_entity_effect(
        entity,
        EffectCategory::Style,
        &mut store.react_effects,
        &mut store.react_entity_effects,
        &mut store.react_effect_to_entity,
        &mut store.react_pending_entity_effects,
        |_| {},
    );

    let (read_sig, _) =
        ReactiveStore::create_signal(1, &mut store.react_signals, &mut store.react_subscribers);
    let topo_entities = EntitiesSlot::default();
    let parents = ParentsSecondary::default();
    let dfs_seq = FlatDfsSequenceVec::default();

    ReactiveStore::provide(
        Some(entity),
        read_sig,
        &mut store.react_providers,
        &topo_entities,
        &parents,
        &dfs_seq,
    );

    // despawn 実行
    store.despawn(entity);

    assert!(!store.react_effects.contains_key(eff1));
    assert!(!store.react_effect_to_entity.contains_key(eff1));
    assert!(!store.react_pending_entity_effects.contains(&eff1));
    assert!(!store.react_entity_effects.contains(entity));
    assert!(!store.react_providers.contains(entity));
}
