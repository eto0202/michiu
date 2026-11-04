use crate::MichiuError;
use std::sync::OnceLock;
use windows::{
    System::DispatcherQueueController,
    UI::Composition::Compositor,
    Win32::{
        Foundation::HMODULE,
        Graphics::{
            Direct3D::D3D_DRIVER_TYPE_HARDWARE,
            Direct3D11::{
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
                ID3D11Device,
            },
            DirectComposition::{DCompositionCreateDevice2, IDCompositionDesktopDevice},
            Imaging::{CLSID_WICImagingFactory, IWICImagingFactory},
        },
        System::{
            Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
            WinRT::{
                CreateDispatcherQueueController, DQTAT_COM_STA, DQTYPE_THREAD_CURRENT,
                DispatcherQueueOptions,
            },
        },
    },
};

#[derive(Debug, Clone)]
pub(crate) struct PlatformContext {
    pub(crate) d3d11: ID3D11Device,
    pub(crate) dcomp: IDCompositionDesktopDevice,
    pub(crate) compositor: Compositor,
    pub(crate) wic: IWICImagingFactory,
    pub(crate) controller: DispatcherQueueController,
}

unsafe impl Send for PlatformContext {}
unsafe impl Sync for PlatformContext {}

impl PlatformContext {
    #[inline]
    pub(crate) fn new() -> crate::Result<&'static Self> {
        static INSTANCE: OnceLock<PlatformContext> = OnceLock::new();

        if let Some(manager) = INSTANCE.get() {
            return Ok(manager);
        }

        let manager = Self::try_create_devices()?;

        // 完全に成功した場合にのみ OnceLock に実体をセットする。
        // 複数スレッドで同時に成功した場合、最初に完了した方がセットされ、
        // もう一方は set() で Err(manager) を返すが、
        // 最終的に get() 側で正しく1つのインスタンスに統合されるため安全。
        let _ = INSTANCE.set(manager);

        INSTANCE
            .get()
            .ok_or(MichiuError::UninitializedDeviceManager)
    }

    #[inline]
    fn try_create_devices() -> crate::Result<Self> {
        let controller = Self::create_dispatcher_queue()?;

        // D3D11 デバイスを作成
        let mut d3d11_device: Option<ID3D11Device> = None;
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&raw mut d3d11_device),
                None,
                None,
            )?;
        }
        let d3d11 = d3d11_device.ok_or(MichiuError::D3d11DeviceCreationFailed)?;

        // WinRT の Compositor を作成 (STAスレッドと DispatcherQueue が必要)
        let compositor = Compositor::new()?;

        // DCompositionCreateDevice ではなく DCompositionCreateDevice2 を使用。
        // これにより IDCompositionDesktopDevice の生成が正しくサポートされる。
        let dcomp: IDCompositionDesktopDevice = unsafe { DCompositionCreateDevice2(None) }?;

        let wic: IWICImagingFactory =
            unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)? };

        Ok(Self {
            d3d11,
            dcomp,
            compositor,
            wic,
            controller,
        })
    }

    #[inline]
    fn create_dispatcher_queue() -> crate::Result<DispatcherQueueController> {
        // DispatcherQueue を現在のスレッド（UIスレッド）に作成
        #[expect(clippy::as_conversions, clippy::cast_possible_truncation)]
        let options = DispatcherQueueOptions {
            dwSize: std::mem::size_of::<DispatcherQueueOptions>() as u32,
            threadType: DQTYPE_THREAD_CURRENT,
            apartmentType: DQTAT_COM_STA,
        };
        unsafe {
            CreateDispatcherQueueController(options)
                .map_err(|_| MichiuError::DispatcherQueueControllerCreationFailed)
        }
    }

    #[inline]
    pub(crate) fn commit(&self) -> crate::Result<()> {
        unsafe {
            self.dcomp
                .Commit()
                .map_err(|e| MichiuError::WindowsApiError { source: e })
        }
    }
}
