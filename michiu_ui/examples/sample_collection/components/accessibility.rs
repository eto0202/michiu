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
        .tag::<a11y::AAnyGroup>()
        .label("Tag", ts())
        .child(h_flex(ts().gap(16.0)).children([
            tag_button(),
            tag_button(),
            tag_button(),
            user_tag_input(),
        ]))
}

fn tag_button() -> Element {
    div(ts()
        .r(4.0)
        .p(8.0)
        .debug_border_green()
        .pressed(ts().debug_border_red()))
    .tag_a11y::<a11y::AAnyButton>() // 予約済みのタグを使用する場合、`tag` と `tag_a11y` は同じ
    .label("Button", ts().font_size(18.0))
    .on_click_with(|cx| {
        let els = cx.query_all::<a11y::AAnyButton>(); // 推論用のタグであっても通常通り使用できる。
        for el in els {
            println!("has AAnyButton: {:?}", el.id())
        }
    })
}

fn user_tag_input() -> Element {
    div(ts()
        .r(4.0)
        .p(8.0)
        .debug_border_red()
        .pressed(ts().debug_border_blue()))
    .tag_a11y::<UserMichiuInput>() // 独自の推論用タグを使用する場合は `tag_a11y` で登録する必要がある。
    .label("Input", ts().font_size(18.0))
    .on_click_with(|cx| {
        let els = cx.query_all::<UserMichiuInput>(); // 推論用のタグであっても通常通り使用できる。
        for el in els {
            println!("has UserMichiuInput: {:?}", el.id())
        }
    })
}

struct UserMichiuInput;
impl A11yInferenceTag for UserMichiuInput {
    fn inference(node: &mut accesskit::Node) {
        node.set_role(role::TextInput);
    }
}
