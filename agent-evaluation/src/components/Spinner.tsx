import React, { useState, useEffect } from 'react';
import { Box, Text } from 'ink';

interface SpinnerProps {
  label: string;
}

const spinnerFrames = ['|', '/', '-', '\\'];

export const Spinner: React.FC<SpinnerProps> = ({ label }) => {
  const [frame, setFrame] = useState(0);

  useEffect(() => {
    const timer = setInterval(() => {
      setFrame((prevFrame) => (prevFrame + 1) % spinnerFrames.length);
    }, 100);

    return () => {
      clearInterval(timer);
    };
  }, []);

  return (
    <Box>
      <Text color="#eba1b5">{spinnerFrames[frame]} </Text>
      <Text dimColor>{label}</Text>
    </Box>
  );
};
