PREFIX = arm-none-eabi-

OPTIONAL_CFLAGS = -g -Og -Wall -Wextra
CFLAGS = -fpic -ffreestanding -mcpu=cortex-a7 $(OPTIONAL_CFLAGS)

CC = $(PREFIX)gcc $(CFLAGS)

ARCH = arm

LINKER_FILE = linker.ld

ELF_KERNEL = kitsune.elf
QEMU_KERNEL = kitsune-qemu.elf
BINARY_KERNEL = kernel7.img

BOOT_NAME = boot
BOOT_OBJ = $(BOOT_NAME).o
BOOT_DIR = arch/$(ARCH)

### TARGETS ###
all: $(ELF_KERNEL) $(QEMU_KERNEL) $(BINARY_KERNEL)

.PHONY: clean
clean:
	find . -type f -name '*.o' -delete
	rm -rf $(ELF_KERNEL) $(QEMU_KERNEL) $(BINARY_KERNEL)

# include $(BOOT_DIR)/Makefile

## Kernel
KERNEL_C_FILES = kernel.c
KERNEL_FILES = $(KERNEL_C_FILES) kernel.h

kernel.o: $(KERNEL_FILES)
	$(CC) -D RASPI_VERSION="4" $(CFLAGS) -c -o kernel.o $(KERNEL_C_FILES)

kernel-qemu.o: $(KERNEL_FILES)
	$(CC) -D RASPI_VERSION="2" $(CFLAGS) -c -o kernel-qemu.o $(KERNEL_C_FILES)

LINKER_OPTS = -T $(LINKER_FILE) -Xlinker --nmagic -ffreestanding -nostdlib -lgcc

BOOT_PATH = arch/$(ARCH)/$(BOOT_OBJ)
VECTOR_PATH = arch/$(ARCH)/vector.o

$(ELF_KERNEL): $(LINKER_FILE) $(BOOT_PATH) $(VECTOR_PATH) kernel.o uart.o
	$(CC) $(CFLAGS) $(LINKER_OPTS) -o $(ELF_KERNEL) $(BOOT_PATH) $(VECTOR_PATH) kernel.o uart.o

$(QEMU_KERNEL): $(LINKER_FILE) $(BOOT_PATH) $(VECTOR_PATH) kernel-qemu.o uart.o
	$(CC) $(CFLAGS) $(LINKER_OPTS) -o $(QEMU_KERNEL) $(BOOT_PATH) $(VECTOR_PATH) kernel-qemu.o uart.o

$(BINARY_KERNEL): $(ELF_KERNEL)
	$(PREFIX)objcopy $(ELF_KERNEL) -O binary $(BINARY_KERNEL)

## QEMU
QEMU_ARM = qemu-system-arm
QEMU_OPTS = -M raspi2b -nographic -kernel $(QEMU_KERNEL)

qemu: $(QEMU_KERNEL)
	$(QEMU_ARM) $(QEMU_OPTS)

qemu-gdb: $(QEMU_KERNEL)
	$(QEMU_ARM) -S -s $(QEMU_OPTS)

## Utils
gdb:
	$(PREFIX)gdb

objdump: $(ELF_KERNEL)
	$(PREFIX)objdump -D $(ELF_KERNEL)

readelf: $(ELF_KERNEL)
	$(PREFIX)readelf --all $(ELF_KERNEL)

# kitsune64: linker.ld raspi34-boot.o kernel64.o
# 	aarch64-elf-gcc -T linker.ld -o $(ELF_KERNEL) -ffreestanding -O2 -nostdlib raspi34-boot.o kernel64.o -lgcc
# 	aarch64-elf-objcopy $(ELF_KERNEL) -O binary kernel8.img

# raspi34-boot.o: raspi34-boot.S
# 	aarch64-elf-as -c raspi34-boot.S -o raspi34-boot.o

# kernel64.o: kernel.c
# 	aarch64-elf-gcc -ffreestanding -c kernel.c -o kernel64.o -O2 -Wall -Wextra
