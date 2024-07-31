import _ from 'lodash';
import { useState } from 'react';
import { RGBColor } from 'react-color';

import { Button, ButtonGroup, Stack, useColorMode } from '@chakra-ui/react';

import {
  clear, draw, getTimer, instructionAbort,
  printDeviceTree, sayHello, setColor as apiSetColor
} from './api';

import { ColorPicker } from './ColorPicker';

export const Buttons = () => {
  const { colorMode, toggleColorMode } = useColorMode();

  const [color, setColor] = useState<RGBColor>({ r: 0xff, g: 0x55, b: 0x00 });

  const updateColor = (color: RGBColor) => {
    setColor(color);
    apiSetColor(color);
  };

  return (
    <Stack direction="row">
      <Stack>
        <ButtonGroup>
          <Button onClick={toggleColorMode}>
            {_.capitalize(colorMode)}
          </Button>
          <Button onClick={sayHello}>Hello!</Button>
          <Button onClick={printDeviceTree}>Print Device Tree</Button>
          <Button onClick={getTimer}>Get Timer</Button>
          <Button onClick={instructionAbort}>Instruction Abort</Button>
        </ButtonGroup>

        <ButtonGroup>
          <ColorPicker color={color} onChange={color => updateColor(color)}/>
          <Button onClick={clear}>Clear</Button>

          <Button onClick={() => draw('curve')}>Draw Curve</Button>
          <Button onClick={() => draw('logo')}>Draw Logo</Button>
          <Button onClick={() => draw('kitsune-text')}>Draw Kitsune Text</Button>
        </ButtonGroup>
      </Stack>

      <Stack>
        <ButtonGroup>
          <Button onClick={() => draw('mascot')}>Draw Mascot</Button>
          <Button onClick={() => draw('mascot-with-glasses')}>w/ Glasses</Button>
          <Button onClick={() => draw('mascot-text')}>Draw Mascot Text</Button>
        </ButtonGroup>

        <ButtonGroup>
          <Button onClick={() => draw('aki')}>Draw Aki</Button>
          <Button onClick={() => draw('aki-with-glasses')}>w/ Glasses</Button>
          <Button onClick={() => draw('aki-without-glasses')}>w/o Glasses</Button>
        </ButtonGroup>
      </Stack>
    </Stack>
  );
};
