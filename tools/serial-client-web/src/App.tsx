import {
  Box, Stack, Tab, TabList, TabPanel, TabPanels, Tabs,
} from '@chakra-ui/react';

import { Buttons } from './Buttons';
import { CoprocRegisters } from './CoprocRegisters';
import { CPSR } from './CPSR';
import { Memory } from './Memory';

function App() {
  return (
    <Stack direction="row">
      <Box flexGrow={1}/>
      <Stack width="1000px" padding={2}>
        <Buttons/>

        <Tabs>
          <TabList>
            {["Memory", "Coproc Registers", "CPSR"].map(str => (
              <Tab key={str}>{str}</Tab>
            ))}
          </TabList>

          <TabPanels>
            {[Memory, CoprocRegisters, CPSR].map((Component, index) => (
              <TabPanel key={index} paddingLeft='0' paddingRight='0' paddingBottom='0'>
                <Component/>
              </TabPanel>
            ))}
          </TabPanels>
        </Tabs>
      </Stack>
      <Box flexGrow={1}/>
    </Stack>
  );
}

export default App;
