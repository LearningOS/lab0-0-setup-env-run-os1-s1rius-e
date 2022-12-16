//! 内存管理的实现
//! 
//! 这里实现了用于RV64系统的SV39分页虚拟内存框架，以及跟内存管理相关的所有内容，如物理页帧管理器、
//! 页表、逻辑段和地址空间。
//! 
//! 所有任务和进程都有一个内存集去控制它的虚拟内存。


mod address;
mod frame_allocator;
mod heap_allocator;
mod memory_set;
mod page_table;

pub use address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
use address::{StepByOne, VPNRange};
pub use frame_allocator::{frame_alloc, FrameTracker};
pub use memory_set::remap_test;
pub use memory_set::{MapPermission, MemorySet, KERNEL_SPACE};
pub use page_table::{translated_byte_buffer, translated_refmut, translated_str, PageTableEntry};
use page_table::{PTEFlags, PageTable};

/// 初始化堆内存分配器、物理页帧管理器和内核空间
pub fn init() {
    heap_allocator::init_heap();
    frame_allocator::init_frame_allocator();
    KERNEL_SPACE.lock().activate();
}
