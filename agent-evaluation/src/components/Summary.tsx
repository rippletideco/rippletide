import React from 'react';
import { Box, Text } from 'ink';

interface SummaryProps {
  totalTests: number;
  passed: number;
  failed: number;
  duration: string;
  evaluationUrl: string;
}

export const Summary: React.FC<SummaryProps> = ({ 
  totalTests, 
  passed, 
  failed, 
  duration,
  evaluationUrl 
}) => {
  const passRate = ((passed / totalTests) * 100).toFixed(1);

  return (
    <Box flexDirection="column">
      <Box marginBottom={2}>
        <Text bold color="#eba1b5">Evaluation Complete</Text>
      </Box>
      
      <Box flexDirection="column" marginBottom={2}>
        <Box>
          <Box width={12}>
            <Text dimColor>Tests:</Text>
          </Box>
          <Text>{totalTests}</Text>
        </Box>
        
        <Box>
          <Box width={12}>
            <Text dimColor>Passed:</Text>
          </Box>
          <Text>{passed}</Text>
        </Box>
        
        <Box>
          <Box width={12}>
            <Text dimColor>Failed:</Text>
          </Box>
          <Text>{failed}</Text>
        </Box>
        
        <Box>
          <Box width={12}>
            <Text dimColor>Success:</Text>
          </Box>
          <Text>{passRate}%</Text>
        </Box>
        
        <Box>
          <Box width={12}>
            <Text dimColor>Duration:</Text>
          </Box>
          <Text>{duration}</Text>
        </Box>
      </Box>

      <Box flexDirection="column">
        <Text dimColor>Full results:</Text>
        <Text color="#eba1b5">{evaluationUrl}</Text>
      </Box>
    </Box>
  );
};
