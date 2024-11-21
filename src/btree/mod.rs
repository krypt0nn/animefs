pub mod record;
pub mod tree;

pub type BTreeRecord64 = record::GenericBTreeRecord<8, 8>;
pub type BTree64 = tree::GenericBTree<8, 8>;

pub mod prelude {
    pub use super::record::*;
    pub use super::tree::*;

    pub use super::{
        BTreeRecord64,
        BTree64
    };
}
