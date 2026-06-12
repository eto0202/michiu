use michiu_guard::Validated;
use michiu_window::{
    ComContext, EventPump, FileDropTarget, ImeContext, LogicalSize, MichiuEvent, WindowBuilder,
    WindowEvent, init_dpi_awareness,
};
use std::io::Read;
use std::net::TcpStream;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{E_NOTIMPL, HGLOBAL, LPARAM, POINT, POINTL, S_OK, WPARAM};
use windows::Win32::System::Com::{
    FORMATETC, IAdviseSink, IDataObject, IDataObject_Impl, IEnumFORMATETC, IEnumSTATDATA,
    STGMEDIUM, TYMED_HGLOBAL,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::{CF_HDROP, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropTarget};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Shell::DROPFILES;
use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_IME_COMPOSITION};
use windows_core::{BOOL, HRESULT, Ref, implement};

// cargo test --test com_and_ime

fn run_on_clean_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let handle = std::thread::spawn(f);
    handle.join().expect("Test thread panicked");
}

#[test]
fn test_integration_com_sta_and_ime_relay_lifecycle() {
    let _ = init_dpi_awareness();

    run_on_clean_thread(|| {
        // OLE STA コンテキストの初期化
        let com_ctx = ComContext::new_com_single().expect("Failed to initialize OLE STA context");

        // テスト並行実行時のポート重複を防ぐため、スレッドIDから安全な一意のポートを算出 (10000 - 19999)
        let thread_id = unsafe { GetCurrentThreadId() };
        let test_port = 10000 + (thread_id % 10000) as u16;

        // ドラッグ＆ドロップを有効にし、IME中継用ポートを指定してウィンドウを構築
        let builder = WindowBuilder::new()
            .with_title("COM and IME Integration Window")
            .with_com_context(&com_ctx)
            .with_drag_and_drop(true) // OLE D&D 有効
            .with_ime_expose_port(test_port) // IME 中継サーバー有効化
            .with_inner_size(LogicalSize::new(400.0, 300.0));

        let validated = builder
            .into_unvalidated()
            .try_into()
            .expect("Integration builder validation failed");

        let window = michiu_window::Window::build(validated)
            .expect("Failed to build window with OLE and IME Relay");

        let main_id = window.id();

        // ローカルホストの中継サーバーに TCP クライアントとして安全に接続を試みる
        // 起動直後のリスナーバインド完了に備え、タイムアウト付きでリトライ接続
        let mut client_stream = None;
        let connect_start = Instant::now();
        while connect_start.elapsed() < Duration::from_secs(1) {
            if let Ok(stream) = TcpStream::connect(format!("127.0.0.1:{}", test_port)) {
                // UIスレッドをフリーズさせないよう非ブロッキングに設定
                stream.set_nonblocking(true).unwrap();
                client_stream = Some(stream);
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        let mut stream = client_stream.expect("Failed to connect to localhost IME Relay Server");

        // テスト用に物理IMEの仮想プロパティをあらかじめ少し変更しておく
        if let Ok(ctx) = ImeContext::new(window.hwnd()) {
            ctx.set_open(true);
            ctx.set_conversion_status(2, 0); // 変換モードを仮セット
        }

        // WM_IME_COMPOSITION メッセージを同期送信 (SendMessage) し、IME更新処理を意図的に誘発
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_IME_COMPOSITION,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            );
        }

        // メッセージループを回して、WndProc内の IME 更新翻訳・送信ロジックを実行させる
        let mut event_pump = EventPump::new();
        let loop_start = Instant::now();
        let mut received_json = String::new();

        // まずはTCP経由でJSONテキストを受信する
        while loop_start.elapsed() < Duration::from_secs(2) {
            let _ = event_pump.poll_event();

            let mut buf = [0u8; 1024];
            if let Ok(bytes_read) = stream.read(&mut buf)
                && bytes_read > 0
            {
                let json_str = String::from_utf8_lossy(&buf[..bytes_read]).into_owned();
                let expected_key = format!("\"window_id\":{}", main_id.id());
                if json_str.contains(&expected_key) {
                    received_json = json_str;
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(
            !received_json.is_empty(),
            "Failed to receive JSON from IME server"
        );

        // リアルタイムで取得したJSONテキストを一時ファイル（ime_data.json）として書き出す
        let temp_dir = std::env::temp_dir();
        let temp_file_path = temp_dir.join("ime_data.json");
        std::fs::write(&temp_file_path, &received_json)
            .expect("Failed to write temporary JSON file");

        // OLE ドラッグ＆ドロップの偽装メッセージを生成して自ウィンドウへ
        let drop_files = vec![temp_file_path.clone()];
        let mock_data: IDataObject = MockDataObject { paths: drop_files }.into();

        // WindowsのIDropTargetとして公開されたFileDropTargetを統合テスト側でインスタンス化して偽装Dropを実行
        let drop_target_impl = FileDropTarget::new(window.hwnd());
        let drop_target: IDropTarget = drop_target_impl.into();

        let mut effect = DROPEFFECT_NONE;
        let res_drop = unsafe {
            drop_target.Drop(
                &mock_data,
                MODIFIERKEYS_FLAGS(0),
                POINTL { x: 0, y: 0 },
                &mut effect,
            )
        };

        assert!(res_drop.is_ok(), "Simulated drop target call failed");
        assert_eq!(effect, DROPEFFECT_COPY);

        // イベントループを回し、プッシュされた WindowEvent::FileDropped を回収する
        let loop_start_dnd = Instant::now();
        let mut dnd_verified = false;

        while loop_start_dnd.elapsed() < Duration::from_secs(2) {
            if let Some(event) = event_pump.poll_event()
                && let MichiuEvent::WindowEvent { window_id, event } = event
            {
                assert_eq!(window_id, main_id);

                if let WindowEvent::FileDropped(unvalidated_files) = event {
                    // 検証を行いドロップされたファイルの中身をチェック
                    let validated_res: Result<Validated<Vec<PathBuf>>, &str> = unvalidated_files
                        .validate_with(|files| {
                            assert_eq!(files.len(), 1, "Expected exactly one dropped file");
                            assert_eq!(files[0], temp_file_path, "Dropped file path mismatched");

                            // ドロップされたファイル（一時ファイル）の中身を読み込む
                            let mut file_content = String::new();
                            let mut file = std::fs::File::open(&files[0]).unwrap();
                            file.read_to_string(&mut file_content).unwrap();

                            // ドロップされたファイルの中身が、先ほどIMEサーバーから受信したJSONデータと完全一致することを確認
                            assert_eq!(
                                file_content, received_json,
                                "Dropped file content mismatched the original IME JSON data"
                            );

                            Ok(files)
                        });

                    assert!(validated_res.is_ok(), "FileDropped event validation failed");
                    dnd_verified = true;
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        // 一時ファイルを削除
        let _ = std::fs::remove_file(&temp_file_path);

        assert!(
            dnd_verified,
            "Failed to capture and verify the simulated FileDropped event carrying IME JSON data"
        );

        // クリーンアップ
        window.destroy();
    });
}

#[implement(IDataObject)]
struct MockDataObject {
    paths: Vec<PathBuf>,
}

impl IDataObject_Impl for MockDataObject_Impl {
    fn GetData(&self, pformatetc: *const FORMATETC) -> windows::core::Result<STGMEDIUM> {
        unsafe {
            if (*pformatetc).cfFormat == CF_HDROP.0 {
                let h_global = create_mock_hdrop(&self.paths);
                let mut medium = STGMEDIUM {
                    tymed: TYMED_HGLOBAL.0 as u32,
                    ..Default::default()
                };
                medium.u.hGlobal = h_global;
                return Ok(medium);
            }
        }
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn GetDataHere(&self, _: *const FORMATETC, _: *mut STGMEDIUM) -> windows::core::Result<()> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
        unsafe {
            if !pformatetc.is_null() && (*pformatetc).cfFormat == CF_HDROP.0 {
                return S_OK;
            }
        }
        E_NOTIMPL
    }

    fn GetCanonicalFormatEtc(&self, _: *const FORMATETC, _: *mut FORMATETC) -> HRESULT {
        E_NOTIMPL
    }

    fn SetData(
        &self,
        _: *const FORMATETC,
        _: *const STGMEDIUM,
        _: BOOL,
    ) -> windows::core::Result<()> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn EnumFormatEtc(&self, _: u32) -> windows::core::Result<IEnumFORMATETC> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn DAdvise(
        &self,
        _: *const FORMATETC,
        _: u32,
        _: Ref<IAdviseSink>,
    ) -> windows::core::Result<u32> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn DUnadvise(&self, _: u32) -> windows::core::Result<()> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn EnumDAdvise(&self) -> windows::core::Result<IEnumSTATDATA> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }
}

unsafe fn create_mock_hdrop(paths: &[PathBuf]) -> HGLOBAL {
    let mut path_bytes = Vec::new();
    for p in paths {
        let wide: Vec<u16> = p.as_os_str().encode_wide().chain(Some(0)).collect();
        path_bytes.extend_from_slice(&wide);
    }
    path_bytes.push(0);

    let dropfiles_size = std::mem::size_of::<DROPFILES>();
    let total_size = dropfiles_size + (path_bytes.len() * 2);

    let h_mem = unsafe { GlobalAlloc(GMEM_MOVEABLE, total_size).unwrap() };
    let ptr = unsafe { GlobalLock(h_mem) };

    let dropfiles = DROPFILES {
        pFiles: dropfiles_size as u32,
        pt: POINT { x: 0, y: 0 },
        fNC: false.into(),
        fWide: true.into(),
    };

    unsafe {
        std::ptr::write(ptr as *mut DROPFILES, dropfiles);
        std::ptr::copy_nonoverlapping(
            path_bytes.as_ptr(),
            (ptr as *mut u8).add(dropfiles_size) as *mut u16,
            path_bytes.len(),
        );
    }

    let _ = unsafe { GlobalUnlock(h_mem) };
    h_mem
}
