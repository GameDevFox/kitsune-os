bootName = raspi2-boot
# bootName = raspi34-boot

GCC_OPTS = -g -Og

ELF_KERNEL = kistune.elf
BINARY_KERNEL = kernel7.img

RASPI_VERSION ?= 2

$(BINARY_KERNEL): $(ELF_KERNEL)
	arm-none-eabi-objcopy $(ELF_KERNEL) -O binary $(BINARY_KERNEL)

$(ELF_KERNEL): linker.ld raspi2-boot.o kernel.o
	arm-none-eabi-gcc $(GCC_OPTS) -T linker.ld -Xlinker --nmagic -o $(ELF_KERNEL) -ffreestanding -nostdlib $(bootName).o kernel.o -lgcc

raspi2-boot.o: raspi2-boot.S
	arm-none-eabi-gcc $(GCC_OPTS) -mcpu=cortex-a7 -fpic -ffreestanding -c raspi2-boot.S -o raspi2-boot.o

kernel.o: kernel.c
	arm-none-eabi-gcc $(GCC_OPTS) -D RASPI_VERSION="$(RASPI_VERSION)" -mcpu=arm1176jzf-s -fpic -ffreestanding -std=gnu99 -c kernel.c -o kernel.o -Wall -Wextra

clean:
	rm -rf $(BINARY_KERNEL) $(ELF_KERNEL) raspi2-boot.o kernel.o

QEMU_OPTS = -M raspi2 -nographic

qemu: $(ELF_KERNEL)
	qemu-system-arm $(QEMU_OPTS) -kernel $(ELF_KERNEL)

qemu-gdb: $(ELF_KERNEL)
	qemu-system-arm -S -s $(QEMU_OPTS) -kernel $(ELF_KERNEL)

# kitsune64: linker.ld raspi34-boot.o kernel64.o
# 	aarch64-elf-gcc -T linker.ld -o $(ELF_KERNEL) -ffreestanding -O2 -nostdlib raspi34-boot.o kernel64.o -lgcc
# 	aarch64-elf-objcopy $(ELF_KERNEL) -O binary kernel8.img

# raspi34-boot.o: raspi34-boot.S
# 	aarch64-elf-as -c raspi34-boot.S -o raspi34-boot.o

# kernel64.o: kernel.c
# 	aarch64-elf-gcc -ffreestanding -c kernel.c -o kernel64.o -O2 -Wall -Wextra
