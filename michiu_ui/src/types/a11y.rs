use crate::{Context, MichiuTagRegistry};
use accesskit::{Action, Role};

pub(crate) type InferenceFn = fn(cx: &mut Context, id: accesskit::NodeId, &mut accesskit::Node);

/// `A11yInferenceTag` for accessibility.
///
/// Overrides the library's inference.
///
/// [`accesskit`] is required.
pub trait A11yInferenceTag {
    /// Configure accessibility information inference.
    ///
    /// # Panics / Contract
    /// This method is intended solely for querying the current UI hierarchy and configuring
    /// the accessibility node.
    ///
    /// Do not create or destroy entities, change layouts, or change states within this method.
    /// Doing so may result in unexpected panics or inconsistent UI frame rendering.
    fn inference(cx: &mut Context, id: accesskit::NodeId, node: &mut accesskit::Node);
}

#[macro_export]
macro_rules! define_inference_tags {
    (
        $(
            $(#[$meta:meta])*
            $name:ident => |$cx:ident, $id:ident, $node:ident| $body:block
        ),* $(,)?
    ) => {
        // ZST の定義
        $(
            $(#[$meta])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
            pub struct $name;

            // A11yInferenceTag トレイトの実装
            impl A11yInferenceTag for $name {
                #[inline]
                #[allow(unused)]
                fn inference($cx: &mut Context, $id: ::accesskit::NodeId, $node: &mut ::accesskit::Node) $body
            }
        )*

        // MichiuTagRegistry に登録するメソッド
        impl MichiuTagRegistry {
            pub(crate) fn register_builtin_inferences(&mut self) {
                $(
                    self.register_a11y_inference::<$name>();
                )*
            }
        }
    };
}

define_inference_tags! {
    /// Represents some kind of Button.
    AButton => |cx, id, node| {
        node.set_role(Role::Button);
        node.add_action(Action::Click);
        node.add_action(Action::Focus);
        node.set_keyboard_shortcut("Enter");
    },
    /// Represents some kind of Label.
    ALabel => |cx, id, node| {
        node.set_role(Role::Label);
    },
    /// Represents some kind of Group.
    AContainer => |cx, id, node| {
        node.set_role(Role::GenericContainer);
    },
    /// Represents some kind of Text Input.
    ATextInput => |cx, id, node| {
        node.set_role(Role::TextInput);
        node.add_action(Action::Click);
        node.add_action(Action::Focus);
        node.add_action(Action::SetValue);
    },
    /// Represents some kind of Data Input.
    ADataInput => |cx, id, node| {
        node.set_role(Role::DateInput);
        node.add_action(Action::Click);
        node.add_action(Action::Focus);
        node.add_action(Action::SetValue);
    },
    /// It is interpreted as a Tooltip.
    ATooltip => |cx, id, node| {
        node.set_role(Role::Tooltip);
    },
    /// It is interpreted as a Close Button.
    ACloseButton => |cx, id, node| {
        node.set_role(Role::Button);
        node.add_action(Action::Click);
        node.add_action(Action::Focus);
        node.set_keyboard_shortcut("Enter");
    },
    /// It is interpreted as a Submit Button.
    ASubmitButton => |cx, id, node| {
        node.set_role(Role::Button);
        node.add_action(Action::Click);
        node.add_action(Action::Focus);
        node.set_keyboard_shortcut("Enter");
    },
    /// It is interpreted as a List.
    AList => |cx, id, node| {
        node.set_role(Role::List);
    },
    /// It is interpreted as a List Item.
    AListItem => |cx, id, node| {
        node.set_role(Role::ListItem);
    },
    /// It is interpreted as a Tab.
    ATab => |cx, id, node| {
        node.set_role(Role::Tab);
        node.add_action(Action::Click);
    },
    /// It is interpreted as a Tab List.
    ATabList => |cx, id, node| {
        node.set_role(Role::TabList);
    },
    /// It is interpreted as a Tab Panel.
    ATabPanel => |cx, id, node| {
        node.set_role(Role::TabPanel);
    },
    /// It is interpreted as a Menu.
    AMenu => |cx, id, node| {
        node.set_role(Role::Menu);
    },
    /// It is interpreted as a Menu Item.
    AMenuItem => |cx, id, node| {
        node.set_role(Role::MenuItem);
        node.add_action(Action::Click);
    },
    /// It is interpreted as a Toggle Button.
    AToggleButton => |cx, id, node| {
        node.set_role(Role::Button);
        node.add_action(Action::Click);
    },
    /// It is interpreted as a Switch.
    ASwitch => |cx, id, node| {
        node.set_role(Role::Button);
        node.add_action(Action::Click);
    },
    /// It is interpreted as a Alert.
    AAlert => |cx, id, node| {
        node.set_role(Role::Alert);
    },
    /// It is interpreted as a Status.
    AStatus => |cx, id, node| {
        node.set_role(Role::Status);
    },
    /// It is interpreted as a Progress.
    AProgressBar => |cx, id, node| {
        node.set_role(Role::ProgressIndicator);
    },
    /// It is interpreted as a Slider.
    ASlider => |cx, id, node| {
        node.set_role(Role::Slider);
        node.add_action(Action::Click);
        node.add_action(Action::SetValue);
    },
    /// It is interpreted as a Scrollbar.
    AScrollbar => |cx, id, node| {
        node.set_role(Role::ScrollBar);
        node.add_action(Action::Click);
    },
    /// It is interpreted as a Main.
    AMain => |cx, id, node| {
        node.set_role(Role::Main);
    },
    /// It is interpreted as a Navigation.
    ANavigation => |cx, id, node| {
        node.set_role(Role::Navigation);
    },
    /// It is interpreted as a Header.
    AHeader => |cx, id, node| {
        node.set_role(Role::Header);
    },
    /// It is interpreted as a Footer.
    AFooter => |cx, id, node| {
        node.set_role(Role::Footer);
    },
    /// It is interpreted as a Banner.
    ABanner => |cx, id, node| {
        node.set_role(Role::Banner);
    },
    /// It is interpreted as a Sidebar.
    ASidebar => |cx, id, node| {
        node.set_role(Role::Complementary);
    },
    /// It is interpreted as a Complementary.
    AComplementary => |cx, id, node| {
        node.set_role(Role::Complementary);
    },
    /// It is interpreted as a Search Input.
    ASearchInput => |cx, id, node| {
        node.set_role(Role::SearchInput);
        node.add_action(Action::Click);
        node.add_action(Action::Focus);
        node.add_action(Action::SetValue);
    },
    /// It is interpreted as a Dialog.
    ADialog => |cx, id, node| {
        node.set_role(Role::Dialog);
    },
    /// It is interpreted as a Heading.
    AHeading1 => |cx, id, node| {
        node.set_role(Role::Heading);
    },
    /// It is interpreted as a Image.
    AImage => |cx, id, node| {
        node.set_role(Role::Image);
    },
    /// It is interpreted as a Image.
    AIcon => |cx, id, node| {
        node.set_role(Role::Image);
    },
    /// It is interpreted as a Media.
    AMedia => |cx, id, node| {
        node.set_role(Role::Video);
        node.add_action(Action::Click);
    },
    /// It is interpreted as a Webview2.
    AWebview2 => |cx, id, node| {
        node.set_role(Role::WebView);
    },
    /// It is interpreted as a Link.
    ALink => |cx, id, node| {
        node.set_role(Role::Link);
        node.add_action(Action::Click);
    },
}
