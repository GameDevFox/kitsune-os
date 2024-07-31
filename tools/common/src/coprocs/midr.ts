import { f } from "../field-utils";

export const midr = {
  args: [15,0,0,0,0],
  isReadable: true,
  isWriteable: false,
  fields: [
    f(
      0, 3, 'Rev', 'Revision', 'Shows the patch revision number of the processor',
    ),
    f(
      4, 15, 'Part', 'Primary part number', 'For example 0xC08 for the Cortex-A8 processor.',
      [
        { value: 0xb76, description: '(Raspbeery Pi)' },
        { value: 0xc08, description: 'Cortex-A8 processor (Raspbeery Pi 2)' },
        { value: 0xd03, description: 'Cortex-A53 processor (Raspbeery Pi 3)' },
        // { value: 0xd07, description: 'Cortex-A57 processor' },
        { value: 0xd08, description: 'Cortex-A72 processor (Raspbeery Pi 4)' },
      ]
    ),
    f(
      16, 19, 'Arch', 'Architecture', 'Will be 0xF for ARM architecture v7',
    ),
    f(
      20, 23, 'Var', 'Variant', 'Shows the revision number of the processor',
    ),
    f(
      24, 31, 'Impl', 'Implementer', 'Will be 0x41 for an ARM designed processor.r',
      [
        { value: 0x00, description: 'Reserved for software use' },
        { value: 0x41, description: 'Arm Limited' },
        { value: 0x42, description: 'Broadcom Corporation' },
        { value: 0x43, description: 'Cavium Inc.' },
        { value: 0x44, description: 'Digital Equipment Corporation' },
        { value: 0x46, description: 'Fujitsu Ltd.' },
        { value: 0x49, description: 'Infineon Technologies AG' },
        { value: 0x4d, description: 'Motorola or Freescale Semiconductor Inc.' },
        { value: 0x4e, description: 'NVIDIA Corporation' },
        { value: 0x50, description: 'Applied Micro Circuits Corporation' },
        { value: 0x51, description: 'Qualcomm Inc.' },
        { value: 0x56, description: 'Marvell International Ltd.' },
        { value: 0x69, description: 'Intel Corporation' },
        { value: 0xc0, description: 'Ampere Computing' },
      ]
    ),
  ],
};
