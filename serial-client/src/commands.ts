export const CPSRCommand = (value?: number[]) => {
  return (targetId: number) => {
    const buffer = Buffer.alloc(value ? 7 : 3);

    buffer.write('3', 0);
    buffer.writeUInt8(targetId, 1);
    buffer.writeUInt8(value ? 1 : 0, 2);

    if(value) {
      buffer[3] = value[0];
      buffer[4] = value[1];
      buffer[5] = value[2];
      buffer[6] = value[3];
    }

    return buffer;
  };
};

export const ReadCommand = (
  targetId: number,
  start: number,
  length: number
) => {
  const buf = Buffer.alloc(10);

  buf.write("R");
  buf.writeUIntLE(targetId, 1, 1);
  buf.writeUIntLE(start, 2, 4); // Start
  buf.writeUIntLE(length, 6, 4); // Length

  return buf;
};

export const WriteCommand = (
  targetId: number,
  start: number,
  data: Buffer,
) => {
  const length = data.length;

  const buf = Buffer.alloc(10 + length);

  buf.write("W");
  buf.writeUIntLE(targetId, 1, 1);
  buf.writeUIntLE(start, 2, 4); // Start
  buf.writeUIntLE(length, 6, 4); // Length

  // TODO: Implement progress

  data.forEach((byte, index) => {
    buf.writeUint8(byte, 10 + index);
  });

  return buf;
};

export interface Color {
  r: number;
  g: number;
  b: number;
}

export const SetColorBuffer = (color: Color) => {
  const buffer = Buffer.alloc(5);

  buffer.write('e', 0);
  buffer[1] = color.b;
  buffer[2] = color.g;
  buffer[3] = color.r;
  buffer[4] = 0xff;

  return buffer;
};
