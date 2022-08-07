use alloc::{collections::VecDeque, sync::Arc};

use crate::sync::UPSafeCell;
use crate::task::{add_task, block_current_and_run_next, current_task, TaskControlBlock};

pub struct Semaphore {
    pub inner: UPSafeCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Semaphore {
    pub fn new(res_count: usize) -> Self {
        Self {
            inner: unsafe {
                UPSafeCell::new(SemaphoreInner {
                    count: res_count as isize,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }

    pub fn up(&self) {
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                add_task(task);
            }
        }
    }

    pub fn down(&self) {
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        if inner.count < 0 {
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
        }
    }
    pub fn wait_queue(&self) -> (usize, VecDeque<Arc<TaskControlBlock>>) {
        let inner = self.inner.exclusive_access();
        let available = if inner.count >= 0 {
            inner.count as usize
        } else {
            0
        };
        (available, inner.wait_queue.clone())
    }
    pub fn can_down(&self) -> bool {
        let inner = self.inner.exclusive_access();
        inner.count >= 1
    }
    pub fn countxxx(&self) -> isize {
        let inner = self.inner.exclusive_access();
        inner.count
    }
}
