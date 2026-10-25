#[cfg(feature = "trace-error")]
use crate::trace_error;
use crate::{
    Context, DebugStore, Effects, MichiuError, MichiuTaskSender, MichiuTrace, OptionTraceExt,
    with_context,
};
use slotmap::new_key_type;
use smallvec::SmallVec;
use std::cell::Cell;
use std::marker::PhantomData;

new_key_type! {
    /// A unique ID that identifies a registered signal
    pub struct SignalId;
    /// A unique ID that identifies a registered effect
    pub struct EffectId;
}

thread_local! {
    // 現在メインスレッド上で評価中のエフェクトIDを記録するスレッドローカル
    pub(crate) static ACTIVE_EFFECT: Cell<Option<EffectId>> = const { Cell::new(None) };
    // 現在メインスレッド上で処理中のEntity
    pub(crate) static ACTIVE_ENTITY: Cell<Option<crate::Entity>> = const { Cell::new(None) };
}

/// Automatically restore `ACTIVE_ENTITY` when exiting the scope
pub struct ActiveEntityGuard {
    prev: Option<crate::Entity>,
}

impl ActiveEntityGuard {
    #[inline]
    #[must_use]
    pub fn new(id: crate::Entity) -> Self {
        let prev = ACTIVE_ENTITY.with(|cell| {
            let prev = cell.get();
            cell.set(Some(id));
            prev
        });
        Self { prev }
    }
}

impl Drop for ActiveEntityGuard {
    #[inline]
    fn drop(&mut self) {
        ACTIVE_ENTITY.with(|cell| cell.set(self.prev));
    }
}

/// Signal readout terminal.
#[derive(Debug, PartialEq, Eq)]
pub struct ReadSignal<T> {
    pub(crate) id: SignalId,
    pub(crate) _marker: PhantomData<T>,
}

impl<T> Clone for ReadSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for ReadSignal<T> {}

impl<T> ReadSignal<T> {
    #[must_use]
    pub fn new(id: SignalId) -> Self {
        Self {
            id,
            _marker: PhantomData,
        }
    }
}

impl<T: 'static> ReadSignal<T> {
    /// 依存関係の自動トラッキング
    #[inline]
    fn track(self) {
        ACTIVE_EFFECT.with(|cell| {
            if let Some(active_effect_id) = cell.get() {
                with_context(|cx| {
                    if let Some(subs) = cx.reactive.react_subscribers.get_mut(self.id) {
                        if !subs.contains(&active_effect_id) {
                            subs.push(active_effect_id);
                        }
                    } else {
                        let mut subs: SmallVec<[EffectId; 8]> = SmallVec::new();
                        subs.push(active_effect_id);
                        cx.reactive.react_subscribers.insert(self.id, subs);
                    }
                });
            }
        });
    }

    /// 参照 `&T` を使って処理を行い結果だけを取り出す。
    #[track_caller]
    #[inline]
    pub(crate) fn with<U>(self, f: impl FnOnce(&T) -> U) -> U {
        self.track();

        with_context(|cx| {
            let any_val =
                cx.reactive
                    .react_signals
                    .get(self.id)
                    .unwrap_or_trace(None, &mut cx.debug, || MichiuError::SignalNotFound {
                        id: self.id,
                    });
            let val = any_val
                .downcast_ref::<T>()
                .unwrap_or_trace(None, &mut cx.debug, || MichiuError::DowncastFailed {
                    expected: std::any::type_name::<T>(),
                });
            f(val)
        })
    }
}

impl<T: Clone + 'static> ReadSignal<T> {
    /// Retrieve [`SignalId`].
    #[inline]
    #[must_use]
    pub fn id(&self) -> SignalId {
        self.id
    }
    /// Retrieves (clone) the current value of the signal.
    ///
    /// If an effect is currently being evaluated,
    /// it automatically registers that effect as a dependency of this signal.
    #[inline]
    #[must_use]
    pub fn get(&self) -> T {
        self.with(std::clone::Clone::clone)
    }

    /// For a signal of any type, generates a lazily evaluated closure that returns
    /// either `true_val` or `false_val` based on the result of the conditional closure `cond_fn`.
    #[inline]
    pub fn get_else_by<U: Clone + 'static, F>(
        self,
        cond_fn: F,
        true_val: U,
        false_val: U,
    ) -> impl Fn() -> U + 'static
    where
        F: Fn(&T) -> bool + 'static,
    {
        let sig = self;
        move || {
            let val = sig.get();
            if cond_fn(&val) {
                true_val.clone()
            } else {
                false_val.clone()
            }
        }
    }

    /// For a signal of any type, calls either the `true_fn` or `false_fn` closure—which delays
    /// the creation of a heavy object—based on the result of the conditional closure `cond_fn`.
    #[inline]
    pub fn get_else_with_by<U: 'static, F, FT, FF>(
        self,
        cond_fn: F,
        true_fn: FT,
        false_fn: FF,
    ) -> impl Fn() -> U + 'static
    where
        F: Fn(&T) -> bool + 'static,
        FT: Fn() -> U + 'static,
        FF: Fn() -> U + 'static,
    {
        let sig = self;
        move || {
            let val = sig.get();
            if cond_fn(&val) { true_fn() } else { false_fn() }
        }
    }

    /// Retrieves the current signal value immediately without tracking dependencies.
    #[track_caller]
    #[inline]
    #[must_use]
    pub fn get_untracked(&self) -> T {
        with_context(|cx| {
            let any_val =
                cx.reactive
                    .react_signals
                    .get(self.id)
                    .unwrap_or_trace(None, &mut cx.debug, || MichiuError::SignalNotFound {
                        id: self.id,
                    });
            any_val
                .downcast_ref::<T>()
                .cloned()
                .unwrap_or_trace(None, &mut cx.debug, || MichiuError::DowncastFailed {
                    expected: std::any::type_name::<T>(),
                })
        })
    }

    /// Generates a read-only, unidirectional mapping signal.
    pub fn map<U, F>(&self, map_fn: F) -> ReadSignal<U>
    where
        T: Send + Clone + 'static,
        U: Send + Clone + PartialEq + 'static,
        F: Fn(&T) -> U + Send + Sync + 'static,
    {
        let source_read = *self;

        with_context(|cx| {
            // S (元の現在値) から U の初期値を安全に解決
            let initial_val = map_fn(&source_read.get());
            let (read_u, write_u) = cx.create_signal(initial_val);

            // 順方向同期：S が更新されたら U も更新するエフェクト
            let write_u_clone = write_u;
            create_effect(move |_| {
                let s_val = source_read.get();
                let u_val = map_fn(&s_val);
                if read_u.get_untracked() != u_val {
                    write_u_clone.set(u_val);
                }
            });

            read_u
        })
    }

    /// Generates a pair of adapters for bidirectional binding.
    pub fn bi_map<U, F, G>(
        &self,
        writer: WriteSignal<T>,
        map_read: F,  // S -> T の順変換
        map_write: G, // T -> S の逆変換
    ) -> (ReadSignal<U>, WriteSignal<U>)
    where
        T: Send + Clone + PartialEq + 'static,
        U: Send + Clone + PartialEq + 'static,
        F: Fn(&T) -> U + Send + Sync + 'static,
        G: Fn(U) -> T + Send + Sync + 'static,
    {
        let source_read = *self;

        with_context(|cx| {
            let initial_val = map_read(&source_read.get());
            let (read_u, write_u) = cx.create_signal(initial_val);

            // 順方向同期：S が更新されたら U も更新するエフェクト
            let write_u_clone = write_u;
            create_effect(move |_| {
                let s_val = source_read.get();
                let u_val = map_read(&s_val);
                if read_u.get_untracked() != u_val {
                    write_u_clone.set(u_val);
                }
            });

            // 逆方向同期：U が更新されたら S も更新するエフェクト
            let read_u_clone = read_u;
            create_effect(move |_| {
                let u_val = read_u_clone.get();
                let s_val = map_write(u_val);
                if source_read.get_untracked() != s_val {
                    writer.set(s_val);
                }
            });

            (read_u, write_u)
        })
    }
}

impl ReadSignal<bool> {
    /// Creates a closure that returns `true_val` if the condition is true, and `false_val` if it is false.
    #[inline]
    pub fn get_else<U: Clone + 'static>(
        self,
        true_val: U,
        false_val: U,
    ) -> impl Fn() -> U + 'static {
        let sig = self;
        move || {
            if sig.get() {
                true_val.clone()
            } else {
                false_val.clone()
            }
        }
    }

    /// Creates a closure that returns `true_fn` if the condition is true, and `false_fn` if it is false.
    #[inline]
    pub fn get_else_with<U: 'static, FT, FF>(
        self,
        true_fn: FT,
        false_fn: FF,
    ) -> impl Fn() -> U + 'static
    where
        FT: Fn() -> U + 'static,
        FF: Fn() -> U + 'static,
    {
        let sig = self;
        move || {
            if sig.get() { true_fn() } else { false_fn() }
        }
    }
}

/// Signal write port (for the main thread only).
#[derive(Debug, PartialEq, Eq)]
pub struct WriteSignal<T> {
    pub(crate) id: SignalId,
    pub(crate) _marker: PhantomData<T>,
}

impl<T> Clone for WriteSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for WriteSignal<T> {}

impl<T> WriteSignal<T> {
    #[inline]
    #[must_use]
    pub fn new(id: SignalId) -> Self {
        Self {
            id,
            _marker: PhantomData,
        }
    }
}

impl<T: Send + 'static> WriteSignal<T> {
    /// Retrieve [`SignalId`].
    #[inline]
    #[must_use]
    pub fn id(&self) -> SignalId {
        self.id
    }

    /// Synchronously updates the value of the signal from the main thread.
    ///
    /// If the value changes, all effects that depend on this signal are automatically re-evaluated.
    pub fn set(&self, new_value: T) {
        let mut effects_to_run = SmallVec::new();

        with_context(|cx| {
            // 新しい値に差し替え
            *cx.reactive.react_signals.get_mut(self.id).unwrap_or_trace(
                None,
                &mut cx.debug,
                || MichiuError::SignalNotFound { id: self.id },
            ) = Box::new(new_value);

            // 依存しているエフェクトIDのリストをクローン
            if let Some(subs) = cx.reactive.react_subscribers.get(self.id) {
                effects_to_run.clone_from(subs);
            }
        });

        // 依存エフェクトを順次実行
        for effect_id in effects_to_run {
            // エフェクトがデスポーンされて消滅していない場合のみ実行する
            let exists = with_context(|cx| cx.reactive.react_effects.contains_key(effect_id));
            if exists {
                execute_effect(effect_id);
            }
        }
    }

    /// Retrieves a thread-safe sender ([`SignalSender`]).
    #[inline]
    #[must_use]
    pub fn sender(&self) -> SignalSender<T> {
        // スレッドローカルのメインコンテキストから送信端を一時的に解決
        let sender = with_context(|cx| cx.task_sender());
        SignalSender {
            id: self.id,
            sys_task_sender: sender,
            _marker: PhantomData,
        }
    }

    /// Obtains a thread-safe sender by explicitly specifying the `Context`.
    ///
    /// Can be safely called even from outside the UI construction scope.
    #[inline]
    #[must_use]
    pub fn sender_with(&self, cx: &Context) -> SignalSender<T> {
        SignalSender {
            id: self.id,
            sys_task_sender: cx.task_sender(),
            _marker: PhantomData,
        }
    }
}

/// A thread-safe sender for safely modifying signals on the main thread from a background thread.
pub struct SignalSender<T> {
    pub(crate) id: SignalId,
    pub(crate) sys_task_sender: MichiuTaskSender,
    pub(crate) _marker: PhantomData<T>,
}

impl<T> Clone for SignalSender<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            sys_task_sender: self.sys_task_sender.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T: Send + 'static> SignalSender<T> {
    /// Safely dispatches update tasks from worker threads and other sources to the main thread.
    #[inline]
    pub fn send(&self, value: T) {
        let signal_id = self.id;
        let _ = self.sys_task_sender.send(move |_cx| {
            let write_signal = WriteSignal::<T> {
                id: signal_id,
                _marker: PhantomData,
            };
            write_signal.set(value);
        });
    }
}

/// 指定されたエフェクトをメインスレッドのコンテキスト下で評価する。
#[track_caller]
#[inline]
pub(crate) fn execute_effect(effect_id: EffectId) {
    with_context(|cx| {
        // このダミーが呼び出されたということは、
        // 元のクロージャがまだ実行中（返却前）に、同一のエフェクトが再帰トリガーされたことを意味する
        let dummy = Effects(Box::new(move |cx| {
            #[cfg(feature = "trace-error")]
            trace_error!(None, &mut cx.debug, || MichiuTrace::Error {
                detail: MichiuError::RecursiveEffectDetected { effect_id },
                add: None,
            });
        }));

        let slot = cx
            .reactive
            .react_effects
            .get_mut(effect_id)
            .unwrap_or_trace(None, &mut cx.debug, || MichiuError::EffectNotFound {
                id: effect_id,
            });

        // エフェクトのクロージャを一時的にダミーのプレースホルダと入れ替えて安全に取り出す
        let mut effect_closure = std::mem::replace(slot, dummy);

        // 依存追跡状態を退避・更新
        let prev_effect = ACTIVE_EFFECT.with(|cell| {
            let prev = cell.get();
            cell.set(Some(effect_id));
            prev
        });

        // 実行（内部で get() が呼ばれたシグナルと、この effect_id が自動で紐づく）
        effect_closure.0(cx);

        // 実行完了後、退避していた元のエフェクトIDを正確に復元する
        ACTIVE_EFFECT.with(|cell| cell.set(prev_effect));

        // プレースホルダがあった場所に元のクロージャを書き戻す
        if let Some(slot) = cx.reactive.react_effects.get_mut(effect_id) {
            *slot = effect_closure;
        }
    });
}

/// 新しいエフェクトを構築し評価を開始する。
/// このエフェクトは、内部で `get()` されたすべてのシグナルが変更された際に自動的に再実行される。
#[inline]
pub(crate) fn create_effect<F>(f: F) -> EffectId
where
    F: FnMut(&mut Context) + 'static,
{
    // SoA にクロージャを登録
    let id = with_context(|cx| cx.reactive.react_effects.insert(Effects(Box::new(f))));
    // 初回評価を実行し、同時にシグナルとの依存関係マップを自動構築する
    execute_effect(id);
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bind_context;
    use slotmap::SlotMap;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    // `test_active_entity_guard_lifecycle`
    //  - 初期状態（`None`）からガード生成で `Some(id1)` に設定されること。
    //  - ネストしたガード（`Some(id2)`）を作成し、内側のドロップで `Some(id1)`、外側のドロップで `None` に順次復元されること。
    //
    // `test_signal_basic_get_and_set`
    //  - 初期値が `get()`, `with()`, `get_untracked()` で正しく取得できること。
    //  - `write_signal.set(new_val)` 後に更新された値が取得できること。
    //
    // `test_signal_not_found_panics`
    //  - 未登録の `SignalId` に対して `get()` を呼んだ場合、`MichiuError::SignalNotFound` でパニックすること。
    //
    // `test_signal_downcast_failed_panics`
    //  - `i32` 型として登録されたシグナルを `ReadSignal<String>` として `get()` しようとした場合、`MichiuError::DowncastFailed` でパニックすること。
    //
    // `test_effect_initial_run_and_tracking`
    //  - `create_effect` の登録時に初回評価が即座に実行されること。
    //  - エフェクト内で `read_signal.get()` を呼ぶと、`cx.reactive.react_subscribers` に自動で依存関係が構築されること。
    //  - `read_signal.get_untracked()` で呼んだ場合は依存関係に登録されないこと。
    //  - `write_signal.set()` を呼び出すと、依存しているエフェクトのみが再評価されること。
    //  - 無関係なシグナルを `set()` しても再評価されないこと。
    //
    // `test_multiple_signals_dependency`
    //  - 1つのエフェクトが2つのシグナル（A, B）を購読している場合、A の更新、B の更新のどちらでもエフェクトが再実行されること。
    //
    // `test_recursive_effect_protection`
    //  - エフェクト自身の実行ブロック内で、自身をトリガーする `write_signal.set()` を実行。
    //  - スタックオーバーフロー（無限再帰）を起こさず安全にスキップされ、エラーハンドリング（`RecursiveEffectDetected`）が機能すること。
    //
    // `test_read_signal_map`
    //  - 元シグナル S をマッピングした派生シグナル U（例: 数値 -> 文字列）を作成。
    //  - S を更新すると、U も自動的に変換後の値へ同期されること。
    //
    // `test_signal_bi_map`
    //  - 双方向マッピングを作成。
    //  - 元シグナル S を更新 -> 派生シグナル U に反映されること。
    //  - 派生シグナル U を更新 -> 元シグナル S に逆変換されて反映されること。
    //
    // `test_get_else_and_get_else_with`
    //  - `ReadSignal<bool>` に対する `get_else`, `get_else_with` で、真偽値に応じて期待する値やクロージャ結果が返ること。
    //
    // `test_get_else_by_and_get_else_with_by`
    //  - 任意の型シグナルに対する述語関数 `cond_fn` の結果に基づき、適切な分岐の値が返ること。
    //
    // `test_signal_sender`
    //  - `write_signal.sender()` を別スレッドに移動し、`sender.send(new_val)` を実行。
    //  - メインスレッドのタスクキューを消化した後に、シグナル値が正しく更新され、依存エフェクトがトリガーされること。

    #[test]
    fn test_active_entity_guard_lifecycle() {
        let mut sm = SlotMap::<crate::Entity, ()>::with_key();
        let e1 = sm.insert(());
        let e2 = sm.insert(());

        assert_eq!(ACTIVE_ENTITY.with(Cell::get), None);

        {
            let _g1 = ActiveEntityGuard::new(e1);
            assert_eq!(ACTIVE_ENTITY.with(Cell::get), Some(e1));

            {
                let _g2 = ActiveEntityGuard::new(e2);
                assert_eq!(ACTIVE_ENTITY.with(Cell::get), Some(e2));
            }

            // 内側ドロップで e1 に復元
            assert_eq!(ACTIVE_ENTITY.with(Cell::get), Some(e1));
        }

        // 外側ドロップで None に復元
        assert_eq!(ACTIVE_ENTITY.with(Cell::get), None);
    }

    #[test]
    fn test_signal_basic_get_and_set() {
        let mut cx = Context::default();
        let _guard = bind_context(&mut cx);

        let (r, w) = with_context(|c| c.create_signal(10));

        assert_eq!(r.get(), 10);
        assert_eq!(r.get_untracked(), 10);
        assert_eq!(r.with(|&v| v * 2), 20);

        w.set(42);
        assert_eq!(r.get(), 42);
        assert_eq!(r.get_untracked(), 42);
        assert_eq!(r.with(|&v| v + 1), 43);
    }

    #[test]
    fn test_signal_not_found_panics() {
        let mut cx = Context::default();
        let _guard = bind_context(&mut cx);

        let missing_signal = ReadSignal::<i32>::new(SignalId::default());

        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = missing_signal.get();
        }));
        assert!(res.is_err());
    }

    #[test]
    fn test_signal_downcast_failed_panics() {
        let mut cx = Context::default();
        let _guard = bind_context(&mut cx);

        let (_r, w) = with_context(|c| c.create_signal(100_i32));
        let fake_r = ReadSignal::<String>::new(w.id());

        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = fake_r.get();
        }));
        assert!(res.is_err());
    }

    #[test]
    fn test_effect_initial_run_and_tracking() {
        let mut cx = Context::default();
        let _guard = bind_context(&mut cx);

        let (r_tracked, w_tracked) = with_context(|c| c.create_signal(1));
        let (r_untracked, w_untracked) = with_context(|c| c.create_signal(100));

        let eval_count = Arc::new(AtomicUsize::new(0));
        let eval_count_clone = Arc::clone(&eval_count);

        // エフェクト作成時に初回評価が即座に行われる
        create_effect(move |_| {
            eval_count_clone.fetch_add(1, Ordering::SeqCst);
            let _ = r_tracked.get();
            let _ = r_untracked.get_untracked();
        });

        assert_eq!(eval_count.load(Ordering::SeqCst), 1);

        // トラッキングされているシグナルを更新 -> 再評価される
        w_tracked.set(2);
        assert_eq!(eval_count.load(Ordering::SeqCst), 2);

        // トラッキングされていないシグナルを更新 -> 再評価されない
        w_untracked.set(200);
        assert_eq!(eval_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    #[expect(clippy::cast_sign_loss)]
    fn test_multiple_signals_dependency() {
        let mut cx = Context::default();
        let _guard = bind_context(&mut cx);

        let (r1, w1) = with_context(|c| c.create_signal(10));
        let (r2, w2) = with_context(|c| c.create_signal(20));

        let sum = Arc::new(AtomicUsize::new(0));
        let sum_clone = Arc::clone(&sum);

        create_effect(move |_| {
            sum_clone.store((r1.get() + r2.get()) as usize, Ordering::SeqCst);
        });

        assert_eq!(sum.load(Ordering::SeqCst), 30);

        w1.set(15);
        assert_eq!(sum.load(Ordering::SeqCst), 35);

        w2.set(25);
        assert_eq!(sum.load(Ordering::SeqCst), 40);
    }

    #[test]
    fn test_recursive_effect_protection() {
        let mut cx = Context::default();
        let _guard = bind_context(&mut cx);

        let (r, w) = with_context(|c| c.create_signal(0));
        let run_count = Arc::new(AtomicUsize::new(0));
        let run_count_clone = Arc::clone(&run_count);

        create_effect(move |_| {
            let count = run_count_clone.fetch_add(1, Ordering::SeqCst);
            let _val = r.get();

            // 自身をトリガーする set を実行（再帰呼び出しを意図）
            if count == 0 {
                w.set(999);
            }
        });

        // 無限ループにならず、ダミー置き換えにより再帰が抑制されること
        assert_eq!(run_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_read_signal_map() {
        let mut cx = Context::default();
        let _guard = bind_context(&mut cx);

        let (r, w) = with_context(|c| c.create_signal(5));
        let mapped = r.map(|&v| v * 10);

        assert_eq!(mapped.get(), 50);

        w.set(7);
        assert_eq!(mapped.get(), 70);
    }

    #[test]
    fn test_signal_bi_map() {
        let mut cx = Context::default();
        let _guard = bind_context(&mut cx);

        let (r_int, w_int) = with_context(|c| c.create_signal(10));
        let (r_str, w_str) = r_int.bi_map(
            w_int,
            |&v| v.to_string(),
            |s: String| s.parse::<i32>().unwrap_or(0),
        );

        assert_eq!(r_str.get(), "10");

        // 順方向同期
        w_int.set(42);
        assert_eq!(r_str.get(), "42");

        // 逆方向同期
        w_str.set("99".to_string());
        assert_eq!(r_int.get(), 99);
    }

    #[test]
    fn test_get_else_and_get_else_with() {
        let mut cx = Context::default();
        let _guard = bind_context(&mut cx);

        let (r, w) = with_context(|c| c.create_signal(true));

        let getter = r.get_else("yes", "no");
        assert_eq!(getter(), "yes");

        w.set(false);
        assert_eq!(getter(), "no");

        let getter_fn = r.get_else_with(|| 100, || 200);
        assert_eq!(getter_fn(), 200);

        w.set(true);
        assert_eq!(getter_fn(), 100);
    }

    #[test]
    fn test_get_else_by_and_get_else_with_by() {
        let mut cx = Context::default();
        let _guard = bind_context(&mut cx);

        let (r, w) = with_context(|c| c.create_signal(15));

        let getter = r.get_else_by(|&v| v > 10, "large", "small");
        assert_eq!(getter(), "large");

        w.set(5);
        assert_eq!(getter(), "small");

        let getter_fn = r.get_else_with_by(|&v| v == 0, || "zero", || "nonzero");
        assert_eq!(getter_fn(), "nonzero");

        w.set(0);
        assert_eq!(getter_fn(), "zero");
    }

    #[test]
    #[expect(clippy::expect_used)]
    fn test_signal_sender() {
        let mut cx = Context::default();
        let _guard = bind_context(&mut cx);

        let (r, w) = with_context(|c| c.create_signal(0));
        let sender = w.sender();

        // sender を別スレッドに move して送信を実行
        let handle = std::thread::spawn(move || {
            sender.send(42);
        });
        handle.join().expect("Worker thread panicked");

        // メインスレッド側でキューされたタスクを消化
        cx.process_main_thread_tasks();
        assert_eq!(r.get(), 42);
    }
}
