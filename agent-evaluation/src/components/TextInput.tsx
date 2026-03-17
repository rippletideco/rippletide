import React, { useState } from 'react';
import { Box, Text } from 'ink';
import InkTextInput from 'ink-text-input';

interface TextInputProps {
  label: string;
  onSubmit: (value: string) => void;
  placeholder?: string;
}

export const TextInput: React.FC<TextInputProps> = ({ label, onSubmit, placeholder }) => {
  const [value, setValue] = useState('');

  return (
    <Box flexDirection="column">
      <Box marginBottom={1}>
        <Text dimColor>{label}</Text>
      </Box>
      <Box>
        <Text color="#eba1b5">&gt; </Text>
        <InkTextInput
          value={value}
          onChange={setValue}
          onSubmit={() => onSubmit(value)}
          placeholder={placeholder}
        />
      </Box>
    </Box>
  );
};
