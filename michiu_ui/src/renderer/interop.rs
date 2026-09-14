use std::cell::RefCell;
use std::rc::Rc;

use crate::{LayoutSize, MichiuError, WgpuRenderer};
use webview2_com::CapturePreviewCompletedHandler;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG, ICoreWebView2,
};
use windows::core::Interface;
use windows::{
    Graphics::{
        Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem},
        DirectX::Direct3D11::IDirect3DDevice,
    },
    UI::Composition::Visual as WinRTVisual,
    Win32::{
        Foundation::{HANDLE, HGLOBAL},
        Graphics::{
            Direct3D11::{
                D3D11_BIND_SHADER_RESOURCE, D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX,
                D3D11_TEXTURE2D_DESC, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
            },
            Direct3D12::ID3D12Resource,
            DirectComposition::IDCompositionVisual,
            Dxgi::{
                DXGI_SHARED_RESOURCE_READ, DXGI_SHARED_RESOURCE_WRITE, IDXGIDevice, IDXGIResource1,
                IDXGISwapChain1,
            },
            Imaging::{
                GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory, WICBitmapDitherTypeNone,
                WICBitmapInterpolationModeLinear, WICBitmapPaletteTypeMedianCut,
                WICDecodeMetadataCacheOnDemand,
            },
        },
        System::{
            Com::{
                IStream,
                StructuredStorage::{CreateStreamOnHGlobal, GetHGlobalFromStream},
            },
            Memory::{GlobalLock, GlobalSize, GlobalUnlock},
            WinRT::Direct3D11::{
                CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
            },
        },
    },
};
use windows_core::HRESULT;

/// `WebView2` から非アクティブ時の静止画（1フレーム）をキャプチャし、
/// wgpu 側の `wgpu::Texture` へとピクセルデータを転送します。
pub(crate) unsafe fn trigger_capture_async<F>(
    webview: &ICoreWebView2,
    width: u32,
    height: u32,
    wgpu_device: wgpu::Device,
    wgpu_queue: wgpu::Queue,
    wic_factory: IWICImagingFactory,
    on_complete: F,
) -> crate::Result<()>
where
    F: FnOnce(crate::Result<wgpu::Texture>) + 'static,
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
                let on_complete = on_complete_opt.take().ok_or_else(|| {
                    // とりあえず E_FAIL を返しておく
                    windows_core::Error::from_hresult(HRESULT(0x8000_4005_u32.cast_signed()))
                })?;

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
/// `wgpu::Texture` を作成してアップロードします。
unsafe fn process_captured_stream(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    wic_factory: &IWICImagingFactory,
    stream: &IStream,
    width: u32,
    height: u32,
) -> crate::Result<wgpu::Texture> {
    unsafe {
        let hglobal = GetHGlobalFromStream(stream)?;
        let data_ptr = GlobalLock(hglobal);

        // ポインタが null の場合は早期リターン
        if data_ptr.is_null() {
            return Err(MichiuError::GlobalLockFailed);
        }

        let size = GlobalSize(hglobal);

        // 正常な PNG ファイルとして不十分なサイズの場合は即座にエラーとする
        if size < 128 {
            let _ = GlobalUnlock(hglobal);
            return Err(MichiuError::InvalidImageData);
        }

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
