use crate::parser::ast::{Node, Position};

/// Per-execution-instance copy of a Scope's nodes
#[derive(Debug, Clone)]
pub struct NodeList {
    nodes: Vec<Option<Node>>,
}

impl NodeList {
    pub fn new(nodes: &[Node]) -> Self {
        Self {
            nodes: nodes.iter().cloned().map(Some).collect(),
        }
    }

    /// Enumerates over live node (index, &Node)
    pub fn iter_live(&self) -> impl Iterator<Item = (usize, &Node)> {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| Some((i, n.as_ref()?)))
    }

    /// The node at `index` if it is still live
    pub fn get(&self, index: usize) -> Option<&Node> {
        self.nodes.get(index)?.as_ref()
    }

    /// Deletes the node at `index`
    fn delete(&mut self, index: usize) {
        if let Some(slot) = self.nodes.get_mut(index) {
            *slot = None;
        }
    }

    /// `MOVE LAST NODE TO x y / BY x y`
    pub fn move_node(
        &mut self,
        index: usize,
        new_pos: Position,
        in_bounds: impl Fn(Position) -> bool, // scope's [1,width]*[1,height] check
    ) {
        let Some(Some(node)) = self.nodes.get_mut(index) else {
            return;
        };
        if in_bounds(new_pos) {
            node.position = new_pos;
        } else {
            self.delete(index);
        }
    }
}
