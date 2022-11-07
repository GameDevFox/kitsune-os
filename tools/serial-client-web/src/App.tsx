import _ from 'lodash';
import { useEffect, useState } from 'react';
import { RGBColor } from 'react-color';

import { BsBookmark } from 'react-icons/bs';
import { ImArrowDown, ImArrowUp } from 'react-icons/im';

import {
  Box, Button, ButtonGroup, Center, Heading,
  Icon, IconButton, Select, Stack, Tab, TabList,
  TabPanel, TabPanels, Tabs, useColorMode,
} from '@chakra-ui/react';

import { clear, draw, getTimer, loadSymbols, printDeviceTree, sayHello, setColor as apiSetColor } from './api';
import { Address } from './Address';
import { Bookmark, Bookmarks } from './Bookmarks';
import { ColorPicker } from './ColorPicker';
import { CoprocRegisters } from './coproc-registers';
import { CPSR } from './CPSR';
import { MemoryTable } from './MemoryTable';

const BOOKMARKS = 'bookmarks';

const loadBookmarks = () => JSON.parse(localStorage.getItem(BOOKMARKS) || 'null');

function App() {
  const { colorMode, toggleColorMode } = useColorMode();

  const [color, setColor] = useState<RGBColor>({ r: 0xff, g: 0x55, b: 0x00 });

  const [address, setAddress] = useState<number>(0x8000);
  const [offset, setOffset] = useState<number>(0);

  const [symbols, setSymbols] = useState<Record<string, number>>({});

  const [bookmarks, setBookmarks] = useState<Bookmark[]>(
    loadBookmarks() ||
    [
      { address: 0x8000 },
      { address: 0xa000 },
    ]
  );

  useEffect(() => {
    loadSymbols().then(res => {
      const { symbols } = res.data;
      setSymbols(symbols);
    });
  }, []);

  const updateBookmarks = (newBookmarks: Bookmark[]) => {
    setBookmarks(newBookmarks);
    localStorage.setItem(BOOKMARKS, JSON.stringify(newBookmarks));
  };

  const createBookmark = () => updateBookmarks([...bookmarks, { address }]);

  const removeBookmarkAt = (index: number) => {
    const newBookmarks = [
      ...bookmarks.slice(0, index),
      ...bookmarks.slice(index + 1, bookmarks.length)
    ];
    updateBookmarks(newBookmarks);
  };

  const updateColor = (color: RGBColor) => {
    setColor(color);
    apiSetColor(color);
  };

  return (
    <Center>
      <Stack>
        <ButtonGroup>
          <Button onClick={toggleColorMode}>
            {_.capitalize(colorMode)}
          </Button>
          <Button onClick={sayHello}>Hello!</Button>
          <Button onClick={printDeviceTree}>Print Device Tree</Button>
          <Button onClick={getTimer}>Get Timer</Button>
        </ButtonGroup>

        <ButtonGroup>
          <ColorPicker color={color} onChange={color => updateColor(color)}/>

          <Button onClick={() => draw('curve')}>Draw Curve</Button>
          <Button onClick={() => draw('mascot')}>Draw Mascot w/ Glasses</Button>
          <Button onClick={() => draw('mascot-no-glasses')}>Draw Mascot</Button>
          <Button onClick={() => draw('logo')}>Draw Logo</Button>
          <Button onClick={() => draw('kitsune-text')}>Draw Kitsune Text</Button>

          <Button onClick={clear}>Clear</Button>
        </ButtonGroup>

        <Tabs>
          <TabList>
            <Tab>Coproc Registers</Tab>
            <Tab>CPSR</Tab>
          </TabList>

          <TabPanels>
            <TabPanel paddingLeft='0' paddingRight='0' paddingBottom='0'>
              <CoprocRegisters/>
            </TabPanel>
            <TabPanel paddingLeft='0' paddingRight='0' paddingBottom='0'>
              <CPSR/>
            </TabPanel>
          </TabPanels>
        </Tabs>

        <Stack direction='row'>
          <IconButton
            aria-label='Bookmark' icon={<Icon as={BsBookmark}/>}
            onClick={createBookmark}
          />

          <Address
            address={address} onChangeAddress={setAddress}
            offset={offset} onChangeOffset={setOffset}
          />
        </Stack>

        <Stack direction='row'>
            <Box>
              <Select width='150px'
                onChange={e => setAddress(parseInt(e.currentTarget.value))}
              >
                {Object.entries(symbols).map(([name, address]) => (
                  <option key={name} value={address}>{name}</option>
                ))}
              </Select>

              {bookmarks.length && (
                <>
                  <Heading size='sm'>Bookmarks</Heading>
                  <Bookmarks value={bookmarks} selected={address}
                    onClick={address => setAddress(address)}
                    onDelete={index => removeBookmarkAt(index)}
                  />
                </>
              )}
            </Box>

          <MemoryTable address={address + offset}/>

          <Stack>
            <IconButton
              flexGrow='1' aria-label='prev' icon={<Icon as={ImArrowUp}/>}
              onClick={() => setAddress(address - 0x40)}
            />
            <IconButton
              flexGrow='1' aria-label='next' icon={<Icon as={ImArrowDown}/>}
              onClick={() => setAddress(address + 0x40)}
            />
          </Stack>
        </Stack>
      </Stack>
    </Center>
  );
}

export default App;
