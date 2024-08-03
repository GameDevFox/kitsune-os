import { a, f } from "../field-utils";

export const ttbcr = {
  args: [15,0,2,0,2],
  isReadable: true,
  isWriteable: true,
  fields: [
    f(
      0, 2, 'N', 'N', 'Indicate the width of the base address held in TTBR0',
    ),
    a(3, 3, 'SBZP'),
    f(
      4, 4, 'PD0', 'PD0', 'Translation table walk disable for translations using TTBR0',
      [
        { value: 0, description: 'Perform translation table walks using TTBR0.' },
        { value: 1, description: 'A TLB miss on an address that is translated using TTBR0 generates a Translation fault. No translation table walk is performed.' },
      ],
    ),
    f(
      5, 5, 'PD1', 'PD1', 'Translation table walk disable for translations using TTBR1',
      [
        { value: 0, description: 'Perform translation table walks using TTBR1.' },
        { value: 1, description: 'A TLB miss on an address that is translated using TTBR1 generates a Translation fault. No translation table walk is performed.' },
      ],
    ),
    a(6, 30, 'SBZP'),
    f(
      31, 31, 'EAE', 'Extended Address Enable', '',
      [
        { value: 0, description: 'Use the 32-bit translation system, with the Short-descriptor translation table format. ' +
          'In this case, the format of the TTBCR is as described in this section.' },
        { value: 1, description: 'Use the 40-bit translation system, with the Long-descriptor translation table format. ' +
          'In this case, the format of the TTBCR is as described in TTBCR format when using the Long-descriptor translation table format on page B4-1718.' },
      ],
    ),
  ],
};
