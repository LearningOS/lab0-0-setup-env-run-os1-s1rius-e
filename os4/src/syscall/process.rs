//! 过程管理系统调用

use crate::config::MAX_SYSCALL_NUM;
use crate::mm::translated_type_buffer;
use crate::task::{
    current_user_token, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus,
    mmap, mnumap, update_current_task_syscall_times, get_current_task_syscall_times,
    get_current_task_first_time,
};
use crate::timer::get_time_us;

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

/// 任务退出并呈现退出代码
pub fn sys_exit(exit_code: i32) -> ! {
    info!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// 当前任务为其他任务放弃资源
pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

/// 获取带秒数和毫秒数的时间
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    let us = get_time_us();
    let time = translated_type_buffer(
        current_user_token(), ts);
    time.sec = us / 1_000_000;
    time.usec = us % 1_000_000;
    0
}

// CLUE: 从 ch4 开始不再对调度算法进行测试~
pub fn sys_set_priority(_prio: isize) -> isize {
    -1
}

/// 申请内存
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    mmap(start, len, port)
}

/// 释放内存
pub fn sys_munmap(start: usize, len: usize) -> isize {
    let ret = mnumap(start, len);
    ret
}

/// 获取任务信息
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    // let syscall_times = get_current_task_syscall_times();
    let curr_time = get_time_us() / 1000;
    let first_time = get_current_task_first_time();
    let task_info = translated_type_buffer(
        current_user_token(), ti);
    task_info.status = TaskStatus::Running;
    task_info.syscall_times = get_current_task_syscall_times();
    task_info.time = curr_time - first_time;
    0
}

/// 更新系统调用次数
pub fn sys_update_syscall_times(syscall_id: usize) {
    update_current_task_syscall_times(syscall_id);
}
