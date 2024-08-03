import { ByteReader } from "../types";

import { mainReader } from "./mainReader";

export const targetReader: ByteReader = (byte, frame) => {
  frame.target = byte;
  frame.byteReader = mainReader;
};
