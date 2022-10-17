import _ from 'lodash';
import { useEffect, useState } from 'react';

import { BsBookmark, BsXLg } from 'react-icons/bs';
import { ImArrowDown, ImArrowUp } from 'react-icons/im';

import {
  Box,
  Button, ButtonGroup, Center, Heading, Icon, IconButton, Input,
  InputGroup, InputLeftAddon, InputRightElement, Stack, useColorMode,
} from '@chakra-ui/react';

import { clear, draw, sayHello } from './api';
import { Bookmark, Bookmarks } from './Bookmarks';
import { MemoryTable } from './memory-table';
import { CoprocRegisters } from './CoprocRegisters';

const BOOKMARKS = 'bookmarks';

const loadBookmarks = () => JSON.parse(localStorage.getItem(BOOKMARKS) || 'null');

function App() {
  const { colorMode, toggleColorMode } = useColorMode();

  const [address, setAddress] = useState<number>(0x8000);
  const [addressInput, setAddressInput] = useState<string>(
    address.toString(16)
  );

  const [offset, setOffset] = useState<number>(0);
  const [offsetInput, setOffsetInput] = useState<string>(
    offset.toString(16)
  );

  const [bookmarks, setBookmarks] = useState<Bookmark[]>(
    loadBookmarks() ||
    [
      { address: 0x8000 },
      { address: 0xa000 },
    ]
  );

  useEffect(() => {
    setAddressInput(address.toString(16));
  }, [address]);

  useEffect(() => {
    setOffsetInput(offset.toString(16));
  }, [offset]);

  const updateAddress = () => setAddress(Number(`0x${addressInput}`));
  const updateOffset = () => setOffset(Number(`0x${offsetInput}`));

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

          <InputGroup>
            <InputLeftAddon paddingInlineEnd='1'>Address: 0x</InputLeftAddon>
            <Input
              paddingInlineStart='1'
              type='input' value={addressInput}
              onChange={e => setAddressInput(e.currentTarget.value)}
              onKeyDown={e => {
                if(e.code === 'Enter')
                  updateAddress();
              }}
            />
          </InputGroup>

          <InputGroup>
            <InputLeftAddon paddingInlineEnd='1'>Offset: 0x</InputLeftAddon>
            <Input
              paddingInlineStart='1'
              type='input' value={offsetInput}
              onChange={e => setOffsetInput(e.currentTarget.value)}
              onKeyDown={e => {
                if(e.code === 'Enter')
                  updateOffset();
              }}
            />
            <InputRightElement>
              <IconButton
                aria-label='clear' size='sm' icon={<Icon as={BsXLg}/>}
                onClick={() => setOffset(0)}
              />
            </InputRightElement>
          </InputGroup>

          <Button onClick={updateAddress}>Go</Button>
        </Stack>

        <Stack direction='row'>
          {bookmarks.length && (
            <Box>
              <Heading size='sm'>Bookmarks</Heading>
              <Bookmarks value={bookmarks}
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
