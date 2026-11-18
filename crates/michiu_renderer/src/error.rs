use thiserror::Error;

pub type MichiuRendererResult<T> = std::result::Result<T, MichiuRendererError>;

// 後々追加
#[derive(Error, Debug, Clone)]
pub enum MichiuRendererError {
    #[error("Windows API Error: {source}")]
    WindowsApiError {
        #[source]
        #[from]
        source: windows_core::Error,
    },

    #[error("Failded surface creation: {source}")]
    CreateSurfaceError {
        #[source]
        #[from]
        source: michiu_ui::wgpu::CreateSurfaceError,
    },

    #[error("Failded surface creation: {source}")]
    RequestAdapterError {
        #[source]
        #[from]
        source: michiu_ui::wgpu::RequestAdapterError,
    },

    #[error("Failded request device: {source}")]
    RequestDeviceError {
        #[source]
        #[from]
        source: michiu_ui::wgpu::RequestDeviceError,
    },

    #[error("Failded request device type")]
    RequestDeviceTypeError,

    #[error("Failed to create D3D11 hardware device.")]
    D3d11DeviceCreationFailed,

    #[error("Failed to create DispatcherQueueController.")]
    DispatcherQueueControllerCreationFailed,

    #[error(
        "DCompDeviceManager has not been initialized.\n\
         Please perform the initialization process first."
    )]
    UninitializedDeviceManager,

    #[error("Failed to initialize webview2: {source}")]
    WebView2Error {
        #[source]
        source: windows_core::Error,
    },
}
