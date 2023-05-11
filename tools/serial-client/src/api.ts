import { Color, ReadCommand, SetColorBuffer } from "./commands";
import { Request } from './request';

export const images = {
  curve: '7',
  mascot: '8',
  'mascot-no-glasses': '8g',
  logo: '9',
  'kitsune-text': 'r',
};

export type ImageNames = keyof typeof images;

export const Api = (
  write: (data: Uint8Array) => void,
  request: Request,
) => {
  const clear = () => write(Buffer.from("0"));
  const hello = () => write(Buffer.from("1"));

  const draw = (name: ImageNames) => {
    if(!(name in images))
      throw `No such object to draw: ${name}`;

    const toWrite = images[name];
    write(Buffer.from(toWrite));
  };

  const printDeviceTree = () => write(Buffer.from("d"));
  const printTimer = () => write(Buffer.from("t"));

  const readMemory = (
    start: number, length: number, fn: (data: Buffer) => void
  ) => {
    request({
      command: targetId => ReadCommand(targetId, start, length),
      fn,
    });
  };

  const setColor = (color: Color) => {
    const buffer = SetColorBuffer(color);
    write(buffer);
  }

  return {
    clear, draw, hello,
    printDeviceTree, printTimer,
    readMemory,
    setColor,
  };
};

export type Api = ReturnType<typeof Api>;
