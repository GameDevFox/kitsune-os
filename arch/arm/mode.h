// AArch32 mode

#define CPSR_MODE_MASK 0x1f

// 0x10 USR (User)
// 0x11 FIQ
// 0x12 IRQ
#define SVC_MODE  0x13 // (Supervisor)

// 0x16 MON (Monitor)
// 0x17 ABT (Abort)

#define HYP_MODE  0x1a // (Hypervisor)
// 0x1b UND (Undefined)

// 0x1f SYS (System)