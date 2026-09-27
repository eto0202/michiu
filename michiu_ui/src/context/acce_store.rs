use crate::{EntityId, TaskSender};
pub use accesskit::Role;
use accesskit::{
    ActionHandler, ActionRequest, ActivationHandler, Node, NodeId, TreeId, TreeInfo, TreeUpdate,
};
use accesskit_windows::SubclassingAdapter;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, Sender, channel},
};
use windows::Win32::Foundation::HWND;

pub struct NodeSnapshot {
    pub id: NodeId,
    /// 直下の子要素リスト
    pub children: Vec<NodeId>,
    /// 画面上の絶対座標
    pub bounds: accesskit::Rect,
    /// ユーザーが明示指定したロール
    pub role: Option<Role>,

    /// クリックイベントの購読有無
    pub is_clickable: bool,
    /// テキスト入力可能か
    pub is_scrollable: bool,
    /// チェック状態
    pub checked: Option<bool>,
    /// 無効化フラグ
    pub is_disabled: bool,
    /// 非表示フラグ
    pub is_hidden: bool,

    /// アクセシブル名・表示テキスト
    pub name: Option<Box<str>>,
    /// 入力値・現在値
    pub value: Option<Box<str>>,
}

pub struct AccessibilitySnapshot {
    /// ルート要素のID
    pub root_id: NodeId,
    /// 現在フォーカスされている要素
    pub focused_id: Option<NodeId>,
    /// DFS順またはソート順に並んだノードのスナップショット配列
    pub nodes: Vec<NodeSnapshot>,
}

/// 構築済みツリーと再利用バッファ
pub struct AccessibilityWorkerResult {
    pub update: TreeUpdate,
    pub buffer: Vec<NodeSnapshot>,
}

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
        self.store(true, Ordering::Release);

        let root_id = NodeId(u64::MAX);
        let root_node = Node::new(Role::Window);

        Some(TreeUpdate {
            nodes: vec![(root_id, root_node)],
            tree: Some(TreeInfo::new(root_id)),
            tree_id: TreeId::ROOT,
            focus: root_id,
        })
    }
}

pub(crate) struct AccessibilityStore {
    pub(crate) acce_worker_sender: Option<Sender<AccessibilitySnapshot>>,
    pub(crate) acce_buffer: Option<Vec<NodeSnapshot>>,
    pub(crate) acce_adapter: Option<SubclassingAdapter>,
    pub(crate) acce_is_active: Arc<AtomicBool>,
}

impl AccessibilityStore {
    #[inline]
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            acce_worker_sender: None,
            acce_buffer: None,
            acce_adapter: None,
            acce_is_active: Arc::new(AtomicBool::new(false)),
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn init(hwnd: HWND) -> Self {
        let is_active = Arc::new(AtomicBool::new(false));
        let activation_handler = MichiuActivationHandler(Arc::clone(&is_active));

        Self {
            acce_worker_sender: None,
            acce_buffer: Some(Vec::with_capacity(128)),
            acce_adapter: Some(SubclassingAdapter::new(
                hwnd,
                activation_handler,
                MichiuActionHandler::new(),
            )),
            acce_is_active: is_active,
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

    #[inline]
    #[must_use]
    pub(crate) fn is_active(&self) -> bool {
        self.acce_is_active.load(Ordering::Acquire)
    }

    /// 支援技術がアクティブかつワーカー未起動の場合にスレッドを立ち上げる
    #[inline]
    pub(crate) fn ensure_worker_spawned(&mut self, sys_task_sender: TaskSender) {
        if !self.is_active() || self.acce_worker_sender.is_some() {
            return;
        }

        let (to_worker_tx, to_worker_rx) = channel::<AccessibilitySnapshot>();

        std::thread::Builder::new()
            .name("michiu_accessibility_worker".into())
            .spawn(move || {
                Self::worker_loop(&to_worker_rx, sys_task_sender);
            })
            .ok();

        self.acce_worker_sender = Some(to_worker_tx);
    }

    fn worker_loop(rx: &Receiver<AccessibilitySnapshot>, task_sender: TaskSender) {
        while let Ok(mut snapshot) = rx.recv() {
            let mut update_nodes = Vec::with_capacity(snapshot.nodes.len());

            for node_data in snapshot.nodes.drain(..) {
                let role = node_data.role.unwrap_or({
                    if node_data.is_clickable {
                        Role::Button
                    } else {
                        Role::GenericContainer
                    }
                });

                let mut node = Node::new(role);
                node.set_bounds(node_data.bounds);
                node.set_children(node_data.children);
                if let Some(name) = node_data.name {
                    node.set_label(name);
                }

                update_nodes.push((node_data.id, node));
            }

            let update = TreeUpdate {
                nodes: update_nodes,
                tree: Some(TreeInfo::new(snapshot.root_id)),
                tree_id: TreeId::ROOT,
                focus: snapshot.root_id,
            };

            let buffer = snapshot.nodes; // drain済み（len: 0, cap: 保持）

            // メインスレッドへタスクとして送信
            let _ = task_sender.send(move |cx| {
                if let Some(ref mut adapter) = cx.acce.acce_adapter
                    && let Some(events) = adapter.update_if_active(|| update)
                {
                    events.raise();
                }
                // 空バッファを返却して次回フレームで再利用
                cx.acce.acce_buffer = Some(buffer);
            });
        }
    }

    /// 再利用可能なバッファを取り出す（存在しなければ新規確保）
    #[inline]
    pub(crate) fn take_buffer(&mut self) -> Vec<NodeSnapshot> {
        self.acce_buffer
            .take()
            .unwrap_or_else(|| Vec::with_capacity(128))
    }

    /// スナップショットをワーカーへ送信
    #[inline]
    pub(crate) fn send_snapshot(&mut self, snapshot: AccessibilitySnapshot) {
        if let Some(ref sender) = self.acce_worker_sender {
            let _ = sender.send(snapshot);
        }
    }

    pub(crate) fn build_accessibility_snapshot(&self, buffer: &mut Vec<NodeSnapshot>) {
        todo!()
    }
}

impl Default for AccessibilityStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
