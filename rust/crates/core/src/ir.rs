use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::Schema as ArrowSchema;
use indexmap::IndexMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Decimal(String),
    String(String),
    Date(String),
    Time(String),
    DateTime(String),
    Uuid(String),
    Binary(Vec<u8>),
    List(Vec<Node>),
    Object(IndexMap<String, Node>),
}

/// Independent Arrow table struct for zero-copy integration.
/// Decoupled from Node to keep the IR clean (scalars/containers only).
/// `batches` holds one or more RecordBatches that together form the logical table.
#[derive(Debug, Clone)]
pub struct ArrowTable {
    pub schema: Arc<ArrowSchema>,
    pub batches: Vec<RecordBatch>,
}

impl ArrowTable {
    /// Total row count across all batches.
    pub fn num_rows(&self) -> usize {
        self.batches.iter().map(|b| b.num_rows()).sum()
    }
}

impl PartialEq for ArrowTable {
    fn eq(&self, other: &Self) -> bool {
        self.schema == other.schema && self.batches == other.batches
    }
}
