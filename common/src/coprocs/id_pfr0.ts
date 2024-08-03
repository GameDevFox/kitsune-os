import { fromBinary } from "../binary";
import { a, f } from "../field-utils";

export const id_pfr0 = {
  args: [15,0,0,1,0],
  isReadable: true,
  isWriteable: false,
  fields: [
    f(
      0, 3, 'State0', 'ARM instruction set support', '',
      [
        { value: fromBinary('0000'), description: 'ARM instruction set not implemented' },
        { value: fromBinary('0001'), description: 'ARM instruction set implemented' },
      ]
    ),
    f(
      4, 7, 'State1', 'Thumb instruction set support', '',
      [
        { value: fromBinary('0000'), description: 'Thumb instruction set not implemented' },
        { value: fromBinary('0001'), description: 'Thumb encodings before the introduction of Thumb-2 technology implemented' },
        { value: fromBinary('0010'), description: 'Reserved' },
        { value: fromBinary('0011'), description: 'Thumb encodings after the introduction of Thumb-2 technology implemented, for all ' +
          '16-bit and 32-bit Thumb basic instructions' },
      ]
    ),
    f(
      8, 11, 'State2', 'Jazelle extension support', '',
      [
        { value: fromBinary('0000'), description: 'Not implemented' },
        { value: fromBinary('0001'), description: 'Jazelle extension implemented, without clearing of JOSCR.CV on exception entry' },
        { value: fromBinary('0010'), description: 'Jazelle extension implemented, with clearing of JOSCR.CV on exception entry' },
      ]
    ),
    f(
      12, 15, 'State3', 'ThumbEE instruction set support', '',
      [
        { value: fromBinary('0000'), description: 'Not implemented' },
        { value: fromBinary('0001'), description: 'ThumbEE instruction set implemented' },
      ]
    ),
    a(16, 31, 'RAZ'),
  ],
};
