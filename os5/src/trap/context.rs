//! [`TrapContext`]的实现

use riscv::register::sstatus::{self, Sstatus, SPP};

#[repr(C)]
/// 陷入上下文结构体，包含sstatus、sepc和registers
pub struct TrapContext {
    /// 通用寄存器x0-31
    pub x: [usize; 32],
    /// ssstatus
    pub sstatus: Sstatus,
    /// sepc
    pub sepc: usize,
    /// 内核地址空间的信令
    pub kernel_satp: usize,
    /// 当前应用程序的内核栈指针
    pub kernel_sp: usize,
    /// trap handler在内核中的虚拟地址
    pub trap_handler: usize,
}

impl TrapContext {
    pub fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }
    pub fn app_init_context(
        entry: usize,
        sp: usize,
        kernel_satp: usize,
        kernel_sp: usize,
        trap_handler: usize,
    ) -> Self {
        let mut sstatus = sstatus::read();
        // 陷入回来后设置CPU特权级为用户级
        sstatus.set_spp(SPP::User);
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,
            kernel_satp,
            kernel_sp,
            trap_handler,
        };
        cx.set_sp(sp);
        cx
    }
}
