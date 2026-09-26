use crate::EntityId;
pub use accesskit::Role;
use accesskit::{ActionHandler, ActionRequest, ActivationHandler, NodeId, TreeId, TreeUpdate};
use accesskit_windows::SubclassingAdapter;
use std::sync::mpsc::Sender;
use windows::Win32::Foundation::HWND;

struct Snapshot;

struct MichiuActionHandler;

impl ActionHandler for MichiuActionHandler {
    fn do_action(&mut self, request: ActionRequest) {
        // イベントループへ転送
    }
}

impl MichiuActionHandler {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {}
    }
}

struct MichiuActivationHandler;

impl ActivationHandler for MichiuActivationHandler {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        Some(TreeUpdate {
            nodes: Vec::new(),
            tree: None,
            tree_id: TreeId::ROOT,
            focus: NodeId(1),
        })
    }
}

impl MichiuActivationHandler {
    #[inline]
    pub(crate) fn new() -> Self {
        Self
    }
}

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
            acce_adapter: Some(SubclassingAdapter::new(
                hwnd,
                MichiuActivationHandler::new(),
                MichiuActionHandler::new(),
            )),
            acce_worker_sender: None,
            acce_is_active: false,
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

#[cfg(test)]
mod tests;
