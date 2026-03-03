import React from 'react';
import { Box, Text } from 'ink';

export const Header: React.FC = () => {
  return (
    <Box flexDirection="column" marginBottom={2}>
      <Text bold color="#eba1b5">Rippletide Evaluation</Text>
      <Text bold color="#eba1b5">How It Works</Text>
      <Text dimColor>1. Connect your endpoint  2. Add your knowledge source  3. Run the evaluation</Text>
      <Text color="gray">━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</Text>
    </Box>
  );
};
