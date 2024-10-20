//! Process management syscalls
use crate::{
    config::{MAX_SYSCALL_NUM, PAGE_SIZE},
    mm::{copy_out, unmap_and_dealloc, MapPermission, PageTable, StepByOne, VirtAddr, VirtPageNum},
    task::{
        change_program_brk, current_user_token, exit_current_and_run_next, insert_framed_area,
        suspend_current_and_run_next, task_syscall_times, TaskStatus,
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
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
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

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, prot: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");

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
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
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
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
