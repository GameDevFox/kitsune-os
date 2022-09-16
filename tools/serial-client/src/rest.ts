import { FileHandle, writeFile } from 'fs/promises';

import cors from 'cors';
import express from 'express';

import { defaultOutHandler, OutHandler, setOutHandler } from './output-handler';

const ReadCommand = (start: number, length: number) => {
  const buf = Buffer.alloc(9);
  buf.write("R");
  buf.writeUIntLE(start, 1, 4); // Start
  buf.writeUIntLE(length, 5, 4); // Length

  return buf;
};

// TODO: What do we do with the extra bytes???
const ReadBytesHandler = (length: number, fn: (buffer: Buffer) => void) => {
  let index = 0;
  const buffer = Buffer.alloc(length);

  const handler: OutHandler = data => {
    const { bytesRead } = data;

    data.buffer.copy(buffer, index, 0, bytesRead);
    index += data.bytesRead

    if(index >= length)
      fn(buffer);
  };

  return handler;
};

interface Request {
  start: number,
  length: number,
  fn: (buffer: Buffer) => void,
}

const requests: Request[] = [];

export const buildRestApp = (inFile: FileHandle) => {

  const addRequest = (request: Request) => {
    requests.push(request);

    if(requests.length > 1)
      return;

    handleRequests();
  };

  const handleRequests = () => {
    const request = requests[0];
    const { start, length, fn } = request;

    const handler = ReadBytesHandler(length, data => {
      fn(data);

      // Remove to mark as done
      requests.shift();

      if(requests.length) {
        // Do the next one
        handleRequests();
      } else {
        // We're done... restore the default handler
        setOutHandler(defaultOutHandler);
      }
    });
    setOutHandler(handler);

    const buf = ReadCommand(start, length);
    inFile.write(buf);

    requests.shift();
  };

  const app = express();

  app.use(cors());

  app.get("/hello", (req, res) => {
    inFile.write("1");
    res.send({ done: true });
  });

  app.get("/clear", (req, res) => {
    inFile.write("0");
    res.send({ done: true });
  });

  app.get("/draw", (req, res) => {
    inFile.write("789");
    res.send({ done: true });
  });

  app.get("/memory/:start/length/:length", (req, res) => {
    const start = Number(req.params.start);
    const length = Number(req.params.length);

    addRequest({
      start, length,
      fn: data => {
        res.send({ done: true, data });
      }
    });
  });

  return app;
};
