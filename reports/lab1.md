## 简单总结你实现的功能

在原有框架基础上，增加了任务初次运行的时间以及在发生系统调用时的次数记录，完成了任务信息的更新和查询。

## 问答题

1. 运行三个bad测例(ch2b_bad_*.rs)并描述程序出错行为。
sbi: RustSBI version 0.3.0-alpha.4, adapting to RISC-V SBI v1.0.0
+ `ch2b_bad_address.rs`: 对错误地址进行写操作，触发存储页异常。
+ `ch2b_bad_instructions.rs`: 在U-Mode调用S-Mode的指令，触发非法指令异常。
+ `ch2b_bad_register.rs`: 在U-Mode调用S-Mode的寄存器，触发非法指令异常。

2. 深入理解trap.S中__alltraps和__restore的作用。
+ 刚进入__restore时，a0代表函数的第一个输入参数，指向内核栈上的Trap上下文。两种使用场景：a）启动第一个任务；b）Trap后返回用户态。
+ 这几行代码从内核栈中恢复sstatus、sepc和sscratch这三个寄存器。sstatus可以从多方面控制S特权级的CPU行为和执行状态，sepc记录了Trap发生之前执行的最后一条指令地址，恢复这两个寄存器即可回到Trap前的特权级和指令地址，而sscratch比较特殊，它在Trap发生时暂存用户态的地址，当需要切换特权级时，只要交换sp和sscratch两个寄存器内容就完成了内核栈和用户栈的切换。
+ 因为x2寄存器是栈指针(sp)寄存器，其值记录在sscratch中；x4寄存器是线程指针(tp)寄存器，目前应用程序单线程没有使用。
+ L63执行的`csrrw sp, sscratch, sp`交换了sp和sscratch寄存器内容，交换后sp指向用户栈，sscratch指向内核栈。
+ __restore中发生状态切换在`csrrw sp, sscratch, sp`这条指令，因为这条指令后sp指向用户栈，完成了内核态到用户态的转换。
+ L13执行`csrrw sp, sscratch, sp`后，sp指向内核栈，sscratch指向用户栈。
+ 从U态进入S态是在`csrrw sp, sscratch, sp`这条指令发生的。
