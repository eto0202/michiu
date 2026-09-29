use michiu_ui::{
    a11y::{self, A11yInferenceTag},
    prelude::*,
};

pub fn container() -> Element {
    v_flex(ts().gap(16.0).p(16.0)).children([role_group(), tag_group()])
}

fn role_group() -> Element {
    v_flex(ts().p(8.0).gap(16.0))
        .a11y(role::Group, "Role Group") // `a11y` や `a11y_n` は `tag` や `tag_a11y` よりも優先される
        .label("Role", ts())
        .child(h_flex(ts().gap(16.0)).children([
            role_button(),
            role_button(),
            role_button(),
            role_button(),
        ]))
}

fn role_button() -> Element {
    div(ts()
        .r(4.0)
        .p(8.0)
        .debug_border_yellow()
        .pressed(ts().debug_border_green()))
    .a11y_n(role::Button) // label 無しの場合は `a11y_n` を使用できる。
    .label("Button", ts().font_size(18.0))
}

fn tag_group() -> Element {
    v_flex(ts().p(8.0).gap(16.0))
        .tag::<a11y::AContainer>()
        .label("Tag", ts())
        .child(h_flex(ts().gap(16.0)).children([
            tag_button(),
            tag_button(),
            tag_button(),
            user_tag_button(),
        ]))
}

fn tag_button() -> Element {
    div(ts()
        .r(4.0)
        .p(8.0)
        .debug_border_green()
        .pressed(ts().debug_border_red()))
    .tag_a11y::<a11y::AButton>() // 予約済みのタグを使用する場合、`tag` と `tag_a11y` は同じ
    .label("Button", ts().font_size(18.0))
    .on_click_with(|cx| {
        let els = cx.query_all::<a11y::AButton>(); // 推論用のタグであっても通常通り使用できる。
        for el in els {
            println!("has AAnyButton: {:?}", el.id())
        }
    })
}

fn user_tag_button() -> Element {
    div(ts()
        .r(4.0)
        .p(8.0)
        .debug_border_red()
        .pressed(ts().debug_border_blue()))
    .tag_a11y::<UserMichiuButton>() // 独自の推論用タグを使用する場合は `tag_a11y` で登録する必要がある。
    .label("Button", ts().font_size(18.0))
    .on_click_with(|cx| {
        let els = cx.query_all::<UserMichiuButton>(); // 推論用のタグであっても通常通り使用できる。
        for el in els {
            println!("has UserMichiuButton: {:?}", el.id())
        }
    })
}

struct UserMichiuButton;
impl A11yInferenceTag for UserMichiuButton {
    fn inference(_cx: &Context, node: &mut accesskit::Node) {
        node.set_role(role::Button);
        node.set_label("User Michiu Button");
    }
}
