//! 任务管理实现
//! 
//! 所有任务管理相关如开始和切换任务的实现都在这里。
//! 
//! 一个[`TaskManager`]的唯一全局实例命名为`TASK_MANAGER`，它控制操作系统中的所有任务。
//! 
//! 看到[`__switch`]时要小心。围绕此函数的控制流可能不是你所期望的。

mod context;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use crate::config::{MAX_APP_NUM, MAX_SYSCALL_NUM};
use crate::loader::{get_num_app, init_app_cx};
use crate::sync::UPSafeCell;
use crate::timer::get_time_us;
use alloc::vec::Vec;
use lazy_static::*;
pub use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};

pub use context::TaskContext;

/// 任务管理器，管理所有的任务。
/// 
/// `TaskManager`中的函数实现处理所有任务状态转换和任务上下文切换。为方便起见，
/// 你可以在它的模块层中找到这些封装。
/// 
/// 大多数`TaskManager`隐藏在字段`inner`里，以便将借用检查推迟到运行时。
/// 你可以看到如何在`TaskManager`上的现有函数中使用`inner`的例子。
pub struct TaskManager {
    /// 任务总数
    num_app: usize,
    /// 使用内部变量去获取可变访问
    inner: UPSafeCell<TaskManagerInner>,
}

/// 在'UPSafeCell'中的任务管理器内部
struct TaskManagerInner {
    /// 任务列表
    tasks: [TaskControlBlock; MAX_APP_NUM],
    /// 当前`Running`的任务的id
    current_task: usize,
    syscall_times: Vec<Vec<u32>>,
}

lazy_static! {
    /// 通过lazy_static!宏创建`TaskManager`实例，该宏可以使全局变量在运行时初始化而不是编译时
    pub static ref TASK_MANAGER: TaskManager = {
        let num_app = get_num_app();
        let mut tasks = [TaskControlBlock {
            task_cx: TaskContext::zero_init(),
            task_status: TaskStatus::UnInit,
            task_first_time: 0,
        }; MAX_APP_NUM];
        for (i, t) in tasks.iter_mut().enumerate().take(num_app) {
            t.task_cx = TaskContext::goto_restore(init_app_cx(i));
            t.task_status = TaskStatus::Ready;
        }
        let mut times_inner = Vec::<u32>::new();
        for _ in 0..MAX_SYSCALL_NUM {
            times_inner.push(0);
        }
        let mut syscall_times = Vec::new();
        for _ in 0..num_app {
            syscall_times.push(times_inner.clone());
        }
        TaskManager {
            num_app,
            inner: unsafe {
                UPSafeCell::new(TaskManagerInner {
                    tasks,
                    current_task: 0,
                    syscall_times,
                })
            },
        }
    };
}

impl TaskManager {
    /// 运行任务列表中第一个任务。
    /// 
    /// 通常，任务列表中第一个任务是一个空闲任务（我们后面会称其为零进程）。
    /// 但在章节3中，我们静态地加载应用程序，所以第一个任务是一个真正的应用程序。
    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let task0 = &mut inner.tasks[0];
        task0.task_status = TaskStatus::Running;
        task0.task_first_time = get_time_us() / 1000;
        let next_task_cx_ptr = &task0.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();
        // 在这之前，我们应该释放(drop)一个必须被手动释放的本地变量
        unsafe {
            __switch(&mut _unused as *mut TaskContext, next_task_cx_ptr);
        }
        panic!("unreachable in run_first_task!");
    }

    /// 将当前`Running`的任务转为`Ready`状态。
    fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Ready;
    }

    /// 将当前`Running`的任务转为`Exited`状态。
    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Exited;
    }

    /// 寻找下个可运行的任务并返回任务id。
    /// 
    /// 在这种情况下，我们只需返回任务列表中第一个`Ready`的任务。
    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
    }

    /// 将当前`Running`的任务切换到我们找到的任务，
    /// 如果没有`Ready`的任务那就以全部应用程序已运行完成的状态退出
    fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            inner.tasks[next].task_status = TaskStatus::Running;
            if let 0 = inner.tasks[next].task_first_time {
                inner.tasks[next].task_first_time = get_time_us() / 1000;
            }
            inner.current_task = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
            drop(inner);
            // 在这之前，我们应该释放(drop)一个必须被手动释放的本地变量
            unsafe {
                __switch(current_task_cx_ptr, next_task_cx_ptr);
            }
            // 返回用户模式
        } else {
            panic!("All applications completed!");
        }
    }

    /// 更新当前任务的系统调用次数
    fn update_current_task_syscall_times(&self, syscall_id: usize) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        let syscall_times = inner.syscall_times[current].get_mut(syscall_id).unwrap();
        *syscall_times += 1;
    }

    /// 获取当前任务的系统调用次数
    fn get_current_task_syscall_times(&self) -> [u32; MAX_SYSCALL_NUM] {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        let syscall_times = &inner.syscall_times[current];
        let mut times: [u32; MAX_SYSCALL_NUM] = [0; MAX_SYSCALL_NUM];
        for i in 0..MAX_SYSCALL_NUM {
            times[i] = *syscall_times.get(i).unwrap();
        }
        times
    }

    /// 获取当前任务的第一次运行的时间
    fn get_current_task_first_time(&self) -> usize {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_first_time
    }
}

/// 运行任务列表的第一个任务
pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}

/// 将当前`Running`的任务切换到我们找到的任务，
/// 如果没有`Ready`的任务那就以全部应用程序已运行完成的状态退出
fn run_next_task() {
    TASK_MANAGER.run_next_task();
}

/// 将当前`Running`的任务转为`Ready`状态。
fn mark_current_suspended() {
    TASK_MANAGER.mark_current_suspended();
}

/// 将当前`Running`的任务转为`Exited`状态。
fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}

/// 暂停当前`Running`的任务并运行任务列表中的下一个任务
pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    run_next_task();
}

/// 退出当前`Running`的任务并运行任务列表中的下一个任务
pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}

/// 更新当前任务的系统调用次数
pub fn update_current_task_syscall_times(syscall_id: usize) {
    TASK_MANAGER.update_current_task_syscall_times(syscall_id);
}

/// 获取当前任务的系统调用次数
pub fn get_current_task_syscall_times() -> [u32; MAX_SYSCALL_NUM] {
    TASK_MANAGER.get_current_task_syscall_times()
}

/// 获取当前任务的第一次运行时间
pub fn get_current_task_first_time() -> usize {
    TASK_MANAGER.get_current_task_first_time()
}
