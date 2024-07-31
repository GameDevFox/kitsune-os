import { useEffect, useState } from "react";
import { BsXLg } from "react-icons/bs";

import {
  Button, Icon, IconButton, Input,
  InputGroup, InputLeftAddon, InputRightElement
} from "@chakra-ui/react";

interface AddressProps {
  address: number;
  onChangeAddress: (address: number) => void;

  offset: number;
  onChangeOffset: (address: number) => void;
}

export const Address = (props: AddressProps) => {
  const {
    address, onChangeAddress,
    offset, onChangeOffset,
  } = props;

  const [addressInput, setAddressInput] = useState<string>(address.toString(16));
  const [offsetInput, setOffsetInput] = useState<string>(offset.toString(16));

  useEffect(() => {
    setAddressInput(address.toString(16));
  }, [address]);

  useEffect(() => {
    setOffsetInput(offset.toString(16));
  }, [offset]);

  const updateAddress = () => onChangeAddress(Number(`0x${addressInput}`));
  const updateOffset = () => onChangeOffset(Number(`0x${offsetInput}`));

  return (
    <>
      <InputGroup>
        <InputLeftAddon paddingInlineEnd='1'>Address 0x</InputLeftAddon>
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
        <InputLeftAddon paddingInlineEnd='1'>Offset 0x</InputLeftAddon>
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
            onClick={() => onChangeOffset(0)}
          />
        </InputRightElement>
      </InputGroup>

      <Button onClick={() => { updateAddress(); updateOffset(); }}>Go</Button>
    </>
  );
};
