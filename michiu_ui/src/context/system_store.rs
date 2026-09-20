use crate::{
    CapacityConfig, Context, EntityId, InputContents, LayoutPoint, LayoutRect, MichiuSoA,
    ResolvedGeometry, TextEngine, UiaValue, define_sparse_secondary,
};
use cosmic_text::Buffer;
use slotmap::SparseSecondaryMap;
use std::{cell::RefCell, rc::Rc, sync::mpsc::Receiver};
use windows::Win32::UI::Input::{
    Ime::{
        CANDIDATEFORM, CFS_EXCLUDE, CFS_POINT, COMPOSITIONFORM, CPS_COMPLETE, HIMC,
        ImmAssociateContext, ImmGetContext, ImmNotifyIME, ImmReleaseContext, ImmSetCandidateWindow,
        ImmSetCompositionWindow, NI_COMPOSITIONSTR,
    },
    KeyboardAndMouse::GetFocus,
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

define_sparse_secondary!(pub struct TextBufferSparseInner(Rc<Buffer>));
define_sparse_secondary!(pub struct UiaPropertiesSparse(Vec<(i32, UiaValue)>));

pub type TextBufferSparse = RefCell<TextBufferSparseInner>;
pub(crate) type TaskRecv = Box<dyn FnOnce(&mut Context) + Send + 'static>;

pub struct SystemStore {
    pub(crate) sys_text_engine: TextEngine,
    pub(crate) sys_text_buffers: TextBufferSparse,
    pub(crate) sys_task_sender: TaskSender,
    pub(crate) sys_task_receiver: Receiver<TaskRecv>,
    pub(crate) sys_uia_properties: UiaPropertiesSparse,
}

impl SystemStore {
    #[inline]
    #[must_use]
    pub(crate) fn new(sys_task_sender: TaskSender, sys_task_receiver: Receiver<TaskRecv>) -> Self {
        Self {
            sys_text_engine: TextEngine::new(),
            sys_text_buffers: RefCell::new(TextBufferSparseInner(SparseSecondaryMap::new())),
            sys_task_sender,
            sys_task_receiver,
            sys_uia_properties: UiaPropertiesSparse(SparseSecondaryMap::new()),
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn with_capacity(
        sys_task_sender: TaskSender,
        sys_task_receiver: Receiver<TaskRecv>,
        c: &CapacityConfig,
    ) -> Self {
        Self {
            sys_text_engine: TextEngine::new(),
            sys_text_buffers: RefCell::new(TextBufferSparseInner(
                SparseSecondaryMap::with_capacity(c.sys_text_buffers),
            )),
            sys_task_sender,
            sys_task_receiver,
            sys_uia_properties: UiaPropertiesSparse(SparseSecondaryMap::new()),
        }
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.sys_text_buffers.borrow_mut().clear();
        while self.sys_task_receiver.try_recv().is_ok() {}
        self.sys_uia_properties.clear();
    }

    #[inline]
    pub(crate) fn despawn(&mut self, id: EntityId) {
        self.sys_text_buffers.borrow_mut().remove(id);
        self.sys_uia_properties.remove(id);
    }
}

impl SystemStore {
    /// テキスト変更やスタイル更新時にキャッシュを安全に破棄します。
    #[inline]
    pub(crate) fn clear_text_buffer_cache(id: EntityId, sys_text_buffers: &TextBufferSparse) {
        sys_text_buffers.borrow_mut().remove(id);
    }

    /// キャッシュされたレイアウトがあればそれを返し、無ければ安全に生成して保持。
    #[inline]
    pub(crate) fn get_or_create_text_buffer<F>(
        id: EntityId,
        max_width_opt: Option<f32>,
        sys_text_buffers: &TextBufferSparse,
        create_buffer: F,
    ) -> Rc<Buffer>
    where
        F: FnOnce() -> Buffer,
    {
        // キャッシュ存在時に現在の幅の制約と一致しているか検証
        if let Some(buffer) = sys_text_buffers.borrow().find(id).cloned() {
            let cached_size = buffer.size().0; // Option<f32>

            let is_width_matched = match (cached_size, max_width_opt) {
                // 両方とも制限幅がある場合：差が 0.1 未満なら一致
                (Some(cached), Some(current)) => (cached - current).abs() < 1e-1,
                // 両方とも制限なし（折り返しなし）の場合：一致
                (None, None) => true,
                // 片方だけ制限がある場合：不一致
                _ => false,
            };

            if is_width_matched {
                return buffer;
            }
        }

        SystemStore::clear_text_buffer_cache(id, sys_text_buffers);
        let buffer = Rc::new(create_buffer());
        sys_text_buffers.borrow_mut().insert(id, buffer.clone());
        buffer
    }

    #[allow(clippy::cast_possible_truncation)]
    #[inline]
    pub(crate) fn sync_imm_window_position(
        geom: &ResolvedGeometry,
        scale: f32,
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

        let rect = geom.rect;
        let border = geom.border;
        let padding = geom.padding;

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
    pub(crate) fn unassociate_ime(contents: &InputContents, win_default_himc: &mut Option<HIMC>) {
        let hwnd = unsafe { GetFocus() };
        if hwnd.is_invalid() {
            return;
        }

        // IMEが有効な場合、コンテキストを元に戻して早期リターン
        if contents.is_ime {
            if let Some(win_default_himc) = win_default_himc {
                let _ = unsafe { ImmAssociateContext(hwnd, *win_default_himc) };
            }
            return;
        }

        // IMEが無効な場合、コンテキストを無効（default）に関連付けし直す
        let old_himc = unsafe { ImmAssociateContext(hwnd, HIMC::default()) };

        // 取得した古いコンテキストが無効、またはすでにデフォルト値が保存済みの場合は早期リターン
        if old_himc.is_invalid() || win_default_himc.is_some() {
            return;
        }

        // デフォルト値が未保存の場合のみ、ここで新しく保存
        *win_default_himc = Some(old_himc);
    }

    #[inline]
    pub(crate) fn reset_ime_default_state(win_default_himc: Option<&HIMC>) {
        let hwnd = unsafe { GetFocus() };
        if hwnd.is_invalid() {
            return;
        }

        let Some(win_default_himc) = win_default_himc else {
            return;
        };

        let _ = unsafe { ImmAssociateContext(hwnd, *win_default_himc) };
    }

    #[inline]
    pub fn task_receiver_mut(
        &mut self,
    ) -> &mut Receiver<Box<dyn FnOnce(&mut Context) + Send + 'static>> {
        &mut self.sys_task_receiver
    }

    #[inline]
    pub fn task_sender_mut(&mut self) -> &mut TaskSender {
        &mut self.sys_task_sender
    }

    #[inline]
    pub fn text_buffers_mut(&mut self) -> &mut RefCell<TextBufferSparseInner> {
        &mut self.sys_text_buffers
    }

    #[inline]
    pub fn text_engine_mut(&mut self) -> &mut TextEngine {
        &mut self.sys_text_engine
    }

    #[inline]
    pub fn uia_properties_mut(&mut self) -> &mut UiaPropertiesSparse {
        &mut self.sys_uia_properties
    }
}

#[cfg(test)]
mod tests;
