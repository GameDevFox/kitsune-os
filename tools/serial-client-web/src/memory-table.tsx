import { useEffect, useState } from "react";

import { Table, Tbody, Td, Tr } from "@chakra-ui/react";

import { api } from "./api";

const ROW_LENGTH = 16;

const toHex = (value: number) => value.toString(16).padStart(2, '0');

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
    const cells = [0, 4, 8, 12].map(value => (
      <Td
        key={`cell${value}`} padding='1'
        color={value % 8 === 0 ? 'white' : 'grey'}
      >
        <pre>
          {
            memory
              .slice((i * ROW_LENGTH) + value, (i * ROW_LENGTH) + value + 4)
              .map(toHex).join(' ')
          }
        </pre>
      </Td>
    ));

    rows.push(
      <Tr key={`row${i}`}>
        <Td padding='1' paddingRight='4'>{((i * 16) + address).toString(16)}</Td>
        {cells}
      </Tr>
    );
  }

  return (
    <Table variant="striped">
      <Tbody>{rows}</Tbody>
    </Table>
  );
};
