use crate::types::NodeId;

pub trait NodeCollection {
    fn new() -> Self;
    fn clear(&mut self);
    fn push(&mut self, id: NodeId) -> Result<(), ()>;
    fn len(&self) -> usize;
    fn iter(&self) -> Box<dyn Iterator<Item = &NodeId> + '_>;
    fn is_empty(&self) -> bool;
    
}
