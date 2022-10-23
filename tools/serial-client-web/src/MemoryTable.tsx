import { useEffect, useState } from "react";

import { Table, Tbody, Td, Tr } from "@chakra-ui/react";

import { api } from "./api";

const ROW_LENGTH = 16;

const nonPrintCharEmojis: Record<number, string> = {
  0x00: '⎕', // Null
  0x0a: '↓', // Line feed
  0x0d: '↩', // Carridage Return
  0x20: '˽',
  0xff: '∎',
};

const toHex = (value: number) => value.toString(16).padStart(2, '0');
const toAscii = (value: number) => {
  if(nonPrintCharEmojis[value]) {
    return nonPrintCharEmojis[value];
  } else if(0x20 <= value && value < 0x80) {
    return String.fromCharCode(value);
  } else {
    return '★';
  }
}

interface MemoryTableProps {
  address: number;
};

export const MemoryTable = (props: MemoryTableProps) => {
  const { address } = props;

  const [memory, setMemory] = useState<number[]>([]);

  useEffect(() => {
    api.get(`/memory/${address}/length/0x100`)
      .then(res => {
        const { data } = res.data.data;
        setMemory(data);
      });
  }, [address]);

  const rows = [];
  for(let i=0; i<16; i++) {
    const rowBytes = memory.slice(i * ROW_LENGTH, (i + 1) * ROW_LENGTH);

    const hexCells = [0, 4, 8, 12].map(value => (
      <Td
        key={`cell${value}`} padding='1' paddingRight='4'
        color={value % 8 === 0 ? 'auto' : 'blue.500'}
      >
        <pre>
          {
            rowBytes
              .slice(value, value + 4)
              .map(toHex).join(' ')
          }
        </pre>
      </Td>
    ));

    const textCells = [0, 4, 8, 12].map(value => (
      <Td
        key={`cell${value}`} padding='1'
        color={value % 8 === 0 ? 'auto' : 'blue.500'}
      >
        <pre>
          {
            rowBytes
              .slice(value, value + 4)
              .map(toAscii)
          }
        </pre>
      </Td>
    ));

    rows.push(
      <Tr key={`row${i}`}>
        <Td padding='1' paddingRight='4'>{((i * 16) + address).toString(16)}</Td>
        {hexCells}
        {textCells}
      </Tr>
    );
  }

  return (
    <Table variant="striped">
      <Tbody>{rows}</Tbody>
    </Table>
  );
};
