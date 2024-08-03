import { stdout } from "process";

import { setFrameHandler } from ".";
import { BufferReader } from "./output-handler";

export interface RequestEntry {
  command: (targetId: number) => Buffer;
  fn: (buffer: Buffer) => void;
};

// TODO: What do we do with the extra bytes???
export const ReadBytesHandler = (length: number, fn: (data: Buffer) => void) => {
  let index = 0;
  const result = Buffer.alloc(length);

  const handler: BufferReader = data => {
    const byteCount = data.length;

    data.copy(result, index, 0, byteCount);
    index += byteCount;

    if(index >= length)
      fn(result);
  };

  return handler;
};

export const Request = (write: (data: Uint8Array) => void) => {
  const newRequests: Record<string, RequestEntry> = {};

  setFrameHandler(frame => {
    if(frame.target === 0) {
      const message = Buffer.from(frame.bytes).toString();
      stdout.write(message);
      return;
    }

    const request = newRequests[frame.target];
    if(!request) {
      console.error(`No such request for target id: ${frame.target}`);
      return;
    }

    request.fn(Buffer.from(frame.bytes));

    delete newRequests[frame.target];
  });

  return (request: RequestEntry) => {
    // New implementation
    let targetId = 0;
    for(let i=1; i<128; i++) {
      if(i in newRequests)
        continue;

      targetId = i;
      break;
    }

    if(targetId === 0)
      throw new Error(`Ran out of target ids`);

    newRequests[targetId] = request;

    const buffer = request.command(targetId);
    write(buffer);
  };
};

export type Request = ReturnType<typeof Request>;
