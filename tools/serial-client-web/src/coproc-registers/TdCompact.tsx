import { Td } from "@chakra-ui/react";

export const TdCompact = (props: any) => {
  const { children, ...other } = props;

  return (
    <Td textAlign='center'
      paddingTop='1' paddingBottom='1' paddingLeft='0' paddingRight='0'
      {...other}
    >
      {children}
    </Td>
  );
};

