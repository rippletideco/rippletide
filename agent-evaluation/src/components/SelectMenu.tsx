import React, { useState } from 'react';
import { Box, Text, useInput } from 'ink';

interface SelectOption {
  label: string;
  value: string;
  description?: string;
  disabled?: boolean;
}

interface SelectMenuProps {
  title: string;
  options: SelectOption[];
  onSelect: (value: string) => void;
}

export const SelectMenu: React.FC<SelectMenuProps> = ({ title, options, onSelect }) => {
  const [selectedIndex, setSelectedIndex] = useState(0);

  const getNextEnabledIndex = (start: number, direction: 1 | -1) => {
    for (let i = 1; i <= options.length; i++) {
      const idx = (start + direction * i + options.length) % options.length;
      if (!options[idx].disabled) {
        return idx;
      }
    }
    return start;
  };

  useInput((input, key) => {
    if (key.upArrow) {
      setSelectedIndex((prev) => getNextEnabledIndex(prev, -1));
    } else if (key.downArrow) {
      setSelectedIndex((prev) => getNextEnabledIndex(prev, 1));
    } else if (key.return) {
      const option = options[selectedIndex];
      if (!option.disabled) {
        onSelect(option.value);
      }
    }
  });

  return (
    <Box flexDirection="column">
      <Box marginBottom={1}>
        <Text color="white" dimColor>{title}</Text>
      </Box>
      {options.map((option, index) => (
        <Box key={option.value} flexDirection="column" marginBottom={0}>
          <Box>
            <Text color={option.disabled ? '#d0c0cf' : 'white'}>
              {index === selectedIndex ? '> ' : '  '}
            </Text>
            <Text
              color={
                option.disabled
                  ? '#d0c0cf'
                  : index === selectedIndex
                    ? '#eba1b5'
                    : 'white'
              }
              bold={index === selectedIndex && !option.disabled}
            >
              {option.label}
            </Text>
          </Box>
          {option.description && (
            <Box paddingLeft={2}>
              <Text color={option.disabled ? '#d0c0cf' : 'white'} dimColor={option.disabled}>  {option.description}</Text>
            </Box>
          )}
        </Box>
      ))}
      <Box marginTop={1}>
        <Text color="white" dimColor>
          Up/Down to navigate - Enter to select (gray = coming soon)
        </Text>
      </Box>
    </Box>
  );
};
