//! 陷入(Trap)处理功能
//! 
//! 在rCore中，`__alltraps`是陷入的唯一入口点。在[`init()`]的初始化时，我们将`stvec` CSR指向它。
//! 
//! 所有陷入都要经过`__alltraps`，它被定义在`trap.S`中。汇编语言代码只需恢复内核空间上下文即可，
//! 保证Rust代码安全运行，并将控制权转移给[`trap_handler()`]。
//! 
//! 然后它根据异常类型调用不同的功能。例如，计时器中断触发任务抢占，系统调用转到[`syscall()`]。

mod context;

use crate::syscall::syscall;
use crate::task::{exit_current_and_run_next, suspend_current_and_run_next};
use crate::timer::set_next_trigger;
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    sie, stval, stvec,
};

core::arch::global_asm!(include_str!("trap.S"));

/// 初始化CSR的`stvec`为`__alltraps`的入口
pub fn init() {
    extern "C" {
        fn __alltraps();
    }
    unsafe {
        stvec::write(__alltraps as usize, TrapMode::Direct);
    }
}

/// 计时器中断使能
pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

#[no_mangle]
/// 处理一个中断、异常或来自用户空间的系统调用
pub fn trap_handler(cx: &mut TrapContext) -> &mut TrapContext {
    let scause = scause::read(); // 获取陷入的原因
    let stval = stval::read(); // 获取附加信息
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            cx.sepc += 4;
            cx.x[10] = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
        }
        Trap::Exception(Exception::StoreFault) | Trap::Exception(Exception::StorePageFault) => {
            error!("[kernel] PageFault in application, bad addr = {:#x}, bad instruction = {:#x}, core dumped.", stval, cx.sepc);
            exit_current_and_run_next();
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            error!("[kernel] IllegalInstruction in application, core dumped.");
            exit_current_and_run_next();
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            suspend_current_and_run_next();
        }
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
    cx
}

pub use context::TrapContext;
