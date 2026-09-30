use crate::{
    ActiveInteractionStates, ActiveMasksSecondary, CapacityConfig, ChildrenSecondary,
    ComponentMask, Context, DEFAULT_BASIC, DebugStore, Display, EntityId, EventListeners,
    EventListenersSparse, InputContentsSparse, MichiuSoA, MichiuTagRegistry, Overflow,
    RectsSecondary, ResolvedBasicSecondary, TaskSender, TextContentsSparse, a11y::InferenceFn,
    define_sparse_secondary,
};
use accesskit::{
    Action, ActionHandler, ActionRequest, ActivationHandler, Node, NodeId, Role, Toggled, TreeId,
    TreeInfo, TreeUpdate,
};
use accesskit_windows::SubclassingAdapter;
use slotmap::SparseSecondaryMap;
use smallvec::{SmallVec, smallvec};
use std::{
    borrow::Cow,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
};
use windows::Win32::Foundation::HWND;

pub const ROOT_ID: NodeId = NodeId(1);

pub struct NodeSnapshot {
    pub id: NodeId,
    /// 直下の子要素リスト
    pub children: Vec<NodeId>,
    /// ユーザー定義の推論データ
    pub user_node: Option<Node>,
    /// 画面上の絶対座標
    pub bounds: accesskit::Rect,
    /// ユーザーが明示指定したロール
    pub role: Option<Role>,

    pub pressed: Option<bool>,
    pub selected: Option<bool>,
    pub disabled: Option<bool>,
    pub focused: Option<bool>,
    pub blur: bool,
    pub actived: Option<bool>,

    pub is_scrollable: bool,
    pub is_hidden: bool,
    /// テキスト入力可能か
    pub is_text: bool,
    /// 数値入力可能か
    pub is_numeric: bool,
    /// マスキングがあるならパスワード
    pub is_password: bool,

    /// アクセシブル名・表示テキスト
    pub label: Option<Arc<Cow<'static, str>>>,
    /// 入力値・現在値
    pub value: Option<Arc<Cow<'static, str>>>,
    /// プレースホルダー
    pub placeholder: Option<Arc<Cow<'static, str>>>,
    /// スクロールバー
    pub scrollbar: bool,
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

        let root_id = ROOT_ID;
        let root_node = Node::new(Role::Window);

        Some(TreeUpdate {
            nodes: vec![(root_id, root_node)],
            tree: Some(TreeInfo::new(root_id)),
            tree_id: TreeId::ROOT,
            focus: root_id,
        })
    }
}

define_sparse_secondary!(pub struct AccessibilitySparse(Option<Node>));

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
                // ユーザー定義があればそれをベースにし、無ければデフォルト推論で作成
                let mut node = if let Some(user_node) = node_data.user_node {
                    user_node
                } else {
                    let role = node_data.role.unwrap_or({
                        if node_data.pressed.is_some() {
                            Role::Button
                        } else if node_data.value.is_some() {
                            Role::TextInput
                        } else if node_data.label.is_some() {
                            Role::Label
                        } else {
                            Role::GenericContainer
                        }
                    });
                    Node::new(role)
                };

                // ユーザーノードの Role が Unknown のままならフォールバック
                if node.role() == Role::Unknown {
                    let fallback_role = node_data.role.unwrap_or(if node_data.pressed.is_some() {
                        Role::Button
                    } else {
                        Role::GenericContainer
                    });
                    node.set_role(fallback_role);
                }

                node.set_bounds(node_data.bounds);
                node.set_children(node_data.children);

                if node.label().is_none()
                    && let Some(name) = node_data.label
                {
                    node.set_label(name.to_string());
                }
                if node.value().is_none()
                    && let Some(value) = node_data.value
                {
                    node.set_value(value.to_string());
                }
                if node.placeholder().is_none()
                    && let Some(holder) = node_data.placeholder
                {
                    node.set_placeholder(holder.to_string());
                }

                if let Some(is_on) = node_data.disabled {
                    if is_on {
                        node.set_disabled();
                    } else {
                        node.clear_disabled();
                    }
                }

                if node_data.is_hidden {
                    node.set_hidden();
                }

                if let Some(_is_on) = node_data.pressed {
                    node.add_action(Action::Click);
                    node.set_keyboard_shortcut("Enter");
                }

                if let Some(_is_on) = node_data.focused {
                    node.add_action(Action::Focus);
                }

                if let Some(is_on) = node_data.actived {
                    if is_on {
                        node.set_toggled(Toggled::True);
                    } else {
                        node.set_toggled(Toggled::False);
                    }
                }

                if node_data.blur {
                    node.add_action(Action::Blur);
                }

                if let Some(is_on) = node_data.selected {
                    if is_on {
                        node.set_selected(true);
                    } else {
                        node.set_selected(false);
                    }
                }
                if node_data.is_scrollable {
                    node.set_role(Role::ScrollView);
                    node.add_action(Action::ScrollIntoView);
                }
                if node_data.scrollbar {
                    node.set_role(Role::ScrollBar);
                    node.add_action(Action::Click);
                }
                if node_data.is_text {
                    node.set_role(Role::TextInput);
                    node.add_action(Action::Click);
                    node.add_action(Action::Focus);
                    node.add_action(Action::SetValue);
                }
                if node_data.is_numeric {
                    node.set_role(Role::TextInput);
                    node.add_action(Action::Click);
                    node.add_action(Action::Focus);
                    node.add_action(Action::SetValue);
                }
                if node_data.is_password {
                    node.set_role(Role::PasswordInput);
                    node.add_action(Action::Click);
                    node.add_action(Action::Focus);
                    node.add_action(Action::SetValue);
                }

                // ユーザーが明示的に role を指定していた場合は最優先
                if let Some(explicit_role) = node_data.role {
                    node.set_role(explicit_role);
                }

                update_nodes.push((node_data.id, node));
            }

            let mut window_node = Node::new(Role::Window);
            window_node.set_children(vec![snapshot.root_id]);
            update_nodes.push((ROOT_ID, window_node));

            let tree = TreeInfo::new(ROOT_ID);

            let update = TreeUpdate {
                nodes: update_nodes,
                tree: Some(tree),
                tree_id: TreeId::ROOT,
                focus: snapshot.focused_id.unwrap_or(ROOT_ID),
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
    #[allow(unused_assignments)]
    #[inline]
    pub(crate) fn build_accessibility_snapshot(
        buffer: &mut Vec<NodeSnapshot>,
        id: EntityId,
        user_node: Option<Node>,
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

        let children = topo_children
            .find(id)
            .map_or_else(Vec::new, |c| c.iter().copied().map(Into::into).collect());

        let el_node = acce_accessibility
            .find(id)
            .map(|f| f.clone().unwrap_or(Node::new(Role::Unknown)));

        let mask = topo_active_masks.find_or_default(id, debug);

        let listeners = evt_listeners.find(id);

        let pressed = if listeners.is_some_and(|l| l.on_click.is_some()) {
            Some(mask.has(ComponentMask::STATE_PRESSED))
        } else {
            None
        };

        let is_focusable_cap = listeners.is_some_and(|l| l.on_focus.is_some())
            || mask.has(ComponentMask::STYLE_FOCUSABLE);

        let mut focused = if is_focusable_cap {
            Some(
                mask.has(ComponentMask::STATE_FOCUSED)
                    || mask.has(ComponentMask::STATE_FOCUSED_VISIBLE),
            )
        } else {
            None
        };

        let actived = if listeners.is_some_and(|l| l.on_active.is_some()) {
            Some(mask.has(ComponentMask::STATE_ACTIVED))
        } else {
            None
        };

        let selected = if listeners.is_some_and(|l| l.on_select.is_some()) {
            Some(mask.has(ComponentMask::STATE_SELECTED))
        } else {
            None
        };

        let disabled = if listeners.is_some_and(|l| l.on_disable.is_some()) {
            Some(mask.has(ComponentMask::STATE_DISABLED))
        } else {
            None
        };

        let blur = listeners.is_some_and(|l| l.on_blur.is_some());

        let is_scrollable = if mask.has(ComponentMask::STYLE_OVERFLOW) {
            matches!(
                (basic.overflow.x, basic.overflow.y),
                (_, Overflow::Scroll) | (Overflow::Scroll, _)
            )
        } else {
            false
        };

        let display = if mask.has(ComponentMask::STYLE_DISPLAY) {
            basic.display == Display::None
        } else {
            false
        };

        let is_hidden = display || rect.width == 0.0 || rect.height == 0.0;

        let scrollbar = mask.has(ComponentMask::STYLE_SCROLLBAR);

        // 指定がある場合はそれを、無い場合は自身のテキストコンテンツ、それもない場合は子要素のラベル
        let label = if let Some(node) = &el_node {
            node.label().map(|f| Arc::new(f.to_owned().into()))
        } else if mask.has_text_content() {
            cont_text_contents.find(id).map(|t| Arc::new(t.0.clone()))
        } else if let Some(children) = topo_children.find(id) {
            children.iter().find_map(|c| {
                cont_text_contents
                    .find(*c)
                    .map(|text| Arc::new(text.0.clone()))
            })
        } else {
            None
        };

        let has_input = mask.has_input_content();
        let mut is_text = false;
        let mut is_numeric = false;
        let mut is_password = false;
        let mut placeholder = None;

        let value = if has_input {
            // 入力要素はデフォルトでフォーカス可能
            focused = Some(true);
            if let Some(contents) = cont_input_contents.find(id) {
                if contents.numeric_only {
                    is_numeric = true;
                } else if contents.mask_text.is_some() {
                    is_password = true;
                } else {
                    is_text = true;
                }
                placeholder = contents.placeholder.clone().map(Arc::new);
                Some(Arc::new(contents.to_michiu().0))
            } else {
                None
            }
        } else if let Some(children) = topo_children.find(id) {
            children.iter().find_map(|c| {
                if let Some(contents) = cont_input_contents.find(*c) {
                    if contents.numeric_only {
                        is_numeric = true;
                    } else if contents.mask_text.is_some() {
                        is_password = true;
                    } else {
                        is_text = true;
                    }
                    placeholder = contents.placeholder.clone().map(Arc::new);
                    Some(Arc::new(contents.to_michiu().0))
                } else {
                    None
                }
            })
        } else {
            None
        };

        let mut user_node = user_node;
        let mut role = None;
        if let Some(node) = el_node {
            role = Some(node.role());
            user_node = Some(node);
        }

        buffer.push(NodeSnapshot {
            id: id.into(),
            children,
            user_node,
            bounds: rect.into(),
            role,
            is_scrollable,
            is_hidden,
            label,
            value,
            placeholder,
            is_text,
            is_numeric,
            is_password,
            scrollbar,
            pressed,
            selected,
            disabled,
            focused,
            blur,
            actived,
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
