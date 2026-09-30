#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InteractionState {
    Hovered,
    Focused,
    Pressed,
    Dragged,
}

/// Interaction definitions that can be resolved by propagating from child elements
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InteractionName {
    Hover,
    Focus,
    FocusVisible,
    Press,
    Disable,
    Active,
    Select,
    Drag,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElementState {
    Pressed,
    Released,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub logo: bool,
}

/// Transparency Control for Pointer Events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointerEvents {
    /// Receive pointer events and prevent them from being displayed on the elements below
    #[default]
    Auto,
    /// Ignore pointer events and make the element transparent to the element below it
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UserSelect {
    #[default]
    None,
    Text,
    All,
}
