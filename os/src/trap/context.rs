use riscv::register::sstatus::{self, Sstatus, SPP};

/// Trap Context
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TrapContext {
    /// general regs[0..31]
    pub x: [usize; 32],
    /// CSR sstatus
    pub sstatus: Sstatus,
    /// CSR sepc
    pub sepc: usize,
    pub kernel_satp: usize,  // constant when init kernel
    pub kernel_sp: usize,  // constant when init app
    pub trap_handler: usize,  // constant for all apps
}

impl TrapContext {
    /// set stack pointer to x_2 reg (sp)
    pub fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }
    /// init app context
    pub fn app_init_context(
        entry: usize,
        sp: usize,
        kernel_satp: usize,
        kernel_sp: usize,
        trap_handler: usize,
    ) -> Self {
        let mut sstatus = sstatus::read();  // CSR sstatus
        sstatus.set_spp(SPP::User);  // previous privilege mode: user mode
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,
            kernel_satp,
            kernel_sp,
            trap_handler,
        };
        cx.set_sp(sp);
        cx
    }
}


