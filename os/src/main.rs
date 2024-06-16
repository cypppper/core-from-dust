#![feature(panic_info_message)]
#![no_std]
#![no_main]

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
mod task;

use core::arch::global_asm;
use log::*;

global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

#[no_mangle]
pub fn rust_main() -> ! {
    extern "C" {
        fn stext(); // begin addr of text segment
        fn etext(); // end addr of text segment
        fn srodata(); // start addr of Read-Only data segment
        fn erodata(); // end addr of Read-Only data ssegment
        fn sdata(); // start addr of data segment
        fn edata(); // end addr of data segment
        fn sbss(); // start addr of BSS segment
        fn ebss(); // end addr of BSS segment
        fn boot_stack_lower_bound(); // stack lower bound
        fn boot_stack_top(); // stack top
    }
    clear_bss();
    logging::init();
    trace!("[kernel] Hello, world!");
    trace!(
        "[kernel] .text [{:#x}, {:#x})",
        stext as usize,
        etext as usize
    );
    trace!(
        "[kernel] .rodata [{:#x}, {:#x})",
        srodata as usize, erodata as usize
    );
    trace!(
        "[kernel] .data [{:#x}, {:#x})",
        sdata as usize, edata as usize
    );
    trace!(
        "[kernel] boot_stack top=bottom={:#x}, lower_bound={:#x}",
        boot_stack_top as usize, boot_stack_lower_bound as usize
    );
    trace!("[kernel] .bss [{:#x}, {:#x})", sbss as usize, ebss as usize);
    
    trap::init();
    // batch::init();
    // batch::run_next_app();
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

