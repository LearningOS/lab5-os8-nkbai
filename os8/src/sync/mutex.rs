use alloc::{collections::VecDeque, sync::Arc};

use crate::task::{add_task, current_task};
use crate::task::{block_current_and_run_next, suspend_current_and_run_next};
use crate::task::TaskControlBlock;

use super::UPSafeCell;

pub trait Mutex: Sync + Send {
    fn lock(&self);
    fn unlock(&self);
    fn wait_queue(&self) -> (bool, Option<VecDeque<Arc<TaskControlBlock>>>);
    fn can_lock(&self) -> bool;
}

pub struct MutexSpin {
    locked: UPSafeCell<bool>,
}

impl MutexSpin {
    pub fn new() -> Self {
        Self {
            locked: unsafe { UPSafeCell::new(false) },
        }
    }
}

impl Mutex for MutexSpin {
    fn lock(&self) {
        loop {
            let mut locked = self.locked.exclusive_access();
            if *locked {
                drop(locked);
                suspend_current_and_run_next();
                continue;
            } else {
                *locked = true;
                return;
            }
        }
    }

    fn unlock(&self) {
        let mut locked = self.locked.exclusive_access();
        *locked = false;
    }

    fn wait_queue(&self) -> (bool, Option<VecDeque<Arc<TaskControlBlock>>>) {
        (true, None)
    }

    fn can_lock(&self) -> bool {
        true
    }
}

pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
}

pub struct MutexBlockingInner {
    locked: bool,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl MutexBlocking {
    pub fn new() -> Self {
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }
}

impl Mutex for MutexBlocking {
    fn lock(&self) {
        let mut mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            mutex_inner.wait_queue.push_back(current_task().unwrap());
            drop(mutex_inner);
            block_current_and_run_next();
        } else {
            mutex_inner.locked = true;
        }
    }

    fn unlock(&self) {
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            add_task(waking_task);
        } else {
            mutex_inner.locked = false;
        }
    }

    fn wait_queue(&self) -> (bool, Option<VecDeque<Arc<TaskControlBlock>>>) {
        let inner = self.inner.exclusive_access();
        if inner.wait_queue.is_empty() {
            (!inner.locked, None)
        } else {
            (!inner.locked, Some(inner.wait_queue.clone()))
        }
    }

    fn can_lock(&self) -> bool {
        let inner = self.inner.exclusive_access();
        !inner.locked
    }
}
