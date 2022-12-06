//! 任务管理相关的类型

use super::TaskContext;
use super::{pid_alloc, KernelStack, PidHandle};
use crate::config::{TRAP_CONTEXT, MAX_SYSCALL_NUM, BIG_STRIDE};
use crate::mm::{MemorySet, PhysPageNum, VirtAddr, KERNEL_SPACE};
use crate::sync::UPSafeCell;
use crate::trap::{trap_handler, TrapContext};
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::cell::RefMut;

/// 任务控制块结构体
/// 
/// 直接存储在运行时不会变化的元数据
pub struct TaskControlBlock {
    // 不变的
    /// 进程标识符
    pub pid: PidHandle,
    /// PID对应的内核栈
    pub kernel_stack: KernelStack,
    // 可变的
    inner: UPSafeCell<TaskControlBlockInner>,
}

/// 包含更多进程元数据的结构体
/// 
/// 存储会在运行时变化的元数据，并通过UPSafeCell包装来提供可变引用
pub struct TaskControlBlockInner {
    /// trap上下文所在的物理页帧的物理页号
    pub trap_cx_ppn: PhysPageNum,
    /// 应用数据仅有可能出现在应用地址空间低于base_size字节的区域中
    pub base_size: usize,
    /// 将暂停的任务的任务上下文保存在此
    pub task_cx: TaskContext,
    /// 维护当前进程的执行状态
    pub task_status: TaskStatus,
    /// 应用地址空间
    pub memory_set: MemorySet,
    /// 当前进程的父进程。Weak智能指针不会影响父进程的引用计数
    pub parent: Option<Weak<TaskControlBlock>>,
    /// 当前进程包含所有子进程的任务控制块的向量
    pub children: Vec<Arc<TaskControlBlock>>,
    /// 当主动退出或执行出错由内核终止时被赋值
    pub exit_code: i32,
    /// 任务第一次运行的时间
    pub task_first_time: usize,
    /// 系统调用次数
    pub syscall_times: Vec<u32>,
    /// 步长
    pub stride: usize,
    /// 行程
    pub pass: u64,
}

impl TaskControlBlockInner {
    pub fn init_syscall_times(& mut self) {
        for _ in 0..MAX_SYSCALL_NUM {
            self.syscall_times.push(0);
        }
    }
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    fn get_status(&self) -> TaskStatus {
        self.task_status
    }
    pub fn is_zombie(&self) -> bool {
        self.get_status() == TaskStatus::Zombie
    }
}

impl TaskControlBlock {
    /// 获取TaskControlBlockInner的可变引用
    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }

    /// 创建一个新进程
    /// 
    /// 现在，它只用于初始进程initproc的创建
    pub fn new(elf_data: &[u8]) -> Self {
        // 地址空间包括elf应用头、跳板、陷入上下文、用户栈
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        // 分配一个pid和一个在内核空间的内核栈
        let pid_handle = pid_alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();
        // 压入任务上下文，使第一次任务切换到它的时候可以跳转到trap_return并进入用户态开始执行
        let task_control_block = Self {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: user_sp,
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready,
                    memory_set,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                    task_first_time: 0,
                    syscall_times: Vec::new(),
                    stride: BIG_STRIDE / 16,
                    pass: 0,
                })
            }
        };
        // 初始化系统调用次数
        task_control_block.inner_exclusive_access().init_syscall_times();
        // 准备在用户空间的陷入上下文
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.lock().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
    }
    /// 加载一个新的elf文件，替换原有的应用地址空间中的内容并开始执行
    pub fn exec(&self, elf_data: &[u8]) {
        // 地址空间包括elf应用头、跳板、陷入上下文、用户栈
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();

        // ---- 独占访问内部数据
        let mut inner = self.inner_exclusive_access();
        // 替换地址空间
        inner.memory_set = memory_set;
        // 更新陷入上下文的物理页号
        inner.trap_cx_ppn = trap_cx_ppn;
        // 初始化陷入上下文
        let trap_cx = inner.get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.lock().token(),
            self.kernel_stack.get_top(),
            trap_handler as usize,
        );
        // ---- 自动释放内部数据的访问
    }
    /// 从父进程派生子进程
    pub fn fork(self: &Arc<TaskControlBlock>) -> Arc<TaskControlBlock> {
        // ---- 独占访问父进程的任务控制块
        let mut parent_inner = self.inner_exclusive_access();
        // 拷贝用户空间(包括陷入上下文)
        let memory_set = MemorySet::from_existed_user(&parent_inner.memory_set);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        // 分配一个pid和一个在内核空间的内核栈
        let pid_handle = pid_alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();
        let task_control_block = Arc::new(TaskControlBlock {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: parent_inner.base_size,
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready,
                    memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                    task_first_time: 0,
                    syscall_times: Vec::new(),
                    stride: BIG_STRIDE / 16,
                    pass: 0,
                })
            },
        });
        // 初始化系统调用次数
        task_control_block.inner_exclusive_access().init_syscall_times();
        // 添加子进程
        parent_inner.children.push(task_control_block.clone());
        // 修改trap_cx的kernel_sp
        // ++++ 独占访问子进程的任务管理块
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        trap_cx.kernel_sp = kernel_stack_top;
        // 返回
        task_control_block
        // ---- 自动释放父进程的任务管理块访问
        // ++++ 自动释放子进程的任务管理块访问
    }
    /// 创建并执行一个新的子进程
    pub fn spawn(self: &Arc<TaskControlBlock>, elf_data: &[u8]) -> Arc<TaskControlBlock> {
        // 地址空间包括elf应用头、跳板、陷入上下文、用户栈
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        // 分配一个pid和一个在内核空间的内核栈
        let pid_handle = pid_alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();
        let task_control_block = Arc::new(TaskControlBlock {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: user_sp,
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready,
                    memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                    task_first_time: 0,
                    syscall_times: Vec::new(),
                    stride: BIG_STRIDE / 16,
                    pass: 0,
                })
            },
        });
        // 初始化系统调用次数
        task_control_block.inner_exclusive_access().init_syscall_times();
        // ---- 独占访问父进程的任务控制块
        let mut parent_inner = self.inner_exclusive_access();
        // 添加子进程
        parent_inner.children.push(task_control_block.clone());
        drop(parent_inner);
        // ---- 手动释放父进程的任务控制块
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.lock().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
    }
    pub fn getpid(&self) -> usize {
        self.pid.0
    }
}

#[allow(unused)]
#[derive(Copy, Clone, PartialEq)]
/// 任务状态：未初始化，准备运行，正在运行，僵尸
pub enum TaskStatus {
    UnInit,
    Ready,
    Running,
    Zombie,
}
