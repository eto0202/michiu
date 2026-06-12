use crate::{
    ImeStateUpdate, Modifiers, PhysicalPoint, PhysicalRect, PhysicalSize, WindowId, types::{ElementState, MouseButton}
};
use michiu_guard::Unvalidated;
use std::{any::Any, path::PathBuf};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::Input::KeyboardAndMouse::VIRTUAL_KEY,
};

#[derive(Debug)]
pub enum MichiuEvent {
    /// Events triggered by a specific window
    WindowEvent {
        window_id: WindowId,
        event: WindowEvent,
    },

    /// A custom user-defined event.
    /// Provides extensibility via `Box<dyn Any + Send>`.
    UserEvent(Box<dyn Any + Send>),
}

/// Represents events sent by the windowing system.
#[derive(Debug, Clone)]
pub enum WindowEvent {
    /// Issued when the window is first created. (WM_CREATE)
    Created,

    /// Issued when the user attempts to close the window.
    /// Can be used to intercept the close signal to show "Save changes?" dialogs. (WM_CLOSE)
    CloseRequested,

    /// Issued when the window is being destroyed. (WM_DESTROY)
    Destroyed,

    /// Issued when the window size has changed.
    /// Holds the unvalidated new dimensions. (WM_SIZE)
    Resized(Unvalidated<PhysicalSize>),

    /// Issued when the window has been moved to a new position. (WM_MOVE)
    Moved(Unvalidated<PhysicalPoint>),

    /// Issued when the window gains or loses focus.
    /// `true` if the window gained focus, `false` if it lost it. (WM_SETFOCUS, WM_KILLFOCUS)
    Focused(bool),

    /// Issued when a physical key is pressed or released.
    /// Use this for handling game controls or shortcuts. (WM_KEYDOWN, WM_KEYUP)
    KeyboardInput {
        key_code: Unvalidated<VIRTUAL_KEY>,
        modifiers: Modifiers,
        state: ElementState,
    },

    /// Issued when a character is input (text input).
    /// Handles localized input and repeated key strokes for text fields. (WM_CHAR)
    CharacterInput(char),

    /// Issued when the cursor enters the window's boundaries.
    /// The library internally tracks this using `TrackMouseEvent`.
    CursorEntered,

    /// Issued when the cursor leaves the window's boundaries.
    CursorLeft,

    /// Issued when the mouse cursor is moved within the window. (WM_MOUSEMOVE)
    CursorMoved {
        position: Unvalidated<PhysicalPoint>,
    },

    /// Issued when a mouse button is pressed or released. (WM_LBUTTONDOWN, etc.)
    MouseInput {
        button: MouseButton,
        modifiers: Modifiers,
        state: ElementState,
    },

    /// Issued when the mouse wheel is rotated.
    /// A positive value indicates the wheel was rotated forward, away from the user. (WM_MOUSEWHEEL)
    MouseWheel {
        delta: f32,
    },

    /// Issued when the window content needs to be redrawn. (WM_PAINT)
    RedrawRequested,

    /// Issued when the scale factor of the window changes.
    /// This happens when the user drags the window to a monitor with a different DPI setting.
    /// `scale_factor` is the ratio between physical pixels and logical pixels (e.g., 1.5 for 150% scaling).
    /// `suggested_bounds` contains the new size and position recommended by Windows (WM_DPICHANGED).
    ScaleFactorChanged {
        scale_factor: f64,
        /// Suggested new window position and size to maintain visual consistency.
        suggested_bounds: Unvalidated<PhysicalRect>,
    },

    FileDropped(Unvalidated<Vec<PathBuf>>),

    /// Issued when any IME, TSF, or Input Method state change occurs.
    /// Bundles the entire current state into a single cohesive structure.
    Ime(Unvalidated<ImeStateUpdate>),

    /// UnsafeRaw, unhandled Windows messages.
    /// Use this for specific features not yet wrapped by the library.
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
        let event = WindowEvent::Resized(Unvalidated::new(raw_size));

        if let WindowEvent::Resized(unvalidated_size) = event {
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
            panic!("Expected WindowEvent::Resized");
        }
    }

    #[test]
    fn test_window_event_validation_failure() {
        // OSから不正なサイズ（マイナス幅など、Win32の不具合や予期せぬ値）を受け取ったと仮定
        let invalid_raw_size = PhysicalSize::new(-100, 600);
        let event = WindowEvent::Resized(Unvalidated::new(invalid_raw_size));

        if let WindowEvent::Resized(unvalidated_size) = event {
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
            panic!("Expected WindowEvent::Resized");
        }
    }

    #[test]
    fn test_user_event_downcast() {
        // ユーザーが定義したカスタム構造体やデータの代わりとして String を使用
        let custom_data = String::from("Custom Command");

        // Box に包んで UserEvent とする
        let event = MichiuEvent::UserEvent(Box::new(custom_data));

        if let MichiuEvent::UserEvent(boxed_any) = event {
            // Any型から元の String 型へ安全にダウンキャストできるか検証
            let downcasted = boxed_any.downcast_ref::<String>();
            assert!(downcasted.is_some(), "Downcast to String should succeed");
            assert_eq!(downcasted.unwrap(), "Custom Command");
        } else {
            panic!("Expected MichiuEvent::UserEvent");
        }
    }

    #[test]
    fn test_michiu_event_window_id_binding() {
        // ウィンドウIDが正しく結びついているかのテスト
        let event = MichiuEvent::WindowEvent {
            window_id: WindowId(12345),
            event: WindowEvent::CloseRequested,
        };

        match event {
            MichiuEvent::WindowEvent { window_id, event } => {
                assert_eq!(window_id, WindowId(12345));
                assert!(matches!(event, WindowEvent::CloseRequested));
            }
            _ => panic!("Expected MichiuEvent::WindowEvent"),
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
