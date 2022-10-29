import _ from 'lodash';
import { useState } from 'react';

import { BsBookmark } from 'react-icons/bs';
import { ImArrowDown, ImArrowUp } from 'react-icons/im';

import {
  Box, Button, ButtonGroup, Center, Heading,
  Icon, IconButton, Stack, useColorMode,
} from '@chakra-ui/react';

import { clear, draw, printDeviceTree, sayHello } from './api';
import { Bookmark, Bookmarks } from './Bookmarks';
import { MemoryTable } from './MemoryTable';
import { CoprocRegisters } from './coproc-registers';
import { Address } from './Address';

const BOOKMARKS = 'bookmarks';

const loadBookmarks = () => JSON.parse(localStorage.getItem(BOOKMARKS) || 'null');

function App() {
  const { colorMode, toggleColorMode } = useColorMode();

  const [address, setAddress] = useState<number>(0x8000);
  const [offset, setOffset] = useState<number>(0);

  const [bookmarks, setBookmarks] = useState<Bookmark[]>(
    loadBookmarks() ||
    [
      { address: 0x8000 },
      { address: 0xa000 },
    ]
  );

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

  return (
    <Center>
      <Stack>
        <ButtonGroup>
          <Button onClick={toggleColorMode}>
            {_.capitalize(colorMode)}
          </Button>
          <Button onClick={sayHello}>Hello!</Button>
          <Button onClick={clear}>Clear</Button>
          <Button onClick={printDeviceTree}>Print Device Tree</Button>
          <Button onClick={() => draw('curve')}>Draw Curve</Button>
          <Button onClick={() => draw('mascot')}>Draw Mascot</Button>
          <Button onClick={() => draw('logo')}>Draw Logo</Button>
        </ButtonGroup>

        <CoprocRegisters/>

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
          {bookmarks.length && (
            <Box>
              <Heading size='sm'>Bookmarks</Heading>
              <Bookmarks value={bookmarks} selected={address}
                onClick={address => setAddress(address)}
                onDelete={index => removeBookmarkAt(index)}
              />
            </Box>
          )}

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
