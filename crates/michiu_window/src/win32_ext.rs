use windows::Win32::{
    Foundation::{LPARAM, POINT, WPARAM},
    UI::Input::KeyboardAndMouse::VIRTUAL_KEY,
};

pub trait LParamExt {
    /// Retrieve the lower 16 bits (LOWORD)
    fn loword(self) -> u16;
    /// Retrieve the upper 16 bits (HIWORD)
    fn hiword(self) -> u16;
    /// Retrieve the lower 32 bits as a `u32`
    fn to_u32(self) -> u32;
    /// Retrieve signed (x, y) coordinates as (`i32`, `i32`)
    fn to_point_i32(self) -> (i32, i32);
    /// Retrieve signed (x, y) coordinates as (`i16`, `i16`)
    fn to_point_i16(self) -> (i16, i16);
    /// Retrieve floating-point coordinates (x, y) as (`f32`, `f32`)
    fn to_point_f32(self) -> (f32, f32);
    /// Retrieve unsigned (width, height) as (`i32`, `i32`)
    fn to_size_i32(self) -> (i32, i32);
    /// Retrieve directly as a Win32 `POINT`
    fn to_point(self) -> POINT;
    /// Retrieve the lower 16 bits as a `u32`
    fn loword_u32(self) -> u32;
    /// Retrieves a reference (`&T`) to the structure pointed to by `LPARAM` (`None` if null)
    /// # Safety
    /// The caller must ensure that `LPARAM` is an aligned pointer to a valid `T`
    unsafe fn to_ref<'a, T>(self) -> Option<&'a T>;
    /// Retrieves a mutable reference (`&mut T`) to the structure pointed to by `LPARAM` (`None` if null)
    /// # Safety
    /// The caller must ensure that `LPARAM` is an aligned mutable pointer to a valid `T`
    #[expect(clippy::wrong_self_convention)]
    unsafe fn to_mut<'a, T>(self) -> Option<&'a mut T>;
    /// Retrieve `LPARAM` as a safe raw pointer (`*mut T`)
    fn to_mut_ptr<T>(self) -> *mut T;
    /// Reclaims the `Box<T>` stored in `LPARAM` (returns `None` if `null`)
    /// # Safety
    /// The caller must ensure that `LPARAM` is a valid pointer created using `Box::into_raw`.
    unsafe fn take_box<T>(self) -> Option<Box<T>>;
}

impl LParamExt for LPARAM {
    #[inline]
    fn loword(self) -> u16 {
        let b = self.0.to_le_bytes();
        u16::from_le_bytes([b[0], b[1]])
    }

    #[inline]
    fn hiword(self) -> u16 {
        let b = self.0.to_le_bytes();
        u16::from_le_bytes([b[2], b[3]])
    }

    #[inline]
    fn to_u32(self) -> u32 {
        let b = self.0.to_le_bytes();
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }

    #[inline]
    fn to_point_i16(self) -> (i16, i16) {
        let b = self.0.to_le_bytes();
        (
            i16::from_le_bytes([b[0], b[1]]),
            i16::from_le_bytes([b[2], b[3]]),
        )
    }

    #[inline]
    fn to_point_i32(self) -> (i32, i32) {
        let (x, y) = self.to_point_i16();
        (i32::from(x), i32::from(y))
    }

    #[inline]
    fn to_point_f32(self) -> (f32, f32) {
        let (x, y) = self.to_point_i16();
        (f32::from(x), f32::from(y))
    }

    #[inline]
    fn to_size_i32(self) -> (i32, i32) {
        (i32::from(self.loword()), i32::from(self.hiword()))
    }

    #[inline]
    fn to_point(self) -> POINT {
        let (x, y) = self.to_point_i32();
        POINT { x, y }
    }

    #[inline]
    fn loword_u32(self) -> u32 {
        u32::from(self.loword())
    }

    #[inline]
    #[expect(clippy::as_conversions)]
    unsafe fn to_ref<'a, T>(self) -> Option<&'a T> {
        let ptr = self.0 as *const T;
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { &*ptr })
        }
    }

    #[inline]
    unsafe fn to_mut<'a, T>(self) -> Option<&'a mut T> {
        let ptr = self.to_mut_ptr::<T>();
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { &mut *ptr })
        }
    }

    #[inline]
    #[expect(clippy::as_conversions)]
    fn to_mut_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }

    #[inline]
    unsafe fn take_box<T>(self) -> Option<Box<T>> {
        let ptr = self.to_mut_ptr::<T>();
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { Box::from_raw(ptr) })
        }
    }
}

pub trait WParamExt {
    /// Retrieve the lower 16 bits (LOWORD)
    fn loword(self) -> u16;
    /// Retrieve the upper 16 bits (HIWORD)
    fn hiword(self) -> u16;
    /// Retrieve the upper 16 bits as a signed `i16`
    fn hiword_signed(self) -> i16;
    /// Get the mouse wheel delta value as an `i32`
    fn to_wheel_delta(self) -> i32;
    /// Retrieve the lower 16 bits as a signed `i32` (key codes, commands, etc.)
    fn to_loword_i32(self) -> i32;
    /// Retrieve the lower 16 bits as a `u32` (tray menu ID, etc.)
    fn to_loword_u32(self) -> u32;
    /// Get the Virtual Key Code
    fn to_virtual_key(self) -> VIRTUAL_KEY;
    /// Retrieve a `char` from a UTF-32 / UTF-16 code point
    fn to_char(self) -> Option<char>;
    /// Get the DPI value
    fn to_dpi(self) -> u32;
    /// Get the scale factor
    fn to_scale(self) -> f64;
    /// Retrieve DPI Values and Scale Factors in Bulk
    fn to_dpi_and_scale(self) -> (u32, f64);
    /// Retrieve the lower 32 bits as a `u32`
    fn to_u32(self) -> u32;
}

impl WParamExt for WPARAM {
    #[inline]
    fn loword(self) -> u16 {
        let b = self.0.to_le_bytes();
        u16::from_le_bytes([b[0], b[1]])
    }

    #[inline]
    fn hiword(self) -> u16 {
        let b = self.0.to_le_bytes();
        u16::from_le_bytes([b[2], b[3]])
    }

    #[inline]
    fn hiword_signed(self) -> i16 {
        let b = self.0.to_le_bytes();
        i16::from_le_bytes([b[2], b[3]])
    }

    #[inline]
    fn to_wheel_delta(self) -> i32 {
        i32::from(self.hiword_signed())
    }

    #[inline]
    fn to_loword_i32(self) -> i32 {
        i32::from(self.loword())
    }

    #[inline]
    fn to_loword_u32(self) -> u32 {
        u32::from(self.loword())
    }

    #[inline]
    fn to_virtual_key(self) -> VIRTUAL_KEY {
        VIRTUAL_KEY(self.loword())
    }

    #[inline]
    fn to_char(self) -> Option<char> {
        u32::try_from(self.0).ok().and_then(char::from_u32)
    }

    #[inline]
    fn to_dpi(self) -> u32 {
        u32::from(self.loword())
    }

    #[inline]
    fn to_scale(self) -> f64 {
        let dpi = self.to_dpi();
        f64::from(dpi) / 96.0
    }

    #[inline]
    fn to_dpi_and_scale(self) -> (u32, f64) {
        let dpi = self.to_dpi();
        let scale = f64::from(dpi) / 96.0;
        (dpi, scale)
    }

    #[inline]
    fn to_u32(self) -> u32 {
        let b = self.0.to_le_bytes();
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }
}
