#!/usr/bin/env ts-node

import { spawn } from "child_process";
import { open } from "fs/promises";
import { exit } from "process";
import { Server, Socket } from "net";

import { startStandaloneServer } from "@apollo/server/standalone";

import { Api } from "./api";
import { build as buildGraphQLServer } from "./graphql";
import { Frame, Handler } from "./output-handler";
import { Request } from "./request";
import { buildRestApp } from "./rest";

const restPort = 8080;

let frameHandlerFn = (frame: Frame) => {};
export const setFrameHandler = (fn: (frame: Frame) => void) => frameHandlerFn = fn;

export const handler = Handler(frame => {
  frameHandlerFn(frame);
});

const tcpMode = () => {
  const server = new Server();

  server.on('connection', async (socket: Socket) => {
    const write = (data: Uint8Array) => socket.write(data);
    const request = Request(write);

    const api = Api(write, request);

    // REST
    const restApp = buildRestApp(request, api);
    const restServer = restApp.listen(restPort);
    console.log(`REST app listening on ${restPort} ...`);

    // GraphQL
    const graphQLServer = buildGraphQLServer(api);
    const { url } = await startStandaloneServer(graphQLServer, {
      listen: { port: 4000 },
    });
    console.log(`GraphQL Server ready at: ${url}`);
    console.log();

    socket.on('data', handler);
    socket.on('close', async () => {
      await graphQLServer.stop();
      restServer.close();
    });
  });

  server.listen(8081);
};

const charDeviceMode = async (charDev: string) => {
  // Set baud rate to 115200
  const process = spawn(`stty`, [
    '--file', charDev,
    '115200', '-icrnl', '-opost', '-isig',
    '-icanon', '-iexten', '-echo',
  ]);

  process.on('close', code => {
    if(code)
      console.warn('Warning: Failed to set baud rate on serial device');

    open(charDev, "r").then(async outFile => {
      while(true) {
        const { buffer, bytesRead } = await outFile.read();

        const data = Buffer.alloc(bytesRead);
        buffer.copy(data, 0, 0, bytesRead);

        handler(data);
      }
    });

    open(charDev, "w").then(async inFile => {
      const write = (data: Uint8Array) => inFile.write(data);
      const request = Request(write);

      const api = Api(write, request);

      // REST
      const app = buildRestApp(request, api);
      app.listen(restPort);
      console.log(`REST app listening on ${restPort} ...`);

      // GraphQL
      const graphQLServer = buildGraphQLServer(api);
      const { url } = await startStandaloneServer(graphQLServer, {
        listen: { port: 4000 },
      });
      console.log(`GraphQL Server ready at: ${url}`);
      console.log();
    });
  });
};

if(process.argv.length < 3) {
  tcpMode();
} else if(process.argv.length === 3) {
  const charDev = process.argv[2]
  charDeviceMode(charDev);
} else {
  console.error("Usage: serial-client [char-device]");
  exit(1);
}
