use michiu_guard::Validated;
use michiu_window::{
    ComContext, Event, EventPump, Icon, LogicalSize, MichiuEvent, PhysicalPoint, Tray, TrayBuilder,
    TrayMenuItem, Window, WindowBuilder, WindowHandle, init_dpi_awareness,
};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    CreateWaitableTimerW, INFINITE, SetWaitableTimer, WaitForSingleObject,
};
use windows::Win32::UI::Input::{RAWINPUTDEVICE, RIDEV_INPUTSINK, RegisterRawInputDevices};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, IDI_APPLICATION, KillTimer, LoadIconW, SetTimer,
};

// タイマーを識別するための一意なID
const TIMER_ID: usize = 777;

// 1. スレッド間およびトレイメニュー間でやり取りする、型安全にマージされる結果 enum
#[derive(Debug, Clone)]
enum WorkerReport {
    // スレッドAから報告される、画面上の物理マウス座標
    CursorPos(PhysicalPoint),
    // スレッドBから報告される、高負荷計算の結果値
    Calculation(u128),
    // トレイメニューから送られる終了要求コマンド
    Exit,
}

// スレッドごとの処理特性を識別する区分
enum WorkerRole {
    CursorTracker,
    HeavyCalculator,
}

struct WorkerControl {
    active: bool,
    quit: bool,
}

type ThreadSignal = Arc<(Mutex<WorkerControl>, Condvar)>;

// ワーカースレッドB側で実行させるダミー計算
fn heavy_calculation(n: u32) -> u128 {
    let mut sum: u128 = 0;
    for i in 0..n {
        sum = sum.wrapping_add(i as u128);
    }
    sum
}

// 2. Waitable Timer を用いて高精度にスリープ駆動するワーカー
// 帰りのデータは mpsc::Sender で送り、handle.wake_up() でメインを起こす
fn spawn_worker_thread(
    role: WorkerRole,
    signal: ThreadSignal,
    tx: mpsc::Sender<WorkerReport>,
    handle: Validated<WindowHandle>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let (lock, cvar) = &*signal;

        let mut loop_count: u32 = 0;

        unsafe {
            // Windows OS の Waitable Timer オブジェクトを作成
            let h_timer = CreateWaitableTimerW(None, false, windows::core::PCWSTR::null())
                .expect("Failed to create waitable timer");

            loop {
                let mut control = lock.lock().unwrap();

                // マウス無移動時は、条件変数によりスレッドをサスペンド
                while !control.active && !control.quit {
                    control = cvar.wait(control).unwrap();
                }

                if control.quit {
                    break;
                }

                drop(control);

                match role {
                    // スレッドA: マウス座標取得
                    WorkerRole::CursorTracker => {
                        let mut pt = windows::Win32::Foundation::POINT::default();
                        let _ = GetCursorPos(&mut pt);
                        // mpsc で送信し、即座にUIスレッドのメッセージスリープを起こす
                        tx.send(WorkerReport::CursorPos(PhysicalPoint::new(pt.x, pt.y)))
                            .unwrap();
                        handle.wake_up();
                    }
                    // スレッドB: 重い数値計算
                    WorkerRole::HeavyCalculator => {
                        loop_count = loop_count.wrapping_add(1);
                        let calculated_val = heavy_calculation(100_000 + (loop_count % 1000));
                        tx.send(WorkerReport::Calculation(calculated_val)).unwrap();
                        handle.wake_up();
                    }
                }

                // Waitable Timer による高精度な5msスリープ
                let due_time: i64 = -50_000;
                let _ = SetWaitableTimer(h_timer, &due_time as *const i64, 0, None, None, false);

                // タイマーがシグナル状態（5ms経過）になるまでスレッドを休止
                let _ = WaitForSingleObject(h_timer, INFINITE);
            }

            // タイマーハンドルのクローズ
            let _ = CloseHandle(h_timer);
        }
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 高DPIサポートの初期化
    init_dpi_awareness();

    let com_ctx = ComContext::new_com_single()?;

    let hicon = unsafe { LoadIconW(None, IDI_APPLICATION)? };
    let icon = unsafe { Icon::from_raw(hicon) };

    // 各スレッド・トレイ等から指示を集中回収するチャネルの準備
    let (tx, rx) = mpsc::channel();

    // 3. システムトレイアイコンの構築 (Exit メニュー項目の追加)
    let tx_for_menu = tx.clone();
    let tray_builder = TrayBuilder::new()
        .with_icon(icon.clone())
        .with_tooltip("RawInput Worker Controller")
        .with_menu_item(
            TrayMenuItem::new("Exit Application").with_on_click(move || {
                let _ = tx_for_menu.send(WorkerReport::Exit);
            }),
        );

    let tray = Tray::build(tray_builder.into_unvalidated().try_into()?)?;
    let tray_clone = tray.clone();

    // ワーカースレッド用の制御信号
    let signal: ThreadSignal = Arc::new((
        Mutex::new(WorkerControl {
            active: false,
            quit: false,
        }),
        Condvar::new(),
    ));

    // MessageFilter用のクローン
    let signal_for_filter = signal.clone();

    // 不可視、ヒットテストなしの特殊ウィンドウの構築
    let builder = WindowBuilder::new()
        .with_title("RawInput Invisible Window")
        .with_visible(false)
        .with_hittest(false)
        .with_inner_size(LogicalSize::new(0.0, 0.0))
        .with_com_context(&com_ctx)
        .with_tray(tray)
        .with_message_filter(move |hwnd, msg, _wparam, _lparam| {
            if msg == 0x00FF {
                // WM_INPUT
                unsafe {
                    // マウスが動くたびに、既存の2秒タイマーをリセットして再起動
                    let _ = KillTimer(Some(hwnd), TIMER_ID);
                    let _ = SetTimer(Some(hwnd), TIMER_ID, 2000, None);
                }

                let (lock, cvar) = &*signal_for_filter;
                let mut control = lock.lock().unwrap();
                if !control.active {
                    control.active = true;
                    cvar.notify_all(); // A/B両ワーカーを一挙に起こす
                    println!("[UI Thread] Mouse raw input detected! Activating special workers...");
                }
            }
            None
        });

    let window = Window::build(builder.into_unvalidated().try_into()?)?;
    let handle = window.handle().assume_valid();

    // Windows OS へ RawInput の登録
    let rid = RAWINPUTDEVICE {
        usUsagePage: 1, // Generic Desktop Page
        usUsage: 2,     // Mouse Usage ID
        dwFlags: RIDEV_INPUTSINK,
        hwndTarget: window.hwnd(),
    };
    unsafe {
        RegisterRawInputDevices(&[rid], std::mem::size_of::<RAWINPUTDEVICE>() as u32)
            .expect("Failed to register raw input mouse device");
    }

    // 4. ワーカースレッドの起動
    let worker_a = spawn_worker_thread(
        WorkerRole::CursorTracker,
        signal.clone(),
        tx.clone(),
        handle.clone(),
    );
    let worker_b = spawn_worker_thread(
        WorkerRole::HeavyCalculator,
        signal.clone(),
        tx.clone(),
        handle.clone(),
    );

    // UIスレッドを駆動させるメッセージポンプ
    let mut event_pump = EventPump::new();

    // データ管理用変数
    let mut total_mouse_samples = 0u64;
    let mut total_calc_samples = 0u64;
    let mut last_position = PhysicalPoint::new(0, 0);
    let mut last_calculated_val = 0u128;

    println!(
        "System ready. Move your physical mouse to wake up specialized worker threads. Right-click the tray icon to Exit."
    );

    // 5. wait_event() を用いたイベント駆動ループ
    while let Some(event) = event_pump.wait_event()? {
        #[allow(clippy::single_match)]
        match event {
            MichiuEvent::Window { id: _id, event } => match event {
                Event::CloseRequested => {
                    handle.destroy();
                }
                Event::Destroyed => {
                    break;
                }
                // C. マウスが止まって2秒が経過すると、OSが自動的に `WM_TIMER` をポストして目覚めさせる
                Event::UnsafeRaw { msg: 0x0113, .. } => {
                    println!(
                        "[UI Thread] 2 seconds of silence detected. Suspending all specialized workers..."
                    );

                    let (lock, cvar) = &*signal;
                    let mut control = lock.lock().unwrap();
                    control.active = false;
                    cvar.notify_all();

                    drop(control);

                    // 集約された統計結果をシステムトースト通知
                    let notification_text = format!(
                        "Threads suspended safely.\nFinal Mouse Pos: ({}, {})\nFinal Calc Val: ({})\nProcessed Samples: (Mouse: {}, Calc: {})",
                        last_position.x,
                        last_position.y,
                        last_calculated_val,
                        total_mouse_samples,
                        total_calc_samples
                    );

                    let _ = tray_clone.show_balloon("System Idle Detected", &notification_text);

                    // タイマーはワンショットとして機能させるため、一度 Kill して解除
                    unsafe {
                        let _ = KillTimer(Some(window.hwnd()), TIMER_ID);
                    }

                    // 次の起床に向けて統計データをクリーンアップ
                    total_mouse_samples = 0;
                    total_calc_samples = 0;
                }
                _ => {}
            },
            _ => {}
        }

        // D. wake_up()（WM_NULL）による起床の直後、チャネルに溜まっている全ワーカーの結果を
        // UIスレッド上で一括マージ
        while let Ok(report) = rx.try_recv() {
            match report {
                WorkerReport::CursorPos(pos) => {
                    total_mouse_samples += 1;
                    last_position = pos;
                }
                WorkerReport::Calculation(val) => {
                    total_calc_samples += 1;
                    last_calculated_val = val;
                }
                WorkerReport::Exit => {
                    println!("[UI Thread] Exit command received from System Tray context menu.");
                    handle.destroy();
                }
            }

            let total_combined = total_mouse_samples + total_calc_samples;
            // 適度に間引いて表示
            if total_combined.is_multiple_of(100) {
                println!(
                    "[UI Thread] Mouse Pos: ({:4}, {:4}) | Last Calc Val: {:10} | Samples: (Mouse: {}, Calc: {})",
                    last_position.x,
                    last_position.y,
                    last_calculated_val,
                    total_mouse_samples,
                    total_calc_samples
                );
            }
        }
    }

    // 6. 全てのスレッドを Quit 状態にして安全に終了させる
    {
        let (lock, cvar) = &*signal;
        let mut control = lock.lock().unwrap();
        control.quit = true;
        cvar.notify_all();
    }

    worker_a.join().unwrap();
    worker_b.join().unwrap();

    println!("Application exited cleanly.");
    Ok(())
}
