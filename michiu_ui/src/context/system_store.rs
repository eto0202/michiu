use crate::{
    ActiveMasksSecondary, ActiveTransitionsSparseSecondary, BasicLayoutsSecondary, CapacityConfig,
    ContentStore, Context, DirtyRenderEntitiesVec, EdgeInsets, EntityId, EventStore, InputContents,
    InputContentsSparseSecondary, InteractionPropertiesSecondary, LayoutPoint, LayoutRect,
    LayoutStore, OutputStore, ParentsSecondary, RectsSecondary, RenderStore,
    ResolvedBasicSecondary, ResolvedFlexSecondary, ResolvedGridSparseSecondary,
    ScrollOffsetsSecondary, SelectedRectsSparseSecondary, SelectionStartIndexSparseSecondary,
    TextContentsSparseSecondary, TextEngine, TextSelectionsSparseSecondary,
    TextSpansSparseSecondary, UiaValue, VisualPropertiesSecondary, WindowStore,
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
    pub(crate) sys_text_engine: TextEngine,
    pub(crate) sys_dwrite_layouts: DwriteLayoutsSparseSecondary,
    pub(crate) sys_task_sender: TaskSender,
    pub(crate) sys_task_receiver: Receiver<TaskRecv>,
    pub(crate) sys_uia_properties: UiaPropertiesSparseSecondary,
}

pub(crate) type TaskRecv = Box<dyn FnOnce(&mut Context) + Send + 'static>;

impl SystemStore {
    #[inline]
    #[must_use]
    pub fn new(sys_task_sender: TaskSender, sys_task_receiver: Receiver<TaskRecv>) -> Self {
        Self {
            sys_text_engine: TextEngine::new(),
            sys_dwrite_layouts: RefCell::new(SparseSecondaryMap::new()),
            sys_task_sender,
            sys_task_receiver,
            sys_uia_properties: SparseSecondaryMap::new(),
        }
    }

    #[inline]
    #[must_use]
    pub fn with_capacity(
        sys_task_sender: TaskSender,
        sys_task_receiver: Receiver<TaskRecv>,
        c: &CapacityConfig,
    ) -> Self {
        Self {
            sys_text_engine: TextEngine::new(),
            sys_dwrite_layouts: RefCell::new(SparseSecondaryMap::with_capacity(
                c.sys_dwrite_layouts,
            )),
            sys_task_sender,
            sys_task_receiver,
            sys_uia_properties: SparseSecondaryMap::new(),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.sys_dwrite_layouts.borrow_mut().clear();
        while self.sys_task_receiver.try_recv().is_ok() {}
        self.sys_uia_properties.clear();
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.sys_dwrite_layouts.borrow_mut().remove(id);
        self.sys_uia_properties.remove(id);
    }
}

impl SystemStore {
    /// テキスト変更やスタイル更新時にキャッシュを安全に破棄します。
    #[inline]
    pub(crate) fn clear_layout_cache(
        id: EntityId,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
    ) {
        sys_dwrite_layouts.borrow_mut().remove(id);
    }

    /// キャッシュされたレイアウトがあればそれを返し、無ければ安全に生成して保持。
    #[inline]
    pub(crate) fn get_or_create_layout(
        id: EntityId,
        sys_text_engine: &TextEngine,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        cont_text_contents: &TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        out_rects: &RectsSecondary,
    ) -> Option<IDWriteTextLayout> {
        if let Some(layout) = sys_dwrite_layouts.borrow().get(id) {
            return Some(layout.clone());
        }

        let text = cont_text_contents.get(id)?;
        let (font_size, font_family, font_weight, font_style) =
            RenderStore::get_font_propery(id, rnd_visual);

        let auto_wrap = rnd_visual.get(id).and_then(|v| v.auto_wrap);

        let basic = lay_resolved_basic.get(id).copied().unwrap_or_default();
        let rect = out_rects.get(id).copied().unwrap_or_default();
        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

        let max_width = rect.width - border.right - border.left - padding.right - padding.left;
        // 修正：auto_wrap が有効な場合のみ、計算した最大幅を設定する
        let max_width_opt = if auto_wrap.unwrap_or(false) && max_width > 0.0 {
            Some(max_width)
        } else {
            None
        };

        // キャッシュ存在時に、現在の幅の制約と一致しているか検証
        if let Some(layout) = sys_dwrite_layouts.borrow().get(id) {
            let cached_max_width = unsafe { layout.GetMaxWidth() };
            let current_max_width = max_width_opt.unwrap_or(f32::MAX);

            // 許容誤差 1e-3 内で一致している場合はそのまま再利用。
            // リサイズによって幅が変わっている場合はキャッシュを破棄して再ビルドへ進む。
            if (cached_max_width - current_max_width).abs() < 1e-3 {
                return Some(layout.clone());
            }

            sys_dwrite_layouts.borrow_mut().remove(id);
        }

        let spans = ContentStore::get_text_span(id, cont_text_spans);

        let layout = sys_text_engine.create_layout(
            text,
            font_size,
            font_family,
            font_weight,
            font_style,
            max_width_opt,
            auto_wrap,
            spans,
        );

        sys_dwrite_layouts.borrow_mut().insert(id, layout.clone());
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
}

impl Context {
    /// テキスト変更やスタイル更新時にキャッシュを安全に破棄します。
    #[inline]
    pub(crate) fn clear_layout_cache(&self, id: EntityId) {
        self.system.sys_dwrite_layouts.borrow_mut().remove(id);
    }

    /// キャッシュされたレイアウトがあればそれを返し、無ければ安全に生成して保持します。
    #[inline]
    pub(crate) fn get_or_create_layout(&self, id: EntityId) -> Option<IDWriteTextLayout> {
        SystemStore::get_or_create_layout(
            id,
            &self.system.sys_text_engine,
            &self.system.sys_dwrite_layouts,
            &self.contents.cont_text_contents,
            &self.contents.cont_text_spans,
            &self.layouts.lay_resolved_basic,
            &self.renders.rnd_visual,
            &self.outputs.out_rects,
        )
    }
}
