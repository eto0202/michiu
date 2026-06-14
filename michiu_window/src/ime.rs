use crate::{MichiuError, PhysicalPoint};
#[cfg(any(feature = "serde", docsrs))]
use serde as _;
use std::{
    io::Write,
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        mpsc::{Sender, channel},
    },
};
use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    UI::Input::{
        Ime::{
            CFS_POINT, COMPOSITIONFORM, GCS_COMPSTR, GCS_RESULTSTR, HIMC, IME_COMPOSITION_STRING,
            IME_CONVERSION_MODE, IME_SENTENCE_MODE, ImmGetCompositionStringW,
            ImmGetCompositionWindow, ImmGetContext, ImmGetConversionStatus, ImmGetOpenStatus,
            ImmReleaseContext, ImmSetCompositionWindow, ImmSetConversionStatus, ImmSetOpenStatus,
        },
        KeyboardAndMouse::GetKeyboardLayout,
    },
};

const IMM_ERROR_NODATA: i32 = -1;
const IMM_ERROR_GENERAL: i32 = -2;

/// A safe, thread-affine RAII wrapper managing the Win32 Input Method Context (HIMC).
///
/// Under the hood, this handles safe acquisition ([`ImmGetContext`]) and guaranteed OS release
/// ([`ImmReleaseContext`]) of the IME context. It is strictly thread-affine and should remain
/// on the UI thread of the target window.
///
/// It provides high-level APIs to query and modify the active IME/TSF open status, conversion modes,
/// and composition/result string buffers.
#[derive(Debug)]
pub struct ImeContext {
    hwnd: HWND,
    himc: HIMC,
}

impl ImeContext {
    /// Attempts to acquire the IME context (HIMC) for the specified window handle.
    ///
    /// # Errors
    /// Returns [`MichiuError::ImeContextAcquisitionFailed`] if the window handle is invalid
    /// or if the OS fails to allocate an IME context for this window.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use michiu_window::{Window, WindowBuilder, ImeContext};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let window = Window::build(WindowBuilder::new().into_unvalidated().try_into()?)?;
    /// // Safely acquire the IME context for the window
    /// let ime_ctx = ImeContext::new(window.hwnd())?;
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn new(hwnd: HWND) -> crate::Result<Self> {
        let himc = unsafe { ImmGetContext(hwnd) };
        if himc.is_invalid() {
            Err(MichiuError::ImeContextAcquisitionFailed { hwnd })
        } else {
            Ok(Self { hwnd, himc })
        }
    }

    /// Checks whether the IME is currently active/open (e.g., Japanese/Chinese input mode is ON).
    #[inline]
    pub fn is_open(&self) -> bool {
        unsafe { ImmGetOpenStatus(self.himc).as_bool() }
    }

    /// Programmatically turns the IME open status ON (`true`) or OFF (`false`).
    #[inline]
    pub fn set_open(&self, open: bool) {
        unsafe {
            let _ = ImmSetOpenStatus(self.himc, open);
        }
    }

    /// Retrieves the raw conversion mode and sentence mode flags.
    #[inline]
    pub fn get_conversion_status(&self) -> (u32, u32) {
        let mut conversion = IME_CONVERSION_MODE::default();
        let mut sentence = IME_SENTENCE_MODE::default();
        unsafe {
            if ImmGetConversionStatus(self.himc, Some(&mut conversion), Some(&mut sentence))
                .as_bool()
            {
                (conversion.0, sentence.0)
            } else {
                (
                    IME_CONVERSION_MODE::default().0,
                    IME_SENTENCE_MODE::default().0,
                )
            }
        }
    }

    /// Programmatically sets the active raw conversion mode and sentence mode flags.
    #[inline]
    pub fn set_conversion_status(&self, conversion: u32, sentence: u32) {
        let conversion = IME_CONVERSION_MODE(conversion);
        let sentence = IME_SENTENCE_MODE(sentence);
        unsafe {
            let _ = ImmSetConversionStatus(self.himc, conversion, sentence);
        }
    }

    /// Safely retrieves the current active composition text (the unconfirmed/preedit text buffer).
    ///
    /// Returns `Ok(None)` if there is no active composition string.
    ///
    /// # Errors
    /// Returns [`MichiuError::ImeStringQueryFailed`] if querying the OS buffer fails.
    #[inline]
    pub fn get_composition_string(&self) -> crate::Result<Option<String>> {
        self.get_string(GCS_COMPSTR)
    }

    /// Safely retrieves the finalized result text (the newly confirmed/committed text buffer).
    ///
    /// Returns `Ok(None)` if there is no result string currently available.
    ///
    /// # Errors
    /// Returns [`MichiuError::ImeStringQueryFailed`] if querying the OS buffer fails.
    #[inline]
    pub fn get_result_string(&self) -> crate::Result<Option<String>> {
        self.get_string(GCS_RESULTSTR)
    }

    /// Relocates the physical popup position of the IME candidate window (caret coordinate position).
    ///
    /// This keeps the OS composition candidate box (candidate window) aligned correctly with your cursor.
    #[inline]
    pub fn set_composition_window_position(&self, position: PhysicalPoint) {
        let form = COMPOSITIONFORM {
            dwStyle: CFS_POINT,
            ptCurrentPos: POINT {
                x: position.x,
                y: position.y,
            },
            rcArea: RECT::default(),
        };
        unsafe {
            let _ = ImmSetCompositionWindow(self.himc, &form);
        }
    }

    /// Retrieves the current physical coordinate position of the IME candidate window as set by the OS.
    #[inline]
    pub fn get_composition_window_position(&self) -> Option<PhysicalPoint> {
        let mut form = COMPOSITIONFORM::default();
        unsafe {
            if ImmGetCompositionWindow(self.himc, &mut form).as_bool() {
                Some(PhysicalPoint {
                    x: form.ptCurrentPos.x,
                    y: form.ptCurrentPos.y,
                })
            } else {
                None
            }
        }
    }

    // 動的なIMEバッファサイズをクエリしてStringに変換
    fn get_string(&self, index: IME_COMPOSITION_STRING) -> crate::Result<Option<String>> {
        unsafe {
            // 第一引数に None を渡すことで、必要なバッファサイズをクエリする
            let size_bytes = ImmGetCompositionStringW(self.himc, index, None, 0);
            if size_bytes == 0 || size_bytes == IMM_ERROR_NODATA {
                return Ok(None);
            }

            if size_bytes == IMM_ERROR_GENERAL {
                return Err(MichiuError::ImeStringQueryFailed {
                    index: index.0,
                    source: windows::core::Error::from_thread(),
                });
            }

            if size_bytes < 0 {
                return Ok(None);
            }

            // バッファサイズ (u16の要素数) を、万が一の奇数バイト報告に備えて安全に切り上げて確保
            let size_bytes = size_bytes as usize;
            let u16_count = size_bytes.div_ceil(2);
            let mut buf = vec![0u16; u16_count];

            // 確保されたバッファの実際の最大許容バイト数を第4引数に渡す
            let buffer_capacity_bytes = buf.len() * 2;
            let res_bytes = ImmGetCompositionStringW(
                self.himc,
                index,
                Some(buf.as_mut_ptr() as *mut _),
                buffer_capacity_bytes as u32,
            );

            if res_bytes > 0 && res_bytes != IMM_ERROR_NODATA && res_bytes != IMM_ERROR_GENERAL {
                // 事前クエリ値ではなく、実際に書き込まれたバイト数分のスライスのみを UTF-16 文字列に変換
                let written_u16_count = (res_bytes as usize) / 2;
                let valid_slice = &buf[..std::cmp::min(written_u16_count, buf.len())];
                Ok(String::from_utf16(valid_slice).ok())
            } else {
                Ok(None)
            }
        }
    }
}

// 使用が終わった IME コンテキストは確実に OS に返却
impl Drop for ImeContext {
    fn drop(&mut self) {
        unsafe {
            let _ = ImmReleaseContext(self.hwnd, self.himc);
        }
    }
}

/// Retrieves the active thread's keyboard layout Language ID (LANGID).
///
/// Returns standard LANGIDs, such as `0x0411` (1041) for Japanese, `0x0409` (1033) for US English, etc.
#[inline]
pub fn get_active_keyboard_layout_id() -> u32 {
    unsafe {
        let hkl = GetKeyboardLayout(0);
        // HKLの低位16ビット（LOWORD）に LANGID が格納されている
        (hkl.0 as usize & 0xFFFF) as u32
    }
}

/// Represents a bundled, complete snapshot of the IME and Input Method status for a window.
///
/// This structure implements ([`serde::Serialize`]) and ([`serde::Deserialize`]) when the `serde`
/// feature is enabled, allowing seamless serialization into JSON for cross-process communication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImeStateUpdate {
    /// The unique ID of the window where this IME state update occurred (HWND as isize).
    pub window_id: isize,
    /// Whether the IME input mode is currently active/open (e.g., Japanese mode ON).
    pub is_open: bool,
    /// Raw IME conversion mode flags.
    pub conversion_mode: u32,
    /// Raw IME sentence mode flags.
    pub sentence_mode: u32,
    /// Active thread keyboard layout LANGID (e.g., 0x0411 for Japanese).
    pub keyboard_layout_id: u32,
    /// Current unconfirmed/preedit text buffer (composition string).
    pub composition_text: String,
    /// Newly confirmed/finalized text buffer (result string).
    pub result_text: String,
    /// The physical coordinate of the IME composition candidate window (caret position).
    pub caret_position: Option<PhysicalPoint>,
}

/// A lightweight, zero-dependency TCP JSON broadcast server designed to stream the active window's IME state securely on localhost.
///
/// This server accepts multiple client connections on the local loopback interface. Whenever a window processes
/// IME messages, it pushes an [`ImeStateUpdate`] snapshot to the server, which then serializes and streams it as
/// compliant JSON.
///
/// It runs entirely on dedicated background threads, ensuring that slow or blocked clients do not freeze
/// the main UI thread's rendering loop.
pub struct ImeRelayServer {
    tx: Sender<ImeStateUpdate>,
}

impl ImeRelayServer {
    /// Starts the OLE/TSF IME Relay Server on the specified localhost port.
    ///
    /// # Errors
    /// Returns `std::io::Error` if binding to the localhost port fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use michiu_window::ImeRelayServer;
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // Spin up the local broadcast server on port 12345
    ///     let server = ImeRelayServer::start(12345)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn start(port: u16) -> std::io::Result<Self> {
        let (tx, rx) = channel::<ImeStateUpdate>();
        let clients: Arc<Mutex<Vec<TcpStream>>> = Arc::new(Mutex::new(Vec::new()));

        // 新規接続を待ち受けるリスナースレッド
        let clients_listener = clients.clone();
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))?;

        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                // スレッドをフリーズさせないよう非ブロッキングに
                let _ = stream.set_nonblocking(true);
                let mut guard = clients_listener.lock().unwrap();
                guard.push(stream);
            }
        });

        // イベントを受信し、JSONとして全接続クライアントへプッシュ配信するワーカースレッド
        std::thread::spawn(move || {
            while let Ok(update) = rx.recv() {
                let caret_json = match update.caret_position {
                    Some(pt) => format!("{{\"x\":{},\"y\":{}}}", pt.x, pt.y),
                    None => "null".to_string(),
                };
                // セキュリティ上安全な、完全フラットなJSONをライブラリ単体でシリアライズ
                let json = format!(
                    "{{\"window_id\":{},\"is_open\":{},\"conversion_mode\":{},\"sentence_mode\":{},\"keyboard_layout_id\":{},\"composition_text\":\"{}\",\"result_text\":\"{}\",\"caret_position\":{}}}\n",
                    update.window_id,
                    update.is_open,
                    update.conversion_mode,
                    update.sentence_mode,
                    update.keyboard_layout_id,
                    escape_json(&update.composition_text),
                    escape_json(&update.result_text),
                    caret_json,
                );

                let mut guard = clients.lock().unwrap();
                // 切断されたクライアントを自動検知してリストから安全に除外 (retain_mut)
                guard.retain_mut(|stream| {
                    // 本体の書き込み
                    match stream.write_all(json.as_bytes()) {
                        Ok(()) => {
                            // キャッシュのフラッシュ
                            match stream.flush() {
                                Ok(()) => true, // 配信成功
                                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                    true // 一時的な待機状態は生存とみなす
                                }
                                Err(_) => false, // 致命的なエラーは切断とみなす
                            }
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => true,
                        Err(_) => false,
                    }
                });
            }
        });

        Ok(Self { tx })
    }

    /// Dispatches an [`ImeStateUpdate`] snapshot to the server to broadcast to all connected clients.
    #[inline]
    pub fn send(&self, update: ImeStateUpdate) {
        let _ = self.tx.send(update);
    }
}

fn escape_json(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c.is_ascii_control() => {
                // 0x00〜0x1F, 0x7F などの制御文字を規格通りの \uXXXX 形式へエスケープ
                escaped.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => escaped.push(c),
        }
    }
    escaped
}

#[cfg(any(feature = "serde", test))]
impl serde::Serialize for ImeStateUpdate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ImeStateUpdate", 8)?;
        state.serialize_field("window_id", &self.window_id)?;
        state.serialize_field("is_open", &self.is_open)?;
        state.serialize_field("conversion_mode", &self.conversion_mode)?;
        state.serialize_field("sentence_mode", &self.sentence_mode)?;
        state.serialize_field("keyboard_layout_id", &self.keyboard_layout_id)?;
        state.serialize_field("composition_text", &self.composition_text)?;
        state.serialize_field("result_text", &self.result_text)?;

        // PhysicalPoint をシリアライズ可能なタプル (x, y) に変換して書き出し
        let caret = self.caret_position.map(|pt| (pt.x, pt.y));
        state.serialize_field("caret_position", &caret)?;
        state.end()
    }
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for ImeStateUpdate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{MapAccess, Visitor};
        use std::fmt;

        struct ImeStateUpdateVisitor;

        impl<'de> Visitor<'de> for ImeStateUpdateVisitor {
            type Value = ImeStateUpdate;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("struct ImeStateUpdate")
            }

            fn visit_map<V>(self, mut map: V) -> Result<ImeStateUpdate, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut window_id = None;
                let mut is_open = None;
                let mut conversion_mode = None;
                let mut sentence_mode = None;
                let mut keyboard_layout_id = None;
                let mut composition_text = None;
                let mut result_text = None;
                let mut caret_position_tuple: Option<Option<(i32, i32)>> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "window_id" => window_id = Some(map.next_value()?),
                        "is_open" => is_open = Some(map.next_value()?),
                        "conversion_mode" => conversion_mode = Some(map.next_value()?),
                        "sentence_mode" => sentence_mode = Some(map.next_value()?),
                        "keyboard_layout_id" => keyboard_layout_id = Some(map.next_value()?),
                        "composition_text" => composition_text = Some(map.next_value()?),
                        "result_text" => result_text = Some(map.next_value()?),
                        "caret_position" => caret_position_tuple = Some(map.next_value()?),
                        _ => {
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                let window_id =
                    window_id.ok_or_else(|| serde::de::Error::missing_field("window_id"))?;
                let is_open = is_open.ok_or_else(|| serde::de::Error::missing_field("is_open"))?;
                let conversion_mode = conversion_mode
                    .ok_or_else(|| serde::de::Error::missing_field("conversion_mode"))?;
                let sentence_mode = sentence_mode
                    .ok_or_else(|| serde::de::Error::missing_field("sentence_mode"))?;
                let keyboard_layout_id = keyboard_layout_id
                    .ok_or_else(|| serde::de::Error::missing_field("keyboard_layout_id"))?;
                let composition_text = composition_text
                    .ok_or_else(|| serde::de::Error::missing_field("composition_text"))?;
                let result_text =
                    result_text.ok_or_else(|| serde::de::Error::missing_field("result_text"))?;

                let caret_position = caret_position_tuple
                    .flatten()
                    .map(|(x, y)| PhysicalPoint { x, y });

                Ok(ImeStateUpdate {
                    window_id,
                    is_open,
                    conversion_mode,
                    sentence_mode,
                    keyboard_layout_id,
                    composition_text,
                    result_text,
                    caret_position,
                })
            }
        }

        deserializer.deserialize_struct(
            "ImeStateUpdate",
            &[
                "window_id",
                "is_open",
                "conversion_mode",
                "sentence_mode",
                "keyboard_layout_id",
                "composition_text",
                "result_text",
                "caret_position",
            ],
            ImeStateUpdateVisitor,
        )
    }
}

#[cfg(test)]
mod tests {
    use michiu_guard::Validate;

    use super::*;
    use crate::{Window, WindowBuilder};

    fn run_on_clean_thread<F>(f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let handle = std::thread::spawn(f);
        handle.join().expect("Test thread panicked");
    }

    #[test]
    fn test_ime_context_lifecycle_and_getters_normal() {
        run_on_clean_thread(|| {
            let builder = WindowBuilder::new().with_title("ImeLifecycleTestWindow");
            let window = Window::build(builder.validate_into().unwrap()).unwrap();

            // IMEコンテキストの取得
            let ime_ctx_res = ImeContext::new(window.hwnd());
            assert!(
                ime_ctx_res.is_ok(),
                "Failed to acquire IME context: {:?}",
                ime_ctx_res.err()
            );

            let ime_ctx = ime_ctx_res.unwrap();

            // IMEオープン状態（ON/OFF）の切り替えテスト
            let original_open = ime_ctx.is_open();

            ime_ctx.set_open(!original_open);
            assert_eq!(
                ime_ctx.is_open(),
                !original_open,
                "Failed to toggle IME open status"
            );

            ime_ctx.set_open(original_open);
            assert_eq!(ime_ctx.is_open(), original_open);

            // 生の変換フラグの取得と再適用テスト
            let (original_conv, original_sent) = ime_ctx.get_conversion_status();
            ime_ctx.set_conversion_status(original_conv, original_sent);

            let (new_conv, new_sent) = ime_ctx.get_conversion_status();
            assert_eq!(original_conv, new_conv);
            assert_eq!(original_sent, new_sent);

            // キャレット位置指定テスト
            ime_ctx.set_composition_window_position(PhysicalPoint::new(100, 150));

            // 未確定/確定文字列が正常に Option として取得可能か
            let comp_res = ime_ctx.get_composition_string();
            assert!(
                comp_res.is_ok(),
                "Composition string query returned an error"
            );
            assert!(
                comp_res.unwrap().is_none(),
                "Expected no composition string in blank state"
            );

            // キーボードレイアウトIDの取得テスト
            let layout_id = get_active_keyboard_layout_id();
            assert!(
                layout_id > 0,
                "Keyboard layout ID should be a non-zero value"
            );

            let caret_pos = PhysicalPoint { x: 100, y: 150 };
            ime_ctx.set_composition_window_position(caret_pos);

            // 設定した位置が正しく取得できるか検証
            let queried_pos = ime_ctx.get_composition_window_position();
            assert!(
                queried_pos.is_some(),
                "Failed to query composition window position"
            );
            assert_eq!(
                queried_pos.unwrap(),
                caret_pos,
                "Composition window position coordinates mismatch"
            );

            window.destroy();
        });
    }

    #[test]
    fn test_ime_context_acquisition_failed_abnormal() {
        // すでに存在しない無効な HWND やヌル HWND に対して取得を試みる
        let bad_hwnd = HWND(0 as _);
        let result = ImeContext::new(bad_hwnd);

        assert!(result.is_err(), "Acquiring context for NULL HWND must fail");
        assert!(
            matches!(
                result.unwrap_err(),
                MichiuError::ImeContextAcquisitionFailed { .. }
            ),
            "Expected ImeContextAcquisitionFailed error"
        );
    }

    #[test]
    fn test_escape_json_control_characters_and_symbols() {
        // 通常文字列
        assert_eq!(escape_json("hello"), "hello");

        // 特殊文字・シンボルのエスケープ
        assert_eq!(escape_json("a\\b\"c\nd\re\tf"), "a\\\\b\\\"c\\nd\\re\\tf");

        // 制御コード（ヌル文字 \x00, ベル文字 \x07, バックスペース \x08）
        assert_eq!(
            escape_json("hello\x00world\x07tab\x08"),
            "hello\\u0000world\\u0007tab\\u0008"
        );
    }
}
