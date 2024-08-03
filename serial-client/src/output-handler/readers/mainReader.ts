import { ESCAPE } from "../output-handler";
import { ByteReader } from "../types";

import { escapeReader } from "./escapeReader";

export const mainReader: ByteReader = (byte, frame) => {
  // console.log(`MAIN READER: ${byte.toString(16)}`);

  if(byte === ESCAPE) {
    frame.byteReader = escapeReader;
  } else {
    // TODO: Pass the byte to the previous ByteReader instead
    frame.bytes.push(byte);

    if(frame.givenLength && frame.bytes.length > frame.givenLength) {
      console.warn(
        '\nWARN: Bytes have overflowed length: ' +
          `${frame.bytes.length} > ${frame.givenLength}`
      );

      frame.done();
    }
  }
};
