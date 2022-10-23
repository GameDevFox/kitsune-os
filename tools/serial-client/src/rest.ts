import { json } from 'body-parser';
import cors from 'cors';
import express from 'express';

import { coprocRegisterCodes } from '@kitsune-os/common';

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
  command: Buffer;
  bytesToRead: number;
  fn: (buffer: Buffer) => void;
}

const requests: Request[] = [];

export const buildRestApp = (write: (data: Uint8Array) => void) => {

  const addRequest = (request: Request) => {
    requests.push(request);

    if(requests.length > 1)
      return;

    handleRequests();
  };

  const handleRequests = async () => {
    const request = requests[0];
    const { command, bytesToRead, fn } = request;

    if(bytesToRead) {
      const handler = ReadBytesHandler(bytesToRead, async data => {
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
    }
;
    await write(command);

    requests.shift();
  };

  const app = express();

  app.use(cors());
  app.use(json({ strict: false }));

  app.get("/hello", (req, res) => {
    write(Buffer.from("1"));
    res.send({ success: true });
  });

  app.get("/clear", (req, res) => {
    write(Buffer.from("0"));
    res.send({ success: true });
  });

  app.get("/draw/:name", (req, res) => {
    const { name } = req.params;

    switch(name) {
      case 'curve':
        write(Buffer.from("7"));
        break;
      case 'mascot':
        write(Buffer.from("8"));
        break;
      case 'logo':
        write(Buffer.from("9"));
        break;
      default:
        console.error(`No such object to draw: ${name}`);
    }

    res.send({ success: true });
  });

  app.get("/memory/:start/length/:length", (req, res) => {
    const start = Number(req.params.start);
    const length = Number(req.params.length);

    addRequest({
      command: ReadCommand(start, length),
      bytesToRead: length,
      fn: data => {
        res.send({ success: true, data });
      }
    });
  });

  app.get("/coproc-registers", (req, res) => {
    res.send({ success: true, registers: coprocRegisterCodes });
  });

  app.get("/coproc-registers/:name", (req, res) => {
    const { name } = req.params;

    if(!(name in coprocRegisterCodes)) {
      res.sendStatus(404);
      return;
    }

    const { args, isReadable } = coprocRegisterCodes[name];
    if(!isReadable) {
      res.status(400).send({
        success: false,
        message: `Coproc register is not readable: ${name}`
      });
      return;
    }

    const argStr = args.map(number => number.toString(16)).join('');
    const command =  Buffer.from('`' + argStr);

    addRequest({
      command,
      bytesToRead: 8,
      fn: data => {
        let dataStr: any = data.toString();

        const buffer = Buffer.alloc(4);
        buffer.writeUint32LE(parseInt(`0x${dataStr}`));

        res.send({ success: true, data: buffer });
      }
    });
  });

  app.post("/coproc-registers/:name", (req, res) => {
    const { name } = req.params;
    const { value } = req.body;

    if(!(name in coprocRegisterCodes)) {
      res.sendStatus(404);
      return;
    }

    const { args, isWriteable } = coprocRegisterCodes[name];
    if(!isWriteable) {
      res.status(400).send({
        success: false,
        message: `Coproc register is not writeable: ${name}`
      });
      return;
    }

    const argStr = args.map(number => number.toString(16)).join('');
    const command =  Buffer.from('~' + argStr + value);

    addRequest({
      command,
      bytesToRead: 0,
      fn: () => { // Ignore response for now
        res.send({ success: true });
      }
    });
  });

  app.get("/print-device-tree", (req, res) => {
    write(Buffer.from("d"));
    res.send({ success: true });
  });

  return app;
};
