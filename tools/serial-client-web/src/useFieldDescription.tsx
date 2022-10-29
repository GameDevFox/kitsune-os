import { useState } from "react";

import { Field, getValueByBits, setBit } from "@kitsune-os/common";
import { FieldDescription } from "./coproc-registers/FieldDescription";

export const useFieldDescription = (fields: Field[], value: number[], setValue: (value: number[]) => void) => {
  const [selected, setSelected] = useState<number | undefined>();

  const setByField = (field: Field, update: number) => {
    const { startBit, length } = field;

    let newValue = [...value];
    for(let i = 0; i < length; i++) {
      const bit = update & (1 << i) ? 1 : 0;
      newValue = setBit(newValue, i + startBit, bit);
    }

    setValue(newValue);
  };

  let fieldDescription = null;
  if(selected !== undefined) {
    const field = fields?.find(({ startBit, length }) => {
      return selected >= startBit && selected < startBit + length
    });

    if(field) {
      const bitValue = getValueByBits(value, field.startBit, field.length);
      fieldDescription = <FieldDescription
        field={field} value={bitValue}
        onChange={value => setByField(field, value)}
        onClose={() => setSelected(undefined)}
      />
    }
  }

  return { fieldDescription, selectField: setSelected };
};
