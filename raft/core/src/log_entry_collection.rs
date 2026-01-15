use crate::log_entry::LogEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionError {
    Full,
}

pub trait LogEntryCollection {
    type Payload: Clone;
    type Iter<'a>: Iterator<Item = &'a LogEntry<Self::Payload>>
    where
        Self: 'a,
        Self::Payload: 'a;

    fn new(entries: &[LogEntry<Self::Payload>]) -> Self;
    fn clear(&mut self);
    fn push(&mut self, entry: LogEntry<Self::Payload>) -> Result<(), CollectionError>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn as_slice(&self) -> &[LogEntry<Self::Payload>];
    fn iter(&self) -> Self::Iter<'_>;
    fn truncate(&mut self, index: usize);
}
