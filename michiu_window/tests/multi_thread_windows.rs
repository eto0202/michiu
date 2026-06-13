use michiu_window::{
    ComContext, EventPump, LogicalSize, WindowBuilder, WindowHandle, init_dpi_awareness,
};
use std::{
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

// cargo test --test multi_thread_windows

// スレッド間でやり取りするメッセージ列挙型の定義
#[derive(Debug, Clone)]
enum ThreadMessage {
    // 巡回の接続トポロジーを構築するためのセットアップ情報
    Setup {
        next_tx: Sender<ThreadMessage>,
        next_handle: WindowHandle,
    },
    // 伝搬用のインクリメント用データ
    Chain(u32),
    // 一斉送信用のブロードキャストデータ
    Broadcast(String),
    // スレッドのメッセージループを終了させるコマンド
    Quit,
}

// 個別のスレッドでウィンドウとメッセージループを立ち上げる
fn spawn_isolated_ui_thread(
    title: &'static str,
    rx: Receiver<ThreadMessage>,
    coordinator_tx: Sender<(String, ThreadMessage)>,
) -> (std::thread::JoinHandle<()>, WindowHandle) {
    let (handle_tx, handle_rx) = mpsc::channel();

    let join_handle = std::thread::spawn(move || {
        // スレッドごとに独立した COM STA コンテキストを初期化
        let com_ctx = ComContext::new_com_single().unwrap();

        let builder = WindowBuilder::new()
            .with_title(title)
            .with_com_context(&com_ctx)
            .with_visible(false) // 画面がウィンドウで埋まるのを防ぐため非表示
            .with_inner_size(LogicalSize::new(200.0, 150.0));

        let window =
            michiu_window::Window::build(builder.into_unvalidated().try_into().unwrap()).unwrap();
        let handle = window.handle().assume_valid();

        // 構築したスレッド固有の WindowHandle を呼び出し元に返す
        handle_tx.send(handle).unwrap();

        let mut event_pump = EventPump::new();
        let start_time = Instant::now();

        // スレッドローカルな接続トポロジー状態
        let mut next_tx: Option<Sender<ThreadMessage>> = None;
        let mut next_handle: Option<WindowHandle> = None;

        'message_loop: loop {
            // wake_up() で送られた WM_NULL 等によってメッセージループを安全に1周回転させる
            let _ = event_pump.wait_event();

            // 自分のチャンネルから溜まっているタスクを回収して処理
            while let Ok(msg) = rx.try_recv() {
                // 受信したことをコーディネーターに報告
                coordinator_tx
                    .send((title.to_string(), msg.clone()))
                    .unwrap();

                match msg {
                    ThreadMessage::Setup {
                        next_tx: tx,
                        next_handle: h,
                    } => {
                        next_tx = Some(tx);
                        next_handle = Some(h);
                    }
                    ThreadMessage::Chain(val) => {
                        // 最大3回転まで伝搬を許可
                        if val < 3
                            && let Some(ref tx) = next_tx
                            && let Some(ref h) = next_handle
                        {
                            // 値を増やして次のウィンドウのスレッドへパスする
                            tx.send(ThreadMessage::Chain(val + 1)).unwrap();
                            h.wake_up(); // 次のウィンドウのメッセージループを起こす
                        }
                    }
                    ThreadMessage::Broadcast(_) => {
                        // 一斉送信処理（受信した時点でコーディネーターへ報告されているためここでは何もしない）
                    }
                    ThreadMessage::Quit => {
                        break 'message_loop;
                    }
                }
            }

            // ハングアップ防止用の時間制限
            if start_time.elapsed() > Duration::from_secs(5) {
                break;
            }
        }

        window.destroy();
    });

    let handle = handle_rx.recv().unwrap().into_inner();
    (join_handle, handle)
}

#[test]
fn test_integration_multi_thread_windows_and_broadcasting() {
    let _ = init_dpi_awareness();

    // 各種通信用のチャネル準備
    let (tx_a, rx_a) = mpsc::channel();
    let (tx_b, rx_b) = mpsc::channel();
    let (tx_c, rx_c) = mpsc::channel();

    // 実行履歴をすべて集約するコーディネーター用チャネル
    let (coord_tx, coord_rx) = mpsc::channel();

    // 3つの独立したUIスレッドを立ち上げる
    let (join_a, handle_a) = spawn_isolated_ui_thread("Window A", rx_a, coord_tx.clone());
    let (join_b, handle_b) = spawn_isolated_ui_thread("Window B", rx_b, coord_tx.clone());
    let (join_c, handle_c) = spawn_isolated_ui_thread("Window C", rx_c, coord_tx.clone());

    // 接続トポロジー（A -> B -> C -> A）の動的セットアップ
    // スレッド A は スレッド B を指す
    tx_a.send(ThreadMessage::Setup {
        next_tx: tx_b.clone(),
        next_handle: handle_b.clone(),
    })
    .unwrap();
    handle_a.wake_up();

    // スレッド B は スレッド C を指す
    tx_b.send(ThreadMessage::Setup {
        next_tx: tx_c.clone(),
        next_handle: handle_c.clone(),
    })
    .unwrap();
    handle_b.wake_up();

    // スレッド C は スレッド A を指す
    tx_c.send(ThreadMessage::Setup {
        next_tx: tx_a.clone(),
        next_handle: handle_a.clone(),
    })
    .unwrap();
    handle_c.wake_up();

    // スレッド A に対し、最初の起点となる Chain(1) を投げ、起床させる
    tx_a.send(ThreadMessage::Chain(1)).unwrap();
    handle_a.wake_up();

    // 巡回メッセージ（1 -> 2 -> 3）が安全に伝搬された形跡を回収・検証
    let start_collect = Instant::now();
    let mut chain_history = Vec::new();

    while start_collect.elapsed() < Duration::from_secs(3) {
        if let Ok((from_thread, ThreadMessage::Chain(val))) = coord_rx.try_recv() {
            chain_history.push((from_thread, val));
        }
        if chain_history.len() >= 3 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    // 正しく A(1) -> B(2) -> C(3) の順番かつ増分で巡回処理が行われたことをアサーション
    assert_eq!(
        chain_history.len(),
        3,
        "Chain communication did not complete properly"
    );
    assert_eq!(chain_history[0], ("Window A".to_string(), 1));
    assert_eq!(chain_history[1], ("Window B".to_string(), 2));
    assert_eq!(chain_history[2], ("Window C".to_string(), 3));

    // 全てのスレッドのチャンネルに対して一斉送信し、同時に wake_up で起こす
    let broadcast_msg = ThreadMessage::Broadcast("Broadcast Signal".to_string());

    tx_a.send(broadcast_msg.clone()).unwrap();
    tx_b.send(broadcast_msg.clone()).unwrap();
    tx_c.send(broadcast_msg.clone()).unwrap();

    handle_a.wake_up();
    handle_b.wake_up();
    handle_c.wake_up();

    let start_collect_bc = Instant::now();
    let mut bc_received_threads = Vec::new();

    while start_collect_bc.elapsed() < Duration::from_secs(2) {
        if let Ok((from_thread, ThreadMessage::Broadcast(text))) = coord_rx.try_recv() {
            assert_eq!(text, "Broadcast Signal");
            bc_received_threads.push(from_thread);
        }
        if bc_received_threads.len() >= 3 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    // すべてのスレッド（Window A, B, C）が同時に一斉送信を正しく受信・処理したことを確認
    assert_eq!(
        bc_received_threads.len(),
        3,
        "Broadcast communication did not reach all threads"
    );
    assert!(bc_received_threads.contains(&"Window A".to_string()));
    assert!(bc_received_threads.contains(&"Window B".to_string()));
    assert!(bc_received_threads.contains(&"Window C".to_string()));

    //  全スレッドを終了させて Join
    tx_a.send(ThreadMessage::Quit).unwrap();
    tx_b.send(ThreadMessage::Quit).unwrap();
    tx_c.send(ThreadMessage::Quit).unwrap();

    handle_a.wake_up();
    handle_b.wake_up();
    handle_c.wake_up();

    join_a.join().unwrap();
    join_b.join().unwrap();
    join_c.join().unwrap();
}
