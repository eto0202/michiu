use crate::{
    ImeStateUpdate, Modifiers, PhysicalPoint, PhysicalRect, PhysicalSize, WindowId,
    types::{ElementState, MouseButton},
};
use michiu_guard::Unvalidated;
use std::{any::Any, path::PathBuf};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::Input::KeyboardAndMouse::VIRTUAL_KEY,
};

/// The top-level Event enumeration yielded by the event loop.
#[derive(Debug)]
pub enum MichiuEvent {
    /// Events triggered by a specific active window.
    Window {
        /// Identifies which window generated this event.
        id: WindowId,
        /// The specific windowing or input event.
        event: Event,
    },

    /// A custom, thread-safe user-defined event.
    ///
    /// Extensible via `Box<dyn Any + Send>`.
    User(Box<dyn Any + Send>),
}

/// Represents specific events sent by the OS windowing and input system.
///
/// All raw OS coordinates and keys are kept unvalidated inside [`Unvalidated`] wrappers,
/// forcing you to explicitly validate them at your application boundary.
#[derive(Debug, Clone)]
pub enum Event {
    /// Issued when the window is first instantiated. (WM_CREATE)
    Created,

    /// Issued when the user clicks the "Close" (X) button.
    /// Useful for intercepting close signals to show "Save changes?" dialogs. (WM_CLOSE)
    CloseRequested,

    /// Issued when the window is destroyed. (WM_DESTROY)
    Destroyed,

    /// Issued when the window has been resized.
    /// Holds the unvalidated new dimensions. (WM_SIZE)
    Resized(Unvalidated<PhysicalSize>),

    /// Issued when the window has been relocated.
    /// Holds the unvalidated new coordinate position. (WM_MOVE)
    Moved(Unvalidated<PhysicalPoint>),

    /// Issued when the window gains (`true`) or loses (`false`) focus.
    Focused(bool),

    /// Issued when a physical keyboard key is pressed or released.
    KeyboardInput {
        key_code: Unvalidated<VIRTUAL_KEY>,
        modifiers: Modifiers,
        state: ElementState,
    },

    /// Issued when a character is input (handles localization and key repeats). (WM_CHAR)
    CharacterInput(char),

    /// Issued when the mouse cursor enters the window's boundary.
    CursorEntered,

    /// Issued when the mouse cursor leaves the window's boundary.
    CursorLeft,

    /// Issued when the mouse cursor is moved inside the window. (WM_MOUSEMOVE)
    CursorMoved {
        position: Unvalidated<PhysicalPoint>,
    },

    /// Issued when a mouse button is pressed or released.
    MouseInput {
        button: MouseButton,
        modifiers: Modifiers,
        state: ElementState,
    },

    /// Issued when the mouse wheel is rotated. (positive: forward, negative: backward). (WM_MOUSEWHEEL)
    MouseWheel { delta: f32 },

    /// Issued when the window client area needs to be repainted. (WM_PAINT)
    ///
    /// On Windows, this is automatically wrapped by `BeginPaint`/`EndPaint` inside the loop,
    /// so you can safely render custom graphics (e.g. DirectX, Vulkan, GDI) right away.
    RedrawRequested,

    /// Issued when the DPI scale factor of the window changes. (WM_DPICHANGED)
    ScaleFactorChanged {
        scale_factor: f64,
        /// Suggested new window coordinate position and size recommended by Windows to maintain scaling consistency.
        suggested_bounds: Unvalidated<PhysicalRect>,
    },

    /// Issued when files are dragged and dropped onto the window client area.
    ///
    /// Requires `with_drag_and_drop(true)` and an OLE-initialized [`ComContext`].
    FileDropped(Unvalidated<Vec<PathBuf>>),

    /// Issued when any active Input Method (IME/TSF) state update occurs.
    ///
    /// Bundles a complete, cohesive snapshot of the state update (mode, preedit/confirmed text, layout, caret coordinates).
    Ime(Unvalidated<ImeStateUpdate>),

    /// Raw, unhandled fallback OS window messages.
    ///
    /// Useful for implementing obscure Win32 features not yet natively wrapped by the library.
    UnsafeRaw {
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PhysicalSize;
    use michiu_guard::{Unvalidated, Validated};

    #[test]
    fn test_window_event_validation_success() {
        // OSから正常なサイズ（800x600）を受け取ったと仮定
        let raw_size = PhysicalSize::new(800, 600);
        let event = Event::Resized(Unvalidated::new(raw_size));

        if let Event::Resized(unvalidated_size) = event {
            // `validate_with` を用いて、アプリケーションの境界で安全に検証を行う
            // 例として「幅・高さが共に0より大きいこと」を検証ルールとする
            let validation_result: Result<Validated<PhysicalSize>, &str> = unvalidated_size
                .validate_with(|size| {
                    if size.width > 0 && size.height > 0 {
                        Ok(size)
                    } else {
                        Err("Window dimensions must be positive values.")
                    }
                });

            // 検証が成功し安全な `Validated<T>` が得られることをテスト
            assert!(validation_result.is_ok());

            // 検証済みデータから取り出した値が正しいことを確認
            let validated_size = validation_result.unwrap();
            assert_eq!(validated_size.into_inner(), raw_size);
        } else {
            panic!("Expected Event::Resized");
        }
    }

    #[test]
    fn test_window_event_validation_failure() {
        // OSから不正なサイズ（マイナス幅など、Win32の不具合や予期せぬ値）を受け取ったと仮定
        let invalid_raw_size = PhysicalSize::new(-100, 600);
        let event = Event::Resized(Unvalidated::new(invalid_raw_size));

        if let Event::Resized(unvalidated_size) = event {
            // `validate_with` を用いて同様に検証を行う
            let validation_result: Result<Validated<PhysicalSize>, &str> = unvalidated_size
                .validate_with(|size| {
                    if size.width > 0 && size.height > 0 {
                        Ok(size)
                    } else {
                        Err("Window dimensions must be positive values.")
                    }
                });

            // 検証が確実に失敗しフレームワーク内部へのデータ混入を防げていることをテスト
            assert!(validation_result.is_err());
            assert_eq!(
                validation_result.unwrap_err(),
                "Window dimensions must be positive values."
            );
        } else {
            panic!("Expected Event::Resized");
        }
    }

    #[test]
    fn test_user_event_downcast() {
        // ユーザーが定義したカスタム構造体やデータの代わりとして String を使用
        let custom_data = String::from("Custom Command");

        // Box に包んで User とする
        let event = MichiuEvent::User(Box::new(custom_data));

        if let MichiuEvent::User(boxed_any) = event {
            // Any型から元の String 型へ安全にダウンキャストできるか検証
            let downcasted = boxed_any.downcast_ref::<String>();
            assert!(downcasted.is_some(), "Downcast to String should succeed");
            assert_eq!(downcasted.unwrap(), "Custom Command");
        } else {
            panic!("Expected MichiuEvent::User");
        }
    }

    #[test]
    fn test_michiu_event_window_id_binding() {
        // ウィンドウIDが正しく結びついているかのテスト
        let event = MichiuEvent::Window {
            id: WindowId(12345),
            event: Event::CloseRequested,
        };

        match event {
            MichiuEvent::Window { id, event } => {
                assert_eq!(id, WindowId(12345));
                assert!(matches!(event, Event::CloseRequested));
            }
            _ => panic!("Expected MichiuEvent::Event"),
        }
    }

    #[test]
    fn test_michiu_guard_advanced_recovery_and_map() {
        // OSから不正なマイナス値のサイズが送られてきたと仮定
        let raw_size = Unvalidated::new(PhysicalSize::new(-50, 600));

        // try_validate_with による検証と生データの回収
        let failed_validation = raw_size.try_validate_with(|size| {
            if size.width > 0 && size.height > 0 {
                Ok(size)
            } else {
                Err(("Negative size not allowed", size))
            }
        });

        assert!(failed_validation.is_err());
        // エラー内容とともに、元の Unvalidated データが失われずに回収できるか検証
        let (err_msg, recovered_unvalidated) = failed_validation.unwrap_err();
        assert_eq!(err_msg, "Negative size not allowed");

        //  map による「検証前のデータの正規化（サニタイズ）」
        // マイナス値を絶対値に変換して修復する
        let sanitized = recovered_unvalidated.map(|mut size| {
            size.width = size.width.abs();
            size
        });

        // 修復後のデータで再度 validate_with を行い、成功するか検証
        let final_validation = sanitized.validate_with(|size| {
            if size.width > 0 && size.height > 0 {
                Ok(size)
            } else {
                Err("Negative size not allowed")
            }
        });

        assert!(final_validation.is_ok());
        // 50 に修復されたデータが確定しているか確認
        assert_eq!(
            final_validation.unwrap().into_inner(),
            PhysicalSize::new(50, 600)
        );
    }
}
