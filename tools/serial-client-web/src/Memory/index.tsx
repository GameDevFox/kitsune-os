import { useEffect, useState } from 'react';
import { BsBookmark } from 'react-icons/bs';
import { ImArrowDown, ImArrowUp } from 'react-icons/im';
import { Select } from 'chakra-react-select';

import { Box, Heading, Icon, IconButton, Stack } from "@chakra-ui/react";

import { Address } from "./Address";
import { Bookmark, Bookmarks } from './Bookmarks';
import { EditText } from './EditText';
import { MemoryTable } from './MemoryTable';

import { api, loadSymbols } from '../api';

const BOOKMARKS = 'bookmarks';

const loadBookmarks = () => JSON.parse(localStorage.getItem(BOOKMARKS) || 'null');

export const Memory = () => {
  const [address, setAddress] = useState<number>(0x8000);
  const [offset, setOffset] = useState<number>(0);

  const [memory, setMemory] = useState<number[]>([]);
  const [diff, setDiff] = useState<boolean[]>([]);

  const updateMemory = async (useDiff = false, wrapperFn: any = () => {}) => {
    const oldMemory = memory;

    await wrapperFn();

    const result = await api.get(`/memory/${address}/length/0x100`)
      .then(res => {
        const data: number[] = res.data.data.data;
        setMemory(data);

        return data;
      });

    if(useDiff) {
      const diff = result.map((value, index) => value === oldMemory[index]);
      setDiff(diff);
    } else {
      // Clear diff
      setDiff([]);
    }

    return result;
  };

  useEffect(() => {
    updateMemory();
  }, [address]);

  const [bookmarks, setBookmarks] = useState<Bookmark[]>(
    loadBookmarks() || [
      { address: 0x8000 },
      { address: 0xa000 },
    ]
  );

  const [editText, setEditText] = useState<number[]>([]);
  const [selectionOffset, setSelectionOffset] = useState<number>(0);

  const [symbols, setSymbols] = useState<Record<string, number>>({});

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

  const removeBookmarkAt = (index: number) => {
    const newBookmarks = [
      ...bookmarks.slice(0, index),
      ...bookmarks.slice(index + 1, bookmarks.length)
    ];
    updateBookmarks(newBookmarks);
  };

  const createBookmark = () => updateBookmarks([...bookmarks, { address }]);

  return (
    <Stack>
      <Heading>Memory</Heading>

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

      <EditText
        value={editText}
        onChange={async bytes => {
          const writeAddress = address + offset + selectionOffset;
          const addressStr = `0x${writeAddress.toString(16)}`;

          updateMemory(true, () => {
            return api.post(`/memory/${addressStr}`, { data: bytes });
          });
        }}
        onRefresh={() => updateMemory(true)}
      />

      <Stack direction='row'>
        <Box>
          <Select
            useBasicStyles
            chakraStyles={{ container: provided => ({ ...provided, width: 180 }) }}

            options={Object.entries(symbols).map(([name, address]) => (
              { label: name, value: address }
            ))}
            onChange={e => setAddress(e?.value || 0)}
          />

          {bookmarks.length && (
            <>
              <Heading size='sm' padding={2}>Bookmarks</Heading>
              <Bookmarks value={bookmarks} selected={address}
                onClick={address => setAddress(address)}
                onDelete={index => removeBookmarkAt(index)}
              />
            </>
          )}
        </Box>

        <MemoryTable
          value={memory}
          diff={diff}
          address={address + offset}
          onSelection={(bytes, offset) => {
            setSelectionOffset(offset);
            setEditText(bytes);
          }}
        />

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
  );
};
