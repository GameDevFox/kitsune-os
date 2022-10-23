import _ from "lodash";
import { ReactElement } from "react";

import { Button, Table, Tbody, Tr } from "@chakra-ui/react";
import {
  Field, getBit, getByteIndex, getFieldForBit, setBit
} from "@kitsune-os/common";

import { TdCompact as Td } from './TdCompact';

type Value = number[];

const altCodeDisplay: Record<string, string> = {
  'RAO': '1',
  'RAZ': '0',
};

const getColor = (bitIndex: number) => {
  const byteGroup = getByteIndex(bitIndex) % 2;
  return byteGroup ? 'auto': 'blue.500';
};

interface BitsProps {
  value?: Value;
  onChange?: (value: Value, bitIndex: number, bitValue: number) => void;

  fields?: Field[],
  onClickField?: (field: Field) => void;
};

export const Bits = (props: BitsProps) => {
  const {
    value = [], onChange = () => {},
    fields = [], onClickField = () => {}
  } = props;

  const numberCells: ReactElement[] = [];
  const bitCells: ReactElement[] = [];
  const fieldCells: ReactElement[] = [];

  const fireOnChange = (bitIndex: number, bitValue: number) => {
    const newValue = setBit(value, bitIndex, bitValue);
    onChange(newValue, bitIndex, bitValue);
  };

  _.times(32, bitIndex => {
    const bit = getBit(value, bitIndex);
    const field = getFieldForBit(fields, bitIndex);

    numberCells.push(
      <Td
        key={bitIndex} textAlign='center' color={getColor(bitIndex)}
        paddingTop='1' paddingBottom='1' paddingLeft='0' paddingRight='0'
      >
        <pre>{bitIndex}</pre>
      </Td>
    );

    bitCells.push(
      <Td
        key={bitIndex} textAlign='center'
        paddingTop='1' paddingBottom='1' paddingLeft='0' paddingRight='0'
      >
        <Button disabled={!!field?.alt} size='xs' color={getColor(bitIndex)}
          onClick={() => fireOnChange(bitIndex, 1 - bit)}
        >
          <pre>{bit}</pre>
        </Button>
      </Td>
    );

    if(!field) {
      fieldCells.push(<Td key={bitIndex}></Td>);
    } else {
      const { alt } = field;

      if(alt) {
        let code = 'X';
        if(alt in altCodeDisplay)
          code = altCodeDisplay[alt];

        fieldCells.push(
          <Td key={bitIndex}><pre>{code}</pre></Td>
        );
      } else {
        if(field.startBit === bitIndex) {
          const { code } = field;
          if(!code)
            throw new Error(`No code on field: ${JSON.stringify(field, null, 2)}`);

          const style: Record<string, string> = {};
          if(code.length > 2)
            style['writingMode'] = 'vertical-lr';

          fieldCells.push(
            <Td
              key={bitIndex} style={style} cursor='pointer' color='blue.500'
              onClick={() => onClickField(field)}
            >
              <pre>{code}</pre>
            </Td>
          );
        } else {
          fieldCells.push(
            <Td
              key={bitIndex} color='blue.500' cursor='pointer'
              onClick={() => onClickField(field)}
            >
              <pre>&gt;</pre>
            </Td>
          );
        }
      }
    }
  });

  numberCells.reverse();
  bitCells.reverse();
  fieldCells.reverse();

  return (
    <Table>
      <Tbody>
        <Tr>
          {numberCells}
        </Tr>

        <Tr>
          {bitCells}
        </Tr>

        {fieldCells.length !== 0 && (
          <Tr>
            {fieldCells}
          </Tr>
        )}
      </Tbody>
    </Table>
  );
};
