import _ from "lodash";

const hexToBinMap: Record<string, number[]> = {
  '0': [0, 0, 0, 0], '1': [0, 0, 0, 1], '2': [0, 0, 1, 0], '3': [0, 0, 1, 1],
  '4': [0, 1, 0, 0], '5': [0, 1, 0, 1], '6': [0, 1, 1, 0], '7': [0, 1, 1, 1],
  '8': [1, 0, 0, 0], '9': [1, 0, 0, 1], 'a': [1, 0, 1, 0], 'b': [1, 0, 1, 1],
  'c': [1, 1, 0, 0], 'd': [1, 1, 0, 1], 'e': [1, 1, 1, 0], 'f': [1, 1, 1, 1],
};

const binToHexMap: Record<string, string> = {
  '0000': '0', '0001': '1', '0010': '2', '0011': '3',
  '0100': '4', '0101': '5', '0110': '6', '0111': '7',
  '1000': '8', '1001': '9', '1010': 'a', '1011': 'b',
  '1100': 'c', '1101': 'd', '1110': 'e', '1111': 'f',
};

export const getByteIndex = (bitIndex: number) => Math.floor(bitIndex / 8);
export const getByteSubIndex = (bitIndex: number) => bitIndex % 8;

export const getBit = (bytes: number[], bitIndex: number) => {
  const byteIndex = getByteIndex(bitIndex);
  const byte = bytes[byteIndex];
  const mask = 1 << getByteSubIndex(bitIndex);

  return byte & mask ? 1 : 0;
};

export const setBit = (bytes: number[], bitIndex: number, value: number) => {
  const result = [...bytes];

  const byteIndex = getByteIndex(bitIndex);
  let byte = result[byteIndex];
  const mask = 1 << getByteSubIndex(bitIndex);

  // Update bit
  byte = value ? byte | mask : byte & ~mask;

  // Update byte
  result.splice(byteIndex, 1, byte);

  return result;
};

export const hexToBin = (hexStr: string) => {
  const bitArrays = hexStr.split('')
    .map(char => {
      if(!(char in hexToBinMap))
        throw new Error(`The character ${char} is not a hex character (0-9 a-f)`);

      return hexToBinMap[char];
    });

  return Array.prototype.concat(...bitArrays);
};

export const bytesToBin = (bytes: number[]) => {
  const result: number[] = [];

  bytes.forEach(byte => {
    [0, 1, 2, 3, 4, 5, 6, 7].forEach(bit => {
      result.push(byte & 1 << bit ? 1 : 0);
    });
  });

  return result;
};

export const bytesToBinStr = (bytes: number[]) => {
  const strings = bytesToBin(bytes).map(bit => bit.toString())
  strings.reverse();
  return strings.join('');
};

export const bytesToStr = (bytes: number[]) => {
  return bytes
    .map(number => _.padStart(number.toString(16), 2, '0'))
    .reverse()
    .join('');
};

export const fromBinary = (binaryStr: string) => Number(`0b${binaryStr}`);

export const bitsToBinStr = (bits: number[]) => {
  if(bits.length !== 32)
    throw new Error('');

  const chars = []

  for(let i=0; i<32; i += 4) {
    const bitStr = bits.slice(i, i+4).join('');
    chars.push(binToHexMap[bitStr]);
  }

  return chars.join('');
};

export const getValueByBits = (bytes: number[], startBit: number, length: number) => {
  let result = 0;

  const bits = bytesToBin(bytes);

  for(let i = 0; i < length; i++) {
    if(bits[i + startBit])
      result += 1 << i;
  }

  return result;
};
