#!/usr/bin/env ts-node

import { spawn } from "child_process";
import { open } from "fs/promises";
import { exit } from "process";
import { Server, Socket } from "net";

import { outHandler } from "./output-handler";
import { buildRestApp } from "./rest";

const restPort = 8080;

const tcpMode = () => {
    const server = new Server();

    server.on('connection', (socket: Socket) => {
        const app = buildRestApp(data => socket.write(data));
        const server = app.listen(restPort);

        socket.on('data', outHandler);
        socket.on('close', () => server.close());

        console.log(`Listening on ${restPort} ...`);
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
                outHandler(data);
            }
        });

        open(charDev, "w").then(inFile => {
            const app = buildRestApp(data => inFile.write(data));
            app.listen(restPort);

            console.log(`Listening on ${restPort} ...`);
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
