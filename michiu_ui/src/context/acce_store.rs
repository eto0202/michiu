use crate::{
    ActiveMasksSecondary, CapacityConfig, ChildrenSecondary, ComponentMask, DEFAULT_BASIC,
    DebugStore, Display, EntityId, EventListenersSparse, InputContentsSparse, MichiuSoA, Overflow,
    RectsSecondary, ResolvedBasicSecondary, TaskSender, TextContentsSparse, a11y::InferenceTags,
    define_sparse_secondary,
};
use accesskit::{
    ActionHandler, ActionRequest, ActivationHandler, Node, NodeId, Role, TreeId, TreeInfo,
    TreeUpdate,
};
use accesskit_windows::SubclassingAdapter;
use slotmap::SparseSecondaryMap;
use std::{
    borrow::Cow,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
};
use windows::Win32::Foundation::HWND;

pub const WINDOW_ROOT_ID: NodeId = NodeId(1);

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
    pub label: Option<Box<str>>,
    /// 入力値・現在値
    pub value: Option<Box<str>>,
}

pub struct AccessibilitySnapshot {
    /// ルート要素のID
    pub root_id: NodeId,
    /// 現在フォーカスされている要素
    pub focused_id: Option<NodeId>,
    /// ソート順に並んだノードのスナップショット配列
    pub nodes: Vec<NodeSnapshot>,
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

        let root_id = WINDOW_ROOT_ID;
        let root_node = Node::new(Role::Window);

        Some(TreeUpdate {
            nodes: vec![(root_id, root_node)],
            tree: Some(TreeInfo::new(root_id)),
            tree_id: TreeId::ROOT,
            focus: root_id,
        })
    }
}

define_sparse_secondary!(pub struct AccessibilitySparse((Role, Option<Cow<'static, str>>)));

pub(crate) struct AccessibilityStore {
    pub(crate) acce_worker_sender: Option<Sender<AccessibilitySnapshot>>,
    pub(crate) acce_buffer: Option<Vec<NodeSnapshot>>,
    pub(crate) acce_adapter: Option<SubclassingAdapter>,
    pub(crate) acce_is_active: Arc<AtomicBool>,
    pub(crate) acce_accessibility: AccessibilitySparse,
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
            acce_accessibility: AccessibilitySparse(SparseSecondaryMap::new()),
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn with_capacity(c: &CapacityConfig) -> Self {
        Self {
            acce_accessibility: AccessibilitySparse(SparseSecondaryMap::with_capacity(
                c.acce_accessibility,
            )),
            ..Default::default()
        }
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.acce_accessibility.clear();
    }

    #[inline]
    pub(crate) fn despawn(&mut self, id: EntityId) {
        self.acce_accessibility.remove(id);
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
            acce_accessibility: AccessibilitySparse(SparseSecondaryMap::new()),
        }
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
                Self::worker_loop(to_worker_rx, sys_task_sender);
            })
            .ok();

        self.acce_worker_sender = Some(to_worker_tx);
    }

    /// `TreeUpdate` の構築とタスク送信
    #[allow(clippy::needless_pass_by_value)]
    fn worker_loop(rx: Receiver<AccessibilitySnapshot>, task_sender: TaskSender) {
        while let Ok(mut snapshot) = rx.recv() {
            // ノードが空の場合は OS のツリーを破壊しないようスキップ
            if snapshot.nodes.is_empty() {
                let buffer = snapshot.nodes;
                let _ = task_sender.send(move |context| {
                    context.acce.acce_buffer = Some(buffer);
                });
                continue;
            }

            let mut update_nodes = Vec::with_capacity(snapshot.nodes.len());

            for node_data in snapshot.nodes.drain(..) {
                let role = node_data.role.unwrap_or({
                    // 詳細な推論は後で
                    if node_data.is_clickable {
                        Role::Button
                    } else if node_data.label.is_some() {
                        if node_data.value.is_some() {
                            Role::TextInput
                        } else {
                            Role::Label
                        }
                    } else if node_data.value.is_some() {
                        Role::TextInput
                    } else if node_data.checked.is_some() {
                        Role::CheckBox
                    } else {
                        Role::GenericContainer
                    }
                });

                let mut node = Node::new(role);
                node.set_bounds(node_data.bounds);
                node.set_children(node_data.children);
                if let Some(name) = node_data.label {
                    node.set_label(name);
                }
                if let Some(value) = node_data.value {
                    node.set_value(value);
                }
                if node_data.is_disabled {
                    node.set_disabled();
                }
                if node_data.is_hidden {
                    node.set_hidden();
                }
                if node_data.is_clickable {
                    node.add_action(accesskit::Action::Click);
                }

                update_nodes.push((node_data.id, node));
            }

            let mut window_node = Node::new(Role::Window);
            window_node.set_children(vec![snapshot.root_id]);
            update_nodes.push((WINDOW_ROOT_ID, window_node));

            let update = TreeUpdate {
                nodes: update_nodes,
                tree: Some(TreeInfo::new(WINDOW_ROOT_ID)),
                tree_id: TreeId::ROOT,
                focus: snapshot.focused_id.unwrap_or(WINDOW_ROOT_ID),
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

    /// `TreeUpdate` 用のデータ収集
    #[inline]
    pub(crate) fn build_accessibility_snapshot(
        buffer: &mut Vec<NodeSnapshot>,
        id: EntityId,
        topo_children: &ChildrenSecondary,
        topo_active_masks: &ActiveMasksSecondary,
        evt_listeners: &EventListenersSparse,
        cont_text_contents: &TextContentsSparse,
        cont_input_contents: &InputContentsSparse,
        lay_resolved_basic: &ResolvedBasicSecondary,
        out_rects: &RectsSecondary,
        debug: &mut DebugStore,
        acce_accessibility: &AccessibilitySparse,
    ) {
        let rect = out_rects.find_or_default(id, debug);
        let basic = lay_resolved_basic.find_or(id, &DEFAULT_BASIC, debug);
        let listener = evt_listeners.find(id);

        let children = topo_children
            .find(id)
            .map_or_else(Vec::new, |c| c.iter().copied().map(Into::into).collect());

        let (role, user_label) = acce_accessibility
            .find(id)
            .map_or((None, None), |f| (Some(f.0), f.1.clone()));

        let mask = topo_active_masks.find_or_default(id, debug);

        let is_clickable = if let Some(l) = listener {
            l.on_click.is_some()
        } else {
            false
        };
        let is_scrollable = if mask.has(ComponentMask::STYLE_OVERFLOW) {
            matches!(
                (basic.overflow.x, basic.overflow.y),
                (_, Overflow::Scroll) | (Overflow::Scroll, _)
            )
        } else {
            false
        };
        let checked = if let Some(l) = listener
            && l.on_select.is_some()
        {
            Some(mask.has(ComponentMask::STATE_SELECTED))
        } else {
            None
        };
        let is_disabled = mask.has(ComponentMask::STATE_DISABLED);
        let is_hidden = if mask.has(ComponentMask::STYLE_DISPLAY) {
            basic.display == Display::None
        } else {
            false
        };

        // 指定がある場合はそれを、無い場合は自身のテキストコンテンツ、それもない場合は子要素のラベル
        let label = if user_label.is_some() {
            user_label.map(|f| f.into_owned().into_boxed_str())
        } else if mask.has_text_content() {
            cont_text_contents.find(id).map(|t| t.as_ref().into())
        } else if let Some(children) = topo_children.find(id) {
            children
                .iter()
                .find_map(|c| cont_text_contents.find(*c).map(|text| text.as_ref().into()))
        } else {
            None
        };

        let value = if mask.has_input_content() {
            cont_input_contents
                .find(id)
                .map(|i| i.to_michiu().as_ref().into())
        } else if let Some(children) = topo_children.find(id) {
            children.iter().find_map(|c| {
                cont_input_contents
                    .find(*c)
                    .map(|input| input.to_michiu().as_ref().into())
            })
        } else {
            None
        };

        buffer.push(NodeSnapshot {
            id: id.into(),
            children,
            bounds: rect.into(),
            role,
            is_clickable,
            is_scrollable,
            checked,
            is_disabled,
            is_hidden,
            label,
            value,
        });
    }
}

impl Default for AccessibilityStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
