import { json } from 'body-parser';
import cors from 'cors';
import express from 'express';

import { coprocRegisterCodes } from '@kitsune-os/common';

import { Request } from './request';
import { loadSymbols } from './kernel-symbols';

const ReadCommand = (targetId: number, start: number, length: number) => {
  const buf = Buffer.alloc(10);
  buf.write("R");
  buf.writeUIntLE(targetId, 1, 1);
  buf.writeUIntLE(start, 2, 4); // Start
  buf.writeUIntLE(length, 6, 4); // Length

  return buf;
};

export const buildRestApp = (write: (data: Uint8Array) => void,) => {
  const request = Request(write);

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
      case 'mascot-no-glasses':
        write(Buffer.from("8g"));
        break;
      case 'logo':
        write(Buffer.from("9"));
        break;
      case 'kitsune-text':
        write(Buffer.from("r"));
        break;
      default:
        console.error(`No such object to draw: ${name}`);
    }

    res.send({ success: true });
  });

  app.get("/memory/:start/length/:length", (req, res) => {
    const start = Number(req.params.start);
    const length = Number(req.params.length);

    request({
      command: targetId => ReadCommand(targetId, start, length),
      fn: data => res.send({ success: true, data }),
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
    const command = (targetId: number) => {
      const buffer = Buffer.alloc(2 + argStr.length);
      buffer.write('`', 0);
      buffer.writeUIntLE(targetId, 1, 1);
      buffer.write(argStr, 2);
      return buffer;
    };

    request({
      command,
      fn: data => res.send({ success: true, data }),
    });
  });

  app.post("/coproc-registers/:name", async (req, res) => {
    const { name } = req.params;
    const { value } = req.body;

    const valueNum = parseInt(value, 16);

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

    // TODO: Find a better way to write command buffers
    const command = (targetId: number) => {
      const buffer = Buffer.alloc(1 + 1 + argStr.length + 4);

      let index = 0;
      buffer.write('~', index);  // 0
      buffer.writeUIntLE(targetId, index += 1, 1); // 1
      buffer.write(argStr, index += 1); // 4
      buffer.writeUIntLE(valueNum, index += argStr.length, 4);
      return buffer;
    };

    request({
      command,
      fn: data => res.send({ success: true, data }),
    });
  });

  app.get("/print-device-tree", (req, res) => {
    write(Buffer.from("d"));
    res.send({ success: true });
  });

  app.get("/timer", (req, res) => {
    write(Buffer.from("t"));
    res.send({ success: true });
  });

  const CPSRCommand = (value?: number[]) => {
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

  app.get('/cpsr', (req, res) => {
    request({
      command: CPSRCommand(),
      fn: data => res.send({ success: true, data }),
    });
  });

  app.post('/cpsr', (req, res) => {
    const { value } = req.body;

    request({
      command: CPSRCommand(value),
      fn: data => res.send({ success: true }),
    });
  });

  app.post('/color', (req, res) => {
    const { color } = req.body;

    const buffer = Buffer.alloc(5);
    buffer.write('e', 0);
    buffer[1] = color.b;
    buffer[2] = color.g;
    buffer[3] = color.r;
    buffer[4] = 0xff;

    write(buffer);

    res.send({ success: true })
  });

  app.get('/kernel-symbol', async (req, res) => {
    const symbols = await loadSymbols();
    res.send({ success: true, symbols });
  });

  return app;
};
