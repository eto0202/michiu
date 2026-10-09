use crate::{ComponentMask, Context, Element, MichiuTagRegistry};
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
    // ボタン系
    /// Represents some kind of Button.
    AButton => |_cx, _id, node| {
        node.set_role(Role::Button);
        node.add_action(Action::Click);
    },
    /// It is interpreted as a Close Button.
    ACloseButton => |_cx, _id, node| {
        node.set_role(Role::Button);
        node.add_action(Action::Click);
        if node.label().is_none() {
            node.set_label("Close");
        }
    },
    /// It is interpreted as a Submit Button.
    ASubmitButton => |_cx, _id, node| {
        node.set_role(Role::Button);
        node.add_action(Action::Click);
        if node.label().is_none() {
            node.set_label("Submit");
        }
    },
    /// It is interpreted as a Toggle Button.
    AToggleButton => |_cx, _id, node| {
        node.set_role(Role::Button);
        node.add_action(Action::Click);
    },
    /// It is interpreted as a Switch.
    ASwitch => |_cx, _id, node| {
        node.set_role(Role::Switch);
        node.add_action(Action::Click);
    },
    /// It is interpreted as a Checkbox.
    ACheckBox => |_cx, _id, node| {
        node.set_role(Role::CheckBox);
        node.add_action(Action::Click);
    },
    /// It is interpreted as a Radio Button.
    ARadioButton => |_cx, _id, node| {
        node.set_role(Role::RadioButton);
        node.add_action(Action::Click);
    },
    /// It is interpreted as a Radio Group container.
    ARadioGroup => |_cx, _id, node| {
        node.set_role(Role::RadioGroup);
    },

    // 入力系
    /// Represents some kind of Text Input.
    ATextInput => |_cx, _id, node| {
        node.set_role(Role::TextInput);
        node.add_action(Action::Click);
        node.add_action(Action::SetValue);
    },
    /// Represents some kind of Search Input.
    ASearchInput => |_cx, _id, node| {
        node.set_role(Role::SearchInput);
        node.add_action(Action::Click);
        node.add_action(Action::SetValue);
        if node.label().is_none() {
            node.set_label("Search");
        }
    },
    /// Represents some kind of Date Input.
    ADateInput => |cx, id, node| {
        node.set_role(Role::DateInput);
        node.add_action(Action::Click);
        node.add_action(Action::SetValue);
    },
    /// Represents a Spin Button (numeric stepper input).
    ASpinButton => |_cx, _id, node| {
        node.set_role(Role::SpinButton);
        node.add_action(Action::SetValue);
        node.add_action(Action::Increment);
        node.add_action(Action::Decrement);
    },
    /// Represents some kind of Slider.
    ASlider => |_cx, _id, node| {
        node.set_role(Role::Slider);
        node.add_action(Action::Click);
        node.add_action(Action::SetValue);
        node.add_action(Action::Increment);
        node.add_action(Action::Decrement);
    },

    // コンボボックス・選択リスト
    /// Represents a `ComboBox` (Select dropdown).
    AComboBox => |_cx, _id, node| {
        node.set_role(Role::ComboBox);
        node.add_action(Action::Click);
        node.add_action(Action::Expand);
        node.add_action(Action::Collapse);
    },
    /// Represents a `ListBox` container.
    AListBox => |_cx, _id, node| {
        node.set_role(Role::ListBox);
    },
    /// Represents an Option within a `ListBox`.
    AListBoxOption => |_cx, _id, node| {
        node.set_role(Role::ListBoxOption);
    },

    // ナビゲーション・階層構造
    /// Represents a Tree container.
    ATree => |_cx, _id, node| {
        node.set_role(Role::Tree);
    },
    /// Represents an item within a Tree.
    ATreeItem => |_cx, _id, node| {
        node.set_role(Role::TreeItem);
        node.add_action(Action::Click);
        node.add_action(Action::Expand);
        node.add_action(Action::Collapse);
    },
    /// Represents some kind of Tab.
    ATab => |_cx, _id, node| {
        node.set_role(Role::Tab);
        node.add_action(Action::Click);
    },
    /// Represents some kind of Tab List.
    ATabList => |_cx, _id, node| {
        node.set_role(Role::TabList);
    },
    /// Represents some kind of Tab Panel.
    ATabPanel => |_cx, _id, node| {
        node.set_role(Role::TabPanel);
    },
    /// Represents some kind of Menu.
    AMenu => |_cx, _id, node| {
        node.set_role(Role::Menu);
    },
    /// Represents some kind of Menu Item.
    AMenuItem => |_cx, _id, node| {
        node.set_role(Role::MenuItem);
    },
    /// Represents some kind of Menu Bar.
    AMenuBar => |_cx, _id, node| {
        node.set_role(Role::MenuBar);
    },
    /// Represents some kind of Context Menu.
    AContextMenu => |_cx, _id, node| {
        node.set_role(Role::Menu);
        node.add_action(Action::ShowContextMenu);
    },

    // 見出し
    /// Represents a Heading Level 1.
    AHeading1 => |_cx, _id, node| {
        node.set_role(Role::Heading);
        node.set_level(1);
    },
    /// Represents a Heading Level 2.
    AHeading2 => |_cx, _id, node| {
        node.set_role(Role::Heading);
        node.set_level(2);
    },
    /// Represents a Heading Level 3.
    AHeading3 => |_cx, _id, node| {
        node.set_level(3);
        node.set_role(Role::Heading);
    },

    // その他
    /// Represents some kind of Label.
    ALabel => |_cx, _id, node| {
        node.set_role(Role::Label);
    },
    /// Represents some kind of Group/Container.
    AContainer => |_cx, _id, node| {
        node.set_role(Role::GenericContainer);
    },
    /// Represents a Tooltip.
    ATooltip => |_cx, _id, node| {
        node.set_role(Role::Tooltip);
    },
    /// Represents a Separator / Divider line.
    ASeparator => |_cx, _id, node| {
        node.set_role(Role::Splitter);
    },
    /// It is interpreted as a Scrollbar.
    AScrollbar => |_cx, _id, node| {
        node.set_role(Role::ScrollBar);
    },
    /// It is interpreted as an Alert.
    AAlert => |_cx, _id, node| {
        node.set_role(Role::Alert);
        node.set_live(accesskit::Live::Assertive);
    },
    /// It is interpreted as a Status.
    AStatus => |_cx, _id, node| {
        node.set_role(Role::Status);
        node.set_live(accesskit::Live::Polite);
    },
    /// It is interpreted as a Progress Bar.
    AProgressBar => |_cx, _id, node| {
        node.set_role(Role::ProgressIndicator);
    },
    /// It is interpreted as a Dialog.
    ADialog => |_cx, _id, node| {
        node.set_role(Role::Dialog);
    },
    /// It is interpreted as an Image.
    AImage => |_cx, _id, node| {
        node.set_role(Role::Image);
    },
    /// It is interpreted as an Icon.
    AIcon => |_cx, _id, node| {
        node.set_role(Role::Image);
    },
    /// It is interpreted as a Link.
    ALink => |_cx, _id, node| {
        node.set_role(Role::Link);
        node.add_action(Action::Click);
    },
    /// It is interpreted as a Webview.
    AWebView => |_cx, _id, node| {
        node.set_role(Role::WebView);
    },
}
