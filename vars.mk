ARCH = arm
PREFIX = arm-none-eabi-

OPTIONAL_CFLAGS = -g -Og -Wall -Wextra
CFLAGS = -fpic -ffreestanding -mcpu=cortex-a7 -nostdlib -lgcc $(OPTIONAL_CFLAGS)
ASFLAGS = $(CFLAGS)

LDFLAGS = --nmagic

CC = $(PREFIX)gcc
LD = $(PREFIX)ld $(LDFLAGS)

LINKER_FILE = linker.ld

NAME=kitsune

ELF_KERNEL = $(NAME).elf
QEMU_KERNEL = $(NAME)-qemu.elf
BINARY_KERNEL = $(NAME).img

BOOT_DIR = arch/$(ARCH)
BOOT_PATH = $(BOOT_DIR)/boot.o

VECTOR_PATH = arch/$(ARCH)/vector.o

KERNEL_STUFF = uart.o hex.o image/logo.o

KERNEL_C_FILES = kernel.c
KERNEL_FILES = $(KERNEL_C_FILES) kernel.h
