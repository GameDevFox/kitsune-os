import { AltCode, Field, Value } from "./types";

// AltCode
export const a = (startBit: number, endBit: number, alt: AltCode): Field => {
  if(endBit < startBit)
    throw new Error(`startBit (${startBit}) must be less or equal to endBit (${endBit})`);

  const length = endBit - startBit + 1;
  return { startBit, length, alt };
};

// Field
export const f = (
  startBit: number, endBit: number,
  code: string, name: string, description: string,
  values: Value[] = [],
): Field => {
  if(endBit < startBit)
    throw new Error(`startBit (${startBit}) must be less or equal to endBit (${endBit})`);

  const length = endBit - startBit + 1;
  return { startBit, length, code, name, description, values };
};
