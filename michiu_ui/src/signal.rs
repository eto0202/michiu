use crate::*;
use slotmap::new_key_type;
use smallvec::SmallVec;
use std::cell::Cell;
use std::marker::PhantomData;

new_key_type! {
    /// 登録されたシグナルを識別する一意なID
    pub struct SignalId;
    /// 登録されたエフェクトを識別する一意なID
    pub struct EffectId;
}

thread_local! {
    // 現在メインスレッド上で評価中のエフェクトIDを記録するスレッドローカル領域。
    // これによりシグナル読み出し時の依存関係を自動で構築します。
    pub(crate) static ACTIVE_EFFECT: Cell<Option<EffectId>> = const { Cell::new(None) };
    // 現在メインスレッド上で処理中の UI要素ID (EntityId)
    pub(crate) static ACTIVE_ELEMENT: Cell<Option<crate::EntityId>> = const { Cell::new(None) };
}

/// スコープを抜けた際に自動的に ACTIVE_ELEMENT を復元するRAIIガード
pub struct ActiveElementGuard {
    prev: Option<crate::EntityId>,
}

impl ActiveElementGuard {
    #[inline]
    pub fn new(id: crate::EntityId) -> Self {
        let prev = ACTIVE_ELEMENT.with(|cell| {
            let prev = cell.get();
            cell.set(Some(id));
            prev
        });
        Self { prev }
    }
}

impl Drop for ActiveElementGuard {
    #[inline]
    fn drop(&mut self) {
        ACTIVE_ELEMENT.with(|cell| cell.set(self.prev));
    }
}

/// シグナルの読取端。軽量で Copy 可能。
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

impl<T: Clone + 'static> ReadSignal<T> {
    #[inline]
    pub fn new(id: SignalId) -> Self {
        Self {
            id,
            _marker: PhantomData,
        }
    }
    #[inline]
    pub fn id(&self) -> SignalId {
        self.id
    }
    /// シグナルの現在の値を取得（複製）します。
    /// もし現在エフェクトの評価中であれば、そのエフェクトをこのシグナルの依存先（Subscriber）として自動登録します。
    pub fn get(&self) -> T {
        // 依存関係の追跡（自動サブスクライブ）
        ACTIVE_EFFECT.with(|cell| {
            if let Some(active_effect_id) = cell.get() {
                with_context(|cx| {
                    if let Some(subs) = cx.subscribers.get_mut(self.id) {
                        // すでに依存関係リストに登録されていなければ追加
                        if !subs.contains(&active_effect_id) {
                            subs.push(active_effect_id);
                        }
                    } else {
                        // 新規登録
                        let mut subs = smallvec::SmallVec::new();
                        subs.push(active_effect_id);
                        cx.subscribers.insert(self.id, subs);
                    }
                });
            }
        });

        // 実値の取得とキャスト
        with_context(|cx| {
            let any_val = &cx.signals[self.id];
            any_val
                .downcast_ref::<T>()
                .cloned()
                .expect("Signal type mismatch")
        })
    }

    /// 任意の型のシグナルに対して、条件判定クロージャ `cond_fn` の結果に基づき、
    /// `true_val` または `false_val` を返す遅延評価クロージャを生成します。
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

    /// 任意の型のシグナルに対して、条件判定クロージャ `cond_fn` の結果に基づき、
    /// 重いオブジェクトの生成を遅延させるクロージャ `true_fn` または `false_fn` を呼び出します。
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

    /// 依存関係を追跡せずに現在のシグナルの値を即時取得します。
    #[inline]
    pub fn get_untracked(&self) -> T {
        with_context(|cx| {
            let any_val = &cx.signals[self.id];
            any_val
                .downcast_ref::<T>()
                .cloned()
                .expect("Signal type mismatch")
        })
    }

    /// 読み取り専用の一方向マッピングシグナルを生成します。
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

    /// 双方向バインディング用のアダプタペアを生成します。
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
    /// 状態が true の場合は `true_val` を、false の場合は `false_val` を返すクロージャを生成します。    #[inline]
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

    /// 状態に応じて重いスタイルや要素を生成する場合に、評価を遅延させるためのクロージャ版。
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

/// シグナルの書込端（メインスレッド専用）。軽量で Copy 可能。
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

impl<T: Send + 'static> WriteSignal<T> {
    #[inline]
    pub fn new(id: SignalId) -> Self {
        Self {
            id,
            _marker: PhantomData,
        }
    }
    #[inline]
    pub fn id(&self) -> SignalId {
        self.id
    }
    /// メインスレッド上からシグナルの値を同期的に書き換えます。
    /// 値が書き換わった場合、このシグナルに依存しているすべての子エフェクトを自動的に再評価（実行）します。
    pub fn set(&self, new_value: T) {
        let mut effects_to_run = SmallVec::new();

        with_context(|cx| {
            // 新しい値に差し替え
            cx.signals[self.id] = Box::new(new_value);

            // 依存しているエフェクトIDのリストをクローン
            if let Some(subs) = cx.subscribers.get(self.id) {
                effects_to_run = subs.clone();
            }
        });

        // 依存エフェクトを順次実行
        for effect_id in effects_to_run {
            // エフェクトがデスポーンされて消滅していない場合のみ実行する
            let exists = with_context(|cx| cx.effects.contains_key(effect_id));
            if exists {
                execute_effect(effect_id);
            }
        }
    }

    /// スレッドセーフな送信端（`SignalSender`）を取得します。
    #[inline]
    pub fn sender(&self) -> SignalSender<T> {
        // スレッドローカルのメインコンテキストから送信端を一時的に解決
        let sender = with_context(|cx| cx.task_sender());
        SignalSender {
            id: self.id,
            task_sender: sender,
            _marker: PhantomData,
        }
    }

    /// 明示的なコンテキスト指定により、スレッドセーフな送信端を取得します。
    /// UI構築スコープ外（ACTIVE_CONTEXT が設定されていないタイミング）からでも安全に呼び出せます。
    #[inline]
    pub fn sender_with_cx(&self, cx: &Context) -> SignalSender<T> {
        SignalSender {
            id: self.id,
            task_sender: cx.task_sender(),
            _marker: PhantomData,
        }
    }
}

/// バックグラウンドスレッドからメインスレッドのシグナルを安全に書き換えるためのスレッドセーフな送信端。
pub struct SignalSender<T> {
    pub(crate) id: SignalId,
    pub(crate) task_sender: TaskSender,
    pub(crate) _marker: PhantomData<T>,
}

impl<T> Clone for SignalSender<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            task_sender: self.task_sender.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T: Send + 'static> SignalSender<T> {
    /// ワーカースレッド等から安全にメインスレッドへ更新タスクをディスパッチします。
    pub fn send(&self, value: T) {
        let signal_id = self.id;
        let _ = self.task_sender.send(move |_cx| {
            let write_signal = WriteSignal::<T> {
                id: signal_id,
                _marker: PhantomData,
            };
            write_signal.set(value);
        });
    }
}

/// 指定されたエフェクトをメインスレッドのコンテキスト下で評価（実行）する内部ユーティリティ。
pub(crate) fn execute_effect(effect_id: EffectId) {
    with_context(|cx| {
        // エフェクトのクロージャを一時的にダミーのプレースホルダと入れ替えて安全に取り出す
        // slotMap のキーやバージョンを完全に維持しつつ、多重借用を回避
        let mut effect_closure = std::mem::replace(
            cx.effects.get_mut(effect_id).expect("Effect lost"),
            Box::new(move |_| {
                // このプレースホルダが呼び出されたということは、
                // 元のクロージャがまだ実行中（返却前）に、同一のエフェクトが再帰トリガーされたことを意味する
                eprintln!(
                    "Warning: Cyclic dependency / Infinite loop detected! \
                                     Effect {:?} recursively triggered itself. \
                                     To prevent stack overflow, this recursive run has been skipped.",
                    effect_id
                );
            }),
        );

        // 2. 依存追跡状態を退避・更新
        let prev_effect = ACTIVE_EFFECT.with(|cell| {
            let prev = cell.get();
            cell.set(Some(effect_id));
            prev
        });

        // 実行（内部で get() が呼ばれたシグナルと、この effect_id が自動で紐づきます）
        effect_closure(cx);

        // 実行完了後、退避していた元のエフェクトIDを正確に復元する
        ACTIVE_EFFECT.with(|cell| cell.set(prev_effect));

        // プレースホルダがあった場所に元のクロージャを書き戻す
        if let Some(slot) = cx.effects.get_mut(effect_id) {
            *slot = effect_closure;
        }
    });
}

/// 新しいエフェクトを構築し、評価を開始します。
/// このエフェクトは、内部で get() されたすべてのシグナルが変更された際に自動的に再実行されます。
pub(crate) fn create_effect<F>(f: F) -> EffectId
where
    F: FnMut(&mut Context) + 'static,
{
    // SoA にクロージャを登録
    let id = with_context(|cx| cx.effects.insert(Box::new(f)));
    // 初回評価を実行し、同時にシグナルとの依存関係マップを自動構築する
    execute_effect(id);
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Context, EffectCategory, bind_context, build_ui, div_n};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_signal_reactivity_sync() {
        let mut cx = Context::new();
        let run_count = Arc::new(AtomicU32::new(0));
        let captured_val = Arc::new(AtomicU32::new(0));
        let (run_count_clone, captured_val_clone) = (run_count.clone(), captured_val.clone());

        // 外部に持ち出すための変数
        let mut set_count_handle = None;

        // 1. 構築フェーズ
        build_ui(&mut cx, || {
            let (count, set_count) = create_signal(10u32);
            set_count_handle = Some(set_count); // 外部に退避

            create_effect(move |_cx| {
                run_count_clone.fetch_add(1, Ordering::SeqCst);
                captured_val_clone.store(count.get(), Ordering::SeqCst);
            });

            div_n()
        });

        let set_count = set_count_handle.unwrap();

        // 初回評価の確認
        assert_eq!(run_count.load(Ordering::SeqCst), 1);
        assert_eq!(captured_val.load(Ordering::SeqCst), 10);

        // 2. 更新フェーズ（イベントループ内をシミュレートするためにバインドが必要）
        {
            let _guard = bind_context(&cx);
            set_count.set(20);
        }

        // 再評価されているか確認
        assert_eq!(run_count.load(Ordering::SeqCst), 2);
        assert_eq!(captured_val.load(Ordering::SeqCst), 20);
    }

    #[test]
    fn test_multiple_signals_and_dependencies() {
        let mut cx = Context::new();
        let combined_run_count = Arc::new(AtomicU32::new(0));
        let combined_run_count_clone = combined_run_count.clone();

        let mut signal_handles = None;

        build_ui(&mut cx, || {
            let (a, set_a) = create_signal(1);
            let (b, set_b) = create_signal(2);
            signal_handles = Some((set_a, set_b));

            create_effect(move |_cx| {
                combined_run_count_clone.fetch_add(1, Ordering::SeqCst);
                let _sum = a.get() + b.get();
            });

            div_n()
        });

        let (set_a, set_b) = signal_handles.unwrap();
        assert_eq!(combined_run_count.load(Ordering::SeqCst), 1);

        {
            let _guard = bind_context(&cx);
            set_a.set(10);
            assert_eq!(combined_run_count.load(Ordering::SeqCst), 2);

            set_b.set(20);
            assert_eq!(combined_run_count.load(Ordering::SeqCst), 3);
        }
    }

    #[test]
    fn test_signal_async_update_via_sender() {
        let mut cx = Context::new();
        let run_count = Arc::new(AtomicU32::new(0));
        let run_count_clone = run_count.clone();

        let mut sender_handle = None;

        // 1. 構築フェーズ
        build_ui(&mut cx, || {
            let (count, set_count) = create_signal(0);
            sender_handle = Some(set_count.sender());

            create_effect(move |_cx| {
                run_count_clone.fetch_add(1, Ordering::SeqCst);
                let _ = count.get();
            });

            div_n()
        });

        let sender = sender_handle.unwrap();
        assert_eq!(run_count.load(Ordering::SeqCst), 1);

        // 2. 別スレッドからの更新（sender.send は内部でタスクを投げるだけなのでバインド不要）
        let thread_handle = std::thread::spawn(move || {
            sender.send(100);
        });
        thread_handle.join().unwrap();

        // まだ実行されていない
        assert_eq!(run_count.load(Ordering::SeqCst), 1);

        // 3. メインスレッドでタスク消化（内部で bind_context される）
        cx.process_main_thread_tasks();

        assert_eq!(run_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_effect_cleanup_on_despawn() {
        let mut cx = Context::new();
        let run_count = Arc::new(AtomicU32::new(0));
        let run_count_clone = run_count.clone();

        let mut set_count_handle = None;
        let mut element_handle = None;

        build_ui(&mut cx, || {
            let (count, set_count) = create_signal(0);
            set_count_handle = Some(set_count);

            let el = div_n();
            element_handle = Some(el);
            let el_id = el.id;

            let effect_id = create_effect(move |_cx| {
                run_count_clone.fetch_add(1, Ordering::SeqCst);
                let _ = count.get();
            });

            with_context(|cx| cx.register_element_effect(el_id, EffectCategory::None, effect_id));

            el
        });

        let set_count = set_count_handle.unwrap();
        let el = element_handle.unwrap();

        assert_eq!(run_count.load(Ordering::SeqCst), 1);

        {
            let _guard = bind_context(&cx);
            set_count.set(1);
        }
        assert_eq!(run_count.load(Ordering::SeqCst), 2);

        // 要素を削除
        cx.despawn(el);

        // 更新しても、エフェクトはもう存在しないはず
        {
            let _guard = bind_context(&cx);
            set_count.set(2);
        }
        assert_eq!(run_count.load(Ordering::SeqCst), 2);
    }
}
