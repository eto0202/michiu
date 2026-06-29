use super::*;

#[test]
fn test_color_hex() {
    let color = hex(0xFF3300);
    assert_eq!(color.r, 1.0);
    // 0x33 は 51。51.0 / 255.0 = 0.20 の精度を検証
    assert!((color.g - 0.2).abs() < f32::EPSILON);
    assert_eq!(color.b, 0.0);
    assert_eq!(color.a, 1.0);
}

#[test]
fn test_rect_contains() {
    let rect = LayoutRect::new(10.0, 20.0, 100.0, 50.0);

    // 1. 矩形内部の座標
    assert!(rect.contains(LayoutPoint::new(15.0, 25.0)));
    // 2. 境界線上の座標 (境界を含む仕様の検証)
    assert!(rect.contains(LayoutPoint::new(10.0, 20.0)));
    assert!(rect.contains(LayoutPoint::new(110.0, 70.0)));
    // 3. 矩形外部の座標
    assert!(!rect.contains(LayoutPoint::new(9.9, 25.0)));
    assert!(!rect.contains(LayoutPoint::new(15.0, 19.9)));
    assert!(!rect.contains(LayoutPoint::new(110.1, 70.0)));
    assert!(!rect.contains(LayoutPoint::new(15.0, 70.1)));

    // 4. 幅または高さが 0 以下の場合は常に false となるエッジケース
    let zero_rect = LayoutRect::new(10.0, 20.0, 0.0, 50.0);
    assert!(!zero_rect.contains(LayoutPoint::new(10.0, 25.0)));

    let neg_rect = LayoutRect::new(10.0, 20.0, 100.0, -10.0);
    assert!(!neg_rect.contains(LayoutPoint::new(15.0, 15.0)));
}

#[test]
fn test_rect_intersect() {
    let r1 = LayoutRect::new(0.0, 0.0, 100.0, 100.0);
    let r2 = LayoutRect::new(50.0, 50.0, 100.0, 100.0);

    // 1. 部分的な交差
    let inter1 = r1.intersect(&r2);
    assert_eq!(inter1, LayoutRect::new(50.0, 50.0, 50.0, 50.0));

    // 2. 完全に離れている場合（幅・高さが 0.0 に丸められるか）
    let r3 = LayoutRect::new(200.0, 200.0, 50.0, 50.0);
    let inter2 = r1.intersect(&r3);
    assert_eq!(inter2.width, 0.0);
    assert_eq!(inter2.height, 0.0);

    // 3. 一方がもう一方を完全に内包する場合
    let r4 = LayoutRect::new(10.0, 10.0, 20.0, 20.0);
    let inter3 = r1.intersect(&r4);
    assert_eq!(inter3, r4);
}

#[test]
fn test_edge_insets() {
    let all = EdgeInsets::px_all(10.0);
    assert_eq!(all.top, 10.0);
    assert_eq!(all.right, 10.0);
    assert_eq!(all.bottom, 10.0);
    assert_eq!(all.left, 10.0);

    let sym = EdgeInsets::px_sym(5.0, 15.0);
    assert_eq!(sym.top, 5.0);
    assert_eq!(sym.bottom, 5.0);
    assert_eq!(sym.left, 15.0);
    assert_eq!(sym.right, 15.0);
}

#[test]
fn test_corner_radius() {
    let cr = CornerRadius::all(8.0);
    assert_eq!(cr.top_left, 8.0);
    assert_eq!(cr.top_right, 8.0);
    assert_eq!(cr.bottom_right, 8.0);
    assert_eq!(cr.bottom_left, 8.0);
}

#[test]
fn test_uia_value_from() {
    let v_str = UiaValue::from("hello");
    assert_eq!(v_str, UiaValue::String("hello".into()));

    let v_bool = UiaValue::from(true);
    assert_eq!(v_bool, UiaValue::Bool(true));

    let v_int = UiaValue::from(42);
    assert_eq!(v_int, UiaValue::Int(42));
}

#[test]
fn test_basic_layout_override_with() {
    let mut base = BasicLayout::default();
    let other = BasicLayout {
        size: Size::px(100.0, 200.0),
        display: Display::None,
        ..Default::default()
    };

    // STYLE_SIZE のマスクのみを有効化
    let mut mask = ComponentMask::new(0);
    mask.set(STYLE_SIZE);

    base.override_with(&other, mask);

    // size プロパティのみが正しくオーバーライドされているか検証
    assert_eq!(base.size, other.size);

    // display はマスクに含まれていないため、デフォルト（Flexなど）のまま変更されていないか検証
    assert_eq!(base.display, BasicLayout::default().display);
}

#[test]
fn test_flex_layout_override_with() {
    let mut base = FlexLayout::default();
    let other = FlexLayout {
        flex_grow: 2.0,
        flex_shrink: 3.0,
        ..Default::default()
    };

    let mut mask = ComponentMask::new(0);
    mask.set(STYLE_FLEX_GROW);

    base.override_with(&other, mask);

    // flex_grow のみが上書きされ、flex_shrink は変わらないことを検証
    assert_eq!(base.flex_grow, 2.0);
    assert_eq!(base.flex_shrink, FlexLayout::default().flex_shrink);
}

#[test]
fn test_bytemuck_pod_casting() {
    // GPU等へのアロケーション転送時における、アライメントや bytes キャストの安全性を検証
    let color = Color::rgba_f32(1.0, 0.5, 0.0, 1.0);
    let bytes = bytemuck::bytes_of(&color);
    assert_eq!(bytes.len(), std::mem::size_of::<Color>());
}
