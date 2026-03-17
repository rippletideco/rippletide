import React from 'react';
import { Box, Text } from 'ink';

interface ProgressBarProps {
  progress: number; // 0-100
  label?: string;
}

export const ProgressBar: React.FC<ProgressBarProps> = ({ progress, label }) => {
  const width = 40;
  const filled = Math.round((progress / 100) * width);
  const empty = width - filled;

  return (
    <Box flexDirection="column" marginY={1}>
      {label && (
        <Box marginBottom={1}>
          <Text dimColor>{label}</Text>
        </Box>
      )}
      <Box>
        <Text color="#eba1b5">{'='.repeat(filled)}</Text>
        <Text color="gray">{'-'.repeat(empty)}</Text>
        <Text> </Text>
        <Text color="#eba1b5">{progress.toFixed(0)}%</Text>
      </Box>
    </Box>
  );
};
