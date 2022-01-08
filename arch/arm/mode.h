// AArch32 mode

#define CPSR_MODE_MASK 0x1f
#define CPSR_DISABLE_IRQ_MASK 1 << 7
#define CPSR_DISABLE_FIQ_MASK 1 << 6
#define CPSR_DISABLE_IRQ_FIQ_MASK (CPSR_DISABLE_IRQ_MASK | CPSR_DISABLE_FIQ_MASK)

#define USR_MODE        0x10 // User
#define USER_MODE       0x10

#define FIQ_MODE        0x11

#define IRQ_MODE        0x12

#define SVC_MODE        0x13 // Supervisor
#define SUPERVISOR_MODE 0x13

#define MON_MODE        0x16 // Monitor
#define MONITOR_MODE    0x16

#define ABT_MODE        0x17 // Abort
#define ABORT_MODE      0x17

#define HYP_MODE        0x1a // Hypervisor
#define HYPERVISOR_MODE 0x1a

#define UND_MODE        0x1b // Undefined
#define UNDEFINED_MODE  0x1b

#define SYS_MODE        0x1f // System
#define SYSTEM_MODE     0x1f
