use super::*;
use slotmap::SlotMap;

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
