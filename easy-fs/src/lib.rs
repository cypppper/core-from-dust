#![no_std]
extern crate alloc;
extern crate spin;
extern crate lazy_static;

mod block_cache;
mod block_dev;
mod layout;
mod bitmap;
mod efs;
mod vfs;

pub const BLOCK_SZ: usize = 512;
pub use block_dev::BlockDevice;
pub use layout::DataBlock;
pub use efs::EasyFileSystem;
pub use vfs::Inode;
pub use block_cache::block_cache_sync_all;
