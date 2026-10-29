use super::*;
use slotmap::SlotMap;

// `RenderStore::mark_render_dirty` & `clear_render_dirty` の検証
//
// `test_mark_render_dirty_single_entity`
//  - 対象エンティティに `mark_render_dirty` を呼び出す。
//  - `ComponentMask::STATE_QUEUED_RENDER` がセットされ、`rnd_dirty_entities` に追加されること。
//  - （レイアウトと異なり）親ノードには影響を与えないことを確認。
//
// `test_mark_render_dirty_deduplication`
//  - 同一エンティティに対して複数回 `mark_render_dirty` を呼び出す。
//  - 二重登録されず、キューの要素数が 1 件のままであることを確認。
//
// `test_clear_render_dirty`
//  - 登録済みエンティティのマスクが正常に解除され、キューがクリアされることを確認。
//
// `test_clear_render_dirty_with_despawned_entity`**
//  - キューの中に、既に `topo_active_masks` から削除された（despawn済みの）`Entity` が混在している状態で `clear_render_dirty` を実行。
//  - パニックを起こさず正常にスキップされ、キューが空になることを確認。

#[test]
fn test_mark_render_dirty_single_entity() {
    let mut sm = SlotMap::<Entity, ()>::with_key();
    let entity = sm.insert(());

    let mut active_masks = ActiveMasksSecondary::default();
    let mut dirty_entities = DirtyRenderEntitiesVec::default();
    active_masks.insert(entity, ComponentMask::default());

    RenderStore::mark_render_dirty(entity, &mut active_masks, &mut dirty_entities);

    assert!(
        active_masks
            .at(entity)
            .has(ComponentMask::STATE_QUEUED_RENDER)
    );
    assert_eq!(dirty_entities.as_slice(), &[entity]);
}

#[test]
fn test_mark_render_dirty_deduplication() {
    let mut sm = SlotMap::<Entity, ()>::with_key();
    let entity = sm.insert(());

    let mut active_masks = ActiveMasksSecondary::default();
    let mut dirty_entities = DirtyRenderEntitiesVec::default();
    active_masks.insert(entity, ComponentMask::default());

    RenderStore::mark_render_dirty(entity, &mut active_masks, &mut dirty_entities);
    RenderStore::mark_render_dirty(entity, &mut active_masks, &mut dirty_entities);

    assert_eq!(dirty_entities.len(), 1);
}

#[test]
fn test_clear_render_dirty() {
    let mut sm = SlotMap::<Entity, ()>::with_key();
    let e1 = sm.insert(());
    let e2 = sm.insert(());

    let mut active_masks = ActiveMasksSecondary::default();
    let mut dirty_entities = DirtyRenderEntitiesVec::default();

    let mut mask = ComponentMask::default();
    mask.set(ComponentMask::STATE_QUEUED_RENDER);
    active_masks.insert(e1, mask);
    active_masks.insert(e2, mask);

    dirty_entities.push(e1);
    dirty_entities.push(e2);

    RenderStore::clear_render_dirty(&mut active_masks, &mut dirty_entities);

    assert!(dirty_entities.is_empty());
    assert!(!active_masks.at(e1).has(ComponentMask::STATE_QUEUED_RENDER));
    assert!(!active_masks.at(e2).has(ComponentMask::STATE_QUEUED_RENDER));
}

#[test]
fn test_clear_render_dirty_with_despawned_entity() {
    let mut sm = SlotMap::<Entity, ()>::with_key();
    let live = sm.insert(());
    let dead = sm.insert(());

    let mut active_masks = ActiveMasksSecondary::default();
    let mut dirty_entities = DirtyRenderEntitiesVec::default();

    let mut mask = ComponentMask::default();
    mask.set(ComponentMask::STATE_QUEUED_RENDER);
    active_masks.insert(live, mask);
    // dead は active_masks に登録しない（despawn済みの状態を再現）

    dirty_entities.push(live);
    dirty_entities.push(dead);

    // despawn 済みIDが含まれていてもパニックせず完了すること
    RenderStore::clear_render_dirty(&mut active_masks, &mut dirty_entities);

    assert!(dirty_entities.is_empty());
    assert!(
        !active_masks
            .at(live)
            .has(ComponentMask::STATE_QUEUED_RENDER)
    );
}
