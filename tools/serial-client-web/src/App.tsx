import _ from 'lodash';
import { useEffect, useState } from 'react';

import {
  Button, ButtonGroup, Center, Flex, Input, InputGroup,
  InputLeftAddon, Stack, useColorMode,
} from '@chakra-ui/react';

import { api } from './api';
import { MemoryTable } from './memory-table';

function App() {
  const { colorMode, toggleColorMode } = useColorMode();

  const [address, setAddress] = useState<number>(0x8000);
  const [addressInput, setAddressInput] = useState<string>(
    `0x${address.toString(16)}`
  );

  useEffect(() => {
    setAddressInput(`0x${address.toString(16)}`);
  }, [address]);

  const sayHello = () => api.get('/hello');
  const clear = () => api.get('/clear');
  const draw = (value: string) => api.get(`/draw/${value}`);

  const updateAddress = () => setAddress(Number(addressInput));

  return (
    <Center>
      <Stack>
        <ButtonGroup>
          <Button onClick={toggleColorMode}>
            {_.capitalize(colorMode)}
          </Button>
          <Button onClick={sayHello}>Hello!</Button>
          <Button onClick={clear}>Clear</Button>
          <Button onClick={() => draw('curve')}>Draw Curve</Button>
          <Button onClick={() => draw('mascot')}>Draw Mascot</Button>
          <Button onClick={() => draw('logo')}>Draw Logo</Button>
        </ButtonGroup>

        <Stack direction='row'>
          <InputGroup>
            <InputLeftAddon>Address:</InputLeftAddon>
            <Input
              type="input" width="auto" value={addressInput}
              onChange={e => setAddressInput(e.currentTarget.value)}
            />
          </InputGroup>
          <Button onClick={updateAddress}>Memory</Button>
        </Stack>

        <MemoryTable address={address}/>

        <Flex direction='row' gap='2'>
          <Button flexGrow='1' onClick={() => setAddress(address - 0x40)}>Prev</Button>
          <Button flexGrow='1' onClick={() => setAddress(address + 0x40)}>Next</Button>
        </Flex>
      </Stack>
    </Center>
  );
}

export default App;
