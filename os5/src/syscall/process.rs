//! 过程管理系统调用

use crate::config::MAX_SYSCALL_NUM;
use crate::loader::get_app_data_by_name;
use crate::mm::{translated_refmut, translated_str};
use crate::task::{
    add_task, current_task, current_user_token, exit_current_and_run_next,
    suspend_current_and_run_next, TaskStatus,
    mmap, munmap, update_current_task_syscall_times, get_current_task_syscall_times,
    get_current_task_first_time, set_priority,
};
use crate::timer::get_time_us;
use alloc::sync::Arc;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

#[derive(Clone, Copy)]
pub struct TaskInfo {
    pub status: TaskStatus,
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    pub time: usize,
}

/// 更新系统调用次数
pub fn sys_update_syscall_times(syscall_id: usize) {
    update_current_task_syscall_times(syscall_id);
}

/// 任务退出并呈现退出代码
pub fn sys_exit(exit_code: i32) -> ! {
    info!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// 当前任务为其他任务放弃资源
pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    current_task().unwrap().pid.0 as isize
}

/// 系统调用Fork，它对子进程返回0，对父进程返回子进程的pid
pub fn sys_fork() -> isize {
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // 修改新进程的trap上下文，因为在切换后它会立即返回
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // 我们不必移动到下一条指令，因为之前以及为子进程执行过，fork返回0
    trap_cx.x[10] = 0;
    // 将新进程添加到调度器中
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let task = current_task().unwrap();
        task.exec(data);
        0
    } else {
        -1
    }
}

/// 如果不存在与输入pid相同的子进程，则返回-1。
/// 如果存在pid相同但仍在运行的子进程，则返回-2。
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let task = current_task().unwrap();
    // 找到子进程

    // ---- 独占访问当前任务控制块
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ----释放当前任务控制块访问
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ 临时独占访问子进程的任务控制块
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ 释放子进程的任务控制块访问
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // 确认从子进程列表中移除后子进程会被释放
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ 临时独占访问子进程的任务控制块
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ 释放子进程的任务控制块访问
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- 自动释放当前任务控制块访问
}

/// 获取带秒数和毫秒数的时间
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    let us = get_time_us();
    let time = translated_refmut(
        current_user_token(), ts);
    *time = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    0
}

/// 设置任务的优先级
pub fn sys_set_priority(prio: isize) -> isize {
    if prio <= 1 {
        -1
    } else {
        set_priority(prio as usize);
        prio
    }
}

/// 申请内存
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    mmap(start, len, port)
}

/// 释放内存
pub fn sys_munmap(start: usize, len: usize) -> isize {
    let ret = munmap(start, len);
    ret
}

/// 获取任务信息
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    let curr_time = get_time_us() / 1000;
    let first_time = get_current_task_first_time();
    let task_info = translated_refmut(
        current_user_token(), ti);
    *task_info = TaskInfo {
        status: TaskStatus::Running,
        syscall_times: get_current_task_syscall_times(),
        time: curr_time - first_time,
    };
    0
}

/// 创建子进程并执行
pub fn sys_spawn(path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let current_task = current_task().unwrap();
        let new_task = current_task.spawn(data);
        let new_pid = new_task.pid.0;
        add_task(new_task);
        suspend_current_and_run_next();
        new_pid as isize
    } else {
        -1
    }
}
