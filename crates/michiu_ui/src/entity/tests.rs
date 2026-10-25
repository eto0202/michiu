#![expect(clippy::expect_used, clippy::panic)]
use super::*;

// `test_bind_context_lifecycle`
//  - バインド前：`with_context` がパニックすること。
//  - `bind_context(&mut cx)` 実行中：`with_context` 経由で `cx` に正常アクセスでき、内部状態を変更可能なこと。
//  - ガードがドロップされた後：再び未バインド状態に戻り、`with_context` がパニックすること。
// 
// `test_bind_context_nested`
//  - `cx1` をバインド $\to$ 内側で `cx2` をバインド。
//  - 内側では `with_context` が `cx2` を指していること。
//  - 内側のガードをドロップ $\to$ 再び `cx1` を指すように復元されること。
//  - 外側のガードをドロップ $\to$ 未バインド状態に戻ること。
// 
// `test_context_guard_unwind_safety`
//  - `bind_context(&mut cx)` 中に意図的なパニックを発生させ、`std::panic::catch_unwind` で捕捉。
//  - パニック発生時でも `ContextGuard` の `drop` が実行され、スレッドローカルのバインドが確実に元の状態（`None`）に復元されること。
// 
// `test_active_context_thread_isolation`
//  - メインスレッドで `bind_context(&mut cx)` を維持。
//  - 別スレッド（`std::thread::spawn`）を起動し、そのスレッド内から `with_context` を呼び出す。
//  - 別スレッド側ではコンテキストが共有されず、確実にパニックすること。
// 
// `test_build_ui_lifecycle`
//  - `build_ui(&mut cx, || entity)` を呼び出す。
//  - クロージャ内部から `with_context` を呼び出せること。
//  - クロージャが返した `entity` がルート要素として登録されること。
//  - `build_ui` の終了後、スレッドローカルが元のバインド状態に戻ること。

#[test]
fn test_bind_context_lifecycle() {
    let mut cx = Context::default();

    // バインド前: with_context はパニックする
    let before = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_context(|_| ());
    }));
    assert!(before.is_err());

    // バインド中: with_context でアクセス可能
    {
        let _guard = bind_context(&mut cx);
        let frame = with_context(|c| {
            c.debug.frame = 42;
            c.debug.frame
        });
        assert_eq!(frame, 42);
    }

    // ドロップ後: 再び未バインド状態に戻りパニックする
    let after = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_context(|_| ());
    }));
    assert!(after.is_err());
}

#[test]
fn test_bind_context_nested() {
    let mut cx1 = Context::default();
    let mut cx2 = Context::default();
    cx1.debug.frame = 100;
    cx2.debug.frame = 200;

    {
        let _guard1 = bind_context(&mut cx1);
        assert_eq!(with_context(|c| c.debug.frame), 100);

        {
            let _guard2 = bind_context(&mut cx2);
            assert_eq!(with_context(|c| c.debug.frame), 200);
        }

        // 内側のガード解除後、cx1 に復元されていること
        assert_eq!(with_context(|c| c.debug.frame), 100);
    }

    // 外側のガード解除後、未バインドに戻ること
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_context(|_| ());
    }));
    assert!(res.is_err());
}

#[test]
fn test_context_guard_unwind_safety() {
    let mut cx = Context::default();

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = bind_context(&mut cx);
        panic!("force unwind");
    }));

    // パニック後でもドロップが呼ばれ、スレッドローカルが未バインドに復元されていること
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_context(|_| ());
    }));
    assert!(res.is_err());
}

#[test]
fn test_active_context_thread_isolation() {
    let mut cx = Context::default();
    let _guard = bind_context(&mut cx);

    // メインスレッドではバインドされている
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_context(|_| ());
        }))
        .is_ok()
    );

    // 別スレッドではバインドが共有されずパニックすること
    let handle = std::thread::spawn(|| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_context(|_| ());
        }))
    });

    let thread_result = handle.join().expect("Thread panicked unexpectedly");
    assert!(thread_result.is_err());
}

#[test]
fn test_build_ui_lifecycle() {
    let mut cx = Context::default();

    let root_entity = build_ui(&mut cx, || {
        // build_ui 内で with_context が機能すること
        with_context(|c| c.topology.topo_entities.insert(()))
    });

    // 戻り値の Entity がルート要素として登録されていること
    assert!(cx.topology.topo_entities.contains_key(root_entity));
    assert!(cx.topology.topo_session_roots.is_empty()); // end_session でクリアされる

    // build_ui 終了後は未バインド状態に戻っていること
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_context(|_| ());
    }));
    assert!(res.is_err());
}
