use crate::error::{MichiuError, Result};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::Arc;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResourceEx, DestroyIcon, HICON, IMAGE_ICON, LR_DEFAULTCOLOR, LR_DEFAULTSIZE,
    LR_LOADFROMFILE, LoadImageW, LookupIconIdFromDirectoryEx,
};
use windows::core::PCWSTR;

/// Loads an icon from the specified image file path.
///
/// Under the hood, this converts the path to a wide string and invokes `LoadImageW`
/// with `LR_LOADFROMFILE | LR_DEFAULTSIZE`.
///
/// # Errors
/// Returns [`MichiuError::ResourceLoadFailed`] if the file does not exist, is inaccessible,
/// or if the OS fails to decode the image format.
///
/// # Examples
///
/// ```no_run
/// use michiu_window::Icon;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let icon = Icon::from_path("assets/app_icon.ico")?;
///     Ok(())
/// }
/// ```
#[derive(Clone, Debug)]
pub struct Icon {
    inner: Arc<IconInner>,
}

#[derive(Debug)]
struct IconInner {
    hicon: HICON,
    is_owned: bool,
}

unsafe impl Send for IconInner {}
unsafe impl Sync for IconInner {}

impl Drop for IconInner {
    fn drop(&mut self) {
        if self.is_owned {
            unsafe {
                let _ = DestroyIcon(self.hicon);
            }
        }
    }
}

impl Icon {
    /// Loads an icon from the specified image file path.
    ///
    /// Under the hood, this converts the path to a wide string and invokes `LoadImageW`
    /// with `LR_LOADFROMFILE | LR_DEFAULTSIZE`.
    ///
    /// # Errors
    /// Returns [`MichiuError::ResourceLoadFailed`] if the file does not exist, is inaccessible,
    /// or if the OS fails to decode the image format.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use michiu_window::Icon;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let icon = Icon::from_path("assets/app_icon.ico")?;
    ///     Ok(())
    /// }
    /// ```
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path_wide: Vec<u16> = path
            .as_ref()
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let handle = unsafe {
            LoadImageW(
                None,
                PCWSTR(path_wide.as_ptr()),
                IMAGE_ICON,
                0, // デフォルトのシステム幅
                0, // デフォルトのシステム高さ
                LR_LOADFROMFILE | LR_DEFAULTSIZE,
            )
        }
        .map_err(|err| MichiuError::ResourceLoadFailed {
            path: path.as_ref().to_string_lossy().into_owned().into(),
            source: err,
        })?;

        let hicon = HICON(handle.0);

        Ok(Self {
            inner: Arc::new(IconInner {
                hicon,
                is_owned: true,
            }),
        })
    }

    /// Decodes and generates an icon directly from the byte array of an ICO image file in memory.
    ///
    /// This method automatically parses the ICO directory header in memory using `LookupIconIdFromDirectoryEx`
    /// to locate the optimal icon size matching the current system display context, and then instantiates
    /// the icon using `CreateIconFromResourceEx`.
    ///
    /// # Errors
    /// Returns [`MichiuError::ValidationError`] if the byte array is empty, or if parsing the image directory
    /// header fails due to malformed data.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use michiu_window::Icon;
    /// # const ICO_BYTES: &[u8] = &[
    /// #     0, 0, 1, 0, 1, 0, 16, 16, 2, 0, 1, 0, 1, 0, 176, 0, 0, 0, 22, 0, 0, 0,
    /// #     40, 0, 0, 0, 16, 0, 0, 0, 32, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0
    /// # ];
    /// // In your real application, you can load the ICO file directly at compile-time:
    ///
    /// // Load the raw binary bytes of an ICO file at compile-time
    /// // const ICO_BYTES: &[u8] = include_bytes!("../assets/app_icon.ico");
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let icon = Icon::from_bytes(ICO_BYTES)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            return Err(MichiuError::ValidationError {
                parameter: "bytes",
                message: "Icon byte array cannot be empty.".into(),
            });
        }

        unsafe {
            // メモリ上のファイルから現在のシステムに最適なアイコンサイズのオフセットを探す
            let offset = LookupIconIdFromDirectoryEx(
                bytes.as_ptr(),
                true, // true = Icon (false = Cursor)
                0,
                0,
                LR_DEFAULTCOLOR,
            );

            if offset <= 0 || (offset as usize) >= bytes.len() {
                return Err(MichiuError::ValidationError {
                    parameter: "bytes",
                    message: "Failed to parse image directory header from memory.".into(),
                });
            }

            // 特定された位置のデータから HICON を生成する
            let icon_bits = &bytes[offset as usize..];
            let hicon_raw = CreateIconFromResourceEx(
                icon_bits,
                true,       // true = Icon
                0x00030000, // Windows 3.0 以降の標準バージョン指定
                0,
                0,
                LR_DEFAULTCOLOR,
            )
            .map_err(MichiuError::UnexpectedOsError)?;

            Ok(Self {
                inner: Arc::new(IconInner {
                    hicon: hicon_raw,
                    is_owned: true,
                }),
            })
        }
    }

    /// An escape hatch used to wrap an existing raw Win32 `HICON` handle.
    ///
    /// Useful for integrating with other external Win32/GUI libraries.
    ///
    /// # Safety
    /// The caller must ensure that the provided `hicon` is a valid, active Win32 icon resource.
    /// Note that icons generated this way are considered **non-owned**; they are not automatically
    /// destroyed on drop. The caller is responsible for disposing of the raw `HICON` if necessary.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use michiu_window::Icon;
    /// use windows::Win32::UI::WindowsAndMessaging::{LoadIconW, IDI_APPLICATION};
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // Load a standard built-in system application icon
    ///     let hicon_system = unsafe {
    ///         LoadIconW(None, IDI_APPLICATION).expect("Failed to load system icon")
    ///     };
    ///
    ///     // Safely wrap the system icon as a non-owned Icon wrapper
    ///     let icon = unsafe { Icon::from_raw(hicon_system) };
    ///     Ok(())
    /// }
    /// ```
    pub unsafe fn from_raw(hicon: HICON) -> Self {
        Self {
            inner: Arc::new(IconInner {
                hicon,
                is_owned: false, // 外部所有なので勝手に壊さない
            }),
        }
    }

    /// Retrieves the internal raw HICON (for internal library use)
    pub(crate) fn as_raw(&self) -> HICON {
        self.inner.hicon
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use windows::Win32::UI::WindowsAndMessaging::{IDI_APPLICATION, LoadIconW};

    //  - ICOヘッダ (6 bytes)
    //  - ディレクトリヘッダ (16 bytes)
    //  - DIB (BITMAPINFOHEADER: 40 bytes + パレット: 8 bytes + マスク: 128 bytes = 176 bytes)
    //  - 総ファイルサイズ: 198 bytes
    // -----------------------------------------------------------------
    const MINIMAL_ICO_BYTES: &[u8] = &[
        // ICO Header (6 bytes)
        0, 0, // Reserved (常に 0)
        1, 0, // Type (1 = Icon, 2 = Cursor)
        1, 0, // Image Count (1枚)
        // Icon Directory Entry (16 bytes)
        16, // Width (16 px)
        16, // Height (16 px)
        2,  // Color Count (2色 = モノクロ)
        0,  // Reserved (常に 0)
        1, 0, // Color Planes (1)
        1, 0, // Bits Per Pixel (1 bpp)
        176, 0, 0, 0, // Image Data Size (DIBデータの合計バイト数: 176 bytes)
        22, 0, 0, 0, // Image Data Offset (ヘッダ 6 + エントリ 16 = 22)
        // DIB Data (BITMAPINFOHEADER - 40 bytes)
        40, 0, 0, 0, // biSize (40)
        16, 0, 0, 0, // biWidth (16 px)
        32, 0, 0, 0, // biHeight (XOR 16 + AND 16 = 32 px。Win32仕様で2倍にする)
        1, 0, // biPlanes (1)
        1, 0, // biBitCount (1 bpp)
        0, 0, 0, 0, // biCompression (0 = BI_RGB)
        0, 0, 0, 0, // biSizeImage (0 = 自動計算)
        0, 0, 0, 0, // biXPelsPerMeter
        0, 0, 0, 0, // biYPelsPerMeter
        2, 0, 0, 0, // biClrUsed (2色使用)
        0, 0, 0, 0, // biClrImportant
        // Color Palette (8 bytes - BGRA 2色)
        0, 0, 0, 0, // Color 0: Black
        255, 255, 255, 0, // Color 1: White
        // XOR Mask (16x16 1bpp. 各行16px=2bits。DWORDアラインされて4bytes/行。16行 = 64 bytes)
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
        // AND Mask (16x16 1bpp. XOR同様にDWORDアラインされて4bytes/行。16行 = 64 bytes)
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ];

    #[test]
    fn test_icon_from_bytes_normal() {
        // メモリ上の有効なバイト配列から正しく HICON が生成されるかを検証
        let icon_result = Icon::from_bytes(MINIMAL_ICO_BYTES);
        assert!(
            icon_result.is_ok(),
            "Failed to load icon from minimal bytes: {:?}",
            icon_result.err()
        );

        let icon = icon_result.unwrap();
        // 内部の raw HICON が null でないことを確認
        assert!(!icon.as_raw().is_invalid());
    }

    #[test]
    fn test_icon_from_path_normal() {
        // 有効なバイトデータを一時ファイルに出力してファイルロードの正常系を検証
        let temp_dir = std::env::temp_dir();
        let temp_ico_path = temp_dir.join("michiu_test_minimal_icon.ico");

        fs::write(&temp_ico_path, MINIMAL_ICO_BYTES).expect("Failed to write temp ico file");

        let icon_result = Icon::from_path(&temp_ico_path);

        // クリーンアップはテスト結果を問わず行う
        let _ = fs::remove_file(&temp_ico_path);

        assert!(
            icon_result.is_ok(),
            "Failed to load icon from path: {:?}",
            icon_result.err()
        );
        let icon = icon_result.unwrap();
        assert!(!icon.as_raw().is_invalid());
    }

    #[test]
    fn test_icon_clone() {
        let icon = Icon::from_bytes(MINIMAL_ICO_BYTES).unwrap();

        // クローンによって参照カウントが増え、元のハンドルと同じ値が維持されるか検証
        let icon_cloned = icon.clone();
        assert_eq!(icon.as_raw(), icon_cloned.as_raw());
    }

    #[test]
    fn test_icon_from_raw_escaped() {
        // システムに組み込まれている標準アプリケーションアイコンの HICON を取得
        let hicon_system = unsafe { LoadIconW(None, IDI_APPLICATION) }
            .expect("Failed to load Win32 IDI_APPLICATION icon");

        // 外部（システム）所有の生 HICON から非所有型 Icon を作成
        let icon = unsafe { Icon::from_raw(hicon_system) };
        assert_eq!(icon.as_raw(), hicon_system);

        // クローンし、スコープを抜けてDropされた際に「システム所有のアイコン」が
        // 誤って DestroyIcon されてパニックやリソース破壊を起こさないか確認
        {
            let _cloned = icon.clone();
        } // ここで _cloned が Drop されるが、is_owned == false なので破棄されない
    }

    #[test]
    fn test_icon_from_path_not_found() {
        // 存在しないパスからのロードをテスト
        let fake_path = std::path::Path::new("non_existent_and_fake_icon_file_12345.ico");
        let result = Icon::from_path(fake_path);

        assert!(result.is_err());
        match result.unwrap_err() {
            MichiuError::ResourceLoadFailed { path, .. } => {
                assert!(path.contains("non_existent_and_fake_icon_file_12345.ico"));
            }
            other => panic!("Expected ResourceLoadFailed error, got: {:?}", other),
        }
    }

    #[test]
    fn test_icon_from_bytes_empty() {
        // 空のバイト配列を渡された場合のバリデーションエラーを検証
        let result = Icon::from_bytes(&[]);
        assert!(result.is_err());
        match result.unwrap_err() {
            MichiuError::ValidationError { parameter, message } => {
                assert_eq!(parameter, "bytes");
                assert!(message.contains("cannot be empty"));
            }
            other => panic!("Expected ValidationError error, got: {:?}", other),
        }
    }

    #[test]
    fn test_icon_from_bytes_malformed_header() {
        // 不正な形式を渡された場合のヘッダ解析エラーを検証
        let malformed_bytes = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let result = Icon::from_bytes(&malformed_bytes);

        assert!(result.is_err());
        match result.unwrap_err() {
            MichiuError::ValidationError { parameter, message } => {
                assert_eq!(parameter, "bytes");
                assert!(message.contains("Failed to parse image directory header"));
            }
            other => panic!("Expected ValidationError, got: {:?}", other),
        }
    }
}
