import { useEffect, useState } from "react";
import { BsArrowClockwise } from "react-icons/bs";

import { Button, Icon, Input, InputGroup, InputLeftAddon, Stack } from "@chakra-ui/react";

import { bytesToHex, bytesToStr, cleanHex, formatHex, hexToBytes, strToBytes } from "../utils";

interface EditTextProps {
  value: number[],
  onChange?: (value: number[]) => void,
  onRefresh?: () => void,
};

export const EditText = (props: EditTextProps) => {
  const {
    value,
    onChange = () => {},
    onRefresh = () => {},
  } = props;

  const [myValue, setMyValue] = useState<number[]>([]);

  const [hexValue, setHexValue] = useState<string>('');
  const [strValue, setStrValue] = useState<string>('');

  useEffect(() => {
    setMyValue(value);

    const str = bytesToStr(value);
    setStrValue(str);

    let hex = formatHex(bytesToHex(value));
    setHexValue(hex);
  }, [value]);

  const updateHexValue: React.ChangeEventHandler<HTMLInputElement> = e => {
    const { value } = e.currentTarget;

    const cleanHexStr = cleanHex(value);
    const hex = formatHex(cleanHexStr);
    setHexValue(hex);

    const bytes = hexToBytes(cleanHexStr);
    setMyValue(bytes)

    // Set the other one
    const str = bytesToStr(bytes);
    setStrValue(str);
  };

  const updateStrValue: React.ChangeEventHandler<HTMLInputElement> = e => {
    const { value } = e.currentTarget;

    setStrValue(value);

    const bytes = strToBytes(value);
    setMyValue(bytes);

    // Set the other one
    const hex = formatHex(bytesToHex(bytes));
    setHexValue(formatHex(hex));
  };

  return (
    <Stack direction="row">
      <Stack flexGrow={1}>
        <InputGroup>
          <InputLeftAddon paddingInlineEnd='1'>Value 0x</InputLeftAddon>
          <Input
            style={{ fontFamily: 'monospace' }}
            paddingLeft={1}
            value={hexValue}
            onChange={updateHexValue}
          />
        </InputGroup>

        <InputGroup>
          <InputLeftAddon paddingInlineEnd='1'>Value</InputLeftAddon>
          <Input
            style={{ fontFamily: 'monospace' }}
            paddingLeft={1}
            value={strValue}
            onChange={updateStrValue}
          />
        </InputGroup>
      </Stack>

      <Stack>
        <InputGroup width={150}>
          <InputLeftAddon>Size</InputLeftAddon>
          <Input readOnly value={myValue.length * 8}/>
        </InputGroup>

        <Stack direction="row">
          <Button flexGrow={1} onClick={() => onChange(myValue)}>
            Write
          </Button>

          <Button onClick={() => onRefresh()}>
            <BsArrowClockwise/>
          </Button>
        </Stack>
      </Stack>
    </Stack>
  );
};
