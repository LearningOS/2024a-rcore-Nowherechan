//! Process management syscalls
use alloc::sync::Arc;

use crate::{
    config::{BIG_STRIDE, MAX_SYSCALL_NUM, PAGE_SIZE},
    loader::get_app_data_by_name,
    mm::{
        copy_out, translated_refmut, translated_str, unmap_and_dealloc, MapPermission, PageTable,
        StepByOne, VirtAddr, VirtPageNum,
    },
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next, insert_framed_area,
        suspend_current_and_run_next, task_syscall_times, TaskControlBlock, TaskStatus,
    },
    timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel:pid[{}] sys_yield", current_task().unwrap().pid.0);
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let task = current_task().unwrap();
        task.exec(data);
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    trace!(
        "kernel::pid[{}] sys_waitpid [{}]",
        current_task().unwrap().pid.0,
        pid
    );
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel:pid[{}] sys_get_time", current_task().unwrap().pid.0);
    let us = get_time_us();
    let token = current_user_token();
    copy_out(
        token,
        ts,
        TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        },
    );
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    error!("kernel: sys_task_info NOT IMPLEMENTED YET!");
    copy_out(
        current_user_token(),
        ti,
        TaskInfo {
            status: TaskStatus::Running,
            syscall_times: task_syscall_times(),
            time: get_time_us() / 1_000,
        },
    );
    0
}

/// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, prot: usize) -> isize {
    trace!("kernel: sys_mmap");
    // Check args
    if start % 4096 != 0 {
        error!("sys_mmap check start invalid; start = {}", start);
        return -1;
    }
    if prot == 0 || prot & !0x7 != 0 {
        error!("sys_mmap check prot invalid; prot = {}", prot);
        return -1;
    }
    // Prepare alloc and map info
    let start_va: VirtAddr = VirtAddr::from(start);
    let end_va = VirtAddr::from(start + len);
    let mut permission = MapPermission::empty();
    permission.set(MapPermission::U, true);
    permission.set(MapPermission::R, prot & 1 != 0);
    permission.set(MapPermission::W, prot & 0x2 != 0);
    permission.set(MapPermission::X, prot & 0x4 != 0);
    // Check if already allocated
    let page_table = PageTable::from_token(current_user_token());
    let mut vpn = VirtPageNum(start / PAGE_SIZE);
    let num_of_pages = len / PAGE_SIZE + if len % PAGE_SIZE != 0 { 1 } else { 0 };
    for _ in 0..num_of_pages {
        match page_table.translate(vpn) {
            Some(pte) => {
                if pte.is_valid() {
                    return -1;
                }
            }
            None => {}
        }
        vpn.step();
    }
    insert_framed_area(start_va, end_va, permission);
    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap");
    // Check
    if start % 4096 != 0 {
        return -1;
    }
    let token = current_user_token();
    let start_vpn = VirtPageNum(start / 4096);
    let num_of_pages = len / 4096 + if len % 4096 != 0 { 1 } else { 0 };
    match unmap_and_dealloc(token, start_vpn, num_of_pages) {
        None => -1,
        Some(()) => 0,
    }
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_spawn", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(elf_data) = get_app_data_by_name(path.as_str()) {
        let new_task = Arc::new(TaskControlBlock::new(elf_data));
        let curr_task = current_task().unwrap();
        // Set parent and child
        new_task.inner_exclusive_access().parent = Some(Arc::downgrade(&curr_task));
        curr_task
            .inner_exclusive_access()
            .children
            .push(new_task.clone());
        let new_pid = new_task.pid.0;
        add_task(new_task);
        new_pid as isize
    } else {
        -1
    }
}

// YOUR JOB: Set task priority.
pub fn sys_set_priority(prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority",
        current_task().unwrap().pid.0
    );
    if prio < 2 {
        -1
    } else {
        let curr_task = current_task().unwrap();
        curr_task.inner_exclusive_access().pass = BIG_STRIDE / prio as usize;
        prio
    }
}
