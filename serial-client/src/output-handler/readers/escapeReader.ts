import { END, ESCAPE, LENGTH, LITERAL_START, START, TARGET } from "../output-handler";
import { ByteReader } from "../types";

import { LengthReader } from "./lengthReader";
import { mainReader } from "./mainReader";
import { targetReader } from "./targetReader";

export const escapeReader: ByteReader = (byte, frame) => {
  switch(byte) {
    case LITERAL_START:
      frame.bytes.push(START);
      frame.byteReader = mainReader;
      break;
    case ESCAPE: // Literal escape character
      frame.bytes.push(ESCAPE);
      frame.byteReader = mainReader;
      break;
    case END:
      frame.done();
      break;
    case TARGET:
      if(byte > 127) {
        console.warn(`Target numbers above 127 not yet supported: 0x${byte.toString(16)}`, frame);
        return;
      }

      frame.byteReader = targetReader;
      break;
    case LENGTH:
      frame.byteReader = LengthReader();
      break;
    default:
      console.warn(`Invalid esacpe byte: 0x${byte.toString(16)}`);
      frame.done();
      break;
  }
};
