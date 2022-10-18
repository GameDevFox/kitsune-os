import { BsTrash } from "react-icons/bs";

import { Box, Icon, IconButton, Stack } from "@chakra-ui/react";

export  interface Bookmark {
  name?: string;
  address: number;
}

interface BookmarksProps {
  value: Bookmark[];
  selected?: number;
  onClick?: (address: number) => void;
  onDelete?: (index: number) => void;
}

export const Bookmarks = (props: BookmarksProps) => {
  const {
    value,
    selected,
    onClick = () => {},
    onDelete = (index: number) => {},
  } = props;

  return (
    <Stack>
      {value.map(({ name, address }, index) => (
        <Stack key={index} direction='row'>
          <Box key={name}
            backgroundColor={address === selected ? 'blue.300' : 'blue.600'}
            borderRadius='md' cursor='pointer' flexGrow='1'
            padding='1' paddingLeft='3' paddingRight='3'
            onClick={() => onClick(address)}
          >
            {name && <input style={{ paddingLeft: '4px', borderRadius: '4px' }} value={name}/>}
            <pre>0x{address.toString(16)}</pre>
          </Box>

          <IconButton
            aria-label='Bookmark' size='sm' icon={<Icon as={BsTrash}/>}
            onClick={() => onDelete(index)}
          />
        </Stack>
      ))}
    </Stack>
  );
};
