import React, { useEffect, useState } from "react";

import { Box, Button, Input, Select, Stack } from "@chakra-ui/react";

import { listCoprocRegs, readCoprocReg, writeCoprocReg } from "../api";
import { bytesToStr, getValueByBits, CoprocRegister, getFieldForBit, Field, setBit } from '@kitsune-os/common';
import { Bits } from './Bits';
import { FieldDescription } from './FieldDescription';

export const CoprocRegisters = () => {
  const [coprocRegisters, setCoprocRegisters] = useState<Record<string, CoprocRegister>>({});

  const [name, setName] = useState('SCTLR');
  const [value, setValue] = useState([0, 0, 0, 0]);
  const [selected, setSelected] = useState<string | undefined>();

  useEffect(() => {
    listCoprocRegs().then(({ data }) => {
      const { registers } = data;
      setCoprocRegisters(registers);
    });
  }, []);

  const coprocRegister = coprocRegisters[name];
  if(!coprocRegister)
    return null;

  const readCoproc = () => {
    readCoprocReg(name).then(({ data }) => {
      const bytes: number[] = data.data.data;
      setValue(bytes);
    });
  };

  const writeCoproc = () => {
    const valueStr = bytesToStr(value);
    writeCoprocReg(name, valueStr)
      .then(() => readCoproc());
  };

  const setByField = (field: Field, update: number) => {
    const { startBit, length } = field;

    let newValue = [...value];
    for(let i = 0; i < length; i++) {
      const bit = update & (1 << i) ? 1 : 0;
      newValue = setBit(newValue, i + startBit, bit);
    }

    setValue(newValue);
  };

  const { fields, isReadable, isWriteable } = coprocRegister;

  const valueStr = bytesToStr(value);

  let fieldDescription = null;
  if(selected) {
    const field = fields?.find(field => field.code === selected);

    if(field) {
      const bitValue = getValueByBits(value, field.startBit, field.length);
      fieldDescription = <FieldDescription
        field={field} value={bitValue}
        onChange={value => setByField(field, value)}
        onClose={() => setSelected(undefined)}
      />
    }
  }

  return (
    <Box>
      <Stack direction='row'>
        <Select maxWidth='200px'
          value={name}
          onChange={e => setName(e.currentTarget.value)}
        >
          <option value=''></option>
          {Object.keys(coprocRegisters).map(name => (
            <option key={name} value={name}>{name}</option>
          ))}
        </Select>

        <Button disabled={!isReadable} onClick={readCoproc}>Read</Button>
        <pre><Input readOnly value={valueStr}/></pre>
        <Button disabled={!isWriteable} onClick={writeCoproc}>Write</Button>
      </Stack>

      <Bits value={value} fields={fields}
        onChange={(bytes, bitIndex) => {
          setValue(bytes);

          if(fields) {
            const field = getFieldForBit(fields, bitIndex);
            setSelected(field?.code);
          }
        }}
        onClickField={field => setSelected(field.code)}
      />

      {fieldDescription}
    </Box>
  );
};
