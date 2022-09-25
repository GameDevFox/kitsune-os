import cors from 'cors';
import express from 'express';
import { Socket } from 'net';

import { defaultOutHandler, OutHandler, setOutHandler } from './output-handler';

const ReadCommand = (start: number, length: number) => {
  const buf = Buffer.alloc(9);
  buf.write("R");
  buf.writeUIntLE(start, 1, 4); // Start
  buf.writeUIntLE(length, 5, 4); // Length

  return buf;
};

// TODO: What do we do with the extra bytes???
const ReadBytesHandler = (length: number, fn: (data: Buffer) => void) => {
  let index = 0;
  const result = Buffer.alloc(length);

  const handler: OutHandler = data => {
    const byteCount = data.length;

    data.copy(result, index, 0, byteCount);
    index += byteCount;

    if(index >= length)
      fn(result);
  };

  return handler;
};

interface Request {
  start: number,
  length: number,
  fn: (buffer: Buffer) => void,
}

const requests: Request[] = [];

export const buildRestApp = (socket: Socket) => {

  const addRequest = (request: Request) => {
    requests.push(request);

    if(requests.length > 1)
      return;

    handleRequests();
  };

  const handleRequests = async () => {
    const request = requests[0];
    const { start, length, fn } = request;

    const handler = ReadBytesHandler(length, async data => {
      fn(data);

      // Remove to mark as done
      requests.shift();

      if(requests.length) {
        // Do the next one
        await handleRequests();
      } else {
        // We're done... restore the default handler
        setOutHandler(defaultOutHandler);
      }
    });
    setOutHandler(handler);

    const buf = ReadCommand(start, length);

    await socket.write(buf);

    requests.shift();
  };

  const app = express();

  app.use(cors());

  app.get("/hello", (req, res) => {
    socket.write("1");
    res.send({ done: true });
  });

  app.get("/clear", (req, res) => {
    socket.write("0");
    res.send({ done: true });
  });

  app.get("/draw", (req, res) => {
    socket.write("789");
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
