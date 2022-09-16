#!/usr/bin/env ts-node

import { open } from "fs/promises";

import { outHandler } from "./output-handler";
import { buildRestApp } from "./rest";

const args = process.argv.slice(2);
const [inPath, outPath] = args;

open(outPath, "r").then(async outFile => {
    while(true) {
        const data = await outFile.read();
        outHandler(data);
    }
});

open(inPath, "w").then(inFile => {
    const app = buildRestApp(inFile);
    app.listen(8080);
});


