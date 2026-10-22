use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
        GWL_STYLE, GWLP_USERDATA, GetWindowLongPtrW, SetWindowLongPtrW, WINDOW_EX_STYLE,
        WINDOW_STYLE,
    },
};

/// 型 T のバイトサイズを u32 で取得する
/// 構造体のサイズが 4GB を超えることはほぼあり得ないためキャストを集約
#[inline]
#[expect(clippy::as_conversions, clippy::cast_possible_truncation)]
pub(crate) const fn size_of_u32<T>() -> u32 {
    std::mem::size_of::<T>() as u32
}

/// 変数（値）のバイトサイズを u32 で取得する
#[inline]
#[expect(clippy::as_conversions, clippy::cast_possible_truncation, unused)]
pub(crate) fn size_of_val_u32<T: ?Sized>(val: &T) -> u32 {
    std::mem::size_of_val(val) as u32
}

#[inline]
#[expect(clippy::as_conversions, clippy::cast_possible_truncation)]
pub(crate) fn round_to_i32(val: f64) -> i32 {
    val.round() as i32
}

/// isize の下位32ビットを安全に u32 として取り出す
#[inline]
pub(crate) fn isize_to_u32(val: isize) -> u32 {
    let b = val.to_le_bytes();
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// u32 をビットパターンを維持したまま isize に変換する
#[inline]
#[expect(clippy::expect_used)]
pub(crate) fn u32_to_isize(val: u32) -> isize {
    let usize_val = usize::try_from(val).expect("Windows is at least 32-bit");
    isize::from_ne_bytes(usize_val.to_ne_bytes())
}

#[inline]
pub(crate) fn get_window_style(hwnd: HWND) -> WINDOW_STYLE {
    let val = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
    WINDOW_STYLE(isize_to_u32(val))
}

#[inline]
pub(crate) fn set_window_style(hwnd: HWND, style: WINDOW_STYLE) {
    let _ = unsafe { SetWindowLongPtrW(hwnd, GWL_STYLE, u32_to_isize(style.0)) };
}

#[inline]
pub(crate) fn get_window_ex_style(hwnd: HWND) -> WINDOW_EX_STYLE {
    let val = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
    WINDOW_EX_STYLE(isize_to_u32(val))
}

#[inline]
pub(crate) fn set_window_ex_style(hwnd: HWND, ex_style: WINDOW_EX_STYLE) {
    let _ = unsafe { SetWindowLongPtrW(hwnd, GWL_STYLE, u32_to_isize(ex_style.0)) };
}

/// `GWLP_USERDATA` から読み取り専用の参照 (&T) を取得する
#[inline]
#[allow(clippy::as_conversions)]
pub(crate) fn get_window_userdata<'a, T>(hwnd: HWND) -> Option<&'a T> {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const T };
    if ptr.is_null() {
        None
    } else {
        unsafe { Some(&*ptr) }
    }
}

/// `GWLP_USERDATA` からポインタを取り出して &mut T に安全に変換する
#[inline]
#[allow(clippy::as_conversions)]
pub(crate) fn get_window_userdata_mut<'a, T>(hwnd: HWND) -> Option<&'a mut T> {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut T };
    if ptr.is_null() {
        None
    } else {
        unsafe { Some(&mut *ptr) }
    }
}

/// `GWLP_USERDATA` を 0 にリセットし、保持されていた Box<T> を取り出して返す
#[inline]
#[allow(clippy::as_conversions)]
pub(crate) unsafe fn take_window_userdata<T>(hwnd: HWND) -> Option<Box<T>> {
    // 0 をセットしつつ、元のポインタを取得する
    let old_val = unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
    let old_ptr = old_val as *mut T;

    if old_ptr.is_null() {
        None
    } else {
        Some(unsafe { Box::from_raw(old_ptr) })
    }
}
