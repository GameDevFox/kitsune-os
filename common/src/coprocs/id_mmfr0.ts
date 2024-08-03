import { f } from "../field-utils";

export const id_mmfr0 = {
  args: [15,0,0,1,4],
  isReadable: true,
  isWriteable: false,
  fields: [
    f(
      0, 3, 'VSMA', 'VSMA Support', 'Indicates support for a VMSA.',
      [
        { value: 0b0000, description: 'Not supported.' },
        { value: 0b0001, description: 'Support for IMPLEMENTATION DEFINED VMSA.' },
        { value: 0b0010, description: 'Support for VMSAv6, with Cache and TLB Type Registers implemented.' },
        { value: 0b0011, description: 'Support for VMSAv7, with support for remapping and the Access flag. ARMv7-A profile.' },
        { value: 0b0100, description: 'As for 0b0011, and adds support for the PXN bit in the Short-descriptor translation table format descriptors.' },
        { value: 0b0101, description: 'As for 0b0100, and adds support for the Long-descriptor translation table format.' },
      ],
    ),
    f(
      4, 7, 'PMSA', 'PMSA support', 'Indicates support for a PMSA.',
      [
        { value: 0b0000, description: 'Not supported.' },
        { value: 0b0001, description: 'Support for IMPLEMENTATION DEFINED PMSA.' },
        { value: 0b0010, description: 'Support for PMSAv6, with a Cache Type Register implemented.' },
        { value: 0b0011, description: 'Support for PMSAv7, with support for memory subsections. ARMv7-R profile.' },
      ],
    ),
    f(
      8, 11, 'Out Shr', 'Outermost shareability', 'Indicates the outermost shareability domain implemented.',
      [
        { value: 0b0000, description: 'Implemented as Non-cacheable.' },
        { value: 0b0001, description: 'Implemented with hardware coherency support.' },
        { value: 0b1111, description: 'Shareability ignored.' },
      ],
    ),
    f(
      12, 15, 'Shr Lvl', 'Shareability levels', 'Indicates the number of shareability levels implemented.',
      [
        { value: 0b0000, description: 'One level of shareability implemented.' },
        { value: 0b0001, description: 'Two levels of shareability implemented.' },
      ],
    ),
    f(
      16, 19, 'TCM', 'TCM support', 'Indicates support for TCMs and associated DMAs.',
      [
        { value: 0b0000, description: 'Not supported.' },
        { value: 0b0001, description: 'Support is IMPLEMENTATION DEFINED. ARMv7 requires this setting.' },
        { value: 0b0010, description: 'Support for TCM only, ARMv6 implementation.' },
        { value: 0b0011, description: 'Support for TCM and DMA, ARMv6 implementation.' },
      ],
    ),
    f(
      20, 23, 'AUX', 'Auxiliary registers', 'Indicates support for Auxiliary registers.',
      [
        { value: 0b0000, description: 'None supported.' },
        { value: 0b0001, description: 'Support for Auxiliary Control Register only.' },
        { value: 0b0010, description: 'Support for Auxiliary Fault Status Registers (AIFSR and ADFSR) and Auxiliary Control Register.' },
      ],
    ),
    f(
      24, 27, 'FCSE', 'FCSE support', 'Indicates whether the implementation includes the FCSE.',
      [
        { value: 0b0000, description: 'Not supported.' },
        { value: 0b0001, description: 'Support for FCSE.' },
      ],
    ),
    f(
      28, 31, 'Inn Shr', 'Innermost shareability', 'Indicates the innermost shareability domain implemented.',
      [
        { value: 0b0000, description: 'Implemented as Non-cacheable.' },
        { value: 0b0001, description: 'Implemented with hardware coherency support.' },
        { value: 0b1111, description: 'Shareability ignored.' },
      ],
    ),
  ],
};
