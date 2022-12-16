//! [`TaskManager`]的实现
//! 
//! 它只用于管理进程和根据准备队列调度进程。
//! 其他关于CPU的进程监控职能都在Processor中。

use super::TaskControlBlock;
use crate::config::BIG_STRIDE;
use crate::sync::UPSafeCell;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::*;

pub struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

/// 一个stride调度器。
impl TaskManager {
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
        }
    }
    /// 添加进程到准备队列中
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }
    /// 将一个进程从准备队列中取出
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        let mut min_idx = 0;
        let mut min_pass:u64 = 0;
        for (idx, tcb) in self.ready_queue.iter().enumerate() {
            let inner = tcb.inner_exclusive_access();
            if idx == 0 {
                min_idx = idx;
                min_pass = inner.pass;
            } else {
                let pre_inner = self.ready_queue[idx - 1].inner_exclusive_access();
                let pass_delta = (pre_inner.pass - inner.pass) as i128;
                if (pass_delta > 0) && (pass_delta <= (BIG_STRIDE / 2) as i128) && (inner.pass < min_pass) {
                    min_idx = idx;
                    min_pass = inner.pass;
                }
            }
        }
        self.ready_queue.remove(min_idx)
    }
}

lazy_static! {
    /// 通过lazy_static!创建TASK_MANAGER实例
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

pub fn add_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.exclusive_access().add(task);
}

pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.exclusive_access().fetch()
}
