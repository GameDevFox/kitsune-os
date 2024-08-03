import { a, f } from "../field-utils";

export const id_mmfr3 = {
  args: [15,0,0,1,7],
  isReadable: true,
  isWriteable: false,
  fields: [
    f(
      0, 3, 'CM MVA', 'Cache maintain MVA', 'Indicates the supported cache maintenance operations by MVA, in an implementation with hierarchical caches.',
      [
        { value: 0b0000, description: 'None supported.' },
        { value: 0b0001, description: 'Supported hierarchical cache maintenance operations by MVA are: [TODO]' },
      ],
    ),
    f(
      4, 7, 'CM S/W', 'Cache maintain set/way', 'Indicates the supported cache maintenance operations by set/way, in an implementation with hierarchical caches.',
      [
        { value: 0b0000, description: 'None supported.' },
        { value: 0b0001, description: 'Supported hierarchical cache maintenance operations by set/way are:' },
      ],
    ),
    f(
      8, 11, 'BP', 'BP maintain', 'Indicates the supported branch predictor maintenance operations in an implementation with hierarchical cache maintenance operations.',
      [
        { value: 0b0000, description: 'None supported.' },
        { value: 0b0001, description: 'Supported branch predictor maintenance operations are: [Invalidate all branch predictors]' },
        { value: 0b0010, description: 'As for 0b0001, and adds: [Invalidate branch predictors by MVA]' },
      ],
    ),
    f(
      12, 15, 'Mntc Bdct', 'Maintenance broadcast', 'Indicates whether Cache, TLB and branch predictor operations are broadcast.',
      [
        { value: 0b0000, description: 'Cache, TLB and branch predictor operations only affect local structures.' },
        { value: 0b0001, description: 'Cache and branch predictor operations affect structures according to shareability and defined behavior of instructions. TLB operations only affect local structures.' },
        { value: 0b0010, description: 'Cache, TLB and branch predictor operations affect structures according to shareability and defined behavior of instructions' },
      ],
    ),
    a(16, 19, 'UNK'),
    f(
      20, 23, 'Coh Walk', 'Coherent walk', 'Indicates whether translation table updates require a clean to the point of unification.',
      [
        { value: 0b0000, description: 'Updates to the translation tables require a clean to the point of unification to ensure visibility by subsequent translation table walks.' },
        { value: 0b0001, description: 'Updates to the translation tables do not require a clean to the point of unification to ensure visibility by subsequent translation table walks.' },
      ],
    ),
    f(
      24, 27, 'Cache Mem', 'Cached memory size', 'Indicates the physical memory size supported by the processor caches.',
      [
        { value: 0b0000, description: '4GBbyte, corresponding to a 32-bit physical address range.' },
        { value: 0b0001, description: '64GBbyte, corresponding to a 36-bit physical address range.' },
        { value: 0b0010, description: '1TBbyte, corresponding to a 40-bit physical address range.' },
      ],
    ),
    f(
      28, 31, 'Supersect', 'Supersection support', 'On a VMSA implementation, indicates whether Supersections are supported.',
      [
        { value: 0b0000, description: 'Supersections supported.' },
        { value: 0b1111, description: 'Supersections not supported.' },
      ],
    ),
  ],
};
