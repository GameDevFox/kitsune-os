import { useEffect, useState } from "react";
import { Select } from 'chakra-react-select';

import { Box, Button, Input, Stack } from "@chakra-ui/react";
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

  // Make FieldDescription a component that props are passed into
  const { FieldDescription, selectField } = useFieldDescription(fields || [], value, setValue);

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
        <Select
          useBasicStyles
          chakraStyles={{ container: provided => ({ ...provided, width: 180 }) }}

          options={Object.entries(coprocRegisters).map(([name, value]) => {
            const { args } = value;

            const asmArgs = `p${args[0]}, ${args[1]}, <Rt>, c${args[2]}, c${args[3]}, ${args[4]}`;
            const fullName = `${name} - ${asmArgs}`;
            return { label: fullName, value: name };
          })}
          onChange={e => setName(e?.value || '')}
        />

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

      {FieldDescription}
    </Box>
  );
};
