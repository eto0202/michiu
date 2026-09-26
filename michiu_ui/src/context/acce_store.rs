use crate::EntityId;
pub use accesskit::Role;
use accesskit_windows::SubclassingAdapter;
use std::sync::mpsc::Sender;
use windows::Win32::Foundation::HWND;

struct Snapshot;

pub struct AccessibilityStore {
    pub(crate) acce_adapter: Option<SubclassingAdapter>,
    pub(crate) acce_worker_sender: Option<Sender<Snapshot>>,
    pub(crate) acce_is_active: bool,
}

impl AccessibilityStore {
    #[inline]
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            acce_adapter: None,
            acce_worker_sender: None,
            acce_is_active: false,
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn init(hwnd: HWND) -> Self {
        Self {
            acce_adapter: todo!(),
            acce_worker_sender: todo!(),
            acce_is_active: todo!(),
        }
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        todo!()
    }

    #[inline]
    pub(crate) fn despawn(&mut self, id: EntityId) {
        todo!()
    }
}
