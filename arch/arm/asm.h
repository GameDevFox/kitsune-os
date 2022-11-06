size_t get_cpsr();
size_t get_mode();
size_t get_performance_counter();
size_t live_fn();
size_t live_instruction();
size_t perf_test();
size_t set_cpsr(uint32_t value);
size_t toggle_irqs();
uint32_t get_pmccntr();
void enable_cache();
void initial_register_values();
void set_mode(size_t mode);

void test_bytes(uint32_t start, uint32_t end);
void clear_bytes(void* start, uint32_t length);
void copy_bytes(const void* from, void* to, uint32_t length);
