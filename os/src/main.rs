#![feature(panic_info_message)]
#![feature(alloc_error_handler)]
#![no_std]
#![no_main]

#[macro_use]
extern crate bitflags;
#[macro_use]
mod console;
mod lang_items;
mod sbi;
mod logging;
mod sync;
mod config;
pub mod syscall;
pub mod trap;
mod loader;
pub mod task;
mod timer;
#[path = "boards/qemu.rs"]
mod board;
mod mm;

extern crate alloc;
use core::arch::global_asm;

global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

#[no_mangle]
pub fn rust_main() -> ! {
    // run with sp pointing at boot stack
    clear_bss();
    logging::init();
    mm::init();
    task::add_initproc();
    trap::init();
    trap::enable_timer_interrupt();
    timer::set_next_trigger();
    loader::list_apps();
    task::run_tasks();
    panic!("Unreachable in rust_main!");
}

fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    (sbss as usize..ebss as usize).for_each(|a| {
        unsafe { (a as *mut u8).write_volatile(0) }
    });
}

