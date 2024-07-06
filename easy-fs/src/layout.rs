use core::ops::Index;

use alloc::sync::Arc;
use alloc::vec::Vec;
use crate::{block_cache::get_block_cache, BlockDevice, BLOCK_SZ};


/// Magic number for sanity check
const EFS_MAGIC: u32 = 0x3b800001;
/// The max number of direct inodes
const INODE_DIRECT_COUNT: usize = 28;
/// The max length of inode name
const NAME_LENGTH_LIMIT: usize = 27;
/// The max number of indirect1 block
const INODE_INDIRECT1_COUNT: usize = BLOCK_SZ / 4;
/// The max number of indirect2 block
const INODE_INDIRECT2_COUNT: usize = INODE_INDIRECT1_COUNT * INODE_INDIRECT1_COUNT;
/// The upper bound of direct inode index
const DIRECT_BOUND: usize = INODE_DIRECT_COUNT;
/// The upper bound of indirect1 inode index
const INDIRECT1_BOUND: usize = DIRECT_BOUND + INODE_INDIRECT1_COUNT;
/// The upper bound of indirect2 inode indexs
#[allow(unused)]
const INDIRECT2_BOUND: usize = INDIRECT1_BOUND + INODE_INDIRECT2_COUNT;

pub const DIRENT_SZ: usize = 32;

type IndirectBlock = [u32; BLOCK_SZ / 4];
pub type DataBlock = [u8; BLOCK_SZ];

#[repr(C)]
pub struct SuperBlock {
    magic: u32,
    pub total_blocks: u32,
    pub inode_bitmap_blocks: u32,
    pub inode_area_blocks: u32,
    pub data_bitmap_blocks: u32,
    pub data_area_blocks: u32,
}

impl SuperBlock {
    pub fn initialize(
        &mut self,
        total_blocks: u32,
        inode_bitmap_blocks: u32,
        inode_area_blocks: u32,
        data_bitmap_blocks: u32,
        data_area_blocks: u32,
    ) {
        *self = Self {
            magic: EFS_MAGIC,
            total_blocks,
            inode_bitmap_blocks,
            inode_area_blocks,
            data_bitmap_blocks,
            data_area_blocks,
        }
    }
    /// Check if a super block is valid using efs magic
    pub fn is_valid(&self) -> bool {
        self.magic == EFS_MAGIC
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum DiskInodeType {
    File,
    Directory,
}

#[repr(C)]
pub struct DiskInode{
    pub size: u32,  // bytes of a file/dir
    pub direct: [u32; INODE_DIRECT_COUNT],
    pub indirect1: u32,  // point to a indirect1 block, every u32 point to a data block
    pub indirect2: u32,  // point to a indirect2 block, every u32 point to a indirect1 block
    type_: DiskInodeType,  // 4 byte
}

impl DiskInode {
    /// indirect1 and indirect2 block are allocated only then they are needed.
    pub fn initialize(&mut self, type_: DiskInodeType) {
        self.size = 0;
        self.direct.iter_mut().for_each(|v| *v = 0);
        self.indirect1 = 0;
        self.indirect2 = 0;
        self.type_ = type_;
    }
    pub fn is_dir(&self) -> bool {
        self.type_ == DiskInodeType::Directory
    }
    pub fn is_file(&self) -> bool {
        self.type_ == DiskInodeType::File
    }
    pub fn get_block_id(&self, inner_id: u32, block_device: &Arc<dyn BlockDevice>) -> u32 {
        let inner_id = inner_id as usize;
        if inner_id < INODE_DIRECT_COUNT {
            self.direct[inner_id]
        } else if inner_id < INDIRECT1_BOUND {
            get_block_cache(
                self.indirect1 as usize,
                Arc::clone(block_device),
            )
            .lock()
            .read(0, |indirect_block: &IndirectBlock| {
                indirect_block[inner_id - INODE_DIRECT_COUNT]
            })
        } else {
            let last = inner_id - INDIRECT1_BOUND;
            let indirect1 = get_block_cache(
                self.indirect2 as usize,
                Arc::clone(block_device),
            )
            .lock()
            .read(0, |indirect_block: &IndirectBlock| {
                indirect_block[last / INODE_INDIRECT1_COUNT]
            });
            get_block_cache(
                indirect1 as usize,
                Arc::clone(block_device),
            )
            .lock()
            .read(0, |indirect_block: &IndirectBlock| {
                indirect_block[last % INODE_INDIRECT1_COUNT]
            })
        }
    }
    pub fn data_blocks(&self) -> u32 {
        Self::_data_blocks(self.size)
    }
    fn _data_blocks(size: u32) -> u32 {
        (size + BLOCK_SZ as u32 - 1) / BLOCK_SZ as u32
    }
    /// Return number of data + indirect blocks needed include indirect1/2
    pub fn total_blocks(size: u32) -> u32 {
        let data_blocks = Self::_data_blocks(size) as usize;
        let mut total = data_blocks as usize;
        // indirect1
        if data_blocks > INODE_DIRECT_COUNT {
            total += 1;
        }
        // indirect2
        if data_blocks > INDIRECT1_BOUND {
            total += 1;
            // sub indirect1
            total += (data_blocks - INDIRECT1_BOUND + INODE_INDIRECT1_COUNT - 1) / INODE_INDIRECT1_COUNT;
        }
        total as u32
    }
    pub fn blocks_num_needed(&self, new_size: u32) -> u32 {
        assert!(new_size >= self.size);
        Self::total_blocks(new_size) - Self::total_blocks(self.size)
    }
    pub fn increase_size(
        &mut self,
        new_size: u32,
        new_blocks: Vec<u32>,  // allocated by upper-layer disk-block-manager, efs block id
        block_device: &Arc<dyn BlockDevice>,
    ) {
        let mut current_blocks = self.data_blocks();
        self.size = new_size;
        let mut total_blocks = self.data_blocks();
        let mut new_blocks = new_blocks.into_iter();
        // fill direct
        while current_blocks < total_blocks.min(INODE_DIRECT_COUNT as u32) {
            self.direct[current_blocks as usize] = new_blocks.next().unwrap();
            current_blocks += 1;
        }
        // alloc indirect1
        if total_blocks > INODE_DIRECT_COUNT as u32 {
            if current_blocks == INODE_DIRECT_COUNT as u32 {
                self.indirect1 = new_blocks.next().unwrap();
            }
            current_blocks -= INODE_DIRECT_COUNT as u32;
            total_blocks -= INODE_DIRECT_COUNT as u32;
        } else {
            return;
        }
        // fill indirect1
        get_block_cache(self.indirect1 as usize, Arc::clone(&block_device))
            .lock()
            .modify(0, |indirect_block: &mut IndirectBlock| {
                while current_blocks < total_blocks.min(INODE_INDIRECT1_COUNT as u32) {
                    indirect_block[current_blocks as usize] = new_blocks.next().unwrap();
                    current_blocks += 1;
                }
            });
        if total_blocks > INODE_INDIRECT1_COUNT as u32 {
            if current_blocks == INODE_INDIRECT1_COUNT as u32 {
                self.indirect2 = new_blocks.next().unwrap();
            }
            current_blocks -= INODE_INDIRECT1_COUNT as u32;
            total_blocks -= INODE_INDIRECT1_COUNT as u32;
        }
        // fill indirect2 from (a0, b0) -> (a1, b1)
        let mut a0 = current_blocks / INODE_INDIRECT1_COUNT as u32;
        let mut b0 = current_blocks % INODE_INDIRECT1_COUNT as u32;
        let a1 = total_blocks / INODE_INDIRECT1_COUNT as u32;
        let b1 = total_blocks % INODE_INDIRECT1_COUNT as u32;
        
        get_block_cache(
            self.indirect2 as usize, 
            Arc::clone(block_device),
        )
        .lock()
        .modify(0, |indirect2_block: &mut IndirectBlock| {
            while a0 < a1 || (a0 == a1 && b0 < b1) {
                if b0 == 0 {
                    indirect2_block[a0 as usize] = new_blocks.next().unwrap();
                }
                get_block_cache(
                    indirect2_block[a0 as usize] as usize,
                    Arc::clone(&block_device),
                )
                .lock()
                .modify(0, |indirect1_block: &mut IndirectBlock| {
                    indirect1_block[b0 as usize] = new_blocks.next().unwrap();
                });
                b0 += 1;
                if b0 == INODE_INDIRECT1_COUNT as u32 {
                    b0 = 0;
                    a0 += 1;
                }
            }
        })
    }
    /// Clear size to zero and return efs block-ids that should be deallocated.
    /// 
    /// We will clear the block contents to zero later
    pub fn clear_size(&mut self, block_device: &Arc<dyn BlockDevice>) -> Vec<u32> {
        let mut total_blocks = self.data_blocks() as usize;
        let mut v: Vec<u32> = Vec::new();
        // clear direct
        let mut cur_blocks: usize = 0;
        while cur_blocks < INODE_DIRECT_COUNT.min(total_blocks) {
            v.push(self.direct[cur_blocks]);
            cur_blocks += 1;
        }
        if total_blocks > INODE_DIRECT_COUNT {
            cur_blocks = 0;
            v.push(self.indirect1);
            total_blocks -= INODE_DIRECT_COUNT;
        } else {
            self.initialize(self.type_.clone());
            return v;
        }
        // clear indirect1
        get_block_cache(self.indirect1 as usize, Arc::clone(&block_device))
            .lock()
            .read(0, |indirect_block: &IndirectBlock| {
                let mut indir1_blocks = indirect_block.into_iter();
                while cur_blocks < total_blocks.min(INODE_INDIRECT1_COUNT) {
                    cur_blocks += 1;
                    v.push(*(indir1_blocks.next().unwrap()));
                }
            });
        if total_blocks > INODE_INDIRECT1_COUNT {
            cur_blocks = 0;
            v.push(self.indirect2);
            total_blocks -= INODE_INDIRECT1_COUNT;
        } else {
            self.initialize(self.type_.clone());
            return v;
        }
        get_block_cache(self.indirect2 as usize, Arc::clone(&block_device))
            .lock()
            .read(0, |indirect2_block: &IndirectBlock| {
                let mut a0 = 0;
                let mut b0 = 0;
                let a1 = total_blocks / INODE_INDIRECT1_COUNT;
                let b1 = total_blocks % INODE_INDIRECT1_COUNT;
                while a0 < a1 || (a0 == a1 && b0 < b1) {
                    if b0 == 0 {
                        v.push(indirect2_block[a0]);
                    }
                    get_block_cache(indirect2_block[a0] as usize, Arc::clone(block_device))
                        .lock()
                        .read(0, |indirect1_block: &IndirectBlock| {
                            v.push(indirect1_block[b0]);
                            cur_blocks += 1;
                        });
                    b0 += 1;
                    if b0 == INODE_INDIRECT1_COUNT {
                        b0 = 0;
                        a0 += 1;
                    }
                }
            });
        assert!(cur_blocks == total_blocks);
        self.initialize(self.type_.clone());
        v
    }
    pub fn read_at(
        &self,
        offset: usize,
        buf: &mut [u8],
        block_device: &Arc<dyn BlockDevice>,
    ) -> usize {
        let mut start = offset;
        let end = (offset + buf.len()).min(self.size as usize);
        if start >= end {
            return 0;
        }
        let mut start_block = start / BLOCK_SZ;  // 0-based
        let mut read_size = 0usize;
        loop {
            // calculate end of current block
            let mut end_current_block = (start / BLOCK_SZ + 1) * BLOCK_SZ;
            end_current_block = end_current_block.min(end);
            // read and update read size
            let block_read_size = end_current_block - start;
            let dst = &mut buf[read_size..read_size + block_read_size];
            get_block_cache(
                self.get_block_id(
                    start_block as u32,
                    block_device,
                ) as usize,
                Arc::clone(block_device),
            )
            .lock()
            .read(0, |data_block: &DataBlock| {
                let src = &data_block[start % BLOCK_SZ..start % BLOCK_SZ + block_read_size];
                dst.copy_from_slice(src);
            });
            read_size += block_read_size;
            // move to next block
            if end_current_block == end { break; }
            start_block += 1;
            start = end_current_block;
        }
        read_size
    }
    pub fn write_at(
        &mut self, 
        offset: usize, 
        buf: &[u8], 
        block_device: &Arc<dyn BlockDevice>
    ) -> usize {
        let mut start = offset;
        let end = (start + buf.len()).min(self.size as usize);
        if start >= end {
            return 0;
        }
        let mut write_size = 0;
        loop {
            let start_block = start / BLOCK_SZ;
            let end_current_block = ((start_block + 1) * BLOCK_SZ).min(end);
            // write and update write size
            let cur_block_id = self.get_block_id(start_block as u32, block_device) as usize;
            let block_write_sz = end_current_block - start;
            get_block_cache(cur_block_id, Arc::clone(&block_device))
                .lock()
                .modify(0, |data_block: &mut DataBlock| {
                    let src = &buf[write_size..write_size + block_write_sz];
                    (&mut data_block[start % BLOCK_SZ..start % BLOCK_SZ + block_write_sz]).copy_from_slice(src);
                });
            write_size += block_write_sz;
            start = end_current_block;
            if start == end {
                break;
            }
        }
        write_size
    }
}

#[repr(C)]
pub struct DirEntry {
    name: [u8; NAME_LENGTH_LIMIT + 1],
    inode_number: u32,
}

impl DirEntry {
    pub fn empty() -> Self {
        Self {
            name: [0u8; NAME_LENGTH_LIMIT + 1],
            inode_number: 0,
        }
    }
    pub fn new(name: &str, inode_number: u32) -> Self {
        let mut bytes = [0u8; NAME_LENGTH_LIMIT + 1];
        (&mut bytes[..name.len()]).copy_from_slice(name.as_bytes());
        Self {
            name: bytes,
            inode_number,
        }
    }
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(
                self as *const _ as usize as *const u8, 
                DIRENT_SZ,
            )
        }
    }
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                self as *mut _ as usize as *mut u8, 
                DIRENT_SZ,
            )
        }
    }
    pub fn name(&self) -> &str {
        let len = (0usize..).find(|i| self.name[*i] == 0).unwrap();
        core::str::from_utf8(&self.name[..len]).unwrap()
    }
    pub fn inode_number(&self) -> u32 {
        self.inode_number
    }
}
