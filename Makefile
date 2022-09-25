include vars.mk

### TARGETS ###
all: $(ELF_KERNEL) $(QEMU_KERNEL) $(BINARY_KERNEL)

BOOT_OBJS = $(subst .S,.o,$(wildcard ./arch/$(ARCH)/*.S))
CORE_OBJS = $(subst .c,.o,$(wildcard ./core/*.c))
test: $(BOOT_OBJS) $(CORE_OBJS) image/logo.o config/raspberry-pi-4b.o
	echo $(OBJCOPY)
	# $(CC) $(CFLAGS) -T $(LINKER_FILE) -o ./kernel.o $^

clean:
	find . -type f -name '*.o' -delete
	rm -rf $(ELF_KERNEL) $(QEMU_KERNEL) $(BINARY_KERNEL)

$(ELF_KERNEL): $(LINKER_FILE) $(BOOT_OBJS) $(CORE_OBJS) image/logo.o config/raspberry-pi-4b.o
	$(CC) $(CFLAGS) -Xlinker $(LDFLAGS) -T $(LINKER_FILE) -o $(ELF_KERNEL) $(BOOT_OBJS) $(CORE_OBJS) image/logo.o config/raspberry-pi-4b.o

$(QEMU_KERNEL): $(LINKER_FILE) $(BOOT_OBJS) $(CORE_OBJS) image/logo.o config/qemu-raspberry-pi-2b.o
	$(CC) $(CFLAGS) -Xlinker $(LDFLAGS) -T $(LINKER_FILE) -o $(QEMU_KERNEL) $(BOOT_OBJS) $(CORE_OBJS) image/logo.o config/qemu-raspberry-pi-2b.o

$(BINARY_KERNEL): $(ELF_KERNEL)
	$(PREFIX)objcopy $(ELF_KERNEL) -O binary $(BINARY_KERNEL)

# kernel.o: $(BOOT_OBJS) $(CORE_OBJS) image/logo.o config/raspberry-pi-4b.o
# 	$(CC) $(CFLAGS) -c -o core/kernel.o core/kernel.c

# kernel-qemu.o: kernel.o: $(BOOT_OBJS) $(CORE_OBJS) image/logo.o config/qemu-raspberry-pi-2b.o
# 	$(CC) $(CFLAGS) -c -o core/kernel-qemu.o core/kernel.c

# Images
image/%.o:
	make -C image $*.o

## QEMU
QEMU_ARM = qemu-system-arm
QEMU_OPTS = -M raspi2b -nographic -kernel $(QEMU_KERNEL)

qemu: $(QEMU_KERNEL)
	$(QEMU_ARM) $(QEMU_OPTS)

qemu-fifo: $(QEMU_KERNEL)
	bin/init-fifo
	make qemu < ./fifo/input > ./fifo/output

qemu-gdb: $(QEMU_KERNEL)
	$(QEMU_ARM) -S -s $(QEMU_OPTS)

## Utils
gdb:
	$(PREFIX)gdb

objdump: $(ELF_KERNEL)
	$(PREFIX)objdump -D $(ELF_KERNEL)

readelf: $(ELF_KERNEL)
	$(PREFIX)readelf --all --wide $(ELF_KERNEL)

## Tools
tools:
	make -C tools

.PHONY: clean tools

# kitsune64: linker.ld raspi34-boot.o kernel64.o
# 	aarch64-elf-gcc -T linker.ld -o $(ELF_KERNEL) -ffreestanding -O2 -nostdlib raspi34-boot.o kernel64.o -lgcc
# 	aarch64-elf-objcopy $(ELF_KERNEL) -O binary kernel8.img

# raspi34-boot.o: raspi34-boot.S
# 	aarch64-elf-as -c raspi34-boot.S -o raspi34-boot.o

# kernel64.o: kernel.c
# 	aarch64-elf-gcc -ffreestanding -c kernel.c -o kernel64.o -O2 -Wall -Wextra
