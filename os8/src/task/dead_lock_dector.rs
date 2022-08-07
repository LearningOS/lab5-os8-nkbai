use alloc::vec;
use alloc::vec::Vec;

use crate::sync::{Mutex, Semaphore};
use crate::task::current_task;
use crate::task::process::ProcessControlBlockInner;

#[derive(Debug, Clone)]
struct DeadLockDetector {
    /**
    可利用资源向量 Available ：含有 m 个元素的一维数组，每个元素代表可利用的某一类资源的数目，
    其初值是该类资源的全部可用数目，其值随该类资源的分配和回收而动态地改变。
    Available[j] = k，表示第 j 类资源的可用数量为 k。
     */
    pub available: Vec<i32>,
    /**
    分配矩阵 Allocation：n * m 矩阵，表示每类资源已分配给每个线程的资源数。 Allocation[i,j] = g，则表示线程 i 当前己分得第 j 类资源的数量为 g。
     */
    pub allocation: Vec<Vec<i32>>,
    /**
    需求矩阵 Need：n * m 的矩阵，表示每个线程还需要的各类资源数量。 Need[i,j] = d，则表示线程 i 还需要第 j 类资源的数量为 d 。
     */
    pub need: Vec<Vec<i32>>,
    thread_count: usize,
}

impl DeadLockDetector {
    fn new(thread_count: usize, resource_count: usize) -> Self {
        // println!("thread_count={},resource_count={}", thread_count, resource_count);
        DeadLockDetector {
            available: vec![0; resource_count],
            allocation: vec![vec![0; resource_count]; thread_count],
            need: vec![vec![0; resource_count]; thread_count],
            thread_count,
        }
    }
    //进程p中的线程t,需要获取锁mutext_id
    pub fn build_mutex_detector(p: &ProcessControlBlockInner, tid: usize, mutex_id: usize) -> Self {
        let mut detector = DeadLockDetector::new(p.tasks.len(), p.mutex_list.len());
        detector.need[tid][mutex_id] = 1;
        //还有那些可以分配的
        for (id, mutex) in p.mutex_list.iter().enumerate() {
            if mutex.is_none() {
                detector.available[id] = 0;
                continue;
            }
            let mutex = mutex.as_ref().unwrap();
            let can_lock = mutex.can_lock();
            if can_lock {
                detector.available[id] = 1;
            } else {
                detector.available[id] = 0;
                let (_, wait_queue) = mutex.wait_queue();
                if wait_queue.is_none() {
                    continue;
                }
                for task in wait_queue.unwrap().iter() {
                    let task_inner = task.inner_exclusive_access();
                    let res = task_inner.res.as_ref();
                    if res.is_none() {
                        continue;
                    }
                    let res = res.unwrap();
                    detector.need[res.tid][id] += 1;
                }
            }
        }
        //已经持有的
        for (id, thread) in p.tasks.iter().enumerate() {
            if thread.is_none() {
                continue;
            }
            let thread = thread.as_ref().unwrap();
            let thread_inner = thread.inner_exclusive_access();
            let res = thread_inner.res.as_ref().unwrap();
            assert_eq!(res.tid, id);
            for mutex_id in res.hold_mutex_list.iter() {
                detector.allocation[id][*mutex_id] = 1;
            }
        }
        // println!("detector: {:?}", detector);
        detector
    }
    pub fn build_semaphore_detector(p: &ProcessControlBlockInner, tid: usize, sem_id: usize) -> Self {
        let mut detector = DeadLockDetector::new(p.tasks.len(), p.semaphore_list.len());
        detector.need[tid][sem_id] = 1;
        //被阻塞出的线程,是need的.
        //还有那些可以分配的
        for (id, semaphpore) in p.semaphore_list.iter().enumerate() {
            if semaphpore.is_none() {
                detector.available[id] = 0;
                continue;
            }
            let semaphore = semaphpore.as_ref().unwrap();
            let can_down = semaphore.can_down();
            if can_down {
                detector.available[id] = semaphore.countxxx() as i32;
            } else {
                detector.available[id] = 0;
                let (_, wait_queue) = semaphore.wait_queue();
                for task in wait_queue.iter() {
                    let task_inner = task.inner_exclusive_access();
                    let res = task_inner.res.as_ref();
                    if res.is_none() {
                        continue;
                    }
                    let res = res.unwrap();
                    detector.need[res.tid][id] += 1;
                }
            }
        }
        //已经持有的
        for (id, thread) in p.tasks.iter().enumerate() {
            if thread.is_none() {
                continue;
            }
            let thread = thread.as_ref().unwrap();
            let thread_inner = thread.inner_exclusive_access();
            let res = thread_inner.res.as_ref();
            if res.is_none() {
                continue;
            }
            let res = res.unwrap();
            assert_eq!(res.tid, id);
            for sem_id in res.hold_semaphore_list.iter() {
                detector.allocation[id][*sem_id] += 1;
            }
        }
        // println!("detector: {:?}", detector);
        detector
    }
    pub fn try_alloc(&self) -> bool {
        let mut finish = vec![false; self.thread_count];
        let mut work = self.available.clone();
        loop {
            let mut found_index = false;
            for (tid, b) in finish.iter_mut().enumerate() {
                if !*b {
                    let need = self.need.get(tid).unwrap();
                    if need.iter().enumerate().all(|(index, v)| *v <= work[index]) {
                        found_index = true;
                        *b = true;
                        need.iter().enumerate().for_each(|(index, v)| { work[index] += self.allocation[tid][index]; });
                        break;
                    }
                }
            }
            if finish.iter().all(|x| *x == true) {
                // println!("all finished");
                // println!("finished={:?}", finish);
                // println!("work={:?}", work);
                return true;
            }
            if !found_index {
                // println!("found_index == -1");
                // println!("finished={:?}", finish);
                // println!("work={:?}", work);
                return false;
            }
        }
    }
}

//返回true表示存在死锁,否则表示不存在死锁
pub fn mutex_dead_lock_test(p: &ProcessControlBlockInner, mutex: &dyn Mutex, mutex_id: usize) -> bool {
    let can_lock = mutex.can_lock();
    if can_lock {
        //如果可以lock,那就肯定不存在死锁
        // println!("mutex_dead_lock_test available");
        return false;
    }
    let tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
    let detector = DeadLockDetector::build_mutex_detector(p, tid, mutex_id);
    return !detector.try_alloc();
}

pub fn semaphore_dead_lock_test(p: &ProcessControlBlockInner, sem: &Semaphore, sem_id: usize) -> bool {
    let available = sem.can_down();
    if available {
        // println!("semaphore_dead_lock_test available");
        return false;
    }
    let tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
    let detector = DeadLockDetector::build_semaphore_detector(p, tid, sem_id);
    return !detector.try_alloc();
}