// use core::cell::RefMut;

// use alloc::string::String;
// use alloc::sync::{Arc, Weak};
// use alloc::vec::{Vec};
// use alloc::vec;
// use log::debug;

// use crate::config::TRAP_CONTEXT;
// use crate::fs::stdio::{Stdin, Stdout};
// use crate::fs::File;
// use crate::sync::UPSafeCell;
// use crate::task::TaskContext;
// use crate::trap::{trap_handler, TrapContext};
// use crate::mm::{translated_byte_buffer, translated_refmut, MemorySet, PhysPageNum, VirtAddr, KERNEL_SPACE};
// use crate::task::action::{SignalAction, SignalActions};

// use super::pid::{pid_alloc, KernelStack, PidHandle};
// use super::signals::SignalFlags;


// pub struct TaskControlBlockInner {
//     pub trap_cx_ppn: PhysPageNum,
//     pub base_size: usize,  // below user_sp (include user_sp)
//     pub task_cx: TaskContext,
//     pub task_status: TaskStatus,
//     pub memory_set: MemorySet,
//     pub parent: Option<Weak<TaskControlBlock>>,
//     pub children: Vec<Arc<TaskControlBlock>>,
//     pub exit_code: i32,
//     pub fd_table: Vec<Option<Arc<dyn File + Send + Sync>>>,
    
//     pub signal_mask: SignalFlags,
//     pub signal_actions: SignalActions,
//     pub signals: SignalFlags,
//     pub killed: bool,
//     pub frozen: bool,
//     // the signal which is being handling
//     pub handling_sig: isize,
//     pub trap_ctx_backup: Option<TrapContext>,
// }

// impl TaskControlBlockInner {
//     pub fn get_trap_cx(&self) -> &'static mut TrapContext {
//         self.trap_cx_ppn.get_mut()
//     }
//     pub fn get_user_token(&self) -> usize {
//         self.memory_set.token()
//     }
//     pub fn get_status(&self) -> TaskStatus {
//         self.task_status
//     }
//     pub fn is_zombie(&self) -> bool {
//         self.task_status == TaskStatus::Zombie
//     }
//     pub fn alloc_fd(&mut self) -> usize {
//         if let Some(fd) = self.fd_table.iter()
//             .enumerate()
//             .find(|(_, file)| {
//                 file.is_none()
//             })
//             .map(|(fd, _)| {fd}) {
//                 fd
//         } else {
//             self.fd_table.push(None);
//             self.fd_table.len() - 1
//         }
//     }
// }

// pub struct TaskControlBlock {
//     pub pid: PidHandle,
//     pub kernel_stack: KernelStack,
//     inner: UPSafeCell<TaskControlBlockInner>,  // inner mutability
// }

// impl TaskControlBlock {
//     pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
//         self.inner.exclusive_access()
//     }
//     pub fn getpid(&self) -> usize {
//         self.pid.0
//     }
    
//     pub fn new(elf_data: &[u8]) -> Self {
//         // memory_set with elf program headers/trampoline/trap context/user stack
//         let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
//         let trap_cx_ppn = memory_set
//             .translate(VirtAddr::from(TRAP_CONTEXT).into())
//             .unwrap()
//             .ppn();
//         // alloc a pid and a kernel stack in kernel space
//         let pid_handle = pid_alloc();
//         let kernel_stack = KernelStack::new(&pid_handle);
//         let kernel_stack_top = kernel_stack.get_top();
//         // push a task context which goes to trap_return to the top of the kernel stack
//         let task_control_block = Self {
//             pid: pid_handle,
//             kernel_stack,
//             inner: unsafe { UPSafeCell::new(TaskControlBlockInner {
//                 trap_cx_ppn,
//                 base_size: user_sp,
//                 task_cx: TaskContext::goto_trap_return(kernel_stack_top),
//                 task_status: TaskStatus::Ready,
//                 memory_set,
//                 parent: None,
//                 children: Vec::new(),
//                 exit_code: 0,
//                 fd_table: vec![
//                     // 0 -> stdin
//                     Some(Arc::new(Stdin)),
//                     // 1 -> stdout
//                     Some(Arc::new(Stdout)),
//                     // 2 -> stderr
//                     Some(Arc::new(Stdout)),
//                 ],
//                 signals: SignalFlags::empty(),
//                 signal_mask: SignalFlags::empty(),
//                 handling_sig: -1,
//                 signal_actions: SignalActions::default(),
//                 killed: false,
//                 frozen: false,
//                 trap_ctx_backup: None,
//             })},
//         };
//         // prepare TrapContext in user space
//         let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
//         *trap_cx = TrapContext::app_init_context(
//             entry_point,
//             user_sp,
//             KERNEL_SPACE.exclusive_access().token(),
//             kernel_stack_top,
//             trap_handler as usize,
//         );
//         task_control_block        
//     }
//     pub fn get_trap_cx(&self) -> &'static mut TrapContext {
//         self.inner_exclusive_access().get_trap_cx()
//     }
//     pub fn get_user_token(&self) -> usize {
//         self.inner_exclusive_access().get_user_token()
//     }
//     pub fn fork(self: &Arc<TaskControlBlock>) -> Arc<TaskControlBlock> {
//         // access parent PCB exclusively
//         let mut parent_inner = self.inner.exclusive_access();
//         // copy user space(include trap context)
//         let memory_set = MemorySet::from_existed_user(
//             &parent_inner.memory_set
//         );
//         let trap_cx_ppn = memory_set
//             .translate(VirtAddr::from(TRAP_CONTEXT).into())
//             .unwrap()
//             .ppn();
//         // alloc a pid and a kernel stack in kernel space
//         let pid_handle = pid_alloc();
//         let kernel_stack = KernelStack::new(&pid_handle);
//         let kernel_stack_top = kernel_stack.get_top();
//         // copy fd table
//         let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
//         for fd in parent_inner.fd_table.iter() {
//             if let Some(file) = fd {
//                 new_fd_table.push(Some(file.clone()));
//             } else {
//                 new_fd_table.push(None);
//             }
//         }
//         let task_control_block = Arc::new(TaskControlBlock {
//             pid: pid_handle,
//             kernel_stack,
//             inner: unsafe { UPSafeCell::new(TaskControlBlockInner {
//                 trap_cx_ppn,
//                 base_size: parent_inner.base_size,
//                 task_cx: TaskContext::goto_trap_return(kernel_stack_top),
//                 task_status: TaskStatus::Ready,
//                 memory_set,
//                 parent: Some(Arc::downgrade(self)),
//                 children: Vec::new(),
//                 exit_code: 0,
//                 fd_table: new_fd_table,
//                 signals: SignalFlags::empty(),
//                 signal_mask: parent_inner.signal_mask,
//                 handling_sig: -1,
//                 signal_actions: parent_inner.signal_actions.clone(),
//                 killed: false,
//                 frozen: false,
//                 trap_ctx_backup: None,
//             }) },
//         });
//         // add child
//         parent_inner.children.push(task_control_block.clone());
//         // modify kernel_sp in trap_cx
//         // **** access children PCB exclusively
//         let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
//         trap_cx.kernel_sp = kernel_stack_top;
//         // return
//         task_control_block
//     }
//     pub fn exec(&self, elf_data: &[u8], args: Vec<String>) {
//         // memory_set with elf program headers/trampoline/trap context/user stack
//         let (memory_set, mut user_sp, entry_point) = MemorySet::from_elf(elf_data);
//         let trap_cx_ppn = memory_set
//             .translate(VirtAddr::from(TRAP_CONTEXT).into())
//             .unwrap()
//             .ppn();
//         // push arguments on user stack
//         user_sp -= (args.len() + 1) * core::mem::size_of::<usize>();
//         let argv_base = user_sp;  // user space argv base
//         let mut argv: Vec<_> = (0..=args.len())
//             .map(|arg_id| {
//                 translated_refmut(
//                     memory_set.token(),
//                     (argv_base + arg_id * core::mem::size_of::<usize>()) as *mut usize  
//                 )
//             })
//             .collect();
//         //  -----------------------    USER STACK (KERNEL SPACE token)
//         //   (8byte)   [argv[len]]     value: 0x0000_0000
//         //   (8byte)   [argv[len - 1]] value: virtual addr of real str: args[len - 1]   
//         //   (8byte)   [argv[...]]     value: virtual addr of real str: args[...]   
//         //   (8byte)   [argv[1]]       value: virtual addr of real str: args[1] 
//         //   (8byte)   [argv[0]]       value: virtual addr of real str: args[0]   
//         //  -----------------------    ARGV BASE
//         //   (xbyte)   [args[len - 1]] value: str args[len - 1]: bytes = x
//         //   ...
//         //   (xbyte)   [args[0]]       value: str args[0]:       bytes = x
//         *argv[args.len()] = 0;   
//         for i in 0..args.len() {
//             user_sp -= args[i].len() + 1;
//             *argv[i] = user_sp;
//             let mut p = user_sp;
//             for c in args[i].as_bytes() {
//                 *translated_refmut(memory_set.token(), p as *mut u8) = *c;
//                 p += 1;
//             }
//             *translated_refmut(memory_set.token(), p as *mut u8) = 0;
//         }
//         // make the user_sp aligned to 8B for k210 platform
//         user_sp -= user_sp % (core::mem::size_of::<usize>());
//         // **** access inner exclusively
//         let mut inner = self.inner_exclusive_access();
//         // substitute memory_set
//         inner.memory_set = memory_set;
//         // update trap_cx ppn
//         inner.trap_cx_ppn = trap_cx_ppn;
//         // initialize trap_cx
//         let trap_cx = inner.get_trap_cx();
//         *trap_cx = TrapContext::app_init_context(
//             entry_point,
//             user_sp,
//             KERNEL_SPACE.exclusive_access().token(),
//             self.kernel_stack.get_top(),
//             trap_handler as usize,
//         );
//         trap_cx.x[10] = args.len();
//         trap_cx.x[11] = argv_base;
//         // **** stop exclusively accessing inner automatically
//     }
// }

use core::cell::RefMut;

use alloc::sync::{Arc, Weak};

use crate::{mm::PhysPageNum, sync::UPSafeCell, trap::TrapContext};

use super::{id::{kstack_alloc, KernelStack, TaskUserRes}, process::ProcessControlBlock, TaskContext};

#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    Ready,
    Running,
    // Exited,
    Blocked,
}

pub struct TaskControlBlock {
    pub process: Weak<ProcessControlBlock>,
    pub kstack: KernelStack,
    inner: UPSafeCell<TaskControlBlockInner>,
}

pub struct TaskControlBlockInner {
    pub res: Option<TaskUserRes>,
    /// a ppn of the page of trap_context
    pub trap_cx_ppn: PhysPageNum,
    pub task_cx: TaskContext,
    pub task_status: TaskStatus,
    pub exit_code: Option<i32>,
}

impl TaskControlBlockInner {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
}


impl TaskControlBlock {
    pub fn new(process: Arc<ProcessControlBlock>, ustack_base: usize, alloc_user_res: bool) -> Self {
        let res = TaskUserRes::new(
            process.clone(), 
            ustack_base, 
            alloc_user_res,
        );
        let trap_cx_ppn = res.trap_cx_ppn();
        let kstack = kstack_alloc();
        let kstack_top = kstack.get_top();
        Self {
            process: Arc::downgrade(&process),
            kstack: kstack_alloc(),
            inner: unsafe{ UPSafeCell::new(TaskControlBlockInner{
                res: Some(res),
                trap_cx_ppn,
                task_cx: TaskContext::goto_trap_return(kstack_top),
                task_status: TaskStatus::Ready,
                exit_code: None,
            }) },
        }
    }
    pub fn getpid(&self) -> usize {
        self.process.upgrade().unwrap().pid.0
    }
    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }
    pub fn get_user_token(&self) -> usize {
        let process = self.process.upgrade().unwrap();
        let token = process.inner_exclusive_access().memory_set.token();
        token
    }
}


