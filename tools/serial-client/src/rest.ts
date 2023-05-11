import { json } from 'body-parser';
import cors from 'cors';
import express from 'express';

import { coprocRegisterCodes } from '@kitsune-os/common';

import { Api } from './api';
import { CPSRCommand } from './commands';
import { loadSymbols } from './kernel-symbols';
import { Request } from './request';

export const buildRestApp = (
  request: Request,
  api: Api,
) => {
  const app = express();

  app.use(cors());
  app.use(json({ strict: false }));

  app.get("/hello", (req, res) => {
    api.hello();
    res.send({ success: true });
  });

  app.get("/clear", (req, res) => {
    api.clear();
    res.send({ success: true });
  });

  app.get("/draw/:name", (req, res) => {
    const { name } = req.params;
    api.draw(name as any);

    res.send({ success: true });
  });

  app.get("/memory/:start/length/:length", (req, res) => {
    const start = Number(req.params.start);
    const length = Number(req.params.length);

    api.readMemory(start, length, data => {
      res.send({ success: true, data });
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
    api.printDeviceTree();
    res.send({ success: true });
  });

  app.get("/timer", (req, res) => {
    api.printTimer();
    res.send({ success: true });
  });

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
      fn: () => res.send({ success: true }),
    });
  });

  app.post('/color', (req, res) => {
    const { color } = req.body;
    api.setColor(color);
    res.send({ success: true })
  });

  app.get('/kernel-symbol', async (req, res) => {
    const symbols = await loadSymbols();
    res.send({ success: true, symbols });
  });

  return app;
};
