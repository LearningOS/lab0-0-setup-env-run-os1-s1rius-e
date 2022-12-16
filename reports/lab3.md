## 简单总结你实现的功能

在原有框架基础上，将任务的管理模式修改为根据程序名加载、根据pid管理进程，并增加了fork、exec、spawn、waitpid等系统调用，其中进程的调度使用了stride调度算法。

## 问答题

1. 两个stride=10的进程，使用8bit无符号整型存储pass，p1.pass=255，p2.pass=250，在p2执行一个时间片后，理论上下一次应该p1执行，实际情况是轮到p1执行吗？为什么？
实际还是p2执行，因为pass使用8bit无符号整型存储，p2的pass加上stride后会溢出，导致下一次判断时p2的pass小于p1。
2. 在不考虑溢出的情况下，在进程优先级全部>=2的情况下，如果严格按照算法执行，那么PASS_MAX - PASS_MIN <= BigStride/2。为什么？尝试简单说明。
假设存在一个优先级为2的进程A，由P.stride = BigStride / P.priority可知其stride为BigStride/2，此时系统还存在一个优先级大于2的进程B。若进程A先被调度，则此时A.pass = BigStride/2 (PASS_MAX)，B.pass = 0 (PASS_MIN)，PASS_MAX - PASS_MIN = BigStride/2。下一次调度时，由于B.pass < A.pass，进程B会被调度。此后进程B会被不断调度，一直到B.pass >= A.pass，整个过程中PASS_MAX - PASS_MIN < BigStride/2。由此可得不溢出的情况下，PASS_MAX - PASS_MIN <= BigStride/2成立。
3. 已知以上结论，考虑溢出的情况下，可以为pass设计特别的比较器，让BinaryHeap<Pass>的pop方法能返回真正最小的Pass。补全下列代码中的partial_cmp函数，假设两个Pass永远不会相等。
```Rust
use core::cmp::Ordering;

struct Pass(u64);

impl PartialOrd for Pass {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let pass_delta = (other.0 - self.0) as i128;
        if (pass_delta > 0) && (pass_delta <= (BIG_STRIDE / 2) as i128) {
            Some(Ordering::Less)
        } else {
            Some(Ordering::Greater)
        }
    }
}

impl PatrialEq for Pass {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}
```
