import { stdout } from "process";

const stdOutHandler = (data: Buffer) => {
    stdout.write(data);
};
export type OutHandler = typeof stdOutHandler;

export const defaultOutHandler = stdOutHandler;

let outHandlerFn = stdOutHandler;

export const outHandler = (data: Buffer) => outHandlerFn(data);
export const setOutHandler = (fn: OutHandler) => outHandlerFn = fn;
