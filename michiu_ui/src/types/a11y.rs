use crate::{Context, MichiuTagRegistry};
use accesskit::{Action, Role};

pub(crate) type InferenceFn = fn(cx: &Context, &mut accesskit::Node);

/// `A11yInferenceTag` for accessibility.
///
/// Overrides the library's inference.
///
/// [`accesskit`] is required.
pub trait A11yInferenceTag {
    /// Configure accessibility information inference.
    fn inference(cx: &Context, node: &mut accesskit::Node);
}

#[macro_export]
macro_rules! define_inference_tags {
    (
        $(
            $(#[$meta:meta])*
            $name:ident => |$cx:ident, $node:ident| $body:block
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
                fn inference($cx: &Context, $node: &mut ::accesskit::Node) $body
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
    AButton => |cx, node| {
        node.set_role(Role::Button);
        node.add_action(Action::Click);
    },
    /// Represents some kind of Label.
    ALabel => |cx, node| {
        node.set_role(Role::Label);
    },
    /// Represents some kind of Group.
    AContainer => |cx, node| {
        node.set_role(Role::GenericContainer);
    },
    /// Represents some kind of Text Input.
    ATextInput => |cx, node| {
        node.set_role(Role::TextInput);
    },
    /// Represents some kind of Data Input.
    ADataInput => |cx, node| {
        node.set_role(Role::DateInput);
    },
    /// It is interpreted as a Tooltip.
    ATooltip => |cx, node| {
        node.set_role(Role::Tooltip);
    },
    /// It is interpreted as a List.
    AList => |cx, node| {
        node.set_role(Role::List);
    },
    /// It is interpreted as a List Item.
    AListItem => |cx, node| {
        node.set_role(Role::ListItem);
    },
    /// It is interpreted as a Tab.
    ATab => |cx, node| {
        node.set_role(Role::Tab);
    },
    /// It is interpreted as a Tab List.
    ATabList => |cx, node| {
        node.set_role(Role::TabList);
    },
    /// It is interpreted as a Tab Panel.
    ATabPanel => |cx, node| {
        node.set_role(Role::TabPanel);
    },
    /// It is interpreted as a Menu.
    AMenu => |cx, node| {
        node.set_role(Role::Menu);
    },
    /// It is interpreted as a Menu Item.
    AMenuItem => |cx, node| {
        node.set_role(Role::MenuItem);
    },
    /// It is interpreted as a Toggle Button.
    AToggleButton => |cx, node| {
        node.set_role(Role::Button);
        node.add_action(Action::Click);
    },
    /// It is interpreted as a Switch.
    ASwitch => |cx, node| {
        node.set_role(Role::Button);
        node.add_action(Action::Click);
    },
    /// It is interpreted as a Alert.
    AAlert => |cx, node| {
        node.set_role(Role::Alert);
    },
    /// It is interpreted as a Status.
    AStatus => |cx, node| {
        node.set_role(Role::Status);
    },
    /// It is interpreted as a Progress.
    AProgressBar => |cx, node| {
        node.set_role(Role::ProgressIndicator);
    },
    /// It is interpreted as a Slider.
    ASlider => |cx, node| {
        node.set_role(Role::Slider);
    },
    /// It is interpreted as a Scrollbar.
    AScrollbar => |cx, node| {
        node.set_role(Role::ScrollBar);
    },
    /// It is interpreted as a Main.
    AMain => |cx, node| {
        node.set_role(Role::Main);
    },
    /// It is interpreted as a Navigation.
    ANavigation => |cx, node| {
        node.set_role(Role::Navigation);
    },
    /// It is interpreted as a Header.
    AHeader => |cx, node| {
        node.set_role(Role::Header);
    },
    /// It is interpreted as a Footer.
    AFooter => |cx, node| {
        node.set_role(Role::Footer);
    },
    /// It is interpreted as a Banner.
    ABanner => |cx, node| {
        node.set_role(Role::Banner);
    },
    /// It is interpreted as a Sidebar.
    ASidebar => |cx, node| {
        node.set_role(Role::Complementary);
    },
    /// It is interpreted as a Complementary.
    AComplementary => |cx, node| {
        node.set_role(Role::Complementary);
    },
    /// It is interpreted as a Serch.
    ASearch => |cx, node| {
        node.set_role(Role::Search);
    },
    /// It is interpreted as a Dialog.
    ADialog => |cx, node| {
        node.set_role(Role::Dialog);
    },
    /// It is interpreted as a Heading 1.
    AHeading1 => |cx, node| {
        node.set_role(Role::Heading);
    },
    /// It is interpreted as a Heading 2.
    AHeading2 => |cx, node| {
        node.set_role(Role::Heading);
    },
    /// It is interpreted as a Heading 3.
    AHeading3 => |cx, node| {
        node.set_role(Role::Heading);
    },
    /// It is interpreted as a Heading 4.
    AHeading4 => |cx, node| {
        node.set_role(Role::Heading);
    },
    /// It is interpreted as a Heading 5.
    AHeading5 => |cx, node| {
        node.set_role(Role::Heading);
    },
    /// It is interpreted as a Heading 6.
    AHeading6 => |cx, node| {
        node.set_role(Role::Heading);
    },
    /// It is interpreted as a Image.
    AImage => |cx, node| {
        node.set_role(Role::Image);
    },
    /// It is interpreted as a Image.
    AIcon => |cx, node| {
        node.set_role(Role::Image);
    },
    /// It is interpreted as a Media.
    AMedia => |cx, node| {
        node.set_role(Role::Video);
    },
    /// It is interpreted as a Webview2.
    AWebview2 => |cx, node| {
        node.set_role(Role::WebView);
    },
    /// It is interpreted as a Link.
    ALink => |cx, node| {
        node.set_role(Role::Link);
    },
}
