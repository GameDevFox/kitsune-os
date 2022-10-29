import { stdout } from "process";
import { mainReader } from "./readers/mainReader";

import { Frame } from "./types";

export const START = 0xff;
export const ESCAPE = 0xfe;
  export const LITERAL_START = 0x00;
  export const END           = 0x01;
  export const TARGET        = 0x02;
  export const LENGTH        = 0x03;

const Frame = (done: () => void): Frame => ({
  target: 0,

  bytes: [],

  byteReader: mainReader,
  done,
});

export const Handler = (fn: (frame: Frame) => void) => {
  const frames: Frame[] = [];

  const handler = (data: Buffer) => {
    data.forEach(byte => {
      const frame = frames.length > 0 ?
      frames[frames.length - 1] : // latest frame
      undefined;

      // START ALWAYS creates a new frame
      if(byte === START) {
        const newFrame = Frame(() => {
          const frame = frames.pop();
          if(!frame)
            throw new Error("Can't pop from empty frame stack");

          if(frame.givenLength && frame.bytes.length < frame.givenLength) {
            console.warn(
              '\nWARN: Buffer is shorter than given length: ' +
                `${frame.bytes.length} < ${frame.givenLength}`
            );
          }

          fn(frame);
        });

        frames.push(newFrame);
        return;
      }

      if(!frame) {
        stdout.write(Buffer.from([byte]));
        return;
      }

      frame.byteReader(byte, frame);
    });
  };

  return handler;
};
