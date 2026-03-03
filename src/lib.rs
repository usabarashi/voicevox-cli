#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub mod config;
pub mod domain;
pub mod infrastructure;
pub mod interface;
