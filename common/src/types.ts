export interface Value {
  value: number;
  description: string;
};

export type AltCode =
  'RAO' |   // Read As One
  'RAZ' |   // Read As Zero
  'SBO' |   // Should Be One
  'SBOP' |  // Should Be One or Preserved
  'SBZ' |   // Should Be Zero
  'SBZP' |  // Should Be Zero or Preserved
  'UNK' |   // Unknown
  'WI';     // Writes Ignored

export interface Field {
  startBit: number;
  length: number;

  alt?: AltCode;

  code?: string;
  name?: string;
  description?: string;
  values?: Value[];
}

export interface CoprocRegister {
  args: number[];
  isReadable: boolean;
  isWriteable: boolean;
  fields?: Field[];
}
