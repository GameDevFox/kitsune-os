target remote localhost:1234
add-symbol-file kitsune-qemu.elf

set scheduler-locking on

# # undefined_instruction
# b *(*exception_vector+4)
# # instruction_abort
# b *(*exception_vector+12)
# # data_abort
# b *(*exception_vector+16)
# # irq
# b *(*exception_vector+24)

# b init_irq
# b write_memory
# b sp_start
# b main.c:78 if i == 0x14000

# b cpsr_enable_irq
# commands
# print "Hello, Breakpoint"
# x/16x $ic_addr
# end

# ui

# b prop
