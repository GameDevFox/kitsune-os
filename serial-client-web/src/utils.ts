const nonPrintCharEmojis: Record<number, string> = {
  0x00: '⎕', // Null
  0x0a: '↓', // Line feed
  0x0d: '↩', // Carridage Return
  0x20: '˽', // Space
  0xff: '∎',
};

export const cleanHex = (hex: string) => hex.replace(/[^0-9A-Fa-f]/g, '').trim();

export const formatHex = (hex: string) => cleanHex(hex).replace(/..../g, val => `${val} `).trim();

export const hexToBytes = (hex: string) => {
  if(hex.trim() === "")
    return [];

  const match = hex.match(/..?/g);
  if(!match)
    throw new Error("Cannot convert hex to bytes");

  return match.map(str => parseInt(str, 16));
}

export const toAscii = (value: number) => {
  if(nonPrintCharEmojis[value]) {
    return nonPrintCharEmojis[value];
  } else if(0x20 <= value && value < 0x80) {
    return String.fromCharCode(value);
  } else {
    return null;
  }
}

export const toHex = (value: number) => value.toString(16).padStart(2, '0');

export const strToBytes = (str: string) => Array.from(new TextEncoder().encode(str));

export const bytesToHex = (bytes: number[]) => bytes.map(number => number.toString(16).padStart(2, '0')).join('');
export const bytesToStr = (bytes: number[]) => new TextDecoder().decode(new Uint8Array(bytes));
