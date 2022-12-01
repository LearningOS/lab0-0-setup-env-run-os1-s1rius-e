//! `__switch`的Rust封装
//! 
//! 在此处发生不同任务上下文的切换。实际的实现必不能是Rust，（本质上）必须是汇编语言
//! （因为涉及到寄存器和sp指针等底层操作），所以这个模块实际上是`switch.S`的封装。

core::arch::global_asm!(include_str!("switch.S"));

use super::TaskContext;

extern "C" {
    /// 切换到`next_task_cx_ptr`上下文，保存当前上下文到`current_task_cx_ptr`中。
    pub fn __switch(current_task_cx_ptr: *mut TaskContext, next_task_cx_ptr: *const TaskContext);
}
