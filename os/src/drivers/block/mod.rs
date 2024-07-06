mod virtio_blk;
use alloc::sync::Arc;
use easy_fs::BlockDevice;
use lazy_static::lazy_static;
pub use virtio_blk::VirtIOBlock;
use crate::board::BlockDeviceImpl;

lazy_static! {
    pub static ref BLOCK_DEVICE: Arc<dyn BlockDevice> = Arc::new(BlockDeviceImpl::new());
}
