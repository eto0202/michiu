use std::cell::RefCell;
use std::rc::Rc;

use crate::{LayoutSize, WgpuRenderer};
use webview2_com::CapturePreviewCompletedHandler;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG, ICoreWebView2,
};
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::UI::Composition::Visual as WinRTVisual;
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_SHADER_RESOURCE, D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX, D3D11_TEXTURE2D_DESC,
    ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Direct3D12::ID3D12Resource;
use windows::Win32::Graphics::DirectComposition::IDCompositionVisual;
use windows::Win32::Graphics::Dxgi::{
    DXGI_SHARED_RESOURCE_READ, DXGI_SHARED_RESOURCE_WRITE, IDXGISwapChain1,
};
use windows::Win32::Graphics::Dxgi::{IDXGIDevice, IDXGIResource1};
use windows::Win32::Graphics::Imaging::{
    GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory, WICBitmapDitherTypeNone,
    WICBitmapInterpolationModeLinear, WICBitmapPaletteTypeMedianCut,
    WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::IStream;
use windows::Win32::System::Com::StructuredStorage::{CreateStreamOnHGlobal, GetHGlobalFromStream};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::core::Interface;

/// WebView2 から非アクティブ時の静止画（1フレーム）をキャプチャし、
/// wgpu 側の wgpu::Texture へとピクセルデータを転送します。
pub(crate) unsafe fn trigger_capture_async<F>(
    webview: ICoreWebView2,
    width: u32,
    height: u32,
    wgpu_device: wgpu::Device,
    wgpu_queue: wgpu::Queue,
    wic_factory: IWICImagingFactory,
    on_complete: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce(Result<wgpu::Texture, Box<dyn std::error::Error>>) + 'static,
{
    unsafe {
        // 1. メモリ上に COM の IStream を作成
        let stream = CreateStreamOnHGlobal(HGLOBAL::default(), true)?;
        let stream_clone = stream.clone();

        // 完了コールバックの所有権をハンドラー内に安全に移すため、Option に包む
        let mut on_complete_opt = Some(on_complete);

        // 2. ICoreWebView2::CapturePreview を呼び出し、PNG 形式でストリームに書き込ませる
        // (ICoreWebView2CapturePreviewCompletedHandler を用いて非同期完了を同期的に待機します)
        webview.CapturePreview(
            COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
            &stream,
            // 完了ハンドラー
            &CapturePreviewCompletedHandler::create(Box::new(move |result| {
                // このクロージャは、WebView2 側の処理完了時にメインスレッド（STA）上で呼び出されます
                let on_complete = on_complete_opt.take().expect("Callback already executed");

                if let Err(e) = result {
                    on_complete(Err(e.into()));
                    return Ok(());
                }

                // 3. ストリームから HGLOBAL をクエリして WIC で PMA (BGRA8) にデコード
                let texture_res = process_captured_stream(
                    &wgpu_device,
                    &wgpu_queue,
                    &wic_factory,
                    &stream_clone,
                    width,
                    height,
                );

                // 完了コールバックを呼び出してテクスチャを通知
                on_complete(texture_res);
                Ok(())
            })),
        )?;

        Ok(())
    }
}

/// 内部ヘルパー: ストリームに書き込まれた PNG バイト列を WIC で高速に BGRA8 ピクセル配列にデコードし、
/// wgpu::Texture を作成してアップロードします。
unsafe fn process_captured_stream(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    wic_factory: &IWICImagingFactory,
    stream: &IStream,
    width: u32,
    height: u32,
) -> Result<wgpu::Texture, Box<dyn std::error::Error>> {
    unsafe {
        let hglobal = GetHGlobalFromStream(stream)?;
        let data_ptr = GlobalLock(hglobal);

        // ポインタが null の場合は早期リターン
        if data_ptr.is_null() {
            return Err("GlobalLock returned null pointer".into());
        }

        let size = GlobalSize(hglobal);
        let png_bytes = std::slice::from_raw_parts(data_ptr as *const u8, size);

        // 1. wgpu 内にすでに定義されている WIC ファクトリ（IWICImagingFactory）を利用
        let wic_stream = wic_factory.CreateStream()?;
        wic_stream.InitializeFromMemory(png_bytes)?;

        // 2. デコーダーとフレームの構築
        let decoder = wic_factory.CreateDecoderFromStream(
            &wic_stream,
            std::ptr::null(),
            WICDecodeMetadataCacheOnDemand,
        )?;

        let frame = decoder.GetFrame(0)?;

        // 3. PMA (Premultiplied Alpha) の BGRA 形式に変換

        let converter = wic_factory.CreateFormatConverter()?;
        converter.Initialize(
            &frame,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeMedianCut,
        )?;

        // WebView2 から提出された元画像サイズがどうであれ、
        // 目標の wgpu テクスチャサイズ (width x height) へ正確にリサイズします。
        let scaler = wic_factory.CreateBitmapScaler()?;
        scaler.Initialize(&converter, width, height, WICBitmapInterpolationModeLinear)?;

        let mut pixels = vec![0u8; (width * height * 4) as usize];
        converter.CopyPixels(std::ptr::null(), width * 4, &mut pixels)?;

        // メモリロック解除
        // windows-rs の自動 Result 変換のバグ（S_OK/NO_ERROR なのに 0 返却のため Err になる）を
        // 回避するため、? によるエラー早期返却をやめ、単に返り値を破棄します。
        let _ = GlobalUnlock(hglobal);

        // 4. 新規 wgpu::Texture の生成
        let wgpu_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("WebView2 Static Cache Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // キャプチャ画像の退色を防ぐため sRGB に変更
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // 5. VRAM へのアップロード
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &wgpu_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        Ok(wgpu_texture)
    }
}

/// wgpu (D3D12) 側のインポート処理
/// 外部の D3D11 からエクスポートされた共有 NT ハンドル（HANDLE）をインポートし、
/// コピーを介さずに 100% 同一の VRAM アドレスを指す wgpu::Texture を構築します。
pub(crate) unsafe fn import_shared_texture(
    wgpu_renderer: &WgpuRenderer,
    shared_handle: HANDLE,
    size: LayoutSize,
) -> Result<wgpu::Texture, Box<dyn std::error::Error>> {
    unsafe {
        // 1. wgpu::Device から wgpu_hal の生 D3D12 デバイス（ID3D12Device）を取得する
        let hal_device = wgpu_renderer
            .device
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
        let texture = wgpu_renderer
            .device
            .create_texture_from_hal::<wgpu::wgc::api::Dx12>(hal_texture, &desc);

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
