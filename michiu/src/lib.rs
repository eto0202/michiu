#![cfg_attr(docsrs, feature(doc_cfg))]
//! # michiu
//!
//! The unified facade crate for the `michiu` GUI ecosystem.
//!
//! It integrates and exposes:
//! - `michiu_guard` (Data validation and boundary firewall)
//! - `michiu_window` (Win32 windowing, system tray, COM, and IME subsystem)

#[cfg(feature = "guard")]
#[cfg_attr(docsrs, doc(cfg(feature = "guard")))]
mod guard_facade {
    // 外部から `use michiu::{Unvalidated, Validated, Validate};` のように直接使えるように再エクスポート
    pub use michiu_guard::{Unvalidated, Validate, Validated};
}

#[cfg(feature = "guard")]
pub use guard_facade::*;

#[cfg(feature = "window")]
#[cfg_attr(docsrs, doc(cfg(feature = "window")))]
mod window_facade {
    // 外部から `use michiu::{Window, WindowBuilder, EventPump, ...};` とフラットに使えるように再エクスポート
    pub use michiu_window::{
        // OLE COM・ファイル D&D
        ComContext,
        CursorIcon,
        CustomTrayMenu,
        // 入力状態・座標型
        ElementState,
        Event,
        EventBus,
        EventPump,
        EventSender,
        FileDropTarget,
        // アイコンリソース
        Icon,
        // IME/TSF 制御・JSONリレー
        ImeContext,
        ImeRelayServer,
        ImeStateUpdate,
        LogicalPoint,
        LogicalRect,
        LogicalSize,
        MessageFilter,
        // エラー・トラブルシューティング
        MichiuError,
        // イベント・メッセージポンプ・イベントバス
        MichiuEvent,
        Modifiers,
        MouseButton,

        PreferredAppMode,
        Result,
        RichReport,
        SysErrorInfo,
        // システムトレイ（通知領域）
        Tray,
        TrayBuilder,
        TrayMenuItem,
        // ウィンドウ・ハンドル・ビルダ
        Window,
        WindowBuilder,
        WindowHandle,
        WindowId,
        get_active_keyboard_layout_id,

        init_dpi_awareness,
    };
}

#[cfg(feature = "window")]
pub use window_facade::*;
