// System Control Register
#define SCTLR(reg) p15, 0, reg, c1, c0, 0

// Performance Monitors Cycle Count Register
#define PMCCNTR(reg) p15, 0, reg, c9, c13, 0
#define PMCCNTR_C_FLAG (1 << 2)
