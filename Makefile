include vars.mk

BOOT_OBJS = $(subst .S,.o,$(wildcard ./arch/$(ARCH)/*.S))
CORE_OBJS = $(subst .c,.o,$(wildcard ./core/*.c))
IMAGE_OBJS = image/logo.o image/mascot.o image/no-glasses.o

### TARGETS ###
all: $(ELF_KERNEL) $(QEMU_KERNEL) $(BINARY_KERNEL)

clean:
	find . -type f -name '*.o' -delete
	find . -type f -name '*.data' -delete
	rm -rf $(ELF_KERNEL) $(QEMU_KERNEL) $(BINARY_KERNEL)
.PHONY: clean

$(ELF_KERNEL): $(LINKER_FILE) $(BOOT_OBJS) $(CORE_OBJS) $(IMAGE_OBJS) config/raspberry-pi-4b.o ./dts/device-tree.o
	$(CC) $(CFLAGS) -Xlinker $(LDFLAGS) -T $(LINKER_FILE) -o $(ELF_KERNEL) $(BOOT_OBJS) $(CORE_OBJS) $(IMAGE_OBJS) config/raspberry-pi-4b.o ./dts/device-tree.o

$(QEMU_KERNEL): $(LINKER_FILE) $(BOOT_OBJS) $(CORE_OBJS) $(IMAGE_OBJS) config/qemu-raspberry-pi-2b.o ./dts/device-tree.o
	$(CC) $(CFLAGS) -Xlinker $(LDFLAGS) -T $(LINKER_FILE) -o $(QEMU_KERNEL) $(BOOT_OBJS) $(CORE_OBJS) $(IMAGE_OBJS) config/qemu-raspberry-pi-2b.o ./dts/device-tree.o

$(BINARY_KERNEL): $(ELF_KERNEL)
	$(PREFIX)objcopy $(ELF_KERNEL) -O binary $(BINARY_KERNEL)

# Images
image/%.o:
	make -C image $*.o

# Device Tree for QEMU
dts/device-tree.o: ./dts/4-model-b.dtb
	$(PREFIX)ld -r -b binary ./dts/4-model-b.dtb -o $*.o

## QEMU
QEMU_ARM = qemu-system-arm
QEMU_OPTS = \
	-M raspi2b \
	-global bcm2835-fb.xres=1920 \
	-global bcm2835-fb.yres=1080 \
	-global bcm2835-fb.bpp=32 \
	-global bcm2835-fb.pixo=0 \
	-kernel $(QEMU_KERNEL)

qemu: $(QEMU_KERNEL)
	$(QEMU_ARM) $(QEMU_OPTS) -serial $(shell tty)

qemu-gdb: $(QEMU_KERNEL)
	$(QEMU_ARM) -S -s $(QEMU_OPTS) -serial tcp:localhost:8081

qemu-tcp: $(QEMU_KERNEL)
	$(QEMU_ARM) $(QEMU_OPTS) -serial tcp:localhost:8081

docker-make:
	docker build -t kitsune-os-make .
	docker run -v "$(PWD)":/kitsune-os kitsune-os-make

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
.PHONY: tools

# kitsune64: linker.ld raspi34-boot.o kernel64.o
# 	aarch64-elf-gcc -T linker.ld -o $(ELF_KERNEL) -ffreestanding -O2 -nostdlib raspi34-boot.o kernel64.o -lgcc
# 	aarch64-elf-objcopy $(ELF_KERNEL) -O binary kernel8.img

# raspi34-boot.o: raspi34-boot.S
# 	aarch64-elf-as -c raspi34-boot.S -o raspi34-boot.o

# kernel64.o: kernel.c
# 	aarch64-elf-gcc -ffreestanding -c kernel.c -o kernel64.o -O2 -Wall -Wextra
