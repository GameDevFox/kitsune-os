import { RGBColor, SketchPicker as Picker } from 'react-color';

import { Box } from "@chakra-ui/react";
import { useState } from 'react';

const colorToHex = (color: RGBColor) => {
  const rHex = color['r'].toString(16).padStart(2, '0');
  const gHex = color['g'].toString(16).padStart(2, '0');
  const bHex = color['b'].toString(16).padStart(2, '0');

  return `#${rHex}${gHex}${bHex}`
};

interface Props {
  color: RGBColor;
  onChange: (color: RGBColor) => void;
};

const presetColors = [
  '#D0021B', '#F5A623', '#F8E71C', '#8B572A',
  '#7ED321', '#417505', '#BD10E0', '#9013FE',
  '#4A90E2', '#50E3C2', '#B8E986', '#000000',
  '#4A4A4A', '#9B9B9B', '#FFFFFF', '#FF5500'
];

export const ColorPicker = (props: Props) => {
  const { color, onChange } = props;

  const [showPicker, setShowPicker] = useState(false);

  return (
    <Box position='relative'>
      <Box
        backgroundColor={colorToHex(color)} width='40px' height='40px'
        borderRadius='0.375rem' cursor='pointer'
        onClick={() => setShowPicker(value => !value)}
      />

      {showPicker && (
        <Box position='absolute' top='48px' zIndex='10'>
          <Picker
            presetColors={presetColors} color={color}
            onChangeComplete={({ rgb }) => onChange(rgb)}
          />
        </Box>
      )}
    </Box>
  );
};
