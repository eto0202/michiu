use crate::app::theme::Theme;
pub use michiu_ui::prelude::*;

pub fn container() -> Element {
    // 親コンテナを先に生成して ID を確定
    let container = v_flex(
        ts().gap(16.0)
            .grow()
            .p(16.0)
            .r(4.0)
            .droppable(DropTarget::Child, DragPayload::Element)
            .drag_over(ts().bg_color(dynamic(|t: &Theme| t.background_hover))),
    )
    .label("Droppable", label_style());
    let parent_id = container.id();

    // 確定した ID を引き渡して子を生成しアタッチ
    let section = section_draggable(parent_id);
    let section_id = section.id();

    container.child(section.child(item_draggable(section_id)))
}

fn section_draggable(parent_id: EntityId) -> Element {
    // 静的レイアウトスタイル
    let layout = ts()
        .p(16.0)
        .r(4.0)
        .absolute()
        .top(50.0)
        .left(50.0)
        .size(300.0)
        .border_solid(1.0)
        .droppable(DropTarget::Child, DragPayload::Element)
        .draggable_parent(parent_id, DragPayload::Element, true)
        .draggable_original(ts().hidden())
        .draggable_placeholder(ts().p(16.0).border_solid(1.0));

    // 動的なカラースタイルのみを style_d で評価
    div(layout)
        .style_d(|t: &Theme| {
            ts().bg_color(t.background)
                .border_color(t.border)
                .hovered(ts().bg_color(t.background_hover))
                .drag_over(ts().border_color(t.border_hover))
                .draggable_placeholder(ts().bg_color(t.border).border_color(t.border))
        })
        .label("Draggable & Droppable", label_style())
}

fn item_draggable(parent_id: EntityId) -> Element {
    // ラベルテキストを制御するシグナルを定義
    let (label_text, set_label_text) = create_signal("Draggable".to_string());

    let layout = ts()
        .p(16.0)
        .r(4.0)
        .absolute()
        .top(50.0)
        .left(50.0)
        .size(150.0)
        .border_solid(1.0)
        .draggable_parent(parent_id, DragPayload::Element, true)
        .draggable_original(ts().p(16.0).border_dashed(1.0).opacity_50())
        .draggable_placeholder(ts().p(16.0).border_solid(1.0));

    div(layout)
        .style_d(|t: &Theme| {
            ts().bg_color(t.background)
                .border_color(t.border)
                .hovered(ts().bg_color(t.background_hover))
                .draggable_original(ts().bg_color(t.background).border_color(t.border))
                .draggable_placeholder(ts().bg_color(t.background).border_color(t.border))
        })
        .label(label_text, label_style())
        .on_drag_start(move |_, _| {
            set_label_text.set("Dragging...".to_string());
        })
        .on_element_drop(move |_, _| {
            set_label_text.set("Draggable".to_string());
        })
}

fn label_style() -> ThisStyle {
    ts().text_color(dynamic(|t: &Theme| t.text_muted))
        .font_size(16.0)
}
