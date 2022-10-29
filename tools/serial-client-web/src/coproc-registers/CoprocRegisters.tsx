import React, { useEffect, useState } from "react";

import { Box, Button, Input, Select, Stack } from "@chakra-ui/react";
import { bytesToStr, CoprocRegister } from '@kitsune-os/common';

import { listCoprocRegs, readCoprocReg, writeCoprocReg } from "../api";
import { useFieldDescription } from "../useFieldDescription";

import { Bits } from './Bits';

export const CoprocRegisters = () => {
  const [coprocRegisters, setCoprocRegisters] = useState<Record<string, CoprocRegister>>({});

  const [name, setName] = useState('SCTLR');
  const [value, setValue] = useState([0, 0, 0, 0]);

  useEffect(() => {
    listCoprocRegs().then(({ data }) => {
      const { registers } = data;
      setCoprocRegisters(registers);
    });
  }, []);

  const coprocRegister = coprocRegisters[name];
  const { fields, isReadable, isWriteable } = coprocRegister || {};

  const { fieldDescription, selectField } = useFieldDescription(fields || [], value, setValue);

  const valueStr = bytesToStr(value);

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
          selectField(bitIndex);
        }}
        onClickField={field => selectField(field.startBit)}
      />

      {fieldDescription}
    </Box>
  );
};
