use crate::{
    ContentStore, Context, EdgeInsets, EntityId, InputContents, LayoutPoint, LayoutRect,
    RenderStore, TextContentsSparseSecondary, TextEngine, TextSpansSparseSecondary, UiaValue,
    VisualPropertiesSecondary, WindowStore,
};
use slotmap::{SecondaryMap, SparseSecondaryMap};
use std::{
    cell::RefCell,
    sync::{Arc, mpsc::Receiver},
};
use windows::Win32::{
    Foundation::{HANDLE, HGLOBAL},
    Graphics::DirectWrite::{DWRITE_HIT_TEST_METRICS, IDWriteTextLayout},
    System::{
        DataExchange::{
            CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
        },
        Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
    },
    UI::Input::{
        Ime::{
            CANDIDATEFORM, CFS_EXCLUDE, CFS_POINT, COMPOSITIONFORM, CPS_COMPLETE, HIMC,
            ImmAssociateContext, ImmGetContext, ImmNotifyIME, ImmReleaseContext,
            ImmSetCandidateWindow, ImmSetCompositionWindow, NI_COMPOSITIONSTR,
        },
        KeyboardAndMouse::GetFocus,
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
    /// ライブラリ内部で自動的に Box に包むため、呼び出し側での `Box::new` は不要です。
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

pub(crate) type DwriteLayoutsSparseSecondary =
    RefCell<SparseSecondaryMap<EntityId, IDWriteTextLayout>>;
pub(crate) type UiaPropertiesSparseSecondary = SparseSecondaryMap<EntityId, Vec<(i32, UiaValue)>>;

pub struct SystemStore {
    pub(crate) text_engine: TextEngine,
    pub(crate) dwrite_layouts: DwriteLayoutsSparseSecondary,
    pub(crate) uia_properties: UiaPropertiesSparseSecondary,
    pub(crate) task_sender: TaskSender,
    pub(crate) task_receiver: Receiver<TaskRecv>,
}

pub(crate) type TaskRecv = Box<dyn FnOnce(&mut Context) + Send + 'static>;

impl SystemStore {
    #[inline]
    #[must_use]
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
    #[inline]
    pub(crate) fn clear_layout_cache(id: EntityId, dwrite_layouts: &DwriteLayoutsSparseSecondary) {
        dwrite_layouts.borrow_mut().remove(id);
    }

    /// キャッシュされたレイアウトがあればそれを返し、無ければ安全に生成して保持します。
    #[inline]
    pub(crate) fn get_or_create_layout(
        id: EntityId,
        text_contents: &TextContentsSparseSecondary,
        visual_properties: &VisualPropertiesSecondary,
        dwrite_layouts: &DwriteLayoutsSparseSecondary,
        text_spans: &TextSpansSparseSecondary,
        text_engine: &TextEngine,
    ) -> Option<IDWriteTextLayout> {
        if let Some(layout) = dwrite_layouts.borrow().get(id) {
            return Some(layout.clone());
        }

        let text = text_contents.get(id)?;
        let (font_size, font_family, font_weight, font_style) =
            RenderStore::get_font_propery(id, visual_properties);
        let max_width = None;
        let spans = ContentStore::get_text_span(id, text_spans);

        let layout = text_engine.create_layout(
            text,
            font_size,
            font_family,
            font_weight,
            font_style,
            max_width,
            spans,
        );

        dwrite_layouts.borrow_mut().insert(id, layout.clone());
        Some(layout)
    }

    #[allow(clippy::cast_possible_truncation)]
    #[inline]
    pub(crate) fn sync_imm_window_position(
        rect: LayoutRect,
        scale: f32,
        border: EdgeInsets,
        padding: EdgeInsets,
        caret: LayoutRect,
        caret_offset: f32,
        scroll: LayoutPoint,
    ) {
        let hwnd = unsafe { GetFocus() };
        if hwnd.is_invalid() {
            return;
        }

        let himc = unsafe { ImmGetContext(hwnd) };
        if himc.is_invalid() {
            return;
        }

        // スクロールオフセット（scroll.x / scroll.y）を正確に引いた実座標で同期
        let caret_phys_x =
            (((rect.x + border.left + padding.left + caret.x) - scroll.x) * scale).round() as i32;
        let caret_phys_y =
            (((rect.y + border.top + padding.top + caret.y + caret_offset) - scroll.y) * scale)
                .round() as i32;
        let caret_phys_h = (caret.height * scale).round() as i32;

        // コンポジションウィンドウ位置の指定 (CFS_POINT)
        let comp_form = COMPOSITIONFORM {
            dwStyle: CFS_POINT,
            ptCurrentPos: windows::Win32::Foundation::POINT {
                x: caret_phys_x,
                y: caret_phys_y,
            },
            rcArea: windows::Win32::Foundation::RECT::default(),
        };
        let _ = unsafe { ImmSetCompositionWindow(himc, &raw const comp_form) };

        // 候補ウィンドウ位置の指定 (CFS_EXCLUDE)
        let candidate_form = CANDIDATEFORM {
            dwIndex: 0,
            dwStyle: CFS_EXCLUDE,
            ptCurrentPos: windows::Win32::Foundation::POINT {
                x: caret_phys_x,
                y: caret_phys_y,
            },
            rcArea: windows::Win32::Foundation::RECT {
                left: caret_phys_x,
                top: caret_phys_y,
                right: caret_phys_x + 1,
                bottom: caret_phys_y + caret_phys_h,
            },
        };
        let _ = unsafe { ImmSetCandidateWindow(himc, &raw const candidate_form) };
        let _ = unsafe { ImmReleaseContext(hwnd, himc) };
    }

    #[inline]
    pub(crate) fn force_complete_ime_composition() {
        let hwnd = unsafe { GetFocus() };
        if hwnd.is_invalid() {
            return;
        }

        let himc = unsafe { ImmGetContext(hwnd) };
        if himc.is_invalid() {
            return;
        }

        let _ = unsafe { ImmNotifyIME(himc, NI_COMPOSITIONSTR, CPS_COMPLETE, 0) };
        let _ = unsafe { ImmReleaseContext(hwnd, himc) };
    }

    #[inline]
    pub(crate) fn unassociate_ime(contents: &InputContents, default_himc: &mut Option<HIMC>) {
        let hwnd = unsafe { GetFocus() };
        if hwnd.is_invalid() {
            return;
        }

        // IMEが有効な場合、コンテキストを元に戻して早期リターン
        if contents.is_ime {
            if let Some(default_himc) = default_himc {
                let _ = unsafe { ImmAssociateContext(hwnd, *default_himc) };
            }
            return;
        }

        // IMEが無効な場合、コンテキストを無効（default）に関連付けし直す
        let old_himc = unsafe { ImmAssociateContext(hwnd, HIMC::default()) };

        // 取得した古いコンテキストが無効、またはすでにデフォルト値が保存済みの場合は早期リターン
        if old_himc.is_invalid() || default_himc.is_some() {
            return;
        }

        // デフォルト値が未保存の場合のみ、ここで新しく保存
        *default_himc = Some(old_himc);
    }

    #[inline]
    pub(crate) fn reset_ime_default_state(default_himc: Option<&HIMC>) {
        let hwnd = unsafe { GetFocus() };
        if hwnd.is_invalid() {
            return;
        }

        let Some(default_himc) = default_himc else {
            return;
        };

        let _ = unsafe { ImmAssociateContext(hwnd, *default_himc) };
    }

    // クリップボード API による UTF-16 読み書きヘルパー
    fn win32_set_clipboard(text: &str) -> Result<(), Box<dyn std::error::Error>> {
        let text_u16: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let size = text_u16.len() * 2;
        let h_mem = unsafe { GlobalAlloc(GMEM_MOVEABLE, size)? };
        let ptr = unsafe { GlobalLock(h_mem) };
        unsafe {
            std::ptr::copy_nonoverlapping(text_u16.as_ptr(), ptr.cast::<u16>(), text_u16.len());
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

impl Context {
    /// テキスト変更やスタイル更新時にキャッシュを安全に破棄します。
    #[inline]
    pub(crate) fn clear_layout_cache(&self, id: EntityId) {
        self.system.dwrite_layouts.borrow_mut().remove(id);
    }

    /// キャッシュされたレイアウトがあればそれを返し、無ければ安全に生成して保持します。
    #[inline]
    pub(crate) fn get_or_create_layout(&self, id: EntityId) -> Option<IDWriteTextLayout> {
        let ContentStore {
            text_contents,
            text_spans,
            ..
        } = &self.contents;
        let SystemStore {
            dwrite_layouts,
            text_engine,
            ..
        } = &self.system;
        let RenderStore {
            visual_properties, ..
        } = &self.renders;

        SystemStore::get_or_create_layout(
            id,
            text_contents,
            visual_properties,
            dwrite_layouts,
            text_spans,
            text_engine,
        )
    }
}
