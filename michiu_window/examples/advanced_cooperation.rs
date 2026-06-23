use michiu_window::{
    ComContext, Event, EventPump, Icon, LogicalSize, MichiuEvent, Tray, TrayBuilder, TrayMenuItem,
    Window, WindowBuilder, init_dpi_awareness,
};
use std::sync::mpsc;
use windows::Win32::UI::WindowsAndMessaging::{IDI_APPLICATION, LoadIconW};

// cargo run --example advanced_cooperation

// Define custom centralized application commands
enum AppCommand {
    UpdateTitle(String),
    Exit,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize High-DPI support (Per-Monitor v2)
    init_dpi_awareness();

    // 2. Initialize OLE STA COM context (required for Clipboard, Drag & Drop, and IME)
    let com_ctx = ComContext::new_com_single()?;

    // Load system default application icon
    let hicon = unsafe { LoadIconW(None, IDI_APPLICATION)? };
    let icon = unsafe { Icon::from_raw(hicon) };

    // Create a standard MPSC channel to centralize application logic
    let (tx, rx) = mpsc::channel();

    // 3. Build a System Tray Icon with a native exit menu item
    let tx_for_menu = tx.clone();
    let tray_builder = TrayBuilder::new()
        .with_icon(icon.clone())
        .with_tooltip("Quick Start App")
        // Trigger exit command asynchronously on the main loop when clicked
        .with_menu_item(
            TrayMenuItem::new("Exit Application").with_on_click(move || {
                let _ = tx_for_menu.send(AppCommand::Exit);
            }),
        );

    let tray = Tray::build(tray_builder.into_unvalidated().try_into()?)?;
    let tray_clone = tray.clone();

    // 4. Configure, validate, and build the window
    let builder = WindowBuilder::new()
        .with_title("Michiu Quick Start")
        .with_icon(icon.clone())
        .with_com_context(&com_ctx)
        .with_drag_and_drop(true) // Enable OLE file dropping
        .with_tray(tray) // Bind the system tray icon to the window
        .with_inner_size(LogicalSize::new(640.0, 480.0));

    let validated = builder.into_unvalidated().try_into()?;
    let window = Window::build(validated)?;
    let handle = window.handle().assume_valid();

    // 5. Create an MPSC channel and drive asynchronous tasks in the background
    let handle_clone = handle.clone();
    let tx_clone = tx.clone();

    std::thread::spawn(move || {
        // Simulate a heavy background task (e.g., file processing or network request)
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Send a command to update the UI
        tx_clone
            .send(AppCommand::UpdateTitle(
                "Title updated from Thread!".to_string(),
            ))
            .unwrap();

        // Wake up the UI thread's message loop immediately (low latency, zero busy polling)
        handle_clone.wake_up();
    });

    // 6. Initialize the EventPump to drive the message loop on the UI thread
    let mut event_pump = EventPump::new();

    // Process OS window and input events first (DPI, close requested, resize, drag & drop, etc.)
    while let Some(event) = event_pump.wait_event()? {
        if let MichiuEvent::Window { event, .. } = event {
            match event {
                Event::CloseRequested => {
                    // Safely trigger window destruction directly from the event loop using the handle
                    handle.destroy();
                }
                Event::Destroyed => {
                    // Exit the message loop cleanly after the window is physically destroyed
                    break;
                }
                Event::FileDropped(unvalidated_files) => {
                    // Securely validate raw OS inputs before letting them mutate application state
                    // For example, verify that all dropped paths actually exist on the disk.
                    let validated_files = unvalidated_files.validate_with(|paths| {
                        if paths.iter().all(|path| path.exists() && path.is_file()) {
                            Ok(paths)
                        } else {
                            Err("Some dropped paths do not exist or are directories.")
                        }
                    });

                    if let Ok(files) = validated_files {
                        // `files` is now a safe `Validated<Vec<PathBuf>>`
                        println!("Dropped files validated: {:?}", files.into_inner());
                        let _ = tray_clone.show_balloon(
                            "File Received",
                            "The dropped file has been verified and processed.",
                        );
                    } else {
                        eprintln!("File drop validation failed!");
                    }
                }
                _ => {}
            }
        }

        // Process custom application commands sequentially (Safe UI mutations on the UI thread)
        while let Ok(command) = rx.try_recv() {
            match command {
                AppCommand::UpdateTitle(title) => {
                    window.set_title(title);
                }
                AppCommand::Exit => {
                    handle.destroy();
                }
            }
        }
    }
    Ok(())
}
