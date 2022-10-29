import { ByteReader } from "../types";

import { mainReader } from "./mainReader";

export const LengthReader = (): ByteReader => {
  const bytes: number[] = [];

  return (byte, frame) => {
    bytes.push(byte);

    if(bytes.length === 4) {
      let length = 0;
      length += bytes[0] << 0;
      length += bytes[1] << 8;
      length += bytes[2] << 16;
      length += bytes[3] << 24;

      frame.givenLength = length;

      frame.byteReader = mainReader;
    }
  };
};
