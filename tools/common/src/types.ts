export interface Value {
  value: number;
  description: string;
};

export type AltCode = 'RAO' | 'RAZ' | 'SBO' | 'SBOP' | 'SBZP' | 'SBZ' | 'UNK' | 'WI';

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
