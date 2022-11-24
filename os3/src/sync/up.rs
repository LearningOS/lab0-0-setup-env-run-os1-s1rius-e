//! 单核处理器单线程内部可变封装

use core::cell::{RefCell, RefMut};

/// 为了使用全局变量时避免使用unsafe，这里将一个静态数据结构封装起来。
/// 
/// 仅单线程时安全。
/// 
/// 要得到内部数据的可变引用，应调用`exclusive_access`。
pub struct UPSafeCell<T> {
    /// 内部数据
    inner: RefCell<T>,
}

/// 实现Sync trait使其被标记为线程安全，实际并不能保证。
unsafe impl<T> Sync for UPSafeCell<T> {}

impl<T> UPSafeCell<T> {
    /// 调用者有责任保证内部结构体只用于单线程。
    pub unsafe fn new(value: T) -> Self {
        Self {
            inner: RefCell::new(value),
        }
    }
    /// 重复借用会导致Panic，使用后应及时`drop`。
    pub fn exclusive_access(&self) -> RefMut<'_, T> {
        self.inner.borrow_mut()
    }
}