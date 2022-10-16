import { BsTrash } from "react-icons/bs";

import { Box, Icon, IconButton, Stack } from "@chakra-ui/react";

export  interface Bookmark {
  name?: string;
  address: number;
}

interface BookmarksProps {
  value: Bookmark[];
  onClick?: (address: number) => void;
  onDelete?: (index: number) => void;
}

export const Bookmarks = (props: BookmarksProps) => {
  const {
    value,
    onClick = () => {},
    onDelete = (index: number) => {},
  } = props;

  return (
    <Stack>
      {value.map(({ name, address }, index) => (
        <Stack direction='row'>
          <Box key={name} flexGrow='1'
            backgroundColor='blue.500' borderRadius='md' cursor='pointer' padding='1'
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
