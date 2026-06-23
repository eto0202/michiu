use super::*;

// 1. Arc を用いた Copy-on-Write（CoW）の物理メモリアドレスレベルでの検証
#[test]
fn test_style_cow_behavior() {
    // 白の背景色を持つベーススタイルを作成
    let base_style = ThisStyle::new().bg_color(Color::rgb(1.0, 1.0, 1.0));

    // スタイルを複製する。この段階では Arc の参照カウントが増えるだけで、メモリはコピーされない。
    let cloned_style = base_style.clone();

    // 物理的に全く同じポインタ（メモリ領域）を指し示しているかを厳密に検証
    assert!(Arc::ptr_eq(&base_style.inner, &cloned_style.inner));

    // 複製したスタイルをメソッドチェーンで部分書き換え（サイズを設定）。
    // ここで Arc::make_mut が走り、ディープコピー（メモリの分岐）が実行されるべき。
    let mutated_style = cloned_style.size(Size::px(100.0, 200.0));

    // 物理メモリアドレスが変化し、安全に分岐（隔離）されたかを検証
    assert!(!Arc::ptr_eq(&base_style.inner, &mutated_style.inner));

    // 元のベーススタイルには一切影響がなく、サイズがデフォルト値（未指定）のままであるかを検証
    assert_eq!(
        base_style.inner.basic_layout.size,
        BasicLayout::default().size
    );
    assert_eq!(
        base_style.inner.visual_property.bg_color,
        Some(Color::rgb(1.0, 1.0, 1.0))
    );

    // 分岐したスタイルは、元の背景色を保持しつつ、新しく指定したサイズ情報がマージされているかを検証
    assert_eq!(
        mutated_style.inner.visual_property.bg_color,
        Some(Color::rgb(1.0, 1.0, 1.0))
    );
    assert_ne!(
        mutated_style.inner.basic_layout.size,
        BasicLayout::default().size
    );
}

// 2. 独自メソッドとビットマスクの整合性検証
#[test]
fn test_builder_methods_and_masks() {
    let style = ThisStyle::new()
        .display(Display::Flex)
        .flex_grow(2.0)
        .bg_color(Color::rgb(0.0, 1.0, 0.0));

    let inner = &style.inner;
    let mask = inner.mask;

    // 2.1 基本レイアウトメソッドの動作とマスクの整合性
    assert!(mask.has(STYLE_DISPLAY));
    assert_eq!(inner.basic_layout.display, Display::Flex);

    // 2.2 Flexレイアウトメソッドの動作とマスクの整合性
    assert!(mask.has(STYLE_FLEX_GROW));
    assert_eq!(inner.flex_layout.flex_grow, 2.0);

    // 2.3 ビジュアルプロパティメソッドの動作とマスクの整合性
    assert!(mask.has(STYLE_BG_COLOR));
    assert_eq!(
        inner.visual_property.bg_color,
        Some(Color::rgb(0.0, 1.0, 0.0))
    );

    // メソッドを呼び出していないプロパティのフラグが、マスクに混入していないことを検証
    assert!(!mask.has(STYLE_SIZE));
    assert!(!mask.has(STYLE_FLEX_SHRINK));
    assert!(!mask.has(STYLE_BORDER_COLOR));
}

// 3. 重い GridLayout データの遅延アロケーション（Lazy Allocation）検証
#[test]
fn test_lazy_grid_allocation() {
    let mut style = ThisStyle::new();

    // 初期状態では GridLayout の構造体が確保（Heapアロケーション）されておらず、None であるべき
    assert!(style.inner.grid_layout.is_none());
    assert!(!style.inner.mask.has(STYLE_GRID_LAYOUT));

    // グリッドプロパティ（grid-auto-flow）を設定
    style = style.grid_auto_flow(GridAutoFlow::Row);

    // メソッドが呼ばれて初めて、GridLayout の実体が自動確保（Lazy Init）されたかを検証
    let inner = &style.inner;
    assert!(inner.grid_layout.is_some());
    assert!(inner.mask.has(STYLE_GRID_LAYOUT));
    assert_eq!(
        inner.grid_layout.as_ref().unwrap().grid_auto_flow,
        GridAutoFlow::Row
    );
}

// 4. インタラクティブスタイル（ホバースタイルなど）のネスト保持検証
#[test]
fn test_nested_interactive_styles() {
    let hovered_override = ThisStyle::new().bg_color(Color::rgb(0.0, 0.0, 1.0));

    let style = ThisStyle::new()
        .bg_color(Color::rgb(1.0, 1.0, 1.0))
        .hovered(hovered_override);

    let inner = &style.inner;

    // ホバー設定フラグがオンになっているか
    assert!(inner.mask.has(STATE_HOVERED));

    // ホバー時のオーバーライドスタイルから、指定した背景色（青）が正確に取り出せるかを検証
    let hover_style = inner.interaction_styles.hovered.as_ref().unwrap();
    assert_eq!(
        hover_style.inner.visual_property.bg_color,
        Some(Color::rgb(0.0, 0.0, 1.0))
    );
}
