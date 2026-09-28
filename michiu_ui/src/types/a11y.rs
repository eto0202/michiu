use crate::MichiuTagRegistry;
use accesskit::{Action, Role};

/// Represents some kind of Input.
pub struct AAnyInput;
/// No accessibility information is available.
pub struct ANone;
/// It is interpreted as a Tooltip.
pub struct ATooltip;
/// It is interpreted as a List.
pub struct AList;
/// It is interpreted as a List Item.
pub struct AListItem;
/// It is interpreted as a Tab.
pub struct ATab;
/// It is interpreted as a Tab List.
pub struct ATabList;
/// It is interpreted as a Tab Panel.
pub struct ATabPanel;
/// It is interpreted as a Menu.
pub struct AMenu;
/// It is interpreted as a Menu Item.
pub struct AMenuItem;
/// It is interpreted as a Toggle Button.
pub struct AToggleButton;
/// It is interpreted as a Disclosure.
pub struct ADisclosure;
/// It is interpreted as a Switch.
pub struct ASwitch;
/// It is interpreted as a Alert.
pub struct AAlert;
/// It is interpreted as a Status.
pub struct AStatus;
/// It is interpreted as a Progress.
pub struct AProgressBar;
/// It is interpreted as a Slider.
pub struct ASlider;
/// It is interpreted as a Scrollbar.
pub struct AScrollbar;
/// It is interpreted as a Main.
pub struct AMain;
/// It is interpreted as a Navigation.
pub struct ANavigation;
/// It is interpreted as a Header.
pub struct AHeader;
/// It is interpreted as a Footer.
pub struct AFooter;
/// It is interpreted as a Banner.
pub struct ABanner;
/// It is interpreted as a Sidebar.
pub struct ASidebar;
/// It is interpreted as a Complementary.
pub struct AComplementary;
/// It is interpreted as a Serch.
pub struct ASearch;
/// It is interpreted as a Modal Dialog.
pub struct AModalDialog;
/// It is interpreted as a Dialog.
pub struct ADialog;
/// It is interpreted as a Heading 1.
pub struct AHeading1;
/// It is interpreted as a Heading 2.
pub struct AHeading2;
/// It is interpreted as a Heading 3.
pub struct AHeading3;
/// It is interpreted as a Heading 4.
pub struct AHeading4;
/// It is interpreted as a Heading 5.
pub struct AHeading5;
/// It is interpreted as a Heading 6.
pub struct AHeading6;
/// It is interpreted as a Image.
pub struct AImage;
/// It is interpreted as a Media.
pub struct AMedia;

pub(crate) type InferenceFn = fn(&mut accesskit::Node);

/// `A11yInferenceTag` for accessibility.
///
/// Overrides the library's inference.
///
/// [`accesskit`] is required.
pub trait A11yInferenceTag {
    fn inference(node: &mut accesskit::Node);
}

#[macro_export]
macro_rules! define_inference_tags {
    (
        $(
            $(#[$meta:meta])*
            $name:ident => |$node:ident| $body:block
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
                fn inference($node: &mut ::accesskit::Node) $body
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
    AAnyButton => |node| {
        node.set_role(Role::Button);
        node.add_action(Action::Click);
    },
    /// Represents some kind of Label.
    AAnyLabel => |node| {
        node.set_role(Role::Label);
    },
    /// Represents some kind of Group.
    AAnyGroup => |node| {
        node.set_role(Role::Group);
    }
}
