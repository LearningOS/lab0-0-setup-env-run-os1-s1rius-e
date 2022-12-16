//! [`Processor`]的实现和控制流的交汇
//! 
//! 在这里，用户应用程序在CPU中的连续操作被维护，CPU的当前运行状态被记录，
//! 并且不同应用程序的控制流的交替和转移被执行。

use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::sync::UPSafeCell;
use crate::timer::get_time_us;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use lazy_static::*;

/// 处理器管理结构体
pub struct Processor {
    /// 在当前处理器上正在执行的任务
    current: Option<Arc<TaskControlBlock>>,
    /// 每个核的基础控制流，它助于选择和切换进程
    idle_task_cx: TaskContext,
}

impl Processor {
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(|task| Arc::clone(task))
    }
}

lazy_static! {
    /// 通过lazy_static!创建PROCESSOR的实例
    pub static ref PROCESSOR: UPSafeCell<Processor> =
        unsafe { UPSafeCell::new(Processor::new()) };
}

/// 处理器执行和调度的主要部分
/// 
/// 循环fetch_task来获取需要运行的进程，并通过__switch切换进程
pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // 独占访问即将运行的任务的任务控制块
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            if task_inner.task_first_time == 0 {
                task_inner.task_first_time = get_time_us();
            }
            task_inner.pass += task_inner.stride as u64;
            drop(task_inner);
            // 手动释放即将运行的任务的任务控制块访问
            processor.current = Some(task);
            // 手动释放处理器访问
            drop(processor);
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        }
    }
}

/// 通过task获取当前的任务，并在其原来的位置留下一个None
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

/// 获取当前任务的一份拷贝
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

/// 获取当前任务的地址空间的信令
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    let token = task.inner_exclusive_access().get_user_token();
    token
}

/// 获取当前任务trap上下文的可变引用
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

/// 返回空闲控制流以进行新的调度
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}
