import { FileReadResult } from "fs/promises";
import { stdout } from "process";

const stdOutHandler = (data: FileReadResult<Buffer>) => {
    stdout.write(data.buffer);
};
export type OutHandler = typeof stdOutHandler;

export const defaultOutHandler = stdOutHandler;

let outHandlerFn = stdOutHandler;

export const outHandler = (data: FileReadResult<Buffer>) => outHandlerFn(data);
export const setOutHandler = (fn: OutHandler) => outHandlerFn = fn;
