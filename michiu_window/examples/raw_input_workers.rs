use michiu_guard::Validated;
use michiu_window::{
    ComContext, Event, EventPump, Icon, LogicalSize, MichiuEvent, PhysicalPoint, Tray, TrayBuilder,
    TrayMenuItem, Window, WindowBuilder, WindowHandle, init_dpi_awareness,
};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::Threading::{
    CreateWaitableTimerW, INFINITE, SetWaitableTimer, WaitForSingleObject,
};
use windows::Win32::UI::Input::{RAWINPUTDEVICE, RIDEV_INPUTSINK, RegisterRawInputDevices};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, IDI_APPLICATION, KillTimer, LoadIconW, SetTimer,
};

// cargo run --example raw_input_workers

// A unique ID to identify the timer
const TIMER_ID: usize = 777;

// 1. A type-safe enum that is merged when exchanged between threads and via the tray menu
#[derive(Debug, Clone)]
enum WorkerReport {
    // Physical mouse coordinates on the screen reported by Thread A
    CursorPos(PhysicalPoint),
    // Results of the high-load calculation reported by Thread B
    Calculation(u128),
    // Quit command sent from the tray menu
    Exit,
}

// Categories for identifying the processing characteristics of each thread
enum WorkerRole {
    CursorTracker,
    HeavyCalculator,
}

struct WorkerControl {
    active: bool,
    quit: bool,
}

type ThreadSignal = Arc<(Mutex<WorkerControl>, Condvar)>;

// Dummy calculation to be executed on worker thread B
fn heavy_calculation(n: u32) -> u128 {
    let mut sum: u128 = 0;
    for i in 0..n {
        sum = sum.wrapping_add(i as u128);
    }
    sum
}

// 2. A worker that uses a Waitable Timer for high-precision sleep-driven operation
// Return data is sent using mpsc::Sender, and the main thread is woken up with handle.wake_up()
fn spawn_worker_thread(
    role: WorkerRole,
    signal: ThreadSignal,
    tx: mpsc::Sender<WorkerReport>,
    handle: Validated<WindowHandle>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let (lock, cvar) = &*signal;

        let mut loop_count: u32 = 0;

        // Create a Waitable Timer object in Windows OS
        let h_timer = unsafe {
            CreateWaitableTimerW(None, false, windows::core::PCWSTR::null())
                .expect("Failed to create waitable timer")
        };

        loop {
            let mut control = lock.lock().unwrap();

            // When the mouse is not moving, suspend the thread using a condition variable
            while !control.active && !control.quit {
                control = cvar.wait(control).unwrap();
            }

            if control.quit {
                break;
            }

            drop(control);

            match role {
                // Thread A: Get mouse coordinates
                WorkerRole::CursorTracker => {
                    let mut pt = windows::Win32::Foundation::POINT::default();
                    let _ = unsafe { GetCursorPos(&mut pt) };
                    // mpsc で送信し、即座にUIスレッドのメッセージスリープを起こす
                    tx.send(WorkerReport::CursorPos(PhysicalPoint::new(pt.x, pt.y)))
                        .unwrap();
                    handle.wake_up();
                }
                // Thread B: Heavy numerical calculations
                WorkerRole::HeavyCalculator => {
                    loop_count = loop_count.wrapping_add(1);
                    let calculated_val = heavy_calculation(100_000 + (loop_count % 1000));
                    tx.send(WorkerReport::Calculation(calculated_val)).unwrap();
                    handle.wake_up();
                }
            }

            // High-precision 5-millisecond sleep using a Waitable Timer
            let due_time: i64 = -50_000;
            let _ =
                unsafe { SetWaitableTimer(h_timer, &due_time as *const i64, 0, None, None, false) };

            // Suspend the thread until the timer enters the signal state (5 ms have elapsed)
            let _ = unsafe { WaitForSingleObject(h_timer, INFINITE) };
        }

        let _ = unsafe { CloseHandle(h_timer) };
    })
}

// Helper function for registering RawInput with the Windows OS
fn register_rawinput_devices(hwnd: HWND) {
    let rid = RAWINPUTDEVICE {
        usUsagePage: 1, // Generic Desktop Page
        usUsage: 2,     // Mouse Usage ID
        dwFlags: RIDEV_INPUTSINK,
        hwndTarget: hwnd,
    };
    unsafe {
        RegisterRawInputDevices(&[rid], std::mem::size_of::<RAWINPUTDEVICE>() as u32)
            .expect("Failed to register raw input mouse device");
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize high-DPI support
    init_dpi_awareness();

    let com_ctx = ComContext::new_com_single()?;

    let hicon = unsafe { LoadIconW(None, IDI_APPLICATION)? };
    let icon = unsafe { Icon::from_raw(hicon) };

    // Preparing a channel to centrally collect instructions from each thread, tray, etc.
    let (tx, rx) = mpsc::channel();

    // 3. Creating a system tray icon (Adding an Exit menu item)
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

    // Control signals for worker threads
    let signal: ThreadSignal = Arc::new((
        Mutex::new(WorkerControl {
            active: false,
            quit: false,
        }),
        Condvar::new(),
    ));

    // Clone for MessageFilter
    let signal_for_filter = signal.clone();

    // Creating a special window that is invisible and does not perform hit testing
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

                // Reset and restart the existing 2-second timer every time the mouse moves
                let _ = unsafe { KillTimer(Some(hwnd), TIMER_ID) };
                let _ = unsafe { SetTimer(Some(hwnd), TIMER_ID, 2000, None) };

                let (lock, cvar) = &*signal_for_filter;
                let mut control = lock.lock().unwrap();
                if !control.active {
                    control.active = true;
                    cvar.notify_all(); // Start both Worker A and Worker B at once
                    println!("[UI Thread] Mouse raw input detected! Activating special workers...");
                }
            }
            // Continue with normal processing
            None
        });

    let window = Window::build(builder.into_unvalidated().try_into()?)?;
    let handle = window.handle().assume_valid();

    // Registering RawInput with the Windows OS
    register_rawinput_devices(window.hwnd());

    // 4. Starting the worker thread
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

    // Message pump that drives the UI thread
    let mut event_pump = EventPump::new();

    // Variables for data management
    let mut total_mouse_samples = 0u64;
    let mut total_calc_samples = 0u64;
    let mut last_position = PhysicalPoint::new(0, 0);
    let mut last_calculated_val = 0u128;

    println!(
        "System ready. Move your physical mouse to wake up specialized worker threads. Right-click the tray icon to Exit."
    );

    // 5. Event-driven loop using `wait_event()`
    while let Some(event) = event_pump.wait_event()? {
        if let MichiuEvent::Window { event, .. } = event {
            match event {
                Event::CloseRequested => {
                    handle.destroy();
                }
                Event::Destroyed => {
                    break;
                }
                // If the mouse remains idle for 2 seconds,
                // the OS automatically posts a `WM_TIMER` message to wake it up
                Event::UnsafeRaw { msg: 0x0113, .. } => {
                    println!(
                        "[UI Thread] 2 seconds of silence detected. Suspending all specialized workers..."
                    );

                    let (lock, cvar) = &*signal;
                    let mut control = lock.lock().unwrap();
                    control.active = false;
                    cvar.notify_all();

                    drop(control);

                    // System toast notifications for aggregated statistics
                    let notification_text = format!(
                        "Threads suspended safely.\nFinal Mouse Pos: ({}, {})\nFinal Calc Val: ({})\nProcessed Samples: (Mouse: {}, Calc: {})",
                        last_position.x,
                        last_position.y,
                        last_calculated_val,
                        total_mouse_samples,
                        total_calc_samples
                    );

                    let _ = tray_clone.show_balloon("System Idle Detected", &notification_text);

                    // Set the timer to function as a one-shot timer

                    let _ = unsafe { KillTimer(Some(window.hwnd()), TIMER_ID) };

                    // Clean up statistical data in preparation for the next wake-up
                    total_mouse_samples = 0;
                    total_calc_samples = 0;
                }
                _ => {}
            }
        }

        // Immediately after waking up via wake_up() (WM_NULL),
        // merge all worker results queued in the channel at once on the UI thread
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
            // Display with appropriate spacing
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

    // 6. Set all threads to the Quit state to ensure a safe shutdown
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
