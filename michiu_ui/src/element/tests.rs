use super::*;
use crate::{
    COMP_TEXT_CONTENT, Color, ComponentMask, Context, STATE_QUEUED_RENDER, STYLE_BG_COLOR,
    ThisStyle, VisualProperty, bind_context, build_ui, create_signal, style::StyleInner,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

// 1. build_ui によるセッション管理と自動クリーンアップのテスト
#[test]
fn test_session_cleanup() {
    let mut cx = Context::new();
    let mut child_id = EntityId::default();

    // build_ui は最後に Element を返す必要がある
    let root = build_ui(&mut cx, || {
        let root = div_n();
        let child = div_n(); // どこにもアタッチされない孤児
        child_id = child.id;
        root // root だけを返してセッションを終了
    });

    // root は登録されているが、child は孤児なので despawn されているはず
    assert!(cx.entities.contains_key(root.id));
    assert!(!cx.entities.contains_key(child_id));
}

// 2. 親子関係構築による寿命の維持テスト
#[test]
fn test_parent_ownership() {
    let mut cx = Context::new();
    let mut child_id = EntityId::default();

    let root = build_ui(&mut cx, || {
        let root = div_n();
        let child = div_n();
        child_id = child.id;

        root.child(child) // 親子関係を結ぶ
    });

    // 親子関係があるため、両方とも生存しているはず
    assert!(cx.entities.contains_key(root.id));
    assert!(cx.entities.contains_key(child_id));
    assert_eq!(cx.parents[child_id], Some(root.id));
}

// 3. 静的なスタイル適用の検証
#[test]
fn test_static_style_application() {
    let mut cx = Context::new();

    let style = ThisStyle::new().bg_color(Color::rgb_f32(1.0, 0.0, 0.0));
    let handle = build_ui(&mut cx, || div_n().style(style));

    assert!(cx.active_masks[handle.id].has(STYLE_BG_COLOR));
    let visual = cx.visual_properties.get(handle.id).unwrap();
    assert_eq!(visual.bg_color, Some(Color::rgb_f32(1.0, 0.0, 0.0)));
    assert!(cx.active_masks[handle.id].has(STATE_QUEUED_RENDER));
}

// 4. リアクティブなスタイル適用の検証 (style)
#[test]
fn test_reactive_style_with_signal() {
    let mut cx = Context::new();
    let mut set_toggle_handle = None;

    let handle = build_ui(&mut cx, || {
        let (is_red, set_red) = create_signal(true);
        set_toggle_handle = Some(set_red);

        div_n().style(move || {
            if is_red.get() {
                ThisStyle::new().bg_color(Color::rgb_f32(1.0, 0.0, 0.0))
            } else {
                ThisStyle::new().bg_color(Color::rgb_f32(0.0, 0.0, 1.0))
            }
        })
    });

    let set_red = set_toggle_handle.unwrap();

    // 初期状態: 赤
    assert_eq!(
        cx.visual_properties[handle.id].bg_color,
        Some(Color::rgb_f32(1.0, 0.0, 0.0))
    );

    // シグナル更新
    {
        let _guard = bind_context(&cx);
        set_red.set(false);
    }

    // 自動的に青に更新されているか
    assert_eq!(
        cx.visual_properties[handle.id].bg_color,
        Some(Color::rgb_f32(0.0, 0.0, 1.0))
    );
}

// 5. リアクティブなテキスト更新の検証 (text_with)
#[test]
fn test_reactive_text() {
    let mut cx = Context::new();
    let mut set_text_handle = None;

    let handle = build_ui(&mut cx, || {
        let (name, set_name) = create_signal("Alice");
        set_text_handle = Some(set_name);

        div_n().text(move || format!("Hello, {}", name.get()))
    });

    let set_name = set_text_handle.unwrap();
    assert_eq!(cx.text_contents[handle.id], "Hello, Alice");

    // シグナル更新
    {
        let _guard = bind_context(&cx);
        set_name.set("Bob");
    }

    assert_eq!(cx.text_contents[handle.id], "Hello, Bob");
}

// 6. 動的コンテンツ差し替え (set_content) の検証
#[test]
fn test_reactive_content_switching() {
    let mut cx = Context::new();
    let mut set_show_handle = None;

    let parent = build_ui(&mut cx, || {
        let (show, set_show) = create_signal(true);
        set_show_handle = Some(set_show);

        let container = div_n();
        container.set_contents(move || {
            if show.get() {
                div_n().text("A")
            } else {
                div_n().text("B")
            }
        });
        container
    });

    let set_show = set_show_handle.unwrap();

    // 初期状態の子要素を取得
    let child_id_a = cx.children[parent.id][0];
    assert_eq!(cx.text_contents[child_id_a], "A");

    // シグナル更新（切り替え）
    {
        let _guard = bind_context(&cx);
        set_show.set(false);
    }

    // 古い子が despawn され、新しい子が生成されているか
    assert!(!cx.entities.contains_key(child_id_a));
    let child_id_b = cx.children[parent.id][0];
    assert_eq!(cx.text_contents[child_id_b], "B");
}

// 7. イベントリスナーとシグナルの連動
#[test]
fn test_event_trigger_signal() {
    let mut cx = Context::new();
    let mut clicked_id = EntityId::default();

    build_ui(&mut cx, || {
        let (count, set_count) = create_signal(0u32);

        let btn = div_n().text(move || count.get().to_string()).on_click(move || {
            let next = count.get() + 1;
            set_count.set(next);
        });

        clicked_id = btn.id;
        btn
    });

    assert_eq!(cx.text_contents[clicked_id], "0");

    // 疑似クリック実行
    {
        let _guard = bind_context(&cx);
        let mut handler = cx
            .event_listeners
            .get_mut(clicked_id)
            .and_then(|l| l.on_click.take())
            .expect("Handler not found");

        handler(&mut cx);

        if let Some(listeners) = cx.event_listeners.get_mut(clicked_id) {
            listeners.on_click = Some(handler);
        }
    }

    // テキストが "1" に更新されているか
    assert_eq!(cx.text_contents[clicked_id], "1");
}

// 8. セッション終了時の despawn 安全性テスト
#[test]
fn test_despawn_with_effects() {
    let mut cx = Context::new();
    let mut set_val_handle = None;
    let run_count = Arc::new(AtomicU32::new(0));
    let run_count_clone = run_count.clone();

    let root = build_ui(&mut cx, || {
        let (val, set_val) = create_signal(0);
        set_val_handle = Some(set_val);

        div_n().text(move || {
            run_count_clone.fetch_add(1, Ordering::SeqCst);
            val.get().to_string()
        })
    });

    assert_eq!(run_count.load(Ordering::SeqCst), 1);

    // 手動削除
    cx.despawn(root);

    // シグナルを更新してもエフェクト（text_with）が走らないことを確認
    {
        let _guard = bind_context(&cx);
        set_val_handle.unwrap().set(100);
    }

    assert_eq!(run_count.load(Ordering::SeqCst), 1); // 増えていない
}

// 暗黙コンテキストの強制安全ガードテスト
#[test]
#[should_panic(expected = "No active UI Context found in this thread context")]
fn test_panic_outside_build_ui() {
    // build_ui のバインドがない状態で要素を作ろうとすると、
    // 開発者への通知を促す安全な panic が発生するかを検証
    let _val = div_n();
}

// 寿命管理から離脱し、手動 despawn するライフサイクルテスト
#[test]
fn test_child_ownership_preservation() {
    let mut cx = Context::new();
    let mut child_handle = Element {
        id: EntityId::default(),
    };

    let root_handle = build_ui(&mut cx, || {
        let root = div_n();
        let child = div_n();

        child_handle = child;

        // 親子を構築
        root.child(child)
    });

    // スコープを抜けても、ルートが release されているため、親子ともに Context 上で生き続けているかを検証
    assert!(cx.entities.contains_key(root_handle.id));
    assert!(cx.entities.contains_key(child_handle.id));

    // 親子のトポロジー関係も正しく保存されているかを検証
    assert_eq!(cx.parents[child_handle.id], Some(root_handle.id));
    assert!(cx.children[root_handle.id].contains(&child_handle.id));

    // 外部（手動）デスポーンを実行
    cx.despawn(root_handle);

    // 手動デスポーンによって再帰的に両方が消滅したかを検証
    assert!(!cx.entities.contains_key(root_handle.id));
    assert!(!cx.entities.contains_key(child_handle.id));
}

// ビルダーによるスタイル適用のマッピング検証
#[test]
fn test_style_application() {
    let mut cx = Context::new();

    // テスト用の BG_COLOR 設定を持つスタイルを作成
    let mut custom_style = ThisStyle::new();
    custom_style.inner = Arc::new(StyleInner {
        mask: ComponentMask::new(STYLE_BG_COLOR),
        visual_property: VisualProperty {
            bg_color: Some(Color::rgb_f32(1.0, 0.0, 0.0)),
            ..Default::default()
        },
        ..Default::default()
    });

    let handle = build_ui(&mut cx, || div_n().style(custom_style));

    // 1. BG_COLOR マスクがセットされたか
    assert!(cx.active_masks[handle.id].has(STYLE_BG_COLOR));
    // 2. ビジュアルデータに色情報が代入されたか
    let visual = cx.visual_properties.get(handle.id).unwrap();
    assert_eq!(visual.bg_color, Some(Color::rgb_f32(1.0, 0.0, 0.0)));
    // 3. レンダリングDirtyがセットされたか
    assert!(cx.active_masks[handle.id].has(STATE_QUEUED_RENDER));

    cx.despawn(handle);
}

// 静的テキストと動的テキスト（クロージャ評価）の上書き・マージ検証
#[test]
fn test_static_and_dynamic_contents() {
    let mut cx = Context::new();

    let handle = build_ui(&mut cx, || {
        // 静的テキストを設定したあと、動的テキストで上書き
        div_n().text("Static Text").text(|| "Dynamic Text")
    });

    // text_with の初回自動評価が実行され、"Dynamic Text" が割り当てられているかを検証
    assert_eq!(cx.text_contents[handle.id], "Dynamic Text");
    // text_closures 側に再評価用のクロージャが登録されているかを検証
    assert!(cx.element_effects.contains_key(handle.id));
    assert!(cx.active_masks[handle.id].has(COMP_TEXT_CONTENT));

    cx.despawn(handle);
}

// イベントリスナー（EventListeners）の登録と中継確認
#[test]
fn test_event_listener_registration() {
    let mut cx = Context::new();
    let clicked = Arc::new(AtomicU32::new(0));
    let clicked_clone = clicked.clone();

    let handle = build_ui(&mut cx, || {
        div_n().on_click(move || {
            clicked_clone.fetch_add(1, Ordering::SeqCst);
        })
    });

    // EventListeners が正しく保存されているか取り出して検証
    let mut listeners = cx.event_listeners.remove(handle.id).unwrap();
    let mut click_handler = listeners.on_click.take().unwrap();

    // 実際にコールバックを評価してトリガーされるか確認
    click_handler(&mut cx);
    assert_eq!(clicked.load(Ordering::SeqCst), 1);

    cx.despawn(handle);
}

// UI Automation (UIA) 拡張プロパティ設定の検証
#[test]
fn test_uia_properties() {
    let mut cx = Context::new();

    let handle = build_ui(&mut cx, || {
        div_n()
            .uia_name("Custom Button")
            .uia_automation_id("btn_01")
    });

    // UIA SoA マップにデータが挿入されているかを検証
    let props = cx.uia_properties.get(handle.id).unwrap();
    // Name Property ID は 30005、Automation ID は 30011
    let name_prop = props.iter().find(|(k, _)| *k == 30005).unwrap();
    let auto_prop = props.iter().find(|(k, _)| *k == 30011).unwrap();

    assert_eq!(name_prop.1, UiaValue::String("Custom Button".to_string()));
    assert_eq!(auto_prop.1, UiaValue::String("btn_01".to_string()));
    assert!(cx.active_masks[handle.id].has(COMP_UIA_CONTENT));

    cx.despawn(handle);
}

// 動的コンテンツ差し替え（set_content_with）および破棄時ライフサイクルの検証
#[test]
fn test_element_set_content_with() {
    let mut cx = Context::new();

    // 動的マウントの実行
    let parent_handle = build_ui(&mut cx, || {
        let parent = div_n();

        // クロージャを渡して動的に子要素を差し替える
        with_context(|_| {
            parent.set_contents(|| div_n().text("Dynamically Generated Child"));
        });

        parent
    });

    // 2.1 初回評価により、子が自動的に生成・マウントされているかを検証
    assert!(cx.entities.contains_key(parent_handle.id));
    assert!(cx.element_effects.contains_key(parent_handle.id)); // クロージャが登録されているべき

    // 生成された子要素のIDを特定
    let children = cx.children[parent_handle.id].clone();
    assert_eq!(children.len(), 1);
    let child_id = children[0];

    assert!(cx.entities.contains_key(child_id));
    assert_eq!(cx.parents[child_id], Some(parent_handle.id));

    // 2.2 親コンテナを despawn した際、中身の動的クロージャも SoA 上から完全にリークせず消えるかを検証
    cx.despawn(parent_handle);

    assert!(!cx.entities.contains_key(parent_handle.id));
    assert!(!cx.entities.contains_key(child_id)); // 子要素も再帰的に despawn されているべき
    assert!(!cx.element_effects.contains_key(parent_handle.id)); // メモリリーク防止：クロージャマップからも完全に削除されているべき
}
