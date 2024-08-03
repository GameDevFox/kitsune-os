export interface Frame {
  target: number;
  givenLength?: number;

  bytes: number[];

  byteReader: ByteReader;
  done: () => void;
};

export type ByteReader = (byte: number, frame: Frame) => void;
export type BufferReader = (buffer: Buffer) => void;
