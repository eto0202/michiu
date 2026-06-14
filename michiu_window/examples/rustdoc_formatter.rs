use michiu_window::{
    ComContext, Event, EventPump, Icon, LogicalSize, MichiuEvent, Tray, TrayBuilder, TrayMenuItem,
    Window, WindowBuilder, init_dpi_awareness,
};
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::mpsc,
};
use windows::Win32::UI::{
    Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey, VIRTUAL_KEY},
    WindowsAndMessaging::{IDI_APPLICATION, LoadIconW},
};

const HOTKEY_ID: i32 = 999;
const VK_R: VIRTUAL_KEY = VIRTUAL_KEY(0x52); // R key

// A function that parses comment prefixes (/// or //!) and returns the resulting raw code
fn strip_rustdoc_prefix(input: &str) -> (String, Option<&str>) {
    // Detect whether /// or //! appears first
    let prefix = input
        .lines()
        .map(|l| l.trim_start())
        .find(|l| l.starts_with("///") || l.starts_with("//!"))
        .map(|l| if l.starts_with("///") { "///" } else { "//!" });

    let Some(p) = prefix else {
        // If not commented out, return the text as-is
        return (input.to_string(), None);
    };

    let raw_code = input
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if let Some(mut content) = trimmed.strip_prefix(p) {
                if content.starts_with(' ') {
                    content = &content[1..];
                }
                content
            } else {
                // Lines that are not commented out (such as blank lines) are treated as blank lines
                ""
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    (raw_code, Some(p))
}

// A function that calls the system's `rustfmt` via standard input
fn run_rustfmt(code: &str) -> std::io::Result<String> {
    let mut child = Command::new("rustfmt")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped()) // Receive the error details
        .spawn()?;

    // Feed raw code into standard input
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(code.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let err_msg = String::from_utf8_lossy(&output.stderr).into_owned();
        Err(std::io::Error::other(err_msg))
    }
}

// A function that reapplies the first detected prefix (/// or //!) to formatted code
fn add_rustdoc_prefix(formatted_code: &str, prefix: &str) -> String {
    formatted_code
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                prefix.to_string()
            } else {
                format!("{} {}", prefix, line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n" // Restore the trailing newline
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_dpi_awareness();

    let com_ctx = ComContext::new_com_single()?;

    let hicon = unsafe { LoadIconW(None, IDI_APPLICATION)? };
    let icon = unsafe { Icon::from_raw(hicon) };

    let (tx, rx) = mpsc::channel::<()>();

    // Tray Menu
    let tx_for_menu = tx.clone();
    let tray_builder = TrayBuilder::new()
        .with_icon(icon.clone())
        .with_tooltip("Rustdoc Code Formatter (Ctrl+Shift+R)")
        .with_menu_item(
            TrayMenuItem::new("Exit Application").with_on_click(move || {
                let _ = tx_for_menu.send(());
            }),
        );

    let tray = Tray::build(tray_builder.into_unvalidated().try_into()?)?;
    let tray_clone = tray.clone();

    let builder = WindowBuilder::new()
        .with_title("Rustdoc Formatter Window")
        .with_visible(false)
        .with_taskbar_button(false)
        .with_com_context(&com_ctx)
        .with_tray(tray)
        .with_inner_size(LogicalSize::new(200.0, 150.0));

    let window = Window::build(builder.into_unvalidated().try_into()?)?;
    let handle = window.handle().assume_valid();

    // Registering a hotkey (Ctrl + Shift + R)
    let modifiers = HOT_KEY_MODIFIERS(0x0002 | 0x0004 | 0x4000);
    let _ = unsafe { RegisterHotKey(Some(window.hwnd()), HOTKEY_ID, modifiers, VK_R.0 as u32) };

    let mut event_pump = EventPump::new();

    println!("Intelligent Rustdoc Formatter is active on system tray.");
    println!(
        "How to use:\n1. Copy any rustdoc commented code block (with /// or //!).\n2. Press [Ctrl + Shift + R]\n3. The code INSIDE the comment will be beautifully formatted with rustfmt!"
    );

    while let Some(event) = event_pump.wait_event()? {
        if let MichiuEvent::Window { event, .. } = event {
            match event {
                Event::CloseRequested => {
                    handle.destroy();
                }
                Event::Destroyed => {
                    break;
                }
                // Hotkey detection (Ctrl+Shift+R)
                Event::UnsafeRaw {
                    msg: 0x0312, // WM_HOTKEY
                    wparam,
                    ..
                } if wparam.0 == HOTKEY_ID as usize => {
                    println!("[UI Thread] Formatter hotkey triggered!");

                    // Retrieve a string from the clipboard
                    match window.get_clipboard_text() {
                        Ok(raw_text) => {
                            if raw_text.trim().is_empty() {
                                let _ = tray_clone
                                    .show_balloon("Formatter Warning", "Clipboard is empty.");
                                continue;
                            }

                            let handle_clone = handle.clone();
                            let tray_inner_clone = tray_clone.clone();

                            // Format in a background thread
                            std::thread::spawn(move || {
                                // 1. Detect and strip comment prefixes (such as /// or //!)
                                let (raw_code, detected_prefix) = strip_rustdoc_prefix(&raw_text);

                                // 2. Format raw code using the system's `rustfmt`
                                match run_rustfmt(&raw_code) {
                                    Ok(formatted_code) => {
                                        // 3. Restore the document by reapplying the first prefix found
                                        let final_text = match detected_prefix {
                                            Some(prefix) => {
                                                add_rustdoc_prefix(&formatted_code, prefix)
                                            }
                                            None => formatted_code,
                                        };

                                        // 4. Overwrite the clipboard
                                        handle_clone.set_clipboard_text(final_text);

                                        let _ = tray_inner_clone.show_balloon(
                                            "Rustdoc Formatted!",
                                            "The internal code has been beautifully formatted with rustfmt!"
                                        );
                                    }
                                    Err(err) => {
                                        // Notification when formatting fails due to a syntax error or similar issue
                                        let err_msg = format!("rustfmt failed:\n{}", err);
                                        eprintln!("{}", err_msg);
                                        let _ = tray_inner_clone.show_balloon(
                                            "Format Failed (Syntax Error)",
                                            "Please verify that the code inside the comment has no syntax errors."
                                        );
                                    }
                                }
                            });
                        }
                        Err(_) => {
                            let _ = tray_clone.show_balloon(
                                "Formatter Error",
                                "Failed to retrieve clipboard text.",
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        while let Ok(()) = rx.try_recv() {
            println!("[UI Thread] Exit requested from tray menu.");
            handle.destroy();
        }
    }

    let _ = unsafe { UnregisterHotKey(Some(window.hwnd()), HOTKEY_ID) };

    Ok(())
}
