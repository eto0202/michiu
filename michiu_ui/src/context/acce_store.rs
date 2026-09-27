use crate::EntityId;
pub use accesskit::Role;
use accesskit::{
    ActionHandler, ActionRequest, ActivationHandler, Node, NodeId, TreeId, TreeInfo, TreeUpdate,
};
use accesskit_windows::SubclassingAdapter;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::Sender,
};
use windows::Win32::Foundation::HWND;

struct Snapshot;

struct MichiuActionHandler {
    // UIのイベントキューへ流す送信側
    sender: Option<Sender<ActionRequest>>,
}

impl ActionHandler for MichiuActionHandler {
    fn do_action(&mut self, request: ActionRequest) {
        if let Some(ref sender) = self.sender {
            let _ = sender.send(request);
        }
    }
}

impl MichiuActionHandler {
    #[inline]
    pub(crate) fn new() -> Self {
        Self { sender: None }
    }
}

#[derive(Debug, Clone, derive_more::Deref, derive_more::DerefMut)]
struct MichiuActivationHandler(Arc<AtomicBool>);

impl ActivationHandler for MichiuActivationHandler {
    // 支援技術がアクセシビリティを要求したタイミングで呼び出される
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        self.set(true);

        let root_id = NodeId(1);
        let root_node = Node::new(Role::Window);

        Some(TreeUpdate {
            nodes: vec![(root_id, root_node)],
            tree: Some(TreeInfo::new(root_id)),
            tree_id: TreeId::ROOT,
            focus: root_id,
        })
    }
}

impl MichiuActivationHandler {
    #[inline]
    pub(crate) fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    #[inline]
    pub(crate) fn get(&self) -> bool {
        self.load(Ordering::SeqCst)
    }

    #[inline]
    pub(crate) fn set(&self, value: bool) {
        self.store(value, Ordering::SeqCst);
    }
}

pub(crate) struct AccessibilityStore {
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
