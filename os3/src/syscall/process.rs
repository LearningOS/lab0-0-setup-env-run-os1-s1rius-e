//! 过程管理系统调用

use crate::config::MAX_SYSCALL_NUM;
use crate::task::{exit_current_and_run_next, suspend_current_and_run_next, TaskStatus,
    update_current_task_syscall_times, get_current_task_syscall_times, get_current_task_first_time};
use crate::timer::get_time_us;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

pub struct TaskInfo {
    status: TaskStatus,
    syscall_times: [u32; MAX_SYSCALL_NUM],
    time: usize,
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
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        }
    }
    0
}

/// 获取任务信息
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    let syscall_times = get_current_task_syscall_times();
    let curr_time = get_time_us() / 1000;
    let first_time = get_current_task_first_time();
    unsafe {
        *ti = TaskInfo {
            status: TaskStatus::Running,
            syscall_times: syscall_times,
            time: curr_time - first_time,
        }
    }
    0
}

/// 更新系统调用次数
pub fn sys_update_syscall_times(syscall_id: usize) {
    update_current_task_syscall_times(syscall_id);
}
