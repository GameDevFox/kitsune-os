import { useEffect, useState } from "react";

import { Box, Stack, StyleProps, Table, Tbody, Td, Thead, Tr } from "@chakra-ui/react";

import { toAscii, toHex } from "../utils";
import { TdCompact } from "../TdCompact";

const ADDRESS_COLOR = "#909090";
const OTHER_CHAR = 'X';
const ROW_LENGTH = 16;

const DIFF_STYLE: StyleProps = { backgroundColor: "red.300", color: "black", borderRadius: 2 };
const HIGHLIGHT_STYLE: StyleProps = { backgroundColor: "#3182ce", color: "black" };

const isInRange = (target: number, start: number | null, end: number | null) => {
  if(start !== null && end !== null && start > end)
    throw new Error(`Error: start is greater than end - ${start} > ${end}`);

  if(start !== null && end !== null)
    return target >= start && target <= end
  else if(start !== null)
    return target === start;

  return false;
}

interface MemoryTableProps {
  value: number[];
  diff?: boolean[];
  address: number;
  onSelection?: (bytes: number[], offset: number) => void;
};

export const MemoryTable = (props: MemoryTableProps) => {
  const {
    value,
    diff,
    address,
    onSelection = () => {},
  } = props;

  const [start, setStart] = useState<number | null>(null);
  const [end, setEnd] = useState<number | null>(null);

  useEffect(() => {
    if(start === null) {
      return;
    }

    const newEnd = end === null ? start : end
    const bytes = value.slice(start, newEnd + 1);
    onSelection(bytes, start);
  }, [value, start, end]);

  const onAddressClick = (offset: number) => {
    if(start === null) {
      setStart(offset);
    } else if(end === null) {
      if(start === offset) {
        setStart(null);
      } else {
        setStart(offset > start ? start : offset);
        setEnd(offset > start ? offset : start);
      }
    } else {
      setStart(offset);
      setEnd(null);
    }
  };

  const rows = [];

  for(let i=0; i<16; i++) {
    const rowBytes = value.slice(i * ROW_LENGTH, (i + 1) * ROW_LENGTH);

    const hexCells = [0, 4, 8, 12].map(value => (
      <TdCompact
        key={`cell${value}`}
        color={value % 8 === 0 ? 'auto' : 'blue.500'}
      >
        <Stack direction="row">
          {rowBytes.slice(value, value + 4).map(toHex).map((hexValue, index) => {
            const offset = (i * 16) + value + index;
            let style = {};

            if(diff && offset in diff && diff[offset] === false)
              style = DIFF_STYLE;

            if(isInRange(offset, start, end))
              style = HIGHLIGHT_STYLE;

            return (
              <Box
                key={offset}
                {...style}
                onClick={() => onAddressClick(offset)}
              >
                <pre>{hexValue}</pre>
              </Box>
            );
          })}
        </Stack>
      </TdCompact>
    ));

    const textCells = [0, 4, 8, 12].map(value => (
      <Td
        key={`cell${value}`} padding='1'
        color={value % 8 === 0 ? 'auto' : 'blue.500'}
      >
        <Stack direction="row" spacing={0}>
          {rowBytes
              .slice(value, value + 4)
              .map(toAscii)
              .map((asciiValue, index) => {
                const offset = (i * 16) + value + index;
                let style: any = asciiValue ? {} : { color: "black" };

                if(diff && offset in diff && diff[offset] === false)
                  style = DIFF_STYLE;

                if(isInRange(offset, start, end))
                  style = HIGHLIGHT_STYLE;

                return (
                  <Box
                    key={offset}
                    {...style}
                    onClick={() => onAddressClick(offset)}
                  >
                    <pre>{asciiValue || OTHER_CHAR}</pre>
                  </Box>
                );
              })
          }
        </Stack>
      </Td>
    ));

    rows.push(
      <Tr key={`row${i}`}>
        <TdCompact style={{ color: ADDRESS_COLOR }} paddingRight={4}>
          <pre>
            {((i * 16) + address).toString(16)}
          </pre>
        </TdCompact>
        {hexCells}
        {textCells}
      </Tr>
    );
  }

  return (
    <Table variant="striped">
      <Thead style={{ color: ADDRESS_COLOR }}>
        <Tr>
          <TdCompact paddingRight={4}>Address</TdCompact>
          {[0, 1, 2, 3].map(high => (
            <TdCompact key={high}>
              <Stack direction="row">
                {[0, 1, 2, 3].map(low => (
                  <pre key={low}>{toHex(high * 4 + low)}</pre>
                ))}
              </Stack>
            </TdCompact>
          ))}
          <TdCompact colSpan={4}>Text</TdCompact>
        </Tr>
      </Thead>
      <Tbody>{rows}</Tbody>
    </Table>
  );
};
