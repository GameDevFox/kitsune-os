bootName = raspi2-boot
# bootName = raspi34-boot

GCC_OPTS = -g -Og

ELF_KERNEL = kitsune.elf
QEMU_KERNEL = kitsune-qemu.elf

BINARY_KERNEL = kernel7.img

all: $(ELF_KERNEL) $(QEMU_KERNEL) $(BINARY_KERNEL)

clean:
	rm -rf $(ELF_KERNEL) $(QEMU_KERNEL) $(BINARY_KERNEL) raspi2-boot.o kernel.o kernel-qemu.o

LINKER_OPTS = -T linker.ld -Xlinker --nmagic -ffreestanding -nostdlib -lgcc

$(ELF_KERNEL): linker.ld raspi2-boot.o kernel.o
	arm-none-eabi-gcc $(GCC_OPTS) $(LINKER_OPTS) -o $(ELF_KERNEL) $(bootName).o kernel.o

$(QEMU_KERNEL): linker.ld raspi2-boot.o kernel-qemu.o
	arm-none-eabi-gcc $(GCC_OPTS) $(LINKER_OPTS) -o $(QEMU_KERNEL) $(bootName).o kernel-qemu.o

$(BINARY_KERNEL): $(ELF_KERNEL)
	arm-none-eabi-objcopy $(ELF_KERNEL) -O binary $(BINARY_KERNEL)

raspi2-boot.o: raspi2-boot.S
	arm-none-eabi-gcc $(GCC_OPTS) -mcpu=cortex-a7 -fpic -ffreestanding -c raspi2-boot.S -o raspi2-boot.o

KERNEL_OPTS = -mcpu=arm1176jzf-s -fpic -ffreestanding -std=gnu99 -Wall -Wextra -c

kernel.o: kernel.c
	arm-none-eabi-gcc $(GCC_OPTS) $(KERNEL_OPTS) -D RASPI_VERSION="4" -c -o kernel.o kernel.c

kernel-qemu.o: kernel.c
	arm-none-eabi-gcc $(GCC_OPTS) $(KERNEL_OPTS) -D RASPI_VERSION="2" -c -o kernel-qemu.o kernel.c

QEMU_ARM = qemu-system-arm
QEMU_OPTS = -M raspi2b -nographic -kernel $(QEMU_KERNEL)

qemu: $(QEMU_KERNEL)
	$(QEMU_ARM) $(QEMU_OPTS)

qemu-gdb: $(QEMU_KERNEL)
	$(QEMU_ARM) -S -s $(QEMU_OPTS)

# kitsune64: linker.ld raspi34-boot.o kernel64.o
# 	aarch64-elf-gcc -T linker.ld -o $(ELF_KERNEL) -ffreestanding -O2 -nostdlib raspi34-boot.o kernel64.o -lgcc
# 	aarch64-elf-objcopy $(ELF_KERNEL) -O binary kernel8.img

# raspi34-boot.o: raspi34-boot.S
# 	aarch64-elf-as -c raspi34-boot.S -o raspi34-boot.o

# kernel64.o: kernel.c
# 	aarch64-elf-gcc -ffreestanding -c kernel.c -o kernel64.o -O2 -Wall -Wextra
