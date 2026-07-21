use crate::*;
use slotmap::{SecondaryMap, SparseSecondaryMap};
use std::{cell::RefCell, sync::mpsc::Receiver};
use windows::Win32::Graphics::DirectWrite::IDWriteTextLayout;

pub(crate) type TaskSenderType =
    std::sync::mpsc::Sender<Box<dyn FnOnce(&mut Context) + Send + 'static>>;
/// メインスレッド（UIスレッド）に対して、スレッドセーフに任意のタスクを送信する送信端。
#[derive(Clone)]
pub struct TaskSender {
    pub(crate) inner: TaskSenderType,
    // コアから Win32 を隠蔽するためのウェイクアップコールバック
    pub(crate) waker: Option<std::sync::Arc<dyn Fn() + Send + Sync + 'static>>,
}

impl TaskSender {
    /// ワーカースレッド等からメインスレッドで実行してほしい処理（クロージャ）を送信します。
    /// ライブラリ内部で自動的に Box に包むため、呼び出し側での Box::new は不要です。
    #[allow(clippy::result_unit_err)]
    pub fn send<F>(&self, f: F) -> Result<(), ()>
    where
        F: FnOnce(&mut Context) + Send + 'static,
    {
        // 内部で Box::new に包んで送信し、複雑なエラー型はシンプルな Result<(), ()> に変換して隠蔽する
        self.inner.send(Box::new(f)).map_err(|_| ())?;

        // タスク送信に成功したら即座にメインスレッドをウェイクアップさせる
        if let Some(ref waker) = self.waker {
            waker();
        }
        Ok(())
    }
}

pub struct SystemStore {
    pub(crate) text_engine: TextEngine,
    pub(crate) dwrite_layouts: RefCell<SparseSecondaryMap<EntityId, IDWriteTextLayout>>,
    pub(crate) uia_properties: SparseSecondaryMap<EntityId, Vec<(i32, UiaValue)>>,
    pub(crate) task_sender: TaskSender,
    pub(crate) task_receiver: Receiver<TaskRecv>,
}

pub(crate) type TaskRecv = Box<dyn FnOnce(&mut Context) + Send + 'static>;

impl SystemStore {
    #[inline]
    pub fn new(task_sender: TaskSender, task_receiver: Receiver<TaskRecv>) -> Self {
        Self {
            text_engine: TextEngine::new(),
            dwrite_layouts: RefCell::new(SparseSecondaryMap::new()),
            uia_properties: SparseSecondaryMap::new(),
            task_sender,
            task_receiver,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.dwrite_layouts.borrow_mut().clear();
        self.uia_properties.clear();
        while self.task_receiver.try_recv().is_ok() {}
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.dwrite_layouts.borrow_mut().remove(id);
        self.uia_properties.remove(id);
    }
}
