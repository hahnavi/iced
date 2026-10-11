#![allow(missing_docs)]

use crate::Control;
use crate::futures::futures::channel::mpsc;

use iced_accessibility::A11yTree;
use iced_accessibility::accesskit::{
    ActivationHandler, Node, NodeId, Role, Tree, TreeId, TreeUpdate,
};
use iced_runtime::core;

pub(crate) fn tree_update(window_node_id: u64, title: String, widgets: A11yTree) -> TreeUpdate {
    let root_id = NodeId(window_node_id);
    let mut root = Node::new(Role::Window);
    root.set_label(title);
    root.set_children(
        widgets
            .root()
            .iter()
            .map(|node| NodeId::from(node.id().clone()))
            .collect::<Vec<_>>(),
    );

    let nodes = std::iter::once((root_id, root))
        .chain(Vec::<(NodeId, Node)>::from(widgets))
        .collect();

    TreeUpdate {
        tree_id: TreeId::ROOT,
        nodes,
        tree: Some(Tree::new(root_id)),
        focus: root_id,
    }
}

pub(super) struct WinitActivationHandler {
    pub(super) id: core::window::Id,
    pub(super) node_id: u64,
    pub(super) proxy: mpsc::UnboundedSender<Control>,
    pub(super) event_loop_proxy: winit::event_loop::EventLoopProxy,
    pub(super) title: String,
}

impl ActivationHandler for WinitActivationHandler {
    fn request_initial_tree(&mut self) -> Option<iced_accessibility::accesskit::TreeUpdate> {
        let _ = self
            .proxy
            .unbounded_send(Control::AccessibilityEnabled(self.id, true));
        self.event_loop_proxy.wake_up();
        let mut node = Node::new(Role::Window);
        node.set_label(self.title.clone());
        let root = NodeId(self.node_id);
        Some(TreeUpdate {
            tree_id: TreeId::ROOT,
            nodes: vec![(root, node)],
            tree: Some(Tree::new(root)),
            focus: root,
        })
    }
}

pub(super) struct WinitActionHandler {
    pub(super) id: core::window::Id,
    pub(super) proxy: mpsc::UnboundedSender<Control>,
    pub(super) event_loop_proxy: winit::event_loop::EventLoopProxy,
}

impl iced_accessibility::accesskit::ActionHandler for WinitActionHandler {
    fn do_action(&mut self, request: iced_accessibility::accesskit::ActionRequest) {
        let _ = self
            .proxy
            .unbounded_send(Control::Accessibility(self.id, request));
        self.event_loop_proxy.wake_up();
    }
}

pub(super) struct WinitDeactivationHandler {
    pub(super) id: core::window::Id,
    pub(super) proxy: mpsc::UnboundedSender<Control>,
    pub(super) event_loop_proxy: winit::event_loop::EventLoopProxy,
}

impl iced_accessibility::accesskit::DeactivationHandler for WinitDeactivationHandler {
    fn deactivate_accessibility(&mut self) {
        let _ = self
            .proxy
            .unbounded_send(Control::AccessibilityEnabled(self.id, false));
        self.event_loop_proxy.wake_up();
    }
}

#[cfg(test)]
mod tests {
    use super::tree_update;
    use iced_accessibility::{
        A11yId, A11yTree,
        accesskit::{Node, NodeId, Role},
    };

    #[test]
    fn tree_update_attaches_widget_roots_to_the_window() {
        let widget_id = A11yId::from(iced_accessibility::id::Id::unique());
        let widget_node = Node::new(Role::Button);
        let widget_tree = A11yTree::leaf(widget_node, widget_id.clone());
        let widget_node_id = NodeId::from(widget_id);
        let window_node_id = iced_accessibility::id::window_node_id();

        let update = tree_update(window_node_id, "Accessibility test".to_owned(), widget_tree);

        let root_id = NodeId(window_node_id);
        let root = update
            .nodes
            .iter()
            .find_map(|(id, node)| (*id == root_id).then_some(node))
            .expect("window root in update");

        assert_eq!(root.children(), &[widget_node_id]);
        assert!(update.nodes.iter().any(|(id, _)| *id == widget_node_id));
        assert_eq!(update.focus, root_id);
    }
}
