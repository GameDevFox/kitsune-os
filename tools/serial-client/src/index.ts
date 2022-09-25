#!/usr/bin/env ts-node

import { Server, Socket } from "net";

import { outHandler } from "./output-handler";
import { buildRestApp } from "./rest";

const server = new Server();

server.on('connection', (socket: Socket) => {
    const app = buildRestApp(socket);
    const server = app.listen(8080);

    socket.on('data', outHandler);
    socket.on('close', () => server.close());
});

server.listen(8081);
