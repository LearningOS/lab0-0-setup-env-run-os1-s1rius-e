//! 任务管理相关的类型

use super::TaskContext;

#[derive(Copy, Clone)]
/// 任务控制块结构体
pub struct TaskControlBlock {
    pub task_status: TaskStatus,
    pub task_cx: TaskContext,
    pub task_first_time: usize,
}

#[derive(Copy, Clone, PartialEq)]
/// 任务状态：未初始化，准备运行，正在运行，已退出
pub enum TaskStatus {
    UnInit,
    Ready,
    Running,
    Exited,
}
