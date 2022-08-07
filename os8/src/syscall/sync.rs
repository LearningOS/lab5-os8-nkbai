use alloc::sync::Arc;

use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task, current_thread_id};
use crate::task::dead_lock_dector::{mutex_dead_lock_test, semaphore_dead_lock_test};
use crate::timer::{add_timer, get_time_ms};

pub fn sys_sleep(ms: usize) -> isize {
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}

// LAB5 HINT: you might need to maintain data structures used for deadlock detection
// during sys_mutex_* and sys_semaphore_* syscalls
pub fn sys_mutex_create(blocking: bool) -> isize {
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_list.len() as isize - 1
    }
}

// LAB5 HINT: Return -0xDEAD if deadlock is detected
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    if process_inner.enable_deadlock_detect && mutex_dead_lock_test(&process_inner, mutex.as_ref(), mutex_id) {
        return -0xdead;
    }
    if mutex.can_lock() {
        current_task().unwrap().inner_exclusive_access().res.as_mut().unwrap().hold_mutex(mutex_id, &process_inner);
    }
    drop(process_inner);
    drop(process);
    mutex.lock();
    0
}

pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    //release的时候检测,谁持有,谁释放.
    for task in process_inner.tasks.iter() {
        if task.is_none() {
            continue;
        }
        let task = task.as_ref().unwrap();
        // println!("task try inner_exclusive_access");
        let mut task_inner = task.inner_exclusive_access();
        // println!("111");
        let res = task_inner.res.as_mut();
        if res.is_none() {
            continue;
        }
        res.unwrap().release_mutex(mutex_id);
        // println!("222");
    }
    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}

pub fn sys_semaphore_create(res_count: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_list.len() - 1
    };
    // println!("semaphore create {} count={}", id, res_count);
    id as isize
}

pub fn sys_semaphore_up(sem_id: usize) -> isize {
    // println!("thread {} sem up {}", current_thread_id(), sem_id);
    let process = current_process();
    let process_inner = process.inner_exclusive_access();

    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    for task in process_inner.tasks.iter() {
        if task.is_none() {
            continue;
        }
        let task = task.as_ref().unwrap();
        // println!("task try inner_exclusive_access");
        let mut task_inner = task.inner_exclusive_access();
        // println!("111");
        let res = task_inner.res.as_mut();
        if res.is_none() {
            continue;
        }
        if res.unwrap().release_semaphore(sem_id) {
            break;
        }
        // println!("222");
    }
    drop(process_inner);
    sem.up();
    0
}

// LAB5 HINT: Return -0xDEAD if deadlock is detected
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    // println!("thread {} sem down {}", current_thread_id(), sem_id);
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    // println!("before deadlcok");
    if process_inner.enable_deadlock_detect && semaphore_dead_lock_test(&process_inner, &sem, sem_id) {
        // println!("deadlock");
        return -0xdead;
    }
    // println!("after deadlock");
    drop(process_inner);
    drop(process);
    if sem.can_down() {
        current_task().unwrap().inner_exclusive_access().res.as_mut().unwrap().hold_semaphore(sem_id);
    }
    sem.down();
    0
}

pub fn sys_condvar_create(_arg: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}

pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}

pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}

// LAB5 YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(_enabled: usize) -> isize {
    current_process().inner_exclusive_access().enable_deadlock_detect = true;
    0
}
