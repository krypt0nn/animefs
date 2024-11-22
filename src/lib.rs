#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// It was an experiment. Code is not functional.
// pub mod error_code;

pub mod io;
pub mod filesystem;
pub mod pages;
pub mod btree;

pub mod prelude {
    pub use super::io::prelude::*;
    pub use super::filesystem::prelude::*;
    pub use super::pages::prelude::*;
    pub use super::btree::prelude::*;
}
