#!/usr/bin/env ts-node

import { open } from "fs/promises";
import { exit } from "process";
import { Server, Socket } from "net";

import { outHandler } from "./output-handler";
import { buildRestApp } from "./rest";

const tcpMode = () => {
    const server = new Server();

    server.on('connection', (socket: Socket) => {
        const app = buildRestApp(data => socket.write(data));
        const server = app.listen(8080);

        socket.on('data', outHandler);
        socket.on('close', () => server.close());
    });

    server.listen(8081);
};

const charDeviceMode = (charDev: string) => {
    open(charDev, "r").then(async outFile => {
        while(true) {
            const { buffer, bytesRead } = await outFile.read();

            const data = Buffer.alloc(bytesRead);
            buffer.copy(data, 0, 0, bytesRead);
            outHandler(data);
        }
    });

    open(charDev, "w").then(inFile => {
        const app = buildRestApp(data => inFile.write(data));
        app.listen(8080);
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
