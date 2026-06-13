use eframe::egui;
use michiu_window::{
    ComContext, Event, EventPump, Icon, LogicalSize, MichiuEvent, Tray, TrayBuilder, Window,
    WindowBuilder, WindowHandle, init_dpi_awareness,
};
use std::path::PathBuf;
use std::sync::mpsc;
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_NOTOPMOST, HWND_TOPMOST, IDI_APPLICATION, LoadIconW, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SetWindowPos,
};

// Do we really need to implement this in michiu_window...?

// 1. Control commands sent from the front end to the back end
enum GuiCommand {
    // Open a native window for drag-and-drop (michiu window)
    OpenDropWindow,
    // Close the native window for drag-and-drop operations
    CloseDropWindow,
    // Trigger a toast (balloon) notification
    ShowToast { title: String, body: String },
    // Copy text to the clipboard
    WriteClipboard(String),
    // Load text from the clipboard and synchronize it
    ReadClipboard,
}

// 2. Event data passed from the backend to the frontend
#[derive(Debug, Clone)]
enum IpcMessage {
    // Synchronizing the open/closed status of the drop-down window
    DropWindowStatus(bool),
    // List of drop files that passed boundary validation
    FileDropped(Vec<PathBuf>),
    // Text read from the clipboard
    ClipboardText(String),
}

// 3. eframe application state management structure
struct IpcBridgeApp {
    tx_gui: mpsc::Sender<GuiCommand>,
    rx_ipc: mpsc::Receiver<IpcMessage>,
    // Handle for waking up the background thread
    // Not required when using poll_event()
    handle: WindowHandle,

    // Front-end state
    drop_window_open: bool,
    clipboard_write_buf: String,
    last_clipboard_text: String,
    dropped_files: Vec<PathBuf>,
}

impl IpcBridgeApp {
    fn new(
        _cc: &eframe::CreationContext<'_>,
        tx_gui: mpsc::Sender<GuiCommand>,
        rx_ipc: mpsc::Receiver<IpcMessage>,
        handle: WindowHandle,
    ) -> Self {
        Self {
            tx_gui,
            rx_ipc,
            handle,
            drop_window_open: false,
            clipboard_write_buf: "Type text to write to clipboard".to_string(),
            last_clipboard_text: "(No data read yet)".to_string(),
            dropped_files: Vec::new(),
        }
    }
}

impl eframe::App for IpcBridgeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Retrieve all data received from the backend in one go and synchronize it with the screen state
        while let Ok(msg) = self.rx_ipc.try_recv() {
            match msg {
                IpcMessage::DropWindowStatus(open) => {
                    self.drop_window_open = open;
                }
                IpcMessage::FileDropped(files) => {
                    self.dropped_files = files;
                }
                IpcMessage::ClipboardText(text) => {
                    self.last_clipboard_text = text;
                }
            }
        }

        // Rendering the control panel using egui
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Win32 System IPC Bridge Panel (egui Frontend)");
            ui.separator();

            // A. Tray Notification Test Section
            ui.group(|ui| {
                ui.label(egui::RichText::new("🔔 Native Toast Notification (System Tray):").strong());
                if ui.button("Trigger Toast Notification").clicked() {
                    let _ = self.tx_gui.send(GuiCommand::ShowToast {
                        title: "Hello from egui UI!".to_string(),
                        body: "This system toast was generated dynamically via the IPC bridge.".to_string(),
                    });
                    self.handle.wake_up(); // Start the backend and let it process the data
                }
            });

            // B. Clipboard Control Test Section
            ui.group(|ui| {
                ui.label(egui::RichText::new("Clipboard Interaction:").strong());

                ui.horizontal(|ui| {
                    if ui.button("Read System Clipboard").clicked() {
                        let _ = self.tx_gui.send(GuiCommand::ReadClipboard);
                        self.handle.wake_up();
                    }

                    ui.text_edit_singleline(&mut self.clipboard_write_buf);
                    if ui.button("Write to Clipboard").clicked() {
                        let _ = self.tx_gui.send(GuiCommand::WriteClipboard(self.clipboard_write_buf.clone()));
                        self.handle.wake_up();
                    }
                });

                ui.monospace(format!("Clipboard Content: \"{}\"", self.last_clipboard_text));
            });

            // C. Native D&D Window Integration Section
            ui.group(|ui| {
                ui.label(egui::RichText::new("Co-operative Native File Dropper:").strong());

                if self.drop_window_open {
                    if ui.button("Close Native Drop Area").clicked() {
                        let _ = self.tx_gui.send(GuiCommand::CloseDropWindow);
                        self.handle.wake_up();
                    }
                    ui.colored_label(egui::Color32::GREEN, "The Native Window is currently OPEN. Drag & Drop files into it!");
                } else {
                    if ui.button("Open Native Drop Area").clicked() {
                        let _ = self.tx_gui.send(GuiCommand::OpenDropWindow);
                        self.handle.wake_up();
                    }
                    ui.label("Click above to open the native Win32 window and drag files there.");
                }

                ui.separator();
                ui.label("Dropped files verified through michiu_guard firewall:");
                if self.dropped_files.is_empty() {
                    ui.label("(No files dropped yet)");
                } else {
                    for file in &self.dropped_files {
                        ui.monospace(format!("-> {:?}", file));
                    }
                }
            });

            ui.separator();
            ui.label(egui::RichText::new("Notice: All OS-dependent low-level logic is safely isolated in the Win32 thread.").weak());
        });
    }
}

// 4. A headless Win32 IPC bridge window that runs independently in the background
fn spawn_win32_backend(
    tx_ipc: mpsc::Sender<IpcMessage>,
    rx_gui: mpsc::Receiver<GuiCommand>,
    egui_ctx: egui::Context,
    // A channel for safely sending the launched WindowHandle back to egui
    handle_tx: mpsc::Sender<WindowHandle>,
) {
    std::thread::spawn(move || {
        init_dpi_awareness();

        let com_ctx = ComContext::new_com_single().unwrap();

        let hicon = unsafe { LoadIconW(None, IDI_APPLICATION).expect("Failed to load icon") };
        let icon = unsafe { Icon::from_raw(hicon) };

        let tray_builder = TrayBuilder::new()
            .with_icon(icon.clone())
            .with_tooltip("Win32 IPC Backend Agent");
        let tray = Tray::build(tray_builder.into_unvalidated().try_into().unwrap()).unwrap();
        let tray_clone = tray.clone();

        let builder = WindowBuilder::new()
            .with_title("Michiu Native Drop Receiver")
            .with_icon(icon)
            .with_visible(false)
            .with_hittest(true)
            .with_com_context(&com_ctx)
            .with_drag_and_drop(true)
            .with_inner_size(LogicalSize::new(400.0, 300.0));

        let window = Window::build(builder.into_unvalidated().try_into().unwrap()).unwrap();
        let handle = window.handle().assume_valid().into_inner();
        // Send to the constructed WindowHandleGUI for synchronization
        handle_tx.send(handle.clone()).unwrap();

        // Event loop that waits for and blocks messages
        let mut event_pump = EventPump::new();
        let tx_ipc_clone = tx_ipc.clone();
        let egui_ctx_clone = egui_ctx.clone();

        while let Some(event) = event_pump.wait_event().unwrap() {
            if let MichiuEvent::Window { event, .. } = event {
                match event {
                    Event::CloseRequested => {
                        // If the user clicks the (X) button on the native window,
                        // do not terminate the process;
                        // simply hide it (Close) and synchronize the state with egui
                        window.set_visible(false);
                        let _ = tx_ipc_clone.send(IpcMessage::DropWindowStatus(false));
                        egui_ctx_clone.request_repaint(); // Launch the egui side
                    }
                    Event::Destroyed => {
                        break;
                    }
                    // File drop validation
                    Event::FileDropped(unvalidated_files) => {
                        let validated_files = unvalidated_files.validate_with(|paths| {
                            if paths.iter().all(|path| path.exists() && path.is_file()) {
                                Ok(paths)
                            } else {
                                Err("Some dropped paths do not exist or are directories.")
                            }
                        });

                        if let Ok(files) = validated_files {
                            let _ = tx_ipc_clone.send(IpcMessage::FileDropped(files.into_inner()));
                            egui_ctx_clone.request_repaint();
                        }
                    }
                    _ => {}
                }
            }

            // Safely execute commands received from the front end (egui)
            while let Ok(cmd) = rx_gui.try_recv() {
                match cmd {
                    GuiCommand::OpenDropWindow => {
                        // Display the window in the center of the screen and bring it into focus
                        window.set_visible(true);
                        window.center_on_screen();

                        // Call the Win32 API directly to keep the window always on top (HWND_TOPMOST)
                        // Sorry, I hadn't implemented it yet.
                        let _ = unsafe {
                            SetWindowPos(
                                window.hwnd(),
                                Some(HWND_TOPMOST),
                                0,
                                0,
                                0,
                                0,
                                SWP_NOMOVE | SWP_NOSIZE,
                            )
                        };

                        let _ = tx_ipc.send(IpcMessage::DropWindowStatus(true));
                        egui_ctx.request_repaint();
                    }
                    GuiCommand::CloseDropWindow => {
                        window.set_visible(false);
                        let _ = unsafe {
                            SetWindowPos(
                                window.hwnd(),
                                Some(HWND_NOTOPMOST),
                                0,
                                0,
                                0,
                                0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                            )
                        };

                        let _ = tx_ipc.send(IpcMessage::DropWindowStatus(false));
                        egui_ctx.request_repaint();
                    }
                    GuiCommand::ShowToast { title, body } => {
                        let _ = tray_clone.show_balloon(&title, &body);
                    }
                    GuiCommand::WriteClipboard(text) => {
                        let _ = window.set_clipboard_text(text);
                    }
                    GuiCommand::ReadClipboard => {
                        if let Ok(text) = window.get_clipboard_text() {
                            let _ = tx_ipc.send(IpcMessage::ClipboardText(text));
                            egui_ctx.request_repaint();
                        }
                    }
                }
            }
        }

        window.destroy();
    });
}

// 5. Entry Point
fn main() -> eframe::Result {
    let (tx_gui, rx_gui) = mpsc::channel();
    let (tx_ipc, rx_ipc) = mpsc::channel();

    // Synchronous channel for receiving a WindowHandle from the backend
    let (handle_tx, handle_rx) = mpsc::channel();

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Win32 IPC Bridge Demo",
        native_options,
        Box::new(move |cc| {
            // Start a background thread and pass the channel and the egui ui
            spawn_win32_backend(tx_ipc, rx_gui, cc.egui_ctx.clone(), handle_tx);

            // Wait for the window to finish loading, then safely retrieve the WindowHandle
            let handle = handle_rx.recv().unwrap();

            Ok(Box::new(IpcBridgeApp::new(cc, tx_gui, rx_ipc, handle)))
        }),
    )
}
