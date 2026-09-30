use michiu_ui::{accessibility::*, prelude::*};

pub fn container() -> Element {
    v_flex(ts().gap(16.0).p(16.0))
        // `a11y`, `a11y_with`, and `a11y_role` take precedence over `tag` and `tag_a11y`
        .a11y(Node::new(Role::GenericContainer))
        .children([role_group(), tag_group()])
}

fn role_group() -> Element {
    v_flex(ts().p(8.0).gap(16.0))
        .a11y_with(Node::new(Role::Group), |n| n.set_label("Role Group"))
        .label("Role", ts())
        .child(
            // If `Node` is not specified, `NO_A11Y_NODE` can be used.
            h_flex(ts().gap(16.0)).a11y(NO_A11Y_NODE).children([
                role_button(),
                role_button(),
                role_button(),
                role_button(),
            ]),
        )
}

fn role_button() -> Element {
    div(ts()
        .r(4.0)
        .p(8.0)
        .debug_border_yellow()
        .pressed(ts().debug_border_green()))
    .a11y_role(Role::Button) // Specifying `Role` only
    .label("Button", ts().font_size(18.0))
}

fn tag_group() -> Element {
    v_flex(ts().p(8.0).gap(16.0))
        .tag::<a11y::AContainer>() // Use the library's standard tags
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
    .tag_a11y::<a11y::AButton>() // When using standard tags, `tag` and `tag_a11y` are the same.
    .label("Button", ts().font_size(18.0))
    .on_click_with(|cx| {
        // Even tags intended for inference can be used as regular tags.
        let els = cx.query_all::<a11y::AButton>();
        for el in els {
            println!("has AAnyButton: {:?}", el.id())
        }
    })
}

struct UserMichiuButton;
impl A11yInferenceTag for UserMichiuButton {
    fn inference(_cx: &mut Context, _id: NodeId, node: &mut Node) {
        node.set_role(Role::Button);
        node.set_label("User Michiu Button");
    }
}

fn user_tag_button() -> Element {
    div(ts()
        .r(4.0)
        .p(8.0)
        .debug_border_red()
        .pressed(ts().debug_border_blue()))
    // If you use custom tags for inference, you must register them with `tag_a11y`.
    .tag_a11y::<UserMichiuButton>()
    .label("Button", ts().font_size(18.0))
    .on_click_with(|cx| {
        let els = cx.query_all::<UserMichiuButton>();
        for el in els {
            println!("has UserMichiuButton: {:?}", el.id())
        }
    })
}
