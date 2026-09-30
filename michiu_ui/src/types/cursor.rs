use crate::MichiuError;
use std::sync::Arc;
use windows::Win32::{
    Graphics::Gdi::{
        BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS, DeleteObject,
        GetDC, HGDIOBJ, ReleaseDC,
    },
    UI::WindowsAndMessaging::{CreateIconIndirect, HCURSOR, ICONINFO},
};

/// Global Cursor Types for Propagation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalCursorIcon {
    Default(Option<HCURSOR>),
    Pointer(Option<HCURSOR>),
    Text(Option<HCURSOR>),
    Grab(Option<HCURSOR>),
    Grabbing(Option<HCURSOR>),
    NotAllowed(Option<HCURSOR>),
    ResizeNs(Option<HCURSOR>),
    ResizeEw(Option<HCURSOR>),
    ResizeNesw(Option<HCURSOR>),
    ResizeNwse(Option<HCURSOR>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorIcon {
    Default(Option<HCURSOR>),
    Pointer(Option<HCURSOR>),
    Text(Option<HCURSOR>),
    Grab(Option<HCURSOR>),
    Grabbing(Option<HCURSOR>),
    NotAllowed(Option<HCURSOR>),
    ResizeNs(Option<HCURSOR>),
    ResizeEw(Option<HCURSOR>),
    ResizeNesw(Option<HCURSOR>),
    ResizeNwse(Option<HCURSOR>),
    Global(GlobalCursorIcon),
}

unsafe impl Send for CursorIcon {}
unsafe impl Sync for CursorIcon {}

unsafe impl Send for GlobalCursorIcon {}
unsafe impl Sync for GlobalCursorIcon {}

impl Default for CursorIcon {
    fn default() -> Self {
        CursorIcon::Default(None)
    }
}

impl CursorIcon {
    /// Loads and returns the Windows `HCURSOR` physical handle.
    /// If a custom `HCURSOR` is specified, it takes precedence; otherwise, the OS's default is loaded.
    pub fn to_hcursor(self) -> crate::Result<HCURSOR> {
        use windows::Win32::UI::WindowsAndMessaging::{
            IDC_ARROW, IDC_HAND, IDC_IBEAM, IDC_NO, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS,
            IDC_SIZENWSE, IDC_SIZEWE, LoadCursorW,
        };
        unsafe {
            let idc = match self {
                // 独自カーソル指定時は即座にそのハンドルを返却
                CursorIcon::Default(Some(h))
                | CursorIcon::Pointer(Some(h))
                | CursorIcon::Text(Some(h))
                | CursorIcon::Grab(Some(h))
                | CursorIcon::Grabbing(Some(h))
                | CursorIcon::NotAllowed(Some(h))
                | CursorIcon::ResizeNs(Some(h))
                | CursorIcon::ResizeEw(Some(h))
                | CursorIcon::ResizeNesw(Some(h))
                | CursorIcon::ResizeNwse(Some(h)) => return Ok(h),
                CursorIcon::Global(global_icon) => {
                    match global_icon {
                        GlobalCursorIcon::Default(Some(h))
                        | GlobalCursorIcon::Pointer(Some(h))
                        | GlobalCursorIcon::Text(Some(h))
                        | GlobalCursorIcon::Grab(Some(h))
                        | GlobalCursorIcon::Grabbing(Some(h))
                        | GlobalCursorIcon::NotAllowed(Some(h))
                        | GlobalCursorIcon::ResizeNs(Some(h))
                        | GlobalCursorIcon::ResizeEw(Some(h))
                        | GlobalCursorIcon::ResizeNesw(Some(h))
                        | GlobalCursorIcon::ResizeNwse(Some(h)) => return Ok(h),
                        _ => {}
                    }
                    // None 時は標準システムカーソルにフォールバック
                    match global_icon {
                        GlobalCursorIcon::Default(_) => IDC_ARROW,
                        GlobalCursorIcon::Pointer(_) => IDC_HAND,
                        GlobalCursorIcon::Text(_) => IDC_IBEAM,
                        GlobalCursorIcon::Grab(_) | GlobalCursorIcon::Grabbing(_) => IDC_SIZEALL,
                        GlobalCursorIcon::NotAllowed(_) => IDC_NO,
                        GlobalCursorIcon::ResizeNs(_) => IDC_SIZENS,
                        GlobalCursorIcon::ResizeEw(_) => IDC_SIZEWE,
                        GlobalCursorIcon::ResizeNesw(_) => IDC_SIZENESW,
                        GlobalCursorIcon::ResizeNwse(_) => IDC_SIZENWSE,
                    }
                }

                // 独自カーソル未指定(None)時は、Windows 標準カーソルからロード
                CursorIcon::Default(None) => IDC_ARROW,
                CursorIcon::Pointer(None) => IDC_HAND,
                CursorIcon::Text(None) => IDC_IBEAM,
                CursorIcon::Grab(None) | CursorIcon::Grabbing(None) => IDC_SIZEALL,
                CursorIcon::NotAllowed(None) => IDC_NO,
                CursorIcon::ResizeNs(None) => IDC_SIZENS,
                CursorIcon::ResizeEw(None) => IDC_SIZEWE,
                CursorIcon::ResizeNesw(None) => IDC_SIZENESW,
                CursorIcon::ResizeNwse(None) => IDC_SIZENWSE,
            };

            LoadCursorW(None, idc).map_err(|e| MichiuError::WindowsApiError { source: e })
        }
    }

    /// Generates a custom `HCURSOR` with the specified hotspot coordinates from the RGBA8 pixel data in memory.
    pub fn create_from_rgba(
        rgba_pixels: &[u8],
        width: u32,
        height: u32,
        hotspot_x: u32,
        hotspot_y: u32,
    ) -> crate::Result<HCURSOR> {
        let len = rgba_pixels.len();
        if len != (width * height * 4) as usize {
            return Err(MichiuError::CursorCreationFailed(
                "The pixel buffer size ({len}) does not match the resolution ({width} * {height} * 4)."
                    .into()
            ));
        }

        unsafe {
            let h_dc = GetDC(None);
            if h_dc.is_invalid() {
                return Err(MichiuError::CursorCreationFailed(
                    "Failed to obtain device context (DC).".into(),
                ));
            }

            #[expect(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: 0,
                    ..Default::default()
                },
                ..Default::default()
            };

            let mut pv_bits = std::ptr::null_mut();
            let hbm_color = CreateDIBSection(
                Some(h_dc),
                &raw const bmi,
                DIB_RGB_COLORS,
                &raw mut pv_bits,
                None,
                0,
            )
            .map_err(|e| {
                MichiuError::CursorCreationFailed(format!("Failed to create DIBSection.: {e}"))
            })?;

            if pv_bits.is_null() {
                let _ = ReleaseDC(None, h_dc);
                let _ = DeleteObject(HGDIOBJ(hbm_color.0));
                return Err(MichiuError::CursorCreationFailed(
                    "DIBSection memory allocation failed.".into(),
                ));
            }

            let dest_slice =
                std::slice::from_raw_parts_mut(pv_bits.cast::<u8>(), (width * height * 4) as usize);
            for i in (0..(width * height * 4) as usize).step_by(4) {
                dest_slice[i] = rgba_pixels[i + 2]; // B
                dest_slice[i + 1] = rgba_pixels[i + 1]; // G
                dest_slice[i + 2] = rgba_pixels[i]; // R
                dest_slice[i + 3] = rgba_pixels[i + 3]; // A
            }

            #[expect(clippy::cast_possible_wrap)]
            let hbm_mask = CreateBitmap(width as i32, height as i32, 1, 1, None);

            let icon_info = ICONINFO {
                fIcon: false.into(),
                xHotspot: hotspot_x,
                yHotspot: hotspot_y,
                hbmMask: hbm_mask,
                hbmColor: hbm_color,
            };

            let h_icon = CreateIconIndirect(&raw const icon_info).map_err(|e| {
                MichiuError::CursorCreationFailed(format!("IconIndirect generation failed.: {e}"))
            })?;
            let h_cursor = HCURSOR(h_icon.0);

            let _ = DeleteObject(HGDIOBJ(hbm_color.0));
            let _ = DeleteObject(HGDIOBJ(hbm_mask.0));
            let _ = ReleaseDC(None, h_dc);

            Ok(h_cursor)
        }
    }

    /// Generates a custom `HCURSOR` with the specified hotspot coordinates based on the image file path.
    pub fn create_from_path(
        path: impl AsRef<std::path::Path>,
        hotspot_x: u32,
        hotspot_y: u32,
    ) -> crate::Result<HCURSOR> {
        let p_clone = path.as_ref().to_path_buf();
        let img = image::open(path).map_err(|e| MichiuError::ImageLoadFailed {
            path: p_clone,
            source: Arc::new(e),
        })?;
        let rgba_img = img.to_rgba8();
        let (width, height) = rgba_img.dimensions();

        Self::create_from_rgba(rgba_img.as_raw(), width, height, hotspot_x, hotspot_y)
    }
}
