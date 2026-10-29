#![allow(clippy::unwrap_used, clippy::float_cmp)]

use super::*;
use slotmap::SlotMap;
use smallvec::SmallVec;
use std::time::Duration;
use taffy::{Style, TraversePartialTree};

struct TestFixture {
    entities: SlotMap<Entity, ()>,
    taffy_tree: TaffyTreeEntity,
    taffy_nodes: TaffyNodesSecondary,
    active_masks: ActiveMasksSecondary,
    parents: ParentsSecondary,
    children: ChildrenSecondary,
    dirty_entities: DirtyLayoutEntitiesVec,
    debug: DebugStore,
}

impl TestFixture {
    fn new() -> Self {
        Self {
            entities: SlotMap::with_key(),
            taffy_tree: TaffyTreeEntity::new(),
            taffy_nodes: TaffyNodesSecondary::default(),
            active_masks: ActiveMasksSecondary::default(),
            parents: ParentsSecondary::default(),
            children: ChildrenSecondary::default(),
            dirty_entities: DirtyLayoutEntitiesVec::default(),
            debug: DebugStore::new(),
        }
    }

    fn spawn_node(&mut self, parent: Option<Entity>) -> Entity {
        let entity = self.entities.insert(());
        let node = self
            .taffy_tree
            .new_leaf_with_context(Style::default(), entity)
            .unwrap();

        self.taffy_nodes.insert(entity, node);
        self.active_masks.insert(entity, ComponentMask::default());
        self.parents.insert(entity, parent);
        self.children.insert(entity, SmallVec::default());

        if let Some(p) = parent {
            self.children.at_mut(p).push(entity);
            let p_node = *self.taffy_nodes.at(p);
            self.taffy_tree.add_child(p_node, node).unwrap();
        }

        entity
    }
}

// `LayoutStore::resync_taffy_children_order` の検証
//
// `test_resync_taffy_children_order_reorder`
//  - 親ノードと複数の子ノード（A, B, C）を作成し、Taffy に初期登録。
//  - `ChildrenSecondary` 側の順序を (C, A, B) に変更して `resync_taffy_children_order` を実行。
//  - Taffy 側の `lay_taffy_tree.children(parent_node)` の順序が (C, A, B) に正確に同期されることを確認。
//
// test_resync_taffy_children_order_empty`
//  - 子ノードが存在する状態から、`ChildrenSecondary` を空リストに変更して実行。
//  - Taffy 側の子ノードがすべてデタッチされ、0件になることを確認。

#[test]
fn test_resync_taffy_children_order_reorder() {
    let mut f = TestFixture::new();
    let parent = f.spawn_node(None);
    let c1 = f.spawn_node(Some(parent));
    let c2 = f.spawn_node(Some(parent));
    let c3 = f.spawn_node(Some(parent));

    let p_node = *f.taffy_nodes.at(parent);
    let n1 = *f.taffy_nodes.at(c1);
    let n2 = *f.taffy_nodes.at(c2);
    let n3 = *f.taffy_nodes.at(c3);

    // 順序を (c3, c1, c2) に変更
    f.children.at_mut(parent).clear();
    f.children.at_mut(parent).extend([c3, c1, c2]);

    LayoutStore::resync_taffy_children_order(
        parent,
        &f.children,
        &mut f.taffy_tree,
        &f.taffy_nodes,
        &mut f.debug,
    );

    let taffy_children = f.taffy_tree.children(p_node).unwrap();
    assert_eq!(taffy_children, vec![n3, n1, n2]);
}

#[test]
fn test_resync_taffy_children_order_empty() {
    let mut f = TestFixture::new();
    let parent = f.spawn_node(None);
    let _c1 = f.spawn_node(Some(parent));

    let p_node = *f.taffy_nodes.at(parent);
    assert_eq!(f.taffy_tree.child_count(p_node), 1);

    // 子を空にする
    f.children.at_mut(parent).clear();

    LayoutStore::resync_taffy_children_order(
        parent,
        &f.children,
        &mut f.taffy_tree,
        &f.taffy_nodes,
        &mut f.debug,
    );

    assert_eq!(f.taffy_tree.child_count(p_node), 0);
}

// `LayoutStore::mark_layout_dirty` & `clear_layout_dirty` の検証
//
// `test_mark_layout_dirty_propagation_to_ancestors`
//  - 親 $\to$ 子 $\to$ 孫 のツリー構造を構築。
//  - 孫に対して `mark_layout_dirty` を呼び出す。
//    1. 孫の Taffy ノードが `dirty` 状態になっていること。
//    2. 孫、子、親のすべてに `ComponentMask::STATE_QUEUED_LAYOUT` がセットされること。
//    3. `lay_dirty_entities` に `[孫, 子, 親]` の順で追加されること。
//
// `test_mark_layout_dirty_early_break_on_already_dirty`
//  - 親にあらかじめ `STATE_QUEUED_LAYOUT` をセットしておく。
//  - 孫に対して `mark_layout_dirty` を呼び出す。
//  - 親に到達した時点で探索が打ち切られ、親より上の先祖や重複登録が発生しないことを確認。
//
// `test_clear_layout_dirty`
//  - `STATE_QUEUED_LAYOUT` が付与された複数エンティティをキューに投入。
//  - `clear_layout_dirty` を呼び出し後、全エンティティのマスクが解除され、`lay_dirty_entities` が空になることを確認。

#[test]
fn test_mark_layout_dirty_propagation_to_ancestors() {
    let mut f = TestFixture::new();
    let root = f.spawn_node(None);
    let child = f.spawn_node(Some(root));
    let grandchild = f.spawn_node(Some(child));

    LayoutStore::mark_layout_dirty(
        grandchild,
        &mut f.active_masks,
        &f.parents,
        &mut f.dirty_entities,
        &mut f.taffy_tree,
        &f.taffy_nodes,
        &mut f.debug,
    );

    // 孫の Taffy ノードが dirty 化されているか
    let gc_node = *f.taffy_nodes.at(grandchild);
    assert!(f.taffy_tree.dirty(gc_node).unwrap());

    // 孫 -> 子 -> 親 の順でキューに追加されたか
    assert_eq!(f.dirty_entities.as_slice(), &[grandchild, child, root]);

    // 全祖先にマスクが設定されたか
    assert!(
        f.active_masks
            .at(grandchild)
            .has(ComponentMask::STATE_QUEUED_LAYOUT)
    );
    assert!(
        f.active_masks
            .at(child)
            .has(ComponentMask::STATE_QUEUED_LAYOUT)
    );
    assert!(
        f.active_masks
            .at(root)
            .has(ComponentMask::STATE_QUEUED_LAYOUT)
    );
}

#[test]
fn test_mark_layout_dirty_early_break_on_already_dirty() {
    let mut f = TestFixture::new();
    let root = f.spawn_node(None);
    let child = f.spawn_node(Some(root));
    let grandchild = f.spawn_node(Some(child));

    // あらかじめ child にフラグをセット
    f.active_masks
        .at_mut(child)
        .set(ComponentMask::STATE_QUEUED_LAYOUT);

    LayoutStore::mark_layout_dirty(
        grandchild,
        &mut f.active_masks,
        &f.parents,
        &mut f.dirty_entities,
        &mut f.taffy_tree,
        &f.taffy_nodes,
        &mut f.debug,
    );

    // child で探索が停止するため、キューには grandchild のみ入り、root は追加されない
    assert_eq!(f.dirty_entities.as_slice(), &[grandchild]);
    assert!(
        !f.active_masks
            .at(root)
            .has(ComponentMask::STATE_QUEUED_LAYOUT)
    );
}

#[test]
fn test_clear_layout_dirty() {
    let mut f = TestFixture::new();
    let e1 = f.spawn_node(None);
    let e2 = f.spawn_node(None);

    f.active_masks
        .at_mut(e1)
        .set(ComponentMask::STATE_QUEUED_LAYOUT);
    f.active_masks
        .at_mut(e2)
        .set(ComponentMask::STATE_QUEUED_LAYOUT);
    f.dirty_entities.push(e1);
    f.dirty_entities.push(e2);

    LayoutStore::clear_layout_dirty(&mut f.active_masks, &mut f.dirty_entities);

    assert!(f.dirty_entities.is_empty());
    assert!(
        !f.active_masks
            .at(e1)
            .has(ComponentMask::STATE_QUEUED_LAYOUT)
    );
    assert!(
        !f.active_masks
            .at(e2)
            .has(ComponentMask::STATE_QUEUED_LAYOUT)
    );
}

// `test_flex_direction_and_text_align_lookup`
//  - `FlexLayoutsSecondary::flex_direction(id)`: 登録済み時はその値、未登録時は `Default::default()`（Row等）が返ること。
//  - `ResolvedFlexSecondary::text_algin(id)`: 登録済み時はその値、未登録時は `Default::default()` が返ること。
//
// `test_get_layout_mut_base_vs_interaction`
//  - `ModTarget::Base` 指定時：`lay_base_basic` / `lay_flex` 側の参照が返ること。
//  - `ModTarget::Hover` 等のインタラクション指定時：`rnd_interaction` 側に初期化・挿入され、その修飾スタイル内部の可変参照が返ること。
//
// `test_is_transition_currently_running`
//  - 未登録時：`(false, false)` が返ること。
//  - `PropertyList::Width` 実行中：`(true, false)`。
//  - `PropertyList::Height` 実行中：`(false, true)`。
//  - `PropertyList::Size` 実行中：`(true, true)`。
//
// `test_apply_interaction_styles_cascade`
//  - Hovered, Pressed 等のスタイルを用意し、`active_mask` の状態フラグに応じて優先順位順（カスケード）に `BasicLayout` や `FlexLayout` が上書きされること。
//
// `test_apply_interaction_styles_size_suppression_on_transition`
//  - `is_transitioning: (true, _)` の状態でスタイル適用。
//  - サイズ以外のプロパティは上書きされるが、`ComponentMask::STYLE_SIZE` が unset され、サイズ（幅/高さ）の上書きが抑制されること。
//
// `test_length_to_px_and_edge_insets`
//  - `Length::Px(10.0)` $\to$ `10.0px`。
//  - `Length::Percent(50.0)` $\to$ 幅・高さに対してそれぞれ 50% のピクセル値に解決されること。
//  - `get_physical_border_padding` で top/bottom が `rect.height`、left/right が `rect.width` に基づいて正確に算出されること。
//
// `test_resolve_taffy_style_and_scrollbar`
//  - `BasicLayout`, `FlexLayout`, `GridLayout` の各種設定値が `taffy::Style` に正確にマッピングされること。
//  - `ScrollbarMode::Layout` のスクロールバーが存在する場合、その幅が `scrollbar_width` に反映されること（Overlay や None 時は 0.0）。
//
//* `test_set_taffy_style_and_local_rect`
//  - `set_taffy_style` を実行後、TaffyTree ノードにスタイルが適用されること。
//  - レイアウト計算後の `local_rect_from_taffy` で `(x, y, width, height)` が `LayoutRect` として正確に取り出せること。未計算または不正ノード時は `LayoutRect::ZERO` が返ること。
//
// `test_layout_despawn`
//  - `Basic`, `Flex`, `Grid`, `Resolved`, `TaffyNode`, `Scrollbar` など、当該エンティティに紐づくすべてのレイアウト SoA スロットが一掃されること。

fn setup_entity() -> (SlotMap<Entity, ()>, Entity) {
    let mut sm = SlotMap::<Entity, ()>::with_key();
    let entity = sm.insert(());
    (sm, entity)
}

#[test]
fn test_flex_direction_and_text_align_lookup() {
    let (_sm, e) = setup_entity();

    // FlexLayoutsSecondary::flex_direction
    let mut flex_sec = FlexLayoutsSecondary::default();
    assert_eq!(flex_sec.flex_direction(e), FlexDirection::default());

    let fl = FlexLayout {
        flex_direction: FlexDirection::Column,
        ..Default::default()
    };
    flex_sec.insert(e, fl);
    assert_eq!(flex_sec.flex_direction(e), FlexDirection::Column);

    // ResolvedFlexSecondary::text_algin
    let mut res_flex = ResolvedFlexSecondary::default();
    assert_eq!(res_flex.text_algin(e), TextAlign::default());

    let fl2 = FlexLayout {
        text_align: TextAlign::Center,
        ..Default::default()
    };
    res_flex.insert(e, fl2);
    assert_eq!(res_flex.text_algin(e), TextAlign::Center);
}

#[test]
fn test_get_layout_mut_base_vs_interaction() {
    let (_sm, e) = setup_entity();
    let mut lay_base_basic = BaseBasicLayoutsSecondary::default();
    let mut lay_flex = FlexLayoutsSecondary::default();
    let mut rnd_interaction = InteractionPropertiesSecondary::default();

    lay_base_basic.insert(e, BasicLayout::default());
    lay_flex.insert(e, FlexLayout::default());

    // ModTarget::Base 指定時 -> BaseBasicLayoutsSecondary から取得
    let basic_base = LayoutStore::get_basic_layout_mut(
        e,
        ModTarget::Base,
        &mut lay_base_basic,
        &mut rnd_interaction,
    );
    basic_base.aspect_ratio = Some(1.5);
    assert_eq!(lay_base_basic.at(e).aspect_ratio, Some(1.5));

    // ModTarget::Hover 指定時 -> rnd_interaction に自動初期化されて取得
    let basic_hover = LayoutStore::get_basic_layout_mut(
        e,
        ModTarget::Hovered,
        &mut lay_base_basic,
        &mut rnd_interaction,
    );
    basic_hover.aspect_ratio = Some(2.0);
    assert!(rnd_interaction.contains_key(e));

    // FlexLayout: ModTarget::Base
    let flex_base =
        LayoutStore::get_flex_layout_mut(e, ModTarget::Base, &mut lay_flex, &mut rnd_interaction);
    flex_base.flex_grow = 1.0;
    assert_eq!(lay_flex.at(e).flex_grow, 1.0);
}

#[test]
fn test_is_transition_currently_running() {
    let (_sm, e) = setup_entity();
    let mut rnd_active_transitions = ActiveTransitionsSparse::default();

    // 未登録時
    assert_eq!(
        LayoutStore::is_transition_currently_running(e, &rnd_active_transitions),
        (false, false)
    );

    // Width
    let t_width = crate::ActiveTransition {
        property_list: PropertyList::Width,
        start_time: None,
        duration: Duration::default(),
        curve: crate::AnimationCurve::EaseInOutQuad,
        start_value: crate::TransitionValue::Width(0.0),
        end_value: crate::TransitionValue::Width(0.0),
    };
    rnd_active_transitions.insert(e, vec![t_width]);
    assert_eq!(
        LayoutStore::is_transition_currently_running(e, &rnd_active_transitions),
        (true, false)
    );

    // Height
    let t_height = crate::ActiveTransition {
        property_list: PropertyList::Height,
        start_time: None,
        duration: Duration::default(),
        curve: crate::AnimationCurve::EaseInOutQuad,
        start_value: crate::TransitionValue::Height(0.0),
        end_value: crate::TransitionValue::Height(0.0),
    };
    rnd_active_transitions.insert(e, vec![t_height]);
    assert_eq!(
        LayoutStore::is_transition_currently_running(e, &rnd_active_transitions),
        (false, true)
    );

    // Size (両方)
    let t_size = crate::ActiveTransition {
        property_list: PropertyList::Size,
        start_time: None,
        duration: Duration::default(),
        curve: crate::AnimationCurve::EaseInOutQuad,
        start_value: crate::TransitionValue::Height(0.0),
        end_value: crate::TransitionValue::Height(0.0),
    };
    rnd_active_transitions.insert(e, vec![t_size]);
    assert_eq!(
        LayoutStore::is_transition_currently_running(e, &rnd_active_transitions),
        (true, true)
    );
}

#[test]
fn test_length_to_px_and_edge_insets() {
    let rect = LayoutRect::new(0.0, 0.0, 200.0, 100.0);

    let border = Rect {
        top: Length::Px(10.0),
        right: Length::Percent(20.0),  // 200 * 0.2 = 40.0
        bottom: Length::Percent(50.0), // 100 * 0.5 = 50.0
        left: Length::Px(5.0),
    };

    let insets = LayoutStore::get_physical_border(rect, border);
    assert_eq!(insets.top, 10.0);
    assert_eq!(insets.right, 40.0);
    assert_eq!(insets.bottom, 50.0);
    assert_eq!(insets.left, 5.0);

    let padding = Rect {
        top: Length::Percent(10.0), // 100 * 0.1 = 10.0
        right: Length::Px(15.0),
        bottom: Length::Px(25.0),
        left: Length::Percent(10.0), // 200 * 0.1 = 20.0
    };

    let (b_res, p_res) = LayoutStore::get_physical_border_padding(rect, border, padding);
    assert_eq!(b_res.top, 10.0);
    assert_eq!(p_res.left, 20.0);
}

#[test]
fn test_resolve_taffy_style_and_scrollbar() {
    let (_sm, e) = setup_entity();
    let basic = BasicLayout {
        aspect_ratio: Some(16.0 / 9.0),
        ..Default::default()
    };

    let flex = FlexLayout::default();
    let mut bar_styles = ScrollbarStylesSparse::default();

    // スクロールバーなし
    let style1 = LayoutStore::resolve_taffy_style(e, &basic, &flex, &bar_styles);
    assert_eq!(style1.aspect_ratio, Some(16.0 / 9.0));
    assert_eq!(style1.scrollbar_width, 0.0);

    // ode::Layout のスクロールバー設定時
    let s_style = crate::ScrollbarStyle {
        mode: ScrollbarMode::Layout,
        display: ScrollbarDisplay::Auto,
        width: 14.0,
        ..Default::default()
    };
    let s_state = ScrollBarState {
        style: s_style,
        ..Default::default()
    };
    bar_styles.insert(e, s_state);

    let style2 = LayoutStore::resolve_taffy_style(e, &basic, &flex, &bar_styles);
    assert_eq!(style2.scrollbar_width, 14.0);
}

#[test]
fn test_set_taffy_style_and_local_rect() {
    let (_sm, e) = setup_entity();
    let mut debug = DebugStore::new();

    let mut tree = TaffyTreeEntity::new();
    let mut taffy_nodes = TaffyNodesSecondary::default();
    let bar_styles = ScrollbarStylesSparse::default();

    let node = tree.new_leaf(taffy::Style::default()).unwrap();
    taffy_nodes.insert(e, node);

    let basic = BasicLayout {
        aspect_ratio: Some(2.0),
        ..Default::default()
    };
    let flex = FlexLayout::default();

    // set_taffy_style の呼び出し
    LayoutStore::set_taffy_style(
        e,
        &basic,
        &flex,
        &mut tree,
        &taffy_nodes,
        &bar_styles,
        &mut debug,
    );
    assert_eq!(tree.style(node).unwrap().aspect_ratio, Some(2.0));

    // レイアウト未計算時は ZERO が返る
    let rect_before = LayoutStore::local_rect_from_taffy(e, &tree, &taffy_nodes);
    assert_eq!(rect_before, LayoutRect::ZERO);

    // レイアウト計算後の座標・サイズが LayoutRect として取得できること
    tree.set_style(
        node,
        taffy::Style {
            size: taffy::Size {
                width: taffy::Dimension::length(300.0),
                height: taffy::Dimension::length(150.0),
            },
            ..Default::default()
        },
    )
    .unwrap();

    tree.compute_layout(
        node,
        taffy::Size {
            width: taffy::AvailableSpace::Definite(300.0),
            height: taffy::AvailableSpace::Definite(150.0),
        },
    )
    .unwrap();

    let rect_after = LayoutStore::local_rect_from_taffy(e, &tree, &taffy_nodes);
    assert_eq!(rect_after.width, 300.0);
    assert_eq!(rect_after.height, 150.0);
}

#[test]
fn test_layout_despawn() {
    let (_sm, e) = setup_entity();
    let mut store = LayoutStore::new();

    store.lay_basic.insert(e, BasicLayout::default());
    store.lay_flex.insert(e, FlexLayout::default());
    store.lay_base_basic.insert(e, BasicLayout::default());
    store.lay_base_flex.insert(e, FlexLayout::default());
    store.lay_resolved_basic.insert(e, BasicLayout::default());
    store.lay_resolved_flex.insert(e, FlexLayout::default());
    store.lay_dirty_entities.push(e);

    store.despawn(e);

    assert!(!store.lay_basic.contains(e));
    assert!(!store.lay_flex.contains(e));
    assert!(!store.lay_base_basic.contains(e));
    assert!(!store.lay_base_flex.contains(e));
    assert!(!store.lay_resolved_basic.contains(e));
    assert!(!store.lay_resolved_flex.contains(e));
    assert!(!store.lay_dirty_entities.contains(&e));
}
