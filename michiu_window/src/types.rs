/// Describes the state of a physical input element (e.g., Keyboard Key, Mouse Button).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementState {
    /// The element is currently pressed down.
    Pressed,
    /// The element has been released.
    Released,
}

/// Represents the combined active state of the keyboard modifier keys (Shift, Control, Alt, Win).
///
/// Implemented as a bitmask structure with simple bitwise-like operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers(pub u32);

impl Modifiers {
    pub const SHIFT: Self = Self(1 << 0);
    pub const CONTROL: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);
    pub const LOGO: Self = Self(1 << 3);

    /// Creates an empty modifier state.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Inserts a specific modifier key flag into the active state.
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Checks whether a specific modifier key flag is currently held down.
    ///
    /// # Examples
    ///
    /// ```
    /// # use michiu_window::Modifiers;
    /// let mut mods = Modifiers::empty();
    /// mods.insert(Modifiers::SHIFT);
    /// mods.insert(Modifiers::CONTROL);
    ///
    /// assert!(mods.contains(Modifiers::SHIFT));
    /// assert!(mods.contains(Modifiers::CONTROL));
    /// assert!(!mods.contains(Modifiers::ALT));
    /// ```
    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// Identifies a specific mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// The left mouse button.
    Left,
    /// The right mouse button.
    Right,
    /// The middle wheel mouse button.
    Middle,
    /// Extra mouse side-buttons (e.g., X1, X2, or others mapping to Win32 parameters).
    Other(u16),
}

/// Represents standard system mouse cursor shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorIcon {
    /// The standard arrow cursor.
    #[default]
    Default,
    /// The hand cursor, typically used for links and clickable elements.
    Hand,
    /// The text I-beam cursor, used for hover on editable text regions.
    IBeam,
    /// The waiting hourglass/spinning cursor, indicating background tasks.
    Wait,
    /// The crosshair cursor, used for drawing or precise click locations.
    Cross,
}

/// A point coordinate defined in physical screen pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PhysicalPoint {
    pub x: i32,
    pub y: i32,
}

impl PhysicalPoint {
    /// Creates `PhysicalPoint` instance.
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Converts this physical coordinate point to a logical point based on the specified DPI scaling factor.
    pub fn to_logical(&self, scale_factor: f64) -> LogicalPoint {
        LogicalPoint {
            x: self.x as f64 / scale_factor,
            y: self.y as f64 / scale_factor,
        }
    }
}

/// A window size dimension defined in physical screen pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PhysicalSize {
    pub width: i32,
    pub height: i32,
}

impl PhysicalSize {
    /// Creates `PhysicalSize` instance.
    pub fn new(width: i32, height: i32) -> Self {
        Self { width, height }
    }
    /// Converts this physical size to a logical size based on the specified DPI scaling factor.
    pub fn to_logical(&self, scale_factor: f64) -> LogicalSize {
        LogicalSize {
            width: self.width as f64 / scale_factor,
            height: self.height as f64 / scale_factor,
        }
    }
}

/// A rectangle bounds representation defined in physical screen pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PhysicalRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl PhysicalRect {
    /// Creates `PhysicalRect` instance.
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
    /// Converts this physical rect to a logical rect based on the specified DPI scaling factor.
    pub fn to_logical(&self, scale_factor: f64) -> LogicalRect {
        LogicalRect {
            left: self.left as f64 / scale_factor,
            top: self.top as f64 / scale_factor,
            right: self.right as f64 / scale_factor,
            bottom: self.bottom as f64 / scale_factor,
        }
    }
}

/// A point coordinate defined in logical screen pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LogicalPoint {
    pub x: f64,
    pub y: f64,
}

impl LogicalPoint {
    /// Creates `LogicalPoint` instance.
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    /// Converts this logical coordinate point to a physical point based on the specified DPI scaling factor,
    /// applying standard round-to-nearest rounding.
    pub fn to_physical(&self, scale_factor: f64) -> PhysicalPoint {
        PhysicalPoint {
            x: (self.x * scale_factor).round() as i32,
            y: (self.y * scale_factor).round() as i32,
        }
    }
}

/// A window size dimension defined in logical screen pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LogicalSize {
    pub width: f64,
    pub height: f64,
}

impl LogicalSize {
    /// Creates `LogicalSize` instance.
    pub fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
    /// Converts this logical size to a physical size, applying standard round-to-nearest rounding.
    pub fn to_physical(&self, scale_factor: f64) -> PhysicalSize {
        PhysicalSize {
            width: (self.width * scale_factor).round() as i32,
            height: (self.height * scale_factor).round() as i32,
        }
    }
}

/// A rectangle bounds representation defined in logical screen pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LogicalRect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl LogicalRect {
    /// Creates `LogicalRect` instance.
    pub fn new(left: f64, top: f64, right: f64, bottom: f64) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
    /// Converts this logical rect to a physical rect, applying standard round-to-nearest rounding.
    pub fn to_physical(&self, scale_factor: f64) -> PhysicalRect {
        PhysicalRect {
            left: (self.left * scale_factor).round() as i32,
            top: (self.top * scale_factor).round() as i32,
            right: (self.right * scale_factor).round() as i32,
            bottom: (self.bottom * scale_factor).round() as i32,
        }
    }
}

/// Specifies the preferred application visual mode for standard Win32 menus and titles.
#[repr(i32)]
pub enum PreferredAppMode {
    /// Follows the current default Windows system personalization settings.
    Default = 0,
    /// Permits standard context menus to transition to dark mode if supported by the OS.
    AllowDark = 1,
    /// Forces standard menus and window elements to native dark mode styling.
    ForceDark = 2,
    /// Forces standard menus and window elements to light mode styling.
    ForceLight = 3,
    Max = 4,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modifiers_operations() {
        let mut mods = Modifiers::empty();
        assert!(!mods.contains(Modifiers::SHIFT));

        // SHIFTフラグの挿入
        mods.insert(Modifiers::SHIFT);
        assert!(mods.contains(Modifiers::SHIFT));
        assert!(!mods.contains(Modifiers::CONTROL));

        // CONTROLフラグの挿入（複数フラグの保持）
        mods.insert(Modifiers::CONTROL);
        assert!(mods.contains(Modifiers::SHIFT));
        assert!(mods.contains(Modifiers::CONTROL));

        // LOGOキー
        mods.insert(Modifiers::LOGO);
        assert!(mods.contains(Modifiers::LOGO));
    }

    #[test]
    fn test_coordinate_conversions_physical_to_logical() {
        // スケールファクターが 150% (1.5) の場合
        let scale_factor = 1.5;

        // Point
        let phys_pt = PhysicalPoint::new(15, 30);
        let log_pt = phys_pt.to_logical(scale_factor);
        assert_eq!(log_pt.x, 10.0);
        assert_eq!(log_pt.y, 20.0);

        // Size
        let phys_size = PhysicalSize::new(1920, 1080);
        let log_size = phys_size.to_logical(2.0); // 200% scaling
        assert_eq!(log_size.width, 960.0);
        assert_eq!(log_size.height, 540.0);

        // Rect
        let phys_rect = PhysicalRect::new(0, 0, 150, 300);
        let log_rect = phys_rect.to_logical(1.5);
        assert_eq!(log_rect.left, 0.0);
        assert_eq!(log_rect.top, 0.0);
        assert_eq!(log_rect.right, 100.0);
        assert_eq!(log_rect.bottom, 200.0);
    }

    #[test]
    fn test_coordinate_conversions_logical_to_physical_rounding() {
        // スケールファクターが 150% (1.5) の場合
        let scale_factor = 1.5;

        // 小数点以下の計算結果が四捨五入 (round) されるかを検証
        // 10.4 * 1.5 = 15.6 -> round -> 16
        // 10.2 * 1.5 = 15.3 -> round -> 15
        let log_pt = LogicalPoint::new(10.4, 10.2);
        let phys_pt = log_pt.to_physical(scale_factor);

        assert_eq!(phys_pt.x, 16);
        assert_eq!(phys_pt.y, 15);

        // Size
        let log_size = LogicalSize::new(100.5, 200.5);
        let phys_size = log_size.to_physical(2.0);
        assert_eq!(phys_size.width, 201);
        assert_eq!(phys_size.height, 401);
    }
}
