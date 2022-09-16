import axios from 'axios';
import _ from 'lodash';
import { useEffect, useState } from 'react';

import {
  Button, ButtonGroup, Center, Input, InputGroup,
  InputLeftAddon, Stack, Table, Tbody, Td, Tr, useColorMode,
} from '@chakra-ui/react';

const baseURL = 'http://localhost:8080';

const api = axios.create({ baseURL });

const toHex = (value: number) => value.toString(16).padStart(2, '0');

function App() {
  const { colorMode, toggleColorMode } = useColorMode();

  const [address, setAddress] = useState<number>(0x8000);
  const [addressInput, setAddressInput] = useState<string>(
    `0x${address.toString(16)}`
  );
  const [memory, setMemory] = useState<number[]>([]);

  useEffect(() => {
    loadMemory();
  }, []);

  const sayHello = () => api.get('/hello');
  const clear = () => api.get('/clear');
  const draw = () => api.get('/draw');

  const loadMemory = () => {
    const address = Number(addressInput);

    api.get(`/memory/${address}/length/0x40`)
      .then(res => {
        const { data } = res.data.data;
        setMemory(data);
      });
  };

  const rows = [];
  for(let i=0; i<4; i++) {
    const cells = [0, 4, 8, 12].map(value => (
      <Td
        key={`cell${value}`} padding='1'
        color={value % 8 === 0 ? 'white' : 'grey'}
      >
        <pre>
          {memory.slice((i * 16) + value, (i * 16) + value + 4)
            .map(toHex).join(' ')}
        </pre>
      </Td>
    ));

    rows.push(<Tr key={`row${i}`}>{cells}</Tr>);
  }

  return (
    <Center>
      <Stack>
        <ButtonGroup>
          <Button onClick={toggleColorMode}>
            {_.capitalize(colorMode)}
          </Button>
          <Button onClick={sayHello}>Hello!</Button>
          <Button onClick={clear}>Clear</Button>
          <Button onClick={draw}>Draw</Button>
        </ButtonGroup>

        <Stack direction='row'>
          <InputGroup>
            <InputLeftAddon>Address:</InputLeftAddon>
            <Input
              type="input" width="auto" value={addressInput}
              onChange={e => setAddressInput(e.currentTarget.value)}
            />
          </InputGroup>
          <Button onClick={loadMemory}>Memory</Button>
        </Stack>

        <Table variant="striped">
          <Tbody>
            {rows}
          </Tbody>
        </Table>
      </Stack>
    </Center>
  );
}

export default App;
