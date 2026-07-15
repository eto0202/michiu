use super::*;
use crate::{
    AlignContent, AlignItems, AlignSelf, BasicLayout, BoxShadow, BoxSizing, Color, CornerRadius,
    CursorIcon, Direction, Display, Element, ElementState, EventListeners, FlexDirection,
    FlexLayout, FlexWrap, GridAutoFlow, GridLayout, GridLine, GridPlacement, ImageSource, ImeState,
    InteractionStates, InteractionStyles, JustifyContent, LayoutOverflow, LayoutPoint, LayoutRect,
    LayoutSize, Length, Modifiers, MouseButton, MovieProperty, MovieSource, Overflow, Point,
    Position, Rect, Size, StyleInner, TextAlign, ThisStyle, UiaValue, Val, VirtualKey,
    VisualProperty, auto, bitmap::*, build_ui, create_effect, create_signal, div, div_n, px,
    with_context,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Instant,
};
use taffy::{LengthPercentageAuto, prelude::TaffyMaxContent};

// 1. 生存ライフサイクルと再帰デスポーンの厳格テスト
#[test]
fn test_spawn_and_recursive_despawn() {
    let mut cx = Context::new();

    // 階層を構築: Root -> Child -> Grandchild
    let root = cx.spawn(None);
    let child = cx.spawn(Some(root));
    let grandchild = cx.spawn(Some(child));

    // トポロジー情報のセットアップ
    cx.add_child(root, child);
    cx.add_child(child, grandchild);

    // 各SoAデータにテスト用の実体をセット
    cx.basic_layouts.insert(root, BasicLayout::default());
    cx.basic_layouts.insert(child, BasicLayout::default());
    cx.basic_layouts.insert(grandchild, BasicLayout::default());

    cx.text_contents
        .insert(grandchild, Cow::Borrowed("grandchild text"));

    // 存在を確認
    assert!(cx.entities.contains_key(root));
    assert!(cx.entities.contains_key(child));
    assert!(cx.entities.contains_key(grandchild));

    // ルート要素を破棄（デスポーン）
    cx.despawn_internal(root);

    // 親子のつながりが再帰的にすべて消去されているかを検証
    assert!(!cx.entities.contains_key(root));
    assert!(!cx.entities.contains_key(child));
    assert!(!cx.entities.contains_key(grandchild));

    // SoA側も連動してメモリが完全に解放されているかを検証
    assert!(cx.basic_layouts.get(root).is_none());
    assert!(cx.basic_layouts.get(child).is_none());
    assert!(cx.basic_layouts.get(grandchild).is_none());
    assert!(cx.text_contents.get(grandchild).is_none());
}

// 2. ガベージコレクションと重複登録のないDirtyキュー制御
#[test]
fn test_garbage_collection_and_dirty_queues() {
    let mut cx = Context::new();

    let id = cx.spawn(None);

    // 同一要素に対して複数回レイアウトDirtyフラグを立てる
    cx.mark_layout_dirty(id);
    cx.mark_layout_dirty(id);
    cx.mark_layout_dirty(id);

    // 重複排除フラグ（STATE_QUEUED_LAYOUT）が有効なため、キュー内の要素は1つだけであるべき
    assert_eq!(cx.dirty_layout_entities.len(), 1);
    assert_eq!(cx.dirty_layout_entities[0], id);

    // レンダーDirtyについても同様の重複排除を検証
    cx.mark_render_dirty(id);
    cx.mark_render_dirty(id);
    assert_eq!(cx.dirty_render_entities.len(), 1);

    // 要素を破棄し、GCを実行
    cx.despawn_internal(id);
    cx.gc_inactive_entities();

    // 破棄された要素が各走査リストから瞬時に、かつ確実に排除されているかを検証
    assert!(cx.active_entities.is_empty());
    assert!(cx.dirty_layout_entities.is_empty());
    assert!(cx.dirty_render_entities.is_empty());
}

// 3. DFSレイアウト解決、クリップ領域交差、スクロールオフセット減算の検証
#[test]
fn test_dfs_layout_resolution_clip_and_scroll() {
    let mut cx = Context::new();

    // 階層構築: Container (Root) -> Child
    let root = cx.spawn(None);
    let child = cx.spawn(Some(root));
    cx.add_child(root, child);

    // Container: size(200x200) を SoA へ挿入
    cx.basic_layouts.insert(
        root,
        BasicLayout {
            size: Size::new(Val::Px(200.0), Val::Px(200.0)),
            ..BasicLayout::default()
        },
    );
    cx.mark_layout_dirty(root);

    // Child: size(100x100), location(50, 50) を SoA へ挿入
    cx.basic_layouts.insert(
        child,
        BasicLayout {
            position: Position::Absolute,
            inset: Rect::new(Val::Px(50.0), Val::Auto, Val::Auto, Val::Px(50.0)),
            size: Size::new(Val::Px(100.0), Val::Px(100.0)),
            ..BasicLayout::default()
        },
    );
    cx.mark_layout_dirty(child);

    // 3.1 正常時の絶対座標算出とクリップの伝播を検証 (Taffy計算は自動実行される)
    let window_size = LayoutSize::new(800.0, 600.0);
    cx.sync_layout_and_render_list(root, window_size);

    // Containerの絶対座標は (0.0, 0.0)
    let root_rect = cx.rects[root];
    assert_eq!(root_rect, LayoutRect::new(0.0, 0.0, 200.0, 200.0));

    // Childの絶対座標は Container の絶対座標から相対座標を加算した値になる
    let child_rect = cx.rects[child];
    assert_eq!(child_rect, LayoutRect::new(50.0, 50.0, 100.0, 100.0));

    // 3.2 境界クリップ (STYLE_OVERFLOWオン) の検証
    cx.active_masks.get_mut(root).unwrap().set(STYLE_OVERFLOW);
    cx.mark_layout_dirty(root);

    // 再同期
    cx.sync_layout_and_render_list(root, window_size);
    let child_clip = cx.clip_rects[child];
    assert_eq!(child_clip, LayoutRect::new(0.0, 0.0, 200.0, 200.0));

    // 3.3 スクロールオフセットの減算が子にのみ作用するか検証
    cx.scroll_offsets.insert(root, LayoutPoint::new(10.0, 20.0));
    cx.mark_layout_dirty(root);
    cx.sync_layout_and_render_list(root, window_size);

    assert_eq!(cx.rects[root], LayoutRect::new(0.0, 0.0, 200.0, 200.0));
    assert_eq!(cx.rects[child], LayoutRect::new(40.0, 30.0, 100.0, 100.0));
}

// 4. クリップ枠を考慮した前面優先ヒットテスト (Painter's Algorithm)
#[test]
fn test_hit_testing_with_clipping() {
    let mut cx = Context::new();

    // 階層: Root (Container) -> Child
    let root = cx.spawn(None);
    let child = cx.spawn(Some(root));
    cx.add_child(root, child);

    // Taffyレイアウトを設定
    cx.basic_layouts.insert(
        root,
        BasicLayout {
            size: Size::new(Val::Px(200.0), Val::Px(200.0)),
            ..BasicLayout::default()
        },
    );
    cx.mark_layout_dirty(root);

    cx.basic_layouts.insert(
        child,
        BasicLayout {
            position: Position::Absolute,
            inset: Rect::new(Val::Auto, Val::Px(50.0), Val::Auto, Val::Px(50.0)),
            size: Size::new(Val::Px(100.0), Val::Px(100.0)),
            ..BasicLayout::default()
        },
    );
    cx.mark_layout_dirty(child);

    let window_size = LayoutSize::new(800.0, 600.0);
    cx.sync_layout_and_render_list(root, window_size);

    // 4.1 重なり部分のテスト
    let hit1 = cx.hit_test(LayoutPoint::new(100.0, 100.0));
    assert_eq!(hit1, Some(child));

    // 4.2 重なっていない親単体の部分のテスト
    let hit2 = cx.hit_test(LayoutPoint::new(10.0, 10.0));
    assert_eq!(hit2, Some(root));

    // 4.3 クリップ枠の外に子がはみ出している場合
    cx.rects
        .insert(child, LayoutRect::new(250.0, 250.0, 50.0, 50.0));
    cx.clip_rects
        .insert(child, LayoutRect::new(0.0, 0.0, 200.0, 200.0));

    let hit3 = cx.hit_test(LayoutPoint::new(275.0, 275.0));
    assert_eq!(hit3, None);
}

// 5. オーバーライド優先度（カスケード階層）の厳密な検証
#[test]
fn test_style_cascade_overrides() {
    let mut cx = Context::new();
    let id = cx.spawn(None);

    let mut base_layout = BasicLayout {
        size: Size::new(Val::Px(10.0), Val::Px(10.0)),
        ..Default::default()
    };
    cx.basic_layouts.insert(id, base_layout);

    // 2つの干渉スタイル（Hovered と Disabled）を用意
    let mut hovered_style = ThisStyle::new();
    hovered_style.inner = Arc::new(StyleInner {
        mask: ComponentMask::new(STYLE_SIZE),
        basic_layout: BasicLayout {
            size: Size::new(Val::Px(50.0), Val::Px(50.0)),
            ..Default::default()
        },
        ..Default::default()
    });

    let mut disabled_style = ThisStyle::new();
    disabled_style.inner = Arc::new(StyleInner {
        mask: ComponentMask::new(STYLE_SIZE),
        basic_layout: BasicLayout {
            size: Size::new(Val::Px(100.0), Val::Px(100.0)),
            ..Default::default()
        },
        ..Default::default()
    });

    cx.interaction_properties.insert(
        id,
        InteractionStyles {
            hovered: Some(hovered_style),
            disabled: Some(disabled_style),
            ..Default::default()
        },
    );

    // 5.1 ホバーのみが有効な場合
    cx.active_masks.get_mut(id).unwrap().set(STATE_HOVERED);
    let (resolved_hover, _, _) = cx.resolve_active_layouts(id);
    assert_eq!(resolved_hover.size.width, Val::Px(50.0));

    // 5.2 ホバーと無効化（Disabled）が同時に有効な場合
    cx.active_masks.get_mut(id).unwrap().set(STATE_DISABLED);
    let (resolved_both, _, _) = cx.resolve_active_layouts(id);
    assert_eq!(resolved_both.size.width, Val::Px(100.0));
}

// 6. マウス移動、ホバー状態遷移（Enter/Leave）、相対座標イベント伝播、およびドラッグ検知のテスト
#[test]
fn test_pointer_and_focus_event_injection() {
    let mut cx = Context::new();

    let super_root = cx.spawn(None);
    let id = cx.spawn(Some(super_root));
    cx.add_child(super_root, id);

    cx.basic_layouts.insert(
        super_root,
        BasicLayout {
            size: Size::new(Val::Px(800.0), Val::Px(600.0)),
            ..BasicLayout::default()
        },
    );
    cx.mark_layout_dirty(super_root);

    // Taffyレイアウトシステムとして座標を (50.0, 50.0, 100.0, 100.0) に配置
    cx.basic_layouts.insert(
        id,
        BasicLayout {
            position: Position::Absolute,
            inset: Rect::new(Val::Px(50.0), Val::Auto, Val::Auto, Val::Px(50.0)),
            size: Size::new(Val::Px(100.0), Val::Px(100.0)),
            ..BasicLayout::default()
        },
    );
    cx.mark_layout_dirty(id);

    let window_size = LayoutSize::new(800.0, 600.0);
    cx.sync_layout_and_render_list(super_root, window_size);

    // コールバック呼び出し回数のカウンター
    let enter_count = Arc::new(AtomicU32::new(0));
    let leave_count = Arc::new(AtomicU32::new(0));
    let move_count = Arc::new(AtomicU32::new(0));
    let drag_count = Arc::new(AtomicU32::new(0));

    let enter_clone = enter_count.clone();
    let leave_clone = leave_count.clone();
    let move_clone = move_count.clone();
    let drag_clone = drag_count.clone();

    let mut listeners = EventListeners {
        on_mouse_enter: Some(Box::new(move |_| {
            enter_clone.fetch_add(1, Ordering::SeqCst);
        })),
        ..Default::default()
    };
    listeners.on_mouse_leave = Some(Box::new(move |_| {
        leave_clone.fetch_add(1, Ordering::SeqCst);
    }));
    listeners.on_cursor_moved = Some(Box::new(move |_, relative_pos| {
        let count = move_clone.fetch_add(1, Ordering::SeqCst);
        if count == 0 {
            // 1回目の移動 (80.0, 90.0) -> 相対 (30.0, 40.0)
            assert_eq!(relative_pos, LayoutPoint::new(30.0, 40.0));
        } else if count == 1 {
            // 2回目のドラッグ移動 (90.0, 95.0) -> 相対 (40.0, 45.0)
            assert_eq!(relative_pos, LayoutPoint::new(40.0, 45.0));
        }
    }));
    listeners.on_drag = Some(Box::new(move |_, delta| {
        drag_clone.fetch_add(1, Ordering::SeqCst);
        // 差分移動量
        assert_eq!(delta, LayoutPoint::new(10.0, 5.0));
    }));

    cx.event_listeners.insert(id, listeners);

    // 6.1 要素外への移動（何も起こらない）
    cx.inject_pointer_move(LayoutPoint::new(10.0, 10.0));
    assert_eq!(enter_count.load(Ordering::SeqCst), 0);

    // 6.2 要素内への進入 (Enter & Move 発火)
    cx.inject_pointer_move(LayoutPoint::new(80.0, 90.0));
    assert_eq!(enter_count.load(Ordering::SeqCst), 1);
    assert_eq!(move_count.load(Ordering::SeqCst), 1);
    assert!(cx.active_masks[id].has(STATE_HOVERED));

    // 6.3 プレス状態でドラッグ移動 (Move & Drag 発火)
    cx.interaction_states.pressed = Some(id);
    cx.inject_pointer_move(LayoutPoint::new(90.0, 95.0)); // delta: (10, 5)
    assert_eq!(drag_count.load(Ordering::SeqCst), 1);
    assert_eq!(move_count.load(Ordering::SeqCst), 2); // 2回目のカーソル移動が安全に実行される
    assert!(cx.active_masks[id].has(STATE_DRAGGED));

    // 6.4 要素外への離脱 (Leave 発火)
    cx.interaction_states.pressed = None;
    cx.inject_pointer_move(LayoutPoint::new(200.0, 200.0));
    assert_eq!(leave_count.load(Ordering::SeqCst), 1);
    assert!(!cx.active_masks[id].has(STATE_HOVERED));
}

// 7. プレス、フォーカス切り替え、クリック・右クリックイベント解決の厳密テスト
#[test]
fn test_pointer_button_click_and_right_click() {
    let mut cx = Context::new();

    let super_root = cx.spawn(None);
    let root = cx.spawn(Some(super_root));
    let sibling = cx.spawn(Some(super_root));
    cx.add_child(super_root, root);
    cx.add_child(super_root, sibling);

    // Taffyレイアウト上で兄弟要素を配置
    cx.basic_layouts.insert(
        super_root,
        BasicLayout {
            size: Size::new(Val::Px(800.0), Val::Px(600.0)),
            ..BasicLayout::default()
        },
    );
    cx.mark_layout_dirty(super_root);

    cx.basic_layouts.insert(
        root,
        BasicLayout {
            position: Position::Absolute,
            inset: Rect::new(Val::Auto, Val::Px(0.0), Val::Auto, Val::Px(0.0)),
            size: Size::new(Val::Px(100.0), Val::Px(100.0)),
            ..BasicLayout::default()
        },
    );
    cx.mark_layout_dirty(root);

    cx.basic_layouts.insert(
        sibling,
        BasicLayout {
            position: Position::Absolute,
            inset: Rect::new(Val::Auto, Val::Px(0.0), Val::Auto, Val::Px(100.0)),
            size: Size::new(Val::Px(100.0), Val::Px(100.0)),
            ..BasicLayout::default()
        },
    );
    cx.mark_layout_dirty(sibling);

    let window_size = LayoutSize::new(800.0, 600.0);
    cx.sync_layout_and_render_list(super_root, window_size);

    let blur_fired = Arc::new(AtomicU32::new(0));
    let focus_fired = Arc::new(AtomicU32::new(0));
    let click_fired = Arc::new(AtomicU32::new(0));
    let r_click_fired = Arc::new(AtomicU32::new(0));

    let blur_clone = blur_fired.clone();
    let focus_clone = focus_fired.clone();
    let click_clone = click_fired.clone();
    let r_click_clone = r_click_fired.clone();

    // root にフォーカス、クリックイベント、右クリックイベントを紐づける
    let mut root_listeners = EventListeners {
        on_blur: Some(Box::new(move |_| {
            blur_clone.fetch_add(1, Ordering::SeqCst);
        })),
        ..Default::default()
    };
    root_listeners.on_click = Some(Box::new(move |_| {
        click_clone.fetch_add(1, Ordering::SeqCst);
    }));
    root_listeners.on_right_click = Some(Box::new(move |_| {
        r_click_clone.fetch_add(1, Ordering::SeqCst);
    }));
    cx.event_listeners.insert(root, root_listeners);

    // sibling にフォーカスイベントを紐づける
    let mut sibling_listeners = EventListeners {
        on_focus: Some(Box::new(move |_| {
            focus_clone.fetch_add(1, Ordering::SeqCst);
        })),
        ..Default::default()
    };
    cx.event_listeners.insert(sibling, sibling_listeners);

    // 7.1 初期フォーカスを root にセット
    cx.interaction_states.focused = Some(root);
    cx.active_masks.get_mut(root).unwrap().set(STATE_FOCUSED);

    // 7.2 sibling上でのマウスプレス -> フォーカスの切り替え
    cx.interaction_states.hovered = Some(sibling);
    cx.inject_pointer_button(
        MouseButton::Left,
        ElementState::Pressed,
        Modifiers::default(),
    );

    // root から sibling にフォーカスが移り、blur と focus が1回ずつ発火するべき
    assert_eq!(blur_fired.load(Ordering::SeqCst), 1);
    assert_eq!(focus_fired.load(Ordering::SeqCst), 1);
    assert!(!cx.active_masks[root].has(STATE_FOCUSED));
    assert!(cx.active_masks[sibling].has(STATE_FOCUSED));

    // 7.3 root上で左クリックダウン -> アップによるクリック解決の検証
    cx.interaction_states.hovered = Some(root);
    // ダウン
    cx.inject_pointer_button(
        MouseButton::Left,
        ElementState::Pressed,
        Modifiers::default(),
    );
    assert!(cx.active_masks[root].has(STATE_PRESSED));
    assert_eq!(cx.interaction_states.pressed, Some(root));

    // アップ
    cx.inject_pointer_button(
        MouseButton::Left,
        ElementState::Released,
        Modifiers::default(),
    );
    assert!(!cx.active_masks[root].has(STATE_PRESSED));
    assert_eq!(click_fired.load(Ordering::SeqCst), 1); // 同一要素上でのリリースのため、クリック成立

    // 7.4 右クリックによる解決の検証
    cx.interaction_states.hovered = Some(root);
    cx.inject_pointer_button(
        MouseButton::Right,
        ElementState::Pressed,
        Modifiers::default(),
    );
    cx.inject_pointer_button(
        MouseButton::Right,
        ElementState::Released,
        Modifiers::default(),
    );
    assert_eq!(r_click_fired.load(Ordering::SeqCst), 1); // 右クリック成立
}

// 8. シグナルと text のプッシュ駆動型リアクティブ自動Dirty伝播テスト
#[test]
fn test_reactive_signal_updates() {
    let mut cx = Context::new();
    let mut set_count_handle = None;

    // 1. 構築フェーズ
    let root = build_ui(&mut cx, || {
        let (count, set_count) = create_signal(0);
        set_count_handle = Some(set_count);

        div_n().text(move || format!("Count: {}", count.get()))
    });

    let set_count = set_count_handle.unwrap();

    // 2. 座標同期を実行して active_entities を埋める
    cx.sync_layout_and_render_list(root.id, LayoutSize::new(800.0, 600.0));

    let active_id = cx.active_entities[0];
    assert_eq!(cx.text_contents[active_id], "Count: 0");

    // 3. 更新フェーズ（コンテキストをバインド）
    {
        let _guard = bind_context(&cx);
        set_count.set(1);
    }

    // 依存エフェクトが即座に SoA を更新しているか検証
    assert_eq!(cx.text_contents[active_id], "Count: 1");
    assert!(cx.active_masks[active_id].has(STATE_QUEUED_LAYOUT));
    assert!(cx.active_masks[active_id].has(STATE_QUEUED_RENDER));
}

// 巨大UI構造の負荷・速度検証テスト
// cargo test test_massive_ui_performance -- --nocapture
#[test]
fn test_massive_ui_performance() {
    let mut cx = Context::new();
    let window_size = LayoutSize::new(1920.0, 1080.0);

    println!("\n=== 巨大UI構造の負荷・速度検証テスト（総要素数：約4.6万個） ===");

    let start_build = Instant::now();
    let super_root_handle = build_ui(&mut cx, || {
        let mut root = div_n().style(ThisStyle::new().display(Display::Flex));
        for i in 1..=100 {
            let mut parent = div_n();
            let child_count = i;
            let target_sum = if i == 1 { 100 } else { 100 - i };
            let grandchild_count = if target_sum >= child_count {
                target_sum - child_count
            } else {
                0
            };

            for _ in 0..child_count {
                let mut child = div_n();
                for _ in 0..grandchild_count {
                    child = child.child(div_n());
                }
                parent = parent.child(child);
            }
            root = root.child(parent);
        }
        root
    });
    println!(
        "1. UIツリー構築時間 (46,802要素): {:.2?}",
        start_build.elapsed()
    );

    // Sync
    let start_sync_1 = Instant::now();
    cx.sync_layout_and_render_list(super_root_handle.id, window_size);
    println!("2. 【初回】同期時間: {:.2?}", start_sync_1.elapsed());

    let total_elements = cx.active_entities.len();
    println!("   -> 同期済みアクティブ要素数: {} 個", total_elements);
    assert_eq!(total_elements, 46802);

    // Hit Test
    let start_hit = Instant::now();
    let test_point = LayoutPoint::new(-100.0, -100.0);
    for _ in 0..100 {
        let _ = cx.hit_test(test_point);
    }
    println!("4. 当たり判定（平均）: {:.2?}", start_hit.elapsed() / 100);

    // Reactive Bulk Update
    let mut set_sig_handle = None;
    build_ui(&mut cx, || {
        let (sig, set_sig) = create_signal(String::from("A"));
        set_sig_handle = Some(set_sig);
        for _ in 0..500 {
            super_root_handle.child(div_n().text(move || sig.get().clone()));
        }
        div_n()
    });

    let start_reactive = Instant::now();
    {
        let _guard = bind_context(&cx);
        set_sig_handle.unwrap().set(String::from("B"));
    }
    println!(
        "5. リアクティブ一括解決 (500個): {:.2?}",
        start_reactive.elapsed()
    );

    let start_despawn = Instant::now();
    cx.despawn(super_root_handle);
    cx.gc_inactive_entities();
    println!("6. 全要素デスポーン時間: {:.2?}", start_despawn.elapsed());
}

// 極端に深いネスト構造の負荷・速度検証テスト（250階層のチェーン）
// cargo test test_extreme_deep_nesting_performance -- --nocapture
#[test]
fn test_extreme_deep_nesting_performance() {
    let mut cx = Context::new();

    println!("\n=== A. 極端に深いネスト構造の検証（深さ: 250階層） ===");

    // ルートから順番に200回 child を入れ子にしてチェーンを構築
    let start_build = Instant::now();
    let super_root_handle = build_ui(&mut cx, || {
        let mut current = div_n().style(ThisStyle::new().display(Display::Flex));
        for _ in 0..250 {
            current = div_n()
                .style(ThisStyle::new().display(Display::Flex))
                .child(current);
        }
        current
    });
    let duration_build = start_build.elapsed();
    println!("1. 深いツリー構築時間 (251要素): {:.2?}", duration_build);

    // 座標同期（Taffy計算を自動内包）
    let start_sync = Instant::now();
    let window_size = LayoutSize::new(1920.0, 1080.0);
    cx.sync_layout_and_render_list(super_root_handle.id, window_size);
    let duration_sync = start_sync.elapsed();
    println!(
        "2 & 3. 永続Taffyレイアウト計算 ＆ DFS同期時間: {:.2?}",
        duration_sync
    );
    assert_eq!(cx.active_entities.len(), 251);

    // 200階層の再帰的な一括デスポーン速度を検証
    let start_despawn = Instant::now();
    cx.despawn(super_root_handle);
    cx.gc_inactive_entities();
    let duration_despawn = start_despawn.elapsed();
    println!(
        "4. 再帰デスポーン・GCクリーンアップ時間: {:.2?}",
        duration_despawn
    );
    println!("==========================================================\n");
}

// 極端に平坦（ワイド）な構造の負荷・速度検証テスト（親1つに対して40,000個の直下の子）
// cargo test test_extreme_wide_flat_performance -- --nocapture
#[test]
fn test_extreme_wide_flat_performance() {
    let mut cx = Context::new();

    println!("\n=== B. 極端に平坦（ワイド）な構造の検証（子要素数: 40,000個） ===");

    // 単一の親要素の下に 4万個の子要素をフラットに並べる
    let start_build = Instant::now();
    let root_handle = build_ui(&mut cx, || {
        let root = div_n().style(ThisStyle::new().display(Display::Flex));
        let mut p_root = root;
        for _ in 0..40000 {
            p_root = p_root.child(div_n().style(ThisStyle::new().display(Display::Flex)));
        }
        p_root
    });
    let duration_build = start_build.elapsed();
    println!("1. 平坦ツリー構築時間 (40,001要素): {:.2?}", duration_build);

    // フラットな4万要素に対するDFS同期時間
    let start_sync = Instant::now();
    let window_size = LayoutSize::new(1920.0, 1080.0);
    cx.sync_layout_and_render_list(root_handle.id, window_size);
    let duration_sync = start_sync.elapsed();
    println!(
        "2 & 3. 永続Taffyレイアウト計算 ＆ DFS同期時間: {:.2?}",
        duration_sync
    );
    assert_eq!(cx.active_entities.len(), 40001);

    // 4万個の子要素を持つ親をデスポーンした際の回収速度を検証
    let start_despawn = Instant::now();
    cx.despawn(root_handle);
    cx.gc_inactive_entities();
    let duration_despawn = start_despawn.elapsed();
    println!(
        "4. 平坦デスポーン・GCクリーンアップ時間: {:.2?}",
        duration_despawn
    );
    println!("==========================================================\n");
}

// 部分変更（Single-point）と全体変更（Global）のパフォーマンス比較検証テスト
// cargo test test_single_and_global_mutation_performance -- --nocapture
#[test]
fn test_single_and_global_mutation_performance() {
    let mut cx = Context::new();

    // 最初に標準的な規模（10,001要素）の基盤ツリーを構築
    // Root -> 100 Parents -> 各々が 99 Children = 計 10,001 要素
    let super_root_handle = build_ui(&mut cx, || {
        let root = div_n().style(ThisStyle::new().display(Display::Flex));
        let mut p_root = root;
        for _ in 0..100 {
            let parent = div_n().style(ThisStyle::new().display(Display::Flex));
            let mut p = parent;
            for _ in 0..99 {
                p = p.child(div_n().style(ThisStyle::new().display(Display::Flex)));
            }
            p_root = p_root.child(p);
        }
        p_root
    });

    // 座標同期
    let window_size = LayoutSize::new(1920.0, 1080.0);
    cx.sync_layout_and_render_list(super_root_handle.id, window_size);
    assert_eq!(cx.active_entities.len(), 10001);

    // テスト前にDirtyキューを完全にクリアしておく
    cx.dirty_layout_entities.clear();
    cx.dirty_render_entities.clear();

    // 将来的にレンダラー（wgpu）の描画転送フェーズで行われる
    // `STATE_QUEUED_RENDER` フラグの一括アンセット（解除）をシミュレート
    for &id in &cx.active_entities {
        if let Some(mask) = cx.active_masks.get_mut(id) {
            mask.unset(STATE_QUEUED_RENDER);
        }
    }
    cx.dirty_render_entities.clear();

    println!("\n=== C. 部分変更・全体変更の性能比較（基盤ツリー: 10,001要素） ===");

    // パターン A: 1箇所のみ変更する際（Single-point Mutation）の応答性
    let target_id = cx.active_entities[5000];

    let start_single = Instant::now();
    // 対象 of Hovered をオンにする（ダイレクトセッター）
    cx.set_hovered(target_id, true);
    let duration_single = start_single.elapsed();

    println!(
        "1. 単一要素の状態変更時間 (Single-point Mutation): {:.2?}",
        duration_single
    );

    // 単一プロパティ変更時、Dirtyキューには確実にその1要素だけが記録されているかを検証
    assert_eq!(cx.dirty_render_entities.len(), 1);
    assert_eq!(cx.dirty_render_entities[0], target_id);

    // 状態を一旦リセット
    cx.set_hovered(target_id, false);

    // パターンAの解除動作で立った `target_id` の Queued フラグもテスト移行前にクリア
    if let Some(mask) = cx.active_masks.get_mut(target_id) {
        mask.unset(STATE_QUEUED_RENDER);
    }
    cx.dirty_render_entities.clear();

    // パターン B: 全要素の全ステート・全プロパティの同時変更（Global Mutation）
    let all_elements = cx.active_entities.clone();

    let start_global = Instant::now();
    // 10,001要素すべての、全ステートおよび全スタイルプロパティを一斉に変更する
    for &id in &all_elements {
        // --- 1. 全てのインタラクションステートを同時にオンにする ---
        cx.set_hovered(id, true);
        cx.set_focused(id, true);
        cx.set_pressed(id, true);
        cx.set_disabled(id, true);
        cx.set_actived(id, true);
        cx.set_selected(id, true);
        cx.set_dragged(id, true);

        // --- 2. 基本レイアウトプロパティ (BasicLayout 全18プロパティ) の同時更新 ---
        if let Some(layout) = cx.basic_layouts.get_mut(id) {
            layout.display = Display::None;
            layout.item_is_table = true;
            layout.item_is_replaced = true;
            layout.box_sizing = BoxSizing::ContentBox;
            layout.direction = Direction::Rtl;
            layout.overflow = LayoutOverflow {
                x: Overflow::Scroll,
                y: Overflow::Scroll,
            };
            layout.position = Position::Absolute;
            layout.inset = Rect::new(Val::Px(10.0), Val::Px(10.0), Val::Px(10.0), Val::Px(10.0));
            layout.size = Size::new(Val::Px(200.0), Val::Px(200.0));
            layout.min_size = Size::new(Val::Px(50.0), Val::Px(50.0));
            layout.max_size = Size::new(Val::Px(500.0), Val::Px(500.0));
            layout.aspect_ratio = Some(1.5);
            layout.margin = Rect::new(Val::Px(5.0), Val::Px(5.0), Val::Px(5.0), Val::Px(5.0));
            layout.padding = Rect::new(
                Length::Px(10.0),
                Length::Px(10.0),
                Length::Px(10.0),
                Length::Px(10.0),
            );
            layout.border = Rect::new(
                Length::Px(2.0),
                Length::Px(2.0),
                Length::Px(2.0),
                Length::Px(2.0),
            );
        }

        // --- 3. Flexレイアウトプロパティ (FlexLayout 全13プロパティ) の同時更新・確保 ---
        let mut flex = cx.flex_layouts.get(id).copied().unwrap_or_default();
        flex.align_items = Some(AlignItems::Center);
        flex.align_self = Some(AlignSelf::Stretch);
        flex.justify_items = Some(AlignItems::End);
        flex.justify_self = Some(AlignSelf::Baseline);
        flex.align_content = Some(AlignContent::SpaceBetween);
        flex.justify_content = Some(JustifyContent::SpaceAround);
        flex.gap = Size::new(Val::Px(10.0), Val::Px(10.0));
        flex.text_align = TextAlign::Center;
        flex.flex_direction = FlexDirection::Column;
        flex.flex_wrap = FlexWrap::Wrap;
        flex.flex_basis = Val::Px(0.5);
        flex.flex_grow = 4.0;
        flex.flex_shrink = 2.0;
        cx.flex_layouts.insert(id, flex);

        // --- 4. ビジュアルプロパティ (VisualProperty 全12プロパティ) の同時更新・確保 ---
        let mut visual = cx.visual_properties.get(id).cloned().unwrap_or_default();
        visual.bg_color = Some(Color::rgb_f32(0.5, 0.5, 0.5));
        visual.border_color = Some(Color::rgb_f32(1.0, 1.0, 1.0));
        visual.corner_radius = Some(CornerRadius::all(15.0));
        visual.opacity = Some(0.8);
        visual.shadow_params = Some(BoxShadow {
            offset: LayoutPoint::new(2.0, 2.0),
            blur: 5.0,
            spread: 1.0,
            color: Color::rgb_f32(0.0, 0.0, 0.0),
        });
        visual.transform = Some([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [10.0, 20.0, 0.0, 1.0],
        ]);
        visual.z_index = Some(99);
        visual.cursor = Some(CursorIcon::Pointer(None));
        visual.text_color = Some(Color::rgb_f32(1.0, 0.0, 0.0));
        visual.font_size = Some(18.0);
        cx.visual_properties.insert(id, visual);

        // --- 5. すべてのコンポーネントマスクを強制的にセットする ---
        if let Some(mask) = cx.active_masks.get_mut(id) {
            mask.set(
                STYLE_BASIC_LAYOUT
                    | STYLE_FLEX_LAYOUT
                    | STYLE_VISUAL_PROPERTY
                    | STYLE_INTERACTION_PROPERTY,
            );
        }

        // キューへ登録
        cx.mark_layout_dirty(id);
        cx.mark_render_dirty(id);
    }
    let duration_global = start_global.elapsed();

    println!(
        "2. 全10,001要素の同時状態変更時間 (Global Mutation): {:.2?}",
        duration_global
    );

    // すべての要素（10,001個）が漏れなくレンダラー側の更新Dirtyキューに入ったかを検証
    assert_eq!(cx.dirty_render_entities.len(), 10001);

    // 後片付け
    cx.despawn(super_root_handle);
    cx.gc_inactive_entities();
    println!("==========================================================\n");
}

// 実用アプリケーションのライフサイクルを想定したベンチマークテスト
// cargo test test_realistic_app_lifecycle_performance -- --nocapture
#[test]
fn test_realistic_app_lifecycle_performance() {
    let mut cx = Context::new();
    let window_size = LayoutSize::new(1920.0, 1080.0);

    println!("\n=== 実用アプリ想定ライフサイクル・同期速度検証（総要素数: 約1,000個） ===");

    let mut set_clock_sig_handle = None;

    let start_build = Instant::now();
    let super_root_handle = build_ui(&mut cx, || {
        let (clock_sig, set_clock_sig) = create_signal(String::from("10:00:00"));
        set_clock_sig_handle = Some(set_clock_sig);

        let mut root = div_n().style(
            ThisStyle::new()
                .display(Display::Flex)
                .flex_direction(FlexDirection::Column),
        );

        let mut header = div_n().style(ThisStyle::new().size((1920.0, 60.0)));
        for _ in 0..19 {
            header = header.child(div_n());
        }
        root = root.child(header);

        let mut main = div_n().style(ThisStyle::new().flex_direction(FlexDirection::Row));
        let mut sidebar = div_n().style(ThisStyle::new().size((250.0, 1020.0)));
        for _ in 0..49 {
            sidebar = sidebar.child(div_n());
        }
        main = main.child(sidebar);

        let mut dashboard = div_n().style(ThisStyle::new().flex_direction(FlexDirection::Column));
        for i in 0..100 {
            let mut card = div_n();
            for j in 0..8 {
                if i == 50 && j == 1 {
                    let sig = clock_sig;
                    card = card.child(div_n().text(move || sig.get().clone()));
                } else {
                    card = card.child(div_n());
                }
            }
            dashboard = dashboard.child(card);
        }
        main = main.child(dashboard);
        root.child(main)
    });
    println!("  - UIツリー構築時間: {:.2?}", start_build.elapsed());

    let root_id = super_root_handle.id;

    // Cold Start
    let start_sync_1 = Instant::now();
    cx.sync_layout_and_render_list(root_id, window_size);
    println!(
        "  1. 【Cold Start】同期時間: {:.2?}",
        start_sync_1.elapsed()
    );

    let sidebar_id = cx.active_entities[22];
    let target_card_id = cx.active_entities[500];

    // Idle
    let start_sync_2 = Instant::now();
    cx.sync_layout_and_render_list(root_id, window_size);
    println!("  2. 【静止】同期時間: {:.2?}", start_sync_2.elapsed());

    // Hover
    let start_sync_3 = Instant::now();
    {
        let _guard = bind_context(&cx);
        cx.set_hovered(target_card_id, true);
    }
    cx.sync_layout_and_render_list(root_id, window_size);
    println!(
        "  3. 【ホバー操作】同期時間: {:.2?}",
        start_sync_3.elapsed()
    );

    // Layout Mutation
    let start_sync_4 = Instant::now();
    {
        let _guard = bind_context(&cx);
        if let Some(layout) = cx.basic_layouts.get_mut(sidebar_id) {
            layout.display = Display::None;
        }
        cx.mark_layout_dirty(sidebar_id);
    }
    cx.sync_layout_and_render_list(root_id, window_size);
    println!(
        "  4. 【サイドバー非表示】同期時間: {:.2?}",
        start_sync_4.elapsed()
    );

    // Reactive Update
    let start_sync_5 = Instant::now();
    {
        let _guard = bind_context(&cx);
        set_clock_sig_handle.unwrap().set(String::from("10:00:01"));
    }
    cx.sync_layout_and_render_list(root_id, window_size);
    println!(
        "  5. 【クロージャ解決】同期時間: {:.2?}",
        start_sync_5.elapsed()
    );

    // Scroll
    let start_sync_6 = Instant::now();
    {
        let _guard = bind_context(&cx);
        let main_id = cx.active_entities[21];
        cx.scroll_offsets
            .insert(main_id, LayoutPoint::new(0.0, 10.0));
        cx.mark_layout_dirty(main_id);
    }
    cx.sync_layout_and_render_list(root_id, window_size);
    println!(
        "  6. 【スクロール】同期時間: {:.2?}",
        start_sync_6.elapsed()
    );

    cx.despawn(super_root_handle);
}
