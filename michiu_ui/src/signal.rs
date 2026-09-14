#[cfg(feature = "trace-error")]
use crate::trace_error;
use crate::{
    Context, DebugStore, Effects, MichiuError, MichiuTrace, OptionTraceExt, TaskSender,
    with_context,
};
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

/// スコープを抜けた際に自動的に `ACTIVE_ELEMENT` を復元するRAIIガード
pub struct ActiveElementGuard {
    prev: Option<crate::EntityId>,
}

impl ActiveElementGuard {
    #[inline]
    #[must_use]
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

    /// 参照 `&T` を使って処理を行い結果だけを取り出す
    #[track_caller]
    #[inline]
    pub fn with<U>(&self, f: impl FnOnce(&T) -> U) -> U {
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
    #[inline]
    #[must_use]
    pub fn id(&self) -> SignalId {
        self.id
    }
    /// シグナルの現在の値を取得（複製）します。
    /// もし現在エフェクトの評価中であれば、そのエフェクトをこのシグナルの依存先（Subscriber）として自動登録します。
    #[inline]
    #[must_use]
    pub fn get(&self) -> T {
        self.with(std::clone::Clone::clone)
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
    #[inline]
    #[must_use]
    pub fn id(&self) -> SignalId {
        self.id
    }
    /// メインスレッド上からシグナルの値を同期的に書き換えます。
    /// 値が書き換わった場合、このシグナルに依存しているすべての子エフェクトを自動的に再評価（実行）します。
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

    /// スレッドセーフな送信端（`SignalSender`）を取得します。
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

    /// 明示的なコンテキスト指定により、スレッドセーフな送信端を取得します。
    /// UI構築スコープ外（`ACTIVE_CONTEXT` が設定されていないタイミング）からでも安全に呼び出せます。
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

/// バックグラウンドスレッドからメインスレッドのシグナルを安全に書き換えるためのスレッドセーフな送信端。
pub struct SignalSender<T> {
    pub(crate) id: SignalId,
    pub(crate) sys_task_sender: TaskSender,
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
    /// ワーカースレッド等から安全にメインスレッドへ更新タスクをディスパッチします。
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

/// 指定されたエフェクトをメインスレッドのコンテキスト下で評価（実行）する内部ユーティリティ。
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

        // 実行（内部で get() が呼ばれたシグナルと、この effect_id が自動で紐づきます）
        effect_closure.0(cx);

        // 実行完了後、退避していた元のエフェクトIDを正確に復元する
        ACTIVE_EFFECT.with(|cell| cell.set(prev_effect));

        // プレースホルダがあった場所に元のクロージャを書き戻す
        if let Some(slot) = cx.reactive.react_effects.get_mut(effect_id) {
            *slot = effect_closure;
        }
    });
}

/// 新しいエフェクトを構築し、評価を開始します。
/// このエフェクトは、内部で `get()` されたすべてのシグナルが変更された際に自動的に再実行されます。
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
