//! 进程管理机制的实现
//! 
//! 这里是进程调度的入口，它被其他模块(比如syscall或clock interrupt)所需要。
//! 挂起或退出当前进程时，你可以修改进程状态、通过TASK_MANAGER管理进程队列以及
//! 通过PROCESSOR切换控制流。
//! 
//! 看到[`__switch`]时要小心。围绕此函数的控制流可能不是你所期望的。

mod context;
mod manager;
mod pid;
mod processor;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use crate::config::BIG_STRIDE;
use crate::mm::MapPermission;
use crate::{config::MAX_SYSCALL_NUM, mm::VirtAddr};
use crate::loader::get_app_data_by_name;
use alloc::sync::Arc;
use lazy_static::*;
use manager::fetch_task;
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};

pub use context::TaskContext;
pub use manager::add_task;
pub use pid::{pid_alloc, KernelStack, PidHandle};
pub use processor::{
    current_task, current_trap_cx, current_user_token, run_tasks, schedule, take_current_task,
};

/// 暂停当前的任务并切换到下一个任务
pub fn suspend_current_and_run_next() {
    // 必须已有应用程序在运行
    let task = take_current_task().unwrap();

    // ---- 独占访问当前任务控制块
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    // 将状态改变为Ready
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);
    // ---- 释放当前任务控制块访问

    // 将任务压入准备队列中
    add_task(task);
    // 跳转到调度周期
    schedule(task_cx_ptr);
}

/// 退出当前任务，回收进程资源并切换到下一个任务
pub fn exit_current_and_run_next(exit_code: i32) {
    // 从处理器中取出
    let task = take_current_task().unwrap();
    // ---- 独占访问当前任务控制块
    let mut task_inner = task.inner_exclusive_access();
    // 将状态改变为Zombie
    task_inner.task_status = TaskStatus::Zombie;
    // 记录返回码
    task_inner.exit_code = exit_code;
    // 不要移动到其父级，而是移到initproc下

    // ++++++ 独占访问initproc的任务控制块
    {
        let mut initproc_inner = INITPROC.inner_exclusive_access();
        for child in task_inner.children.iter() {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(child.clone());
        }
    }
    // ++++++ 释放initproc的任务管理块访问

    task_inner.children.clear();
    // 释放用户空间
    task_inner.memory_set.recycle_data_pages();
    drop(task_inner);
    // ---- 释放当前任务控制块访问
    // 手动释放任务来保证rc正确
    drop(task);
    // 我们无需去保存任务上下文
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}

/// 匿名映射申请内存
pub fn mmap(start: usize, len: usize, port: usize) -> isize {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    if !VirtAddr::from(start).aligned() {
        return -1;
    }
    if (port & !0x7 !=0) || (port & 0x7 == 0) {
        return -1;
    }
    let mut permission = MapPermission::U;
    if (port & 0x1) != 0 {
        permission |= MapPermission::R;
    }
    if (port & 0x2) != 0 {
        permission |= MapPermission::W;
    }
    if (port & 0x4) != 0 {
        permission |= MapPermission::X;
    }
    task_inner.memory_set.insert_framed_area(
        start.into(), (start + len).into(), permission
    )
}

/// 匿名映射释放内存
pub fn munmap(start: usize, len: usize) -> isize {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    if !VirtAddr::from(start).aligned() {
        return -1;
    }
    task_inner.memory_set.remove_frame_area(start.into(), (start + len).into())
}

/// 更新当前任务的系统调用次数
pub fn update_current_task_syscall_times(syscall_id: usize) {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let syscall_times = task_inner.syscall_times.get_mut(syscall_id);
    if let Some(times) = syscall_times {
        *times += 1;
    }
}

/// 获取当前任务的系统调用次数
pub fn get_current_task_syscall_times() -> [u32; MAX_SYSCALL_NUM] {
    let task = current_task().unwrap();
    let task_inner = task.inner_exclusive_access();
    let syscall_times = &task_inner.syscall_times;
    let mut times: [u32; MAX_SYSCALL_NUM] = [0; MAX_SYSCALL_NUM];
    for i in 0..MAX_SYSCALL_NUM {
        times[i] = *syscall_times.get(i).unwrap();
    }
    times
}

/// 获取当前任务的第一次运行的时间
pub fn get_current_task_first_time() -> usize {
    let task = current_task().unwrap();
    let task_inner = task.inner_exclusive_access();
    task_inner.task_first_time
}

/// 设置进程的优先级
pub fn set_priority(prio: usize) {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner.stride = BIG_STRIDE / prio;
}

lazy_static! {
    /// 初始化进程的创建
    /// 
    /// "initproc"这个名字可以改为像"usertests"一样的任意应用名，但我们有user_shell，
    /// 所以我们不需要改变它。
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new(TaskControlBlock::new(
        get_app_data_by_name("ch5b_initproc").unwrap()
    ));
}

pub fn add_initproc() {
    add_task(INITPROC.clone());
}
