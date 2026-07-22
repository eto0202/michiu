use crate::*;
use slotmap::{SecondaryMap, SparseSecondaryMap};
use std::{cell::RefCell, sync::mpsc::Receiver};
use windows::Win32::{
    Foundation::{HANDLE, HGLOBAL},
    Graphics::DirectWrite::{DWRITE_HIT_TEST_METRICS, IDWriteTextLayout},
    System::{
        DataExchange::{
            CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
        },
        Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
    },
};

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

impl SystemStore {
    /// テキスト変更やスタイル更新時にキャッシュを安全に破棄します。
    pub(crate) fn clear_layout_cache(id: EntityId, system: &mut SystemStore) {
        system.dwrite_layouts.borrow_mut().remove(id);
    }
    // クリップボード API による UTF-16 読み書きヘルパー
    fn win32_set_clipboard(text: &str) -> Result<(), Box<dyn std::error::Error>> {
        let text_u16: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let size = text_u16.len() * 2;
        let h_mem = unsafe { GlobalAlloc(GMEM_MOVEABLE, size)? };
        let ptr = unsafe { GlobalLock(h_mem) };
        unsafe {
            std::ptr::copy_nonoverlapping(text_u16.as_ptr(), ptr as *mut u16, text_u16.len());
        }
        let _ = unsafe { GlobalUnlock(h_mem) };
        if unsafe { OpenClipboard(None).is_ok() } {
            let _ = unsafe { EmptyClipboard() };
            let _ = unsafe { SetClipboardData(13, Some(HANDLE(h_mem.0))) }; // 13 = CF_UNICODETEXT
            let _ = unsafe { CloseClipboard() };
        }
        Ok(())
    }

    fn win32_get_clipboard() -> Result<String, Box<dyn std::error::Error>> {
        let mut result = String::new();
        if unsafe { OpenClipboard(None).is_ok() } {
            let h_mem = unsafe { GetClipboardData(13)? };
            if !h_mem.is_invalid() {
                let ptr = unsafe { GlobalLock(HGLOBAL(h_mem.0)) };
                if !ptr.is_null() {
                    let u16_ptr = ptr as *const u16;
                    let mut len = 0;
                    while unsafe { *u16_ptr.add(len) } != 0 {
                        len += 1;
                    }
                    let slice = unsafe { std::slice::from_raw_parts(u16_ptr, len) };
                    result = String::from_utf16_lossy(slice);
                    let _ = unsafe { GlobalUnlock(HGLOBAL(h_mem.0)) };
                }
            }
            let _ = unsafe { CloseClipboard() };
        }
        Ok(result)
    }
}
