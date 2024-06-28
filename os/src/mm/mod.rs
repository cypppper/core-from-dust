mod heap_allocator;
mod address;
mod page_table;
mod frame_allocator;
mod memory_set;

pub use memory_set::{KERNEL_SPACE, remap_test, MemorySet, MapPermission};
pub use address::{PhysPageNum, PhysAddr, VirtAddr, VirtPageNum};
pub use page_table::{translated_byte_buffer, translated_str, translated_refmut};

pub fn init() {
    heap_allocator::init_heap();  // enable rust data-structure
    frame_allocator::init_frame_allocator();  // enable physical frame alloc and recycle
    KERNEL_SPACE.exclusive_access().activate();
    // remap_test();
}
