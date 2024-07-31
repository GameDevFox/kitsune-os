import { BsXLg } from "react-icons/bs";

import {
  Box, Heading, Icon, IconButton, Input, InputGroup,
  InputLeftAddon, Stack, Table, Tbody, Th, Tr
} from "@chakra-ui/react";
import { Field } from "@kitsune-os/common";

import { TdCompact as Td } from '../TdCompact';

interface FieldDescriptionProps {
  field: Field;
  value?: number;
  onChange?: (value: number) => void;
  onClose?: () => void;
};

export const FieldDescription = (props: FieldDescriptionProps) => {
  const {
    field, value,
    onChange = () => {}, onClose = () => {}
  } = props;

  const { startBit, length, code, name, description, values } = field;

  let bitStr = `${startBit}`;
  if(length !== 1)
    bitStr += `:${startBit + length - 1}`;

  let valueTable = null;
  if(values && Array.isArray(values) && values.length !== 0) {
    const rows = (values || []).map(possibleValue => {
      const props = possibleValue.value === value ?
        { backgroundColor: 'blue.500' } : false;

      return (
        <Tr
          key={possibleValue.value} {...props} cursor='pointer'
          onClick={() => onChange(possibleValue.value)}
        >
          <Td>0x{possibleValue.value.toString(16)}</Td>
          <Td>{possibleValue.value}</Td>
          <Td width='870px' textAlign='left'>
            {possibleValue.description}
          </Td>
        </Tr>
      );
    });

    valueTable = (
      <>
        <Heading size='md' paddingTop='3'>Values:</Heading>
        <Table>
          <Tbody>
            <Tr>
              <Th>Hex</Th>
              <Th>Dec</Th>
              <Th>Description</Th>
            </Tr>
            {rows}
          </Tbody>
        </Table>
      </>
    );
  } else {
    valueTable = (
      <Stack direction="row" paddingTop="3">
        <Heading size="md" paddingTop="2">Value:</Heading>
        <InputGroup>
          <InputLeftAddon paddingInlineEnd='1'>Hex: 0x</InputLeftAddon>
          <Input readOnly paddingLeft={1} value={value?.toString(16)}/>
        </InputGroup>
        <InputGroup>
          <InputLeftAddon paddingInlineEnd='1'>Dec:</InputLeftAddon>
          <Input readOnly value={value}/>
        </InputGroup>
      </Stack>
    );
  }

  return (
    <Box position='relative'>
      <Heading display='inline'>{code} [{bitStr}]</Heading>
      <Heading size='md' display='inline' paddingLeft='3'>{name}</Heading>
      <Box maxWidth='933px'>{description}</Box>

      {valueTable}

      <Box position='absolute' top='8px' right='8px'>
        <IconButton
          aria-label='close' size='sm' icon={<Icon as={BsXLg}/>}
          onClick={() => onClose && onClose()}
        />
      </Box>
    </Box>
  );
};
