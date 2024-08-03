import { a, f } from "../field-utils";

export const sctlr = {
  args: [15,0,1,0,0],
  isReadable: true,
  isWriteable: true,
  fields: [
    f(
      0, 0, 'M', 'MMU enable',
      'This is a global enable bit for the PL1&0 stage 1 MMU',
      [
        { value: 0, description: 'PL1&0 stage 1 MMU disabled' },
        { value: 1, description: 'PL1&0 stage 1 MMU enabled' }
      ]
    ),
    f(
      1, 1, 'A', 'Alignment check enable',
      'This is the enable bit for Alignment fault checking',
      [
        { value: 0, description: 'Alignment fault checking disabled' },
        { value: 1, description: 'Alignment fault checking enabled' }
      ]
    ),
    f(
      2, 2, 'C', 'Cache enable',
      'This is a global enable bit for data and unified caches',
      [
        { value: 0, description: 'Data and unified caches disabled' },
        { value: 1, description: 'Data and unified caches enabled' }
      ]
    ),
    a(3, 4, 'RAO'),
    f(
      5, 5, 'CP15BEN', 'CP15 barrier enable',
      'If implemented, this is an enable bit for the CP15 DMB, DSB, and ISB barrier operations',
      [
        { value: 0, description: 'CP15 barrier operations disabled. Their encodings are UNDEFINED' },
        { value: 1, description: 'CP15 barrier operations enabled' }
      ]
    ),
    a(6, 6, 'RAO'),
    a(7, 9, 'RAZ'),
    f(
      10, 10, 'SW', 'SWP and SWPB enable',
      'This bit enables the use of SWP and SWPB instructions',
      [
        { value: 0, description: 'SWP and SWPB are UNDEFINED' },
        { value: 1, description: 'SWP and SWPB perform as described in SWP, SWPB on page A8-723' }
      ]
    ),
    f(
      11, 11, 'Z', 'Branch prediction enable',
      '',
      [
        { value: 0, description: 'Program flow prediction disabled' },
        { value: 1, description: 'Program flow prediction enabled' }
      ]
    ),
    f(
      12, 12, 'I', 'Instruction cache enable',
      'This is a global enable bit for instruction caches',
      [
        { value: 0, description: 'Instruction caches disabled' },
        { value: 1, description: 'Instruction caches enabled' }
      ]
    ),
    f(
      13, 13, 'V', 'Vectors bit',
      'This bit selects the base address of the exception vectors',
      [
        {
          value: 0,
          description: 'Low exception vectors, base address 0x00000000. ' +
            'In an implementation that includes the Security Extensions, this base address can be ' +
            're-mapped'
        },
        {
          value: 1,
          description: 'High exception vectors (Hivecs), base address 0xFFFF0000. ' +
            'This base address is never remapped'
        }
      ]
    ),
    f(
      14, 14, 'RR', 'Round Robin select',
      'If the cache implementation supports the use of an alternative replacement ' +
        'strategy that has a more easily predictable worst-case performance, this bit controls whether it is ' +
        'used',
      [
        { value: 0, description: 'Normal replacement strategy, for example, random replacement' },
        { value: 1, description: 'Predictable strategy, for example, round-robin replacement' }
      ]
    ),
    a(15, 15, 'RAZ'),
    a(16, 16, 'RAO'),
    f(
      17, 17, 'HA', 'Hardware Access flag enable',
      'If the implementation provides hardware management of the Access flag this bit enables the Access flag management',
      [
        { value: 0, description: 'Hardware management of Access flag disabled' },
        { value: 1, description: 'Hardware management of Access flag enabled' }
      ]
    ),
    a(18, 18, 'RAO'),
    f(
      19, 19, 'WXN', 'Write permission implies XN',
      'If the implementation provides hardware management of the Access flag this bit enables the Access flag management',
      [
        { value: 0, description: 'Regions with write permission are not forced to XN' },
        { value: 1, description: 'Regions with write permission are forced to XN' }
      ]
    ),
    f(
      20, 20, 'UWXN', 'Unprivileged write permission implies PL1 XN', '',
      [
        { value: 0, description: 'Regions with unprivileged write permission are not forced to XN' },
        { value: 1, description: 'Regions with unprivileged write permission are forced to XN for PL1 accesses' }
      ]
    ),
    f(
      21, 21, 'FI', 'Fast interrupts configuration enable', '',
      [
        { value: 0, description: 'All performance features enabled' },
        { value: 1, description: 'Low interrupt latency configuration. Some performance features disabled' }
      ]
    ),
    a(22, 23, 'RAO'),
    f(
      24, 24, 'VE', 'Interrupt Vectors Enable',
      'This bit controls the vectors used for the FIQ and IRQ interrupts',
      [
        { value: 0, description: 'Use the FIQ and IRQ vectors from the vector table, see the V bit entry' },
        { value: 1, description: 'Use the IMPLEMENTATION DEFINED values for the FIQ and IRQ vectors' }
      ]
    ),
    f(
      25, 25, 'EE', 'Exception Endianness',
      'This bit defines the value of the CPSR.E bit on entry to an exception vector',
      [
        { value: 0, description: 'Little-endian' },
        { value: 1, description: 'Big-endian' }
      ]
    ),
    a(26, 26, 'RAZ'),
    f(
      27, 27, 'NMFI', 'Non-maskable FIQ (NMFI) support', '',
      [
        { value: 0, description: 'Software can mask FIQs by setting the CPSR.F bit to 1' },
        { value: 1, description: 'Software cannot set the CPSR.F bit to 1. This means software cannot mask FIQs' }
      ]
    ),
    f(
      28, 28, 'TRE', 'TEX remap enable', '',
      [
        { value: 0, description: 'TEX remap disabled. TEX[2:0] are used, with the C and B bits, to describe the memory region attributes' },
        { value: 1, description: 'TEX remap enabled. TEX[2:1] are reassigned for use as bits managed by the operating system. The TEX[0], C and B bits, with the MMU remap registers, describe the memory region attributes' }
      ]
    ),
    f(
      29, 29, 'AFE', 'Access flag enable', '',
      [
        { value: 0, description: 'In the translation table descriptors, AP[0] is an access permissions bit. The full range of access permissions is supported. No Access flag is implemented' },
        { value: 1, description: 'In the translation table descriptors, AP[0] is the Access flag. Only the simplified model for access permissions is supported' }
      ]
    ),
    f(
      30, 30, 'TE', 'Thumb Exception enable',
      'This bit controls whether exceptions are taken in ARM or Thumb state',
      [
        { value: 0, description: 'Exceptions, including reset, taken in ARM state' },
        { value: 1, description: 'Exceptions, including reset, taken in Thumb state' }
      ]
    ),
    a(31, 31, 'RAZ'),
  ],
};
