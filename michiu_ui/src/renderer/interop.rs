use crate::{LayoutSize, WgpuRenderer};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Direct3D12::ID3D12Resource;
use windows::Win32::Graphics::Dxgi::IDXGIResource1;
use windows::Win32::Graphics::Dxgi::{DXGI_SHARED_RESOURCE_READ, DXGI_SHARED_RESOURCE_WRITE};
use windows::core::Interface;

/// wgpu (D3D12) 側のインポート処理
/// 外部の D3D11 からエクスポートされた共有 NT ハンドル（HANDLE）をインポートし、
/// コピーを介さずに 100% 同一の VRAM アドレスを指す wgpu::Texture を構築します。
pub(crate) unsafe fn import_shared_texture(
    wgpu_renderer: &WgpuRenderer,
    device: wgpu::Device,
    shared_handle: HANDLE,
    size: LayoutSize,
) -> Result<wgpu::Texture, Box<dyn std::error::Error>> {
    unsafe {
        // 1. wgpu::Device から wgpu_hal の生 D3D12 デバイス（ID3D12Device）を取得する
        let hal_device = device
            .as_hal::<wgpu::wgc::api::Dx12>()
            .ok_or("Not a DX12 backend device")?;
        let raw_d3d12_device = hal_device.raw_device();

        // 2. ID3D12Device::OpenSharedHandle を呼び出し、同じ VRAM 領域を指す ID3D12Resource を開く
        let mut raw_resource: Option<ID3D12Resource> = None;
        raw_d3d12_device.OpenSharedHandle(shared_handle, &mut raw_resource)?;
        let resource = raw_resource.ok_or("Failed to open shared handle on D3D12 device")?;

        // 3. wgpu が扱うテクスチャ記述子（wgpu::TextureDescriptor）を正確に定義する
        let ext_size = wgpu::Extent3d {
            width: size.width.ceil() as u32,
            height: size.height.ceil() as u32,
            depth_or_array_layers: 1,
        };

        let desc = wgpu::TextureDescriptor {
            label: Some("Shared WebView2 Texture"),
            size: ext_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm, // D3D11 側と一致させる
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC, // キャッシュフラッシュ等に対応
            view_formats: &[],
        };

        // 4. wgpu_hal を用いて生リソース（ID3D12Resource）を wgpu_hal::dx12::Texture にラッピングする
        let hal_texture = <wgpu::wgc::api::Dx12 as wgpu::hal::Api>::Device::texture_from_raw(
            resource,
            desc.format,
            desc.dimension,
            desc.size,
            desc.mip_level_count,
            desc.sample_count,
        );

        // 5. HAL テクスチャを、通常の wgpu::Texture インターフェースに適合・養子縁組（Adopt）させる
        let texture = device.create_texture_from_hal::<wgpu::wgc::api::Dx12>(hal_texture, &desc);

        Ok(texture)
    }
}

/// D3D11 側のエクスポート処理
pub(crate) unsafe fn export_shared_handle(
    texture: &ID3D11Texture2D,
) -> Result<HANDLE, Box<dyn std::error::Error>> {
    // D3D11 テクスチャを IDXGIResource1 にキャスト
    let dxgi_resource: IDXGIResource1 = texture.cast()?;

    // wgpu 側（D3D12）で読み書きできるよう、共有 NT ハンドルをエクスポート
    let shared_handle = unsafe {
        dxgi_resource.CreateSharedHandle(
            None,
            DXGI_SHARED_RESOURCE_READ.0 | DXGI_SHARED_RESOURCE_WRITE.0,
            None,
        )
    }?;

    Ok(shared_handle)
}
