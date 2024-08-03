void enable_irq(char irq);
void route_irq_to_cpus(char irq, char cpus);

enum {
  GIC_BASE = 0xff840000,

    GICD = (GIC_BASE + 0x1000), // Distributor

      GICD_CTLR = (GICD + 0x000),

      GICD_IRQ_SET_ENABLE = (GICD + 0x100),
      GICD_IRQ_CLEAR_ENABLE = (GICD + 0x180),

      GICD_IRQ_SET_PENDING = (GICD + 0x200),
      GICD_IRQ_CLEAR_PENDING = (GICD + 0x200),

      GICD_IRQ_PROCESSOR_TARGETS = (GICD + 0x800),

    GICC = (GIC_BASE + 0x2000), // CPU interfaces

      GICC_CTLR = (GICC + 0x0000),

      GICC_IAR = (GICC + 0x000C),
      GICC_EOIR = (GICC + 0x0010),
};

enum {
  VIDEO_CORE_IRQ_BASE = 96,

  VC_IRQ_TIMER_0 = VIDEO_CORE_IRQ_BASE + 0,
  VC_IRQ_TIMER_1 = VIDEO_CORE_IRQ_BASE + 1,
  VC_IRQ_TIMER_2 = VIDEO_CORE_IRQ_BASE + 2,
  VC_IRQ_TIMER_3 = VIDEO_CORE_IRQ_BASE + 3,
};
