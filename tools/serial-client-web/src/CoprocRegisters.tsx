import _ from 'lodash';
import { useEffect, useState } from "react";

import { Button, Input, Select, Stack } from "@chakra-ui/react";

import { listCoprocRegs, readCoprocReg } from "./api";

export const CoprocRegisters = () => {
  const [coprocRegNames, setCoprocRegNames] = useState<string[]>([]);
  const [activeCoprocReg, setActiveCoprocReg] = useState('PMCCNTR');
  const [coprocRegValue, setCoprocRegValue] = useState('');

  useEffect(() => {
    listCoprocRegs().then(res => {
      setCoprocRegNames(res.data.names);
    });
  }, []);

  return (
    <Stack direction='row'>
      <Select maxWidth='200px'
        value={activeCoprocReg}
        onChange={e => setActiveCoprocReg(e.currentTarget.value)}
      >
        <option value=''></option>
        {coprocRegNames.map(name => (
          <option key={name} value={name}>{name}</option>
        ))}
      </Select>
      <Button onClick={() => {
        readCoprocReg(activeCoprocReg).then(res => {
          const bytes: number[] = res.data.data.data;
          const str = bytes
            .map(value => _.padStart(value.toString(16), 2, '0'))
            .reverse()
            .join('');

          setCoprocRegValue(str);
        });
      }}>Read</Button>
      <pre><Input readOnly value={coprocRegValue}/></pre>
    </Stack>
  );
};
