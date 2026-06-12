use michiu_guard::{Unvalidated, Validated};
use michiu_window::{ComContext, Window, WindowBuilder, WindowHandle, init_dpi_awareness};

fn main() {
    // Initialize high-DPI support
    init_dpi_awareness();

    println!("--- [1] Triggering ValidationError (Style Conflict) ---");

    // Intentionally set up a conflict where the window is transparent
    // but attempts to enable the OS's standard window decorations
    let builder: michiu_window::Result<Validated<WindowBuilder>> = WindowBuilder::new()
        .with_title("Conflict Window")
        .with_transparent(true)
        .with_decorations(true)
        .into_unvalidated()
        .try_into();

    match builder {
        Ok(_) => println!("Successfully validated? (Should not happen)"),
        Err(err) => {
            // Display details using RichReport (.report())
            println!("{}", err.report());
        }
    }
    println!("\n---------------------------------------------------------------------\n");

    println!("--- [2] Triggering ThreadMismatch (UI Thread Violation) ---");

    // Perform verification in a separate thread to avoid contaminating the main thread
    let handle = std::thread::spawn(|| {
        let builder = WindowBuilder::new().with_title("Affinity Test Window");
        let window = Window::build(builder.into_unvalidated().try_into().unwrap()).unwrap();
        let handle = window.handle().assume_valid();

        // Start a separate thread (background thread)
        let handle_clone = handle.clone();
        let bg_thread = std::thread::spawn(move || {
            // Call the UI thread constraint check from a background thread,
            // intentionally triggering a ThreadMismatch error
            let check_result = handle_clone.assert_ui_thread();

            match check_result {
                Ok(_) => println!("Assertion passed on foreign thread? (Should not happen)"),
                Err(err) => {
                    println!("{}", err.report());
                }
            }
        });
        bg_thread.join().unwrap();
        window.destroy();
    });
    handle.join().unwrap();
    println!("\n---------------------------------------------------------------------\n");

    println!("--- [3] Triggering InvalidHandleState (Zombie Window) ---");

    let handle = std::thread::spawn(|| {
        let builder = WindowBuilder::new().with_title("Zombie Handle Test");
        let window = Window::build(builder.into_unvalidated().try_into().unwrap()).unwrap();
        let handle = window.handle().assume_valid();

        // Explicitly destroy the constructed window
        window.destroy();

        // By calling `validate` on a zombie handle after it has been destroyed,
        // you can intentionally trigger an `InvalidHandleState` error
        let unvalidate_result = Unvalidated::new(handle.into_inner());
        let validate_result: michiu_window::Result<Validated<WindowHandle>> =
            unvalidate_result.try_into();

        match validate_result {
            Ok(_) => println!("Zombie handle passed validation? (Should not happen)"),
            Err(err) => {
                println!("{}", err.report());
            }
        }
    });
    handle.join().unwrap();
    println!("\n=====================================================================");

    println!("--- [4] Triggering ComInitializationFailed (Raw OS HRESULT Error) ---");

    let handle = std::thread::spawn(|| {
        // First, initialize OLE (STA) for single-threaded use
        let _ctx_sta = ComContext::new_com_single().unwrap();

        // Forces the initialization of a competing Multithreaded COM (MTA) on the same thread
        // Because the Win32 specification does not allow changing the apartment model mid-execution,
        // the OS will reliably
        // return the raw error HRESULT: 0x80010106 (RPC_E_CHANGED_MODE).
        let result_mta = ComContext::new_com_multi();

        match result_mta {
            Ok(_) => println!("COM initialized with conflict? (Should not happen)"),
            Err(err) => {
                println!("{}", err.report());
            }
        }
    });
    handle.join().unwrap();
    println!("\n=====================================================================");
}
