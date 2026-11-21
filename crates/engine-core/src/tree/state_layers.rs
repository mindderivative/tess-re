//! State layers (0.6.11, P13): which of a node's hover, pressed and focus overlays hold, and the
//! start of their fades. The overlay is `PaintProperties::state_layer`; the paint walk draws what
//! it holds, and this keeps it in step with the hovered, pressed and focused nodes.
//!
//! Every change of hover, press, focus, input modality, a node's `disabled` flag or a layer
//! property notes the nodes concerned (`state_layer_pending`); the next `tick_all` decides each
//! one's target with the clock it was given and starts the fade, so a burst of changes in one
//! frame is one fade, and the fade starts when the frame does.

use super::*;
use crate::state_layer::StateHolds;

impl Tree {
    /// Whether `id` is `leaf` or one of its ancestors.
    fn chain_has(&self, leaf: Option<NodeId>, id: NodeId) -> bool {
        leaf.is_some_and(|leaf| self.ancestors(leaf).any(|a| a == id))
    }

    /// Notes `id` for the next tick if it has state layers.
    pub(super) fn note_state_node(&mut self, id: Option<NodeId>) {
        if let Some(id) = id
            && self
                .nodes
                .get(id)
                .is_some_and(|n| n.paint.state_layer.is_some())
        {
            self.state_layer_pending.push(id);
        }
    }

    /// Notes `leaf` and every ancestor of it that has state layers: what a change of hover or
    /// press at `leaf` reaches.
    pub(super) fn note_state_chain(&mut self, leaf: Option<NodeId>) {
        let mut current = leaf;
        while let Some(id) = current {
            let Some(node) = self.nodes.get(id) else {
                break;
            };
            if node.paint.state_layer.is_some() {
                self.state_layer_pending.push(id);
            }
            current = node.parent;
        }
    }

    /// For `Window.set` after a layer property or `disabled` changed on `id`: its layers are
    /// looked at again on the next tick.
    pub fn mark_state_layer(&mut self, id: NodeId) {
        self.note_state_node(Some(id));
        self.dirty = true;
    }

    /// Whether the current focus is "visible": it came from the keyboard or assistive
    /// technology, not a pointer press. The host sets it when focus moves (`focus_layer` draws
    /// only for visible focus unless `focus_layer_any_modality`). `true` until told otherwise.
    pub fn focus_visible(&self) -> bool {
        self.focus_visible
    }

    /// See [`Tree::focus_visible`].
    pub fn set_focus_visible(&mut self, visible: bool) {
        if self.focus_visible != visible {
            self.focus_visible = visible;
            self.note_state_node(self.focused);
        }
    }

    /// Edits `id`'s state layers through `edit` (given the existing ones, or fresh defaults, with
    /// which a node starts having any) and looks at them again on the next tick.
    pub fn edit_state_layer(&mut self, id: NodeId, edit: impl FnOnce(&mut crate::StateLayer)) {
        let Some(node) = self.nodes.get_mut(id) else {
            return;
        };
        let layer = node.paint.state_layer.get_or_insert_with(Default::default);
        edit(layer);
        self.note_state_node(Some(id));
        self.dirty = true;
    }

    /// Starts the fades the noted changes call for (`tick_all` runs it first, before it looks
    /// at what animations started). Whether anything was looked at.
    pub(super) fn sync_state_layers(&mut self, now: Instant) -> bool {
        if self.state_layer_pending.is_empty() {
            return false;
        }
        let mut ids = std::mem::take(&mut self.state_layer_pending);
        ids.sort_unstable();
        ids.dedup();
        let reduced = self.reduced_motion();
        for id in ids {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            let Some(layer) = node.paint.state_layer.as_deref() else {
                continue;
            };
            // A disabled node takes no layer (UX State Layers, "Disabled scheme").
            let holds = if node.access.states.disabled {
                StateHolds::default()
            } else {
                StateHolds {
                    hovered: self.chain_has(self.hovered, id),
                    pressed: self.pressed.is_some_and(|(button, pressed)| {
                        button == PointerButton::Primary && self.chain_has(Some(pressed), id)
                    }),
                    focused: self.focused == Some(id)
                        && (self.focus_visible || layer.focus_any_modality),
                }
            };
            let target = layer.target(holds);
            if let Some(layer) = self
                .nodes
                .get_mut(id)
                .and_then(|n| n.paint.state_layer.as_deref_mut())
            {
                layer.sync(target, now, reduced);
            }
        }
        self.dirty = true;
        true
    }
}
