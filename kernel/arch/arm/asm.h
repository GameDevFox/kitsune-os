size_t get_cpsr();
size_t get_mode();
size_t get_performance_counter();
size_t live_fn();
size_t live_instruction();
size_t perf_test();
size_t set_cpsr(uint32_t value);

void cpsr_enable_irq();
size_t toggle_irqs();

void enable_cache();
void initial_register_values();
void set_mode(size_t mode);

void clear_bytes(void* start, uint32_t length);
void copy_bytes(const void* from, void* to, uint32_t length);
