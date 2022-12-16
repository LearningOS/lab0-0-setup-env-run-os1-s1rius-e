//! 任务管理相关的类型

use super::TaskContext;
use crate::config::{kernel_stack_position, TRAP_CONTEXT, MAX_SYSCALL_NUM};
use crate::mm::{MapPermission, MemorySet, PhysPageNum, VirtAddr, KERNEL_SPACE};
use crate::trap::{trap_handler, TrapContext};
use alloc::vec::Vec;

/// 任务控制块结构体
pub struct TaskControlBlock {
    pub task_status: TaskStatus,
    pub task_cx: TaskContext,
    pub memory_set: MemorySet,
    pub trap_cx_ppn: PhysPageNum,
    pub base_size: usize,
    pub task_first_time: usize,
    pub syscall_times: Vec<u32>,
}

impl TaskControlBlock {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    pub fn new(elf_data: &[u8], app_id: usize) -> Self {
        // 地址空间包括elf应用头、跳板、陷入上下文、用户栈
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        let task_status = TaskStatus::Ready;
        // 在内核空间映射一个内核栈
        let (kernel_stack_bottom, kernel_stack_top) = kernel_stack_position(app_id);
        KERNEL_SPACE.lock().insert_framed_area(
            kernel_stack_bottom.into(),
            kernel_stack_top.into(),
            MapPermission::R | MapPermission::W,
        );
        let mut task_control_block = Self {
            task_status,
            task_cx: TaskContext::goto_trap_return(kernel_stack_top),
            memory_set,
            trap_cx_ppn,
            base_size: user_sp,
            task_first_time: 0,
            syscall_times: Vec::new(),
        };
        // 准备在用户空间的陷入上下文
        let trap_cx = task_control_block.get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.lock().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        // 初始化系统调用次数
        for _ in 0..MAX_SYSCALL_NUM {
            task_control_block.syscall_times.push(0);
        }
        task_control_block
    }
}

#[allow(unused)]
#[derive(Copy, Clone, PartialEq)]
/// 任务状态：未初始化，准备运行，正在运行，已退出
pub enum TaskStatus {
    UnInit,
    Ready,
    Running,
    Exited,
}
