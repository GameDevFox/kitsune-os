import { useState } from "react";

import { Button, Input, Stack } from "@chakra-ui/react";
import { bytesToStr } from "@kitsune-os/common";

import { readCPSR, writeCPSR } from "./api";
import { fields } from "./cspr-fields";
import { useFieldDescription } from "./useFieldDescription";

import { Bits } from "./coproc-registers/Bits";

export const CPSR = () => {
  const [value, setValue] = useState<number[]>([0, 0, 0, 0]);

  const { fieldDescription, selectField } = useFieldDescription(fields, value, setValue);

  const loadCPSR = () => {
    readCPSR().then(result => {
      setValue(result.data.data.data);
    });
  };

  const saveCPSR = () => {
    writeCPSR(value);
  };

  return (
    <>
      <Stack direction='row'>
        <Button onClick={loadCPSR}>Read</Button>
        <pre><Input readOnly value={bytesToStr(value)}/></pre>
        <Button onClick={saveCPSR}>Write</Button>
      </Stack>

      <Bits value={value} fields={fields}
        onChange={(bytes, bitIndex) => {
          setValue(bytes);
          selectField(bitIndex);
        }}
        onClickField={field => selectField(field.startBit)}
      />

      {fieldDescription}
    </>
  );
}
