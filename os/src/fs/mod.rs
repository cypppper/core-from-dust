use crate::mm::UserBuffer;
mod inode;
pub mod stdio;
mod pipe;

/// File trait
pub trait File: Send + Sync {
    /// If readable
    fn readable(&self) -> bool;
    /// If writable
    fn writable(&self) -> bool;
    /// Read file to `UserBuffer`
    fn read(&self, buf: UserBuffer) -> usize;
    /// Write `UserBuffer` to file
    fn write(&self, buf: UserBuffer) -> usize;
}

pub use inode::{open_file, OpenFlags};
pub use pipe::{Pipe, make_pipe};
pub use inode::list_apps;
