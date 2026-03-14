import React, { useState, useEffect } from 'react';
import { Box, Text, useApp } from 'ink';
import { Header } from './components/Header.js';
import { TextInput } from './components/TextInput.js';
import { SelectMenu } from './components/SelectMenu.js';
import { Spinner } from './components/Spinner.js';
import { ProgressBar } from './components/ProgressBar.js';
import { Summary } from './components/Summary.js';
import { api, type CustomEndpointConfig } from './api/client.js';
import { getPineconeQAndA } from './utils/pinecone.js';
import { getPostgreSQLQAndA, parsePostgreSQLConnectionString, type PostgreSQLConfig } from './utils/postgresql.js';
import { BaseError, ValidationError } from './errors/types.js';
import { logger } from './utils/logger.js';
import { analytics } from './utils/analytics.js';

type Step = 
  | 'agent-endpoint' 
  | 'testing-connection'
  | 'connection-failed'
  | 'custom-config'
  | 'custom-headers'
  | 'custom-body'
  | 'custom-response'
  | 'checking-knowledge' 
  | 'select-source' 
  | 'pinecone-url'
  | 'pinecone-api-key'
  | 'fetching-pinecone'
  | 'postgresql-config'
  | 'fetching-postgresql'
  | 'pdf-path-input'
  | 'uploading-pdf'
  | 'running-evaluation' 
  | 'complete';

const knowledgeSources = [
  { label: 'Local Files (qanda.json)', value: 'files', description: 'Use a question-answer file (qanda.json) from the current directory' },
  { label: 'PDF Document', value: 'pdf', description: 'Upload and extract knowledge from a PDF file' },
  { label: 'Pinecone', value: 'pinecone', description: 'Fetch Q&A from Pinecone database' },
  { label: 'PostgreSQL Database', value: 'postgresql', description: 'Connect to PostgreSQL database' },
  { label: 'Current Repository', value: 'repo', description: 'Scan current git repository', disabled: true },
  { label: 'API Endpoint', value: 'api', description: 'Fetch from REST API', disabled: true },
  { label: 'GitHub Repository', value: 'github', description: 'Import from GitHub repo', disabled: true },
  { label: 'Skip (No Knowledge)', value: 'skip', description: 'Run tests without knowledge base', disabled: true },
];

interface AppProps {
  backendUrl?: string;
  dashboardUrl?: string;
  nonInteractive?: boolean;
  agentEndpoint?: string;
  knowledgeSource?: string;
  pineconeUrl?: string;
  pineconeApiKey?: string;
  postgresqlConnection?: string;
  pdfPath?: string;
  customHeaders?: Record<string, string>;
  customBodyTemplate?: string;
  customResponseField?: string;
  templatePath?: string;
  isRemoteTemplate?: boolean;
}

export const App: React.FC<AppProps> = ({ 
  backendUrl, 
  dashboardUrl,
  nonInteractive,
  agentEndpoint: initialAgentEndpoint,
  knowledgeSource: initialKnowledgeSource,
  pineconeUrl: initialPineconeUrl,
  pineconeApiKey: initialPineconeApiKey,
  postgresqlConnection: initialPostgresqlConnection,
  pdfPath: initialPdfPath,
  customHeaders,
  customBodyTemplate,
  customResponseField,
  templatePath,
  isRemoteTemplate
}) => {
  const { exit } = useApp();
  const initialStep = nonInteractive && initialAgentEndpoint ? 'testing-connection' : 'agent-endpoint';
  const [step, setStep] = useState<Step>(initialStep);
  const [agentEndpoint, setAgentEndpoint] = useState(initialAgentEndpoint || '');
  const [connectionTestResult, setConnectionTestResult] = useState<any>(null);
  const [customConfig, setCustomConfig] = useState<CustomEndpointConfig>({
    headers: customHeaders,
    bodyTemplate: customBodyTemplate,
    responseField: customResponseField
  });
  const [knowledgeSource, setKnowledgeSource] = useState(initialKnowledgeSource || '');
  const [knowledgeFound, setKnowledgeFound] = useState(false);
  const [pineconeUrl, setPineconeUrl] = useState(initialPineconeUrl || '');
  const [pineconeApiKey, setPineconeApiKey] = useState(initialPineconeApiKey || '');
  const [pineconeQAndA, setPineconeQAndA] = useState<Array<{question: string, answer: string}>>([]);
  const [pineconeProgress, setPineconeProgress] = useState('');
  const [postgresqlConnectionString, setPostgresqlConnectionString] = useState(initialPostgresqlConnection || '');
  const [postgresqlQAndA, setPostgresqlQAndA] = useState<Array<{question: string, answer: string}>>([]);
  const [postgresqlProgress, setPostgresqlProgress] = useState('');
  const [pdfPath, setPdfPath] = useState(initialPdfPath || '');
  const [pdfQAndA, setPdfQAndA] = useState<Array<{question: string, answer: string}>>([]);
  const [pdfProgress, setPdfProgress] = useState('');
  const [currentAgentId, setCurrentAgentId] = useState<string>('');
  const [evaluationProgress, setEvaluationProgress] = useState(0);
  const [evaluationResult, setEvaluationResult] = useState<any>(null);
  const [currentQuestion, setCurrentQuestion] = useState<string>('');
  const [currentLLMResponse, setCurrentLLMResponse] = useState<string>('');
  const [evaluationLogs, setEvaluationLogs] = useState<Array<{question: string, response: string}>>([]);

  useEffect(() => {
    if (backendUrl) {
      api.setBaseUrl(backendUrl);
    }
  }, [backendUrl]);

  useEffect(() => {
    if (step === 'complete' && nonInteractive) {
      setTimeout(() => {
        if (evaluationResult && !evaluationResult.error) {
          exit();
        } else {
          process.exit(1);
        }
      }, 1000);
    }
  }, [step, nonInteractive, evaluationResult, exit]);

  useEffect(() => {
    if (step === 'testing-connection') {
      (async () => {
        try {
          const result = customConfig.headers || customConfig.bodyTemplate 
            ? await api.testAgentConnectionWithConfig(agentEndpoint, customConfig)
            : await api.testAgentConnection(agentEndpoint);
          setConnectionTestResult(result);
          if (result.success) {
            setTimeout(() => {
              setStep('checking-knowledge');
            }, 1500);
          } else {
            if (nonInteractive) {
              console.error('Connection failed:', result.message);
              setTimeout(() => {
                setStep('checking-knowledge');
              }, 2000);
            } else {
              setTimeout(() => {
                setStep('connection-failed');
              }, 2000);
            }
          }
        } catch (error) {
          logger.error('Error testing connection:', error);
          const message = error instanceof BaseError ? error.userMessage : 'Connection test failed';
          setConnectionTestResult({ 
            success: false, 
            message 
          });
          if (nonInteractive) {
            console.error('Connection failed:', message);
            setTimeout(() => {
              setStep('checking-knowledge');
            }, 2000);
          } else {
            setTimeout(() => {
              setStep('connection-failed');
            }, 2000);
          }
        }
      })();
    }
  }, [step, agentEndpoint, customConfig]);

  useEffect(() => {
    if (step === 'checking-knowledge') {
      (async () => {
        try {
          const result = await api.checkKnowledge();
          setKnowledgeFound(result.found);
        } catch (error) {
          logger.error('Error checking knowledge:', error);
          setKnowledgeFound(false);
        }
        
        if (nonInteractive && knowledgeSource) {
          if (knowledgeSource === 'pinecone') {
            if (pineconeUrl && pineconeApiKey) {
              setStep('fetching-pinecone');
            } else {
              console.error('Pinecone URL and API key required for Pinecone source');
              process.exit(1);
            }
          } else if (knowledgeSource === 'postgresql') {
            if (postgresqlConnectionString) {
              setStep('fetching-postgresql');
            } else {
              console.error('PostgreSQL connection string required for PostgreSQL source');
              process.exit(1);
            }
          } else if (knowledgeSource === 'pdf') {
            if (pdfPath) {
              setStep('uploading-pdf');
            } else {
              console.error('PDF path required for PDF source');
              process.exit(1);
            }
          } else {
            setStep('running-evaluation');
          }
        } else if (nonInteractive && !knowledgeSource) {
          setKnowledgeSource('files');
          setStep('running-evaluation');
        } else {
          setStep('select-source');
        }
      })();
    }
  }, [step, nonInteractive, knowledgeSource, pineconeUrl, pineconeApiKey, postgresqlConnectionString]);

  useEffect(() => {
    if (step === 'fetching-pinecone') {
      (async () => {
        try {
          const qaPairs = await getPineconeQAndA(
            pineconeUrl,
            pineconeApiKey,
            (message) => setPineconeProgress(message)
          );
          setPineconeQAndA(qaPairs);
          analytics.track('evaluation_started', {
            knowledge_source: 'pinecone',
            qa_pairs_count: qaPairs.length,
          });
          setStep('running-evaluation');
        } catch (error: any) {
          logger.error('Error fetching Q&A from Pinecone:', error);
          const errorMessage = error instanceof BaseError ? error.userMessage : error.message;
          setEvaluationResult({
            totalTests: 0,
            passed: 0,
            failed: 0,
            duration: 'Failed',
            evaluationUrl: dashboardUrl || 'https://eval.rippletide.com',
            error: errorMessage,
          });
          setStep('complete');
        }
      })();
    }
  }, [step, pineconeUrl, pineconeApiKey]);

  useEffect(() => {
    if (step === 'fetching-postgresql') {
      (async () => {
        try {
          let config: PostgreSQLConfig;
          
          if (postgresqlConnectionString.startsWith('postgresql://') || postgresqlConnectionString.startsWith('postgres://')) {
            config = parsePostgreSQLConnectionString(postgresqlConnectionString);
          } else {
            const parts = postgresqlConnectionString.split(',');
            if (parts.length !== 5) {
              throw new ValidationError(
                'PostgreSQL connection',
                postgresqlConnectionString,
                'Expected format: host,port,database,user,password or postgresql://...'
              );
            }
            config = {
              host: parts[0].trim(),
              port: parseInt(parts[1].trim()),
              database: parts[2].trim(),
              user: parts[3].trim(),
              password: parts[4].trim()
            };
          }
          
          const qaPairs = await getPostgreSQLQAndA(
            config,
            backendUrl || 'https://agent-evalserver-production.up.railway.app',
            (message) => setPostgresqlProgress(message)
          );
          setPostgresqlQAndA(qaPairs);
          analytics.track('evaluation_started', {
            knowledge_source: 'postgresql',
            qa_pairs_count: qaPairs.length,
          });
          setStep('running-evaluation');
        } catch (error: any) {
          logger.error('Error fetching Q&A from PostgreSQL:', error);
          const errorMessage = error instanceof BaseError ? error.userMessage : error.message;
          setEvaluationResult({
            totalTests: 0,
            passed: 0,
            failed: 0,
            duration: 'Failed',
            evaluationUrl: dashboardUrl || 'https://eval.rippletide.com',
            error: errorMessage,
          });
          setStep('complete');
        }
      })();
    }
  }, [step, postgresqlConnectionString, backendUrl]);

  useEffect(() => {
    if (step === 'uploading-pdf') {
      (async () => {
        try {
          const startTime = Date.now();
          
          setPdfProgress('Generating API key...');
          await api.generateApiKey('CLI Evaluation');
          
          setPdfProgress('Creating agent...');
          const agent = await api.createAgent(agentEndpoint);
          const agentId = agent.id;
          setCurrentAgentId(agentId);
          
          setPdfProgress('Uploading PDF file...');
          const { uploadPdfToAgent } = await import('./api/knowledge.js');
          const { qaPairs } = await uploadPdfToAgent(
            agentId,
            pdfPath,
            (message) => setPdfProgress(message)
          );
          
          setPdfQAndA(qaPairs);
          analytics.track('evaluation_started', {
            knowledge_source: 'pdf',
            pdf_path: pdfPath,
            qa_pairs_count: qaPairs.length,
          });
          setStep('running-evaluation');
        } catch (error: any) {
          logger.error('Error uploading PDF:', error);
          const errorMessage = error instanceof BaseError ? error.userMessage : error.message;
          setEvaluationResult({
            totalTests: 0,
            passed: 0,
            failed: 0,
            duration: 'Failed',
            evaluationUrl: dashboardUrl || 'https://eval.rippletide.com',
            error: errorMessage,
          });
          setStep('complete');
        }
      })();
    }
  }, [step, pdfPath, agentEndpoint, dashboardUrl]);

  useEffect(() => {
    if (step === 'running-evaluation') {
      (async () => {
        try {
          const startTime = Date.now();
          const logs: Array<{question: string, response: string}> = [];
          
          setEvaluationProgress(5);
          await api.generateApiKey('CLI Evaluation');
          
          setEvaluationProgress(10);
          let agentId = currentAgentId;
          if (!agentId) {
            // Ensure we have a body template for the evaluation
            const effectiveBodyTemplate = customConfig.bodyTemplate || '{"message": "[eval-question]"}';
            const agent = await api.createAgent(agentEndpoint, effectiveBodyTemplate);
            agentId = agent.id;
            setCurrentAgentId(agentId);
          }
          
          setEvaluationProgress(30);
          
          // Import knowledge based on source
          let knowledgeData: any = null;

          if (knowledgeSource === 'files') {
            if (templatePath) {
              try {
                if (isRemoteTemplate) {
                  const axios = await import('axios');
                  const qandaUrl = `${templatePath}/qanda.json`;
                  const response = await axios.default.get(qandaUrl, { timeout: 5000 });
                  knowledgeData = response.data;
                } else {
                  const fs = await import('fs');
                  const path = await import('path');
                  const qandaPath = path.join(templatePath, 'qanda.json');
                  if (fs.existsSync(qandaPath)) {
                    knowledgeData = JSON.parse(fs.readFileSync(qandaPath, 'utf-8'));
                  }
                }
              } catch (error) {
                logger.debug('Error loading knowledge from template:', error);
              }
            } else {
              const knowledgeResult = await api.checkKnowledge();
              if (knowledgeResult.found && knowledgeResult.path) {
                try {
                  const fs = await import('fs');
                  knowledgeData = JSON.parse(fs.readFileSync(knowledgeResult.path, 'utf-8'));
                } catch (error) {
                  logger.debug('Error loading knowledge:', error);
                }
              }
            }
          } else if (knowledgeSource === 'pinecone' && pineconeQAndA.length > 0) {
            // Import Pinecone Q&A as knowledge
            knowledgeData = pineconeQAndA;
          } else if (knowledgeSource === 'postgresql' && postgresqlQAndA.length > 0) {
            // Import PostgreSQL Q&A as knowledge
            knowledgeData = postgresqlQAndA;
          } else if (knowledgeSource === 'pdf' && pdfQAndA.length > 0) {
            // Import PDF Q&A as knowledge
            knowledgeData = pdfQAndA;
          }

          if (knowledgeData && Array.isArray(knowledgeData) && knowledgeData.length > 0) {
            setEvaluationProgress(35);
            try {
              const importResult = await api.importKnowledge(agentId, knowledgeData);
              logger.debug('Knowledge import result:', importResult);
              await new Promise(resolve => setTimeout(resolve, 1000));
            } catch (error: any) {
              logger.error('Failed to import knowledge:', error?.message || error);
              logger.debug('Import error details:', error?.response?.data);
            }
          }
          
          setEvaluationProgress(40);
          let testPrompts: Array<{question: string, answer?: string}> | string[] = [];
          if (knowledgeSource === 'files') {
            if (templatePath) {
              try {
                let knowledgeData: any = null;
                if (isRemoteTemplate) {
                  const axios = await import('axios');
                  const qandaUrl = `${templatePath}/qanda.json`;
                  const response = await axios.default.get(qandaUrl, { timeout: 5000 });
                  knowledgeData = response.data;
                } else {
                  const fs = await import('fs');
                  const path = await import('path');
                  const qandaPath = path.join(templatePath, 'qanda.json');
                  if (fs.existsSync(qandaPath)) {
                    knowledgeData = JSON.parse(fs.readFileSync(qandaPath, 'utf-8'));
                  } else {
                    logger.debug('No qanda.json found in template directory:', templatePath);
                  }
                }
                
                if (knowledgeData && Array.isArray(knowledgeData)) {
                  testPrompts = knowledgeData.map((item: any) => ({
                    question: item.question || item.prompt || item.input || 'Test question',
                    answer: item.answer || item.response || item.expectedAnswer
                  }));
                }
              } catch (error) {
                logger.debug('Error loading prompts from template:', error);
                testPrompts = [];
              }
            } else {
              const knowledgeResult = await api.checkKnowledge();
              if (knowledgeResult.found && knowledgeResult.path) {
                try {
                  const fs = await import('fs');
                  const knowledgeData = JSON.parse(fs.readFileSync(knowledgeResult.path, 'utf-8'));
                  if (Array.isArray(knowledgeData)) {
                    testPrompts = knowledgeData.slice(0, 5).map((item: any) => ({
                      question: item.question || item.prompt || item.input || 'Test question',
                      answer: item.answer || item.response || item.expectedAnswer
                    }));
                  }
                } catch (error) {
                  logger.debug('Error loading prompts from knowledge:', error);
                  testPrompts = [];
                }
              }
            }
          } else if (knowledgeSource === 'pinecone' && pineconeQAndA.length > 0) {
            testPrompts = pineconeQAndA.slice(0, 5).map((item) => ({
              question: item.question,
              answer: item.answer
            }));
          } else if (knowledgeSource === 'postgresql' && postgresqlQAndA.length > 0) {
            testPrompts = postgresqlQAndA.slice(0, 5).map((item) => ({
              question: item.question,
              answer: item.answer
            }));
          } else if (knowledgeSource === 'pdf' && pdfQAndA.length > 0) {
            testPrompts = pdfQAndA.slice(0, 5).map((item) => ({
              question: item.question,
              answer: item.answer
            }));
          }
          
          const createdPrompts = await api.addTestPrompts(agentId, testPrompts);
          
          setEvaluationProgress(50);
          const evalConfig = {
            ...customConfig,
            bodyTemplate: customConfig.bodyTemplate || '{"message": "[eval-question]"}'
          };
          const evaluationResults = await api.runAllPromptEvaluations(
            agentId,
            createdPrompts,
            agentEndpoint,
            (current, total, question, llmResponse) => {
              const progress = 50 + Math.round((current / total) * 40);
              setEvaluationProgress(progress);
              
              if (question) {
                setCurrentQuestion(question);
              }
              if (llmResponse) {
                setCurrentLLMResponse(llmResponse);
                logs.push({ question: question || '', response: llmResponse });
                setEvaluationLogs([...logs]);
              }
            },
            evalConfig
          );
          
          setEvaluationProgress(100);
          
          let passed = 0;
          let failed = 0;
          
          evaluationResults.forEach((result: any) => {
            if (result.success) {
              passed++;
            } else {
              failed++;
            }
          });
          
          const duration = Math.round((Date.now() - startTime) / 1000);
          const durationStr = duration > 60 
            ? `${Math.floor(duration / 60)}m ${duration % 60}s`
            : `${duration}s`;
          
          const result = {
            totalTests: createdPrompts.length,
            passed,
            failed,
            duration: durationStr,
            evaluationUrl: `${dashboardUrl || 'https://eval.rippletide.com'}/eval/${agentId}`,
            agentId,
          };
          
          analytics.track('evaluation_completed', {
            total_tests: createdPrompts.length,
            passed_tests: passed,
            failed_tests: failed,
            duration_ms: Date.now() - startTime,
            knowledge_source: knowledgeSource,
            success_rate: createdPrompts.length > 0 ? (passed / createdPrompts.length) * 100 : 0,
          });
          
          setEvaluationResult(result);
          setStep('complete');
        } catch (error) {
          logger.error('Error running evaluation:', error);
          const errorMessage = error instanceof BaseError ? error.userMessage : 'Evaluation failed';
          setEvaluationResult({
            totalTests: 0,
            passed: 0,
            failed: 0,
            duration: 'Failed',
            evaluationUrl: dashboardUrl || 'https://eval.rippletide.com',
            error: errorMessage,
          });
          setStep('complete');
        }
      })();
    }
  }, [step, agentEndpoint, knowledgeSource, pineconeQAndA, postgresqlQAndA, pdfQAndA, currentAgentId]);

  const handleAgentEndpointSubmit = (value: string) => {
    const trimmedValue = value.trim();
    if (!trimmedValue) {
      return;
    }
    setAgentEndpoint(trimmedValue);
    setStep('testing-connection');
  };

  const handleSourceSelect = (value: string) => {
    setKnowledgeSource(value);
    if (value === 'pinecone') {
      setStep('pinecone-url');
    } else if (value === 'postgresql') {
      setStep('postgresql-config');
    } else if (value === 'pdf') {
      setStep('pdf-path-input');
    } else {
      setStep('running-evaluation');
    }
  };

  const handlePineconeUrlSubmit = (value: string) => {
    setPineconeUrl(value);
    setStep('pinecone-api-key');
  };

  const handlePineconeApiKeySubmit = (value: string) => {
    setPineconeApiKey(value);
    setStep('fetching-pinecone');
  };

  const handlePostgresqlConnectionSubmit = (value: string) => {
    setPostgresqlConnectionString(value);
    setStep('fetching-postgresql');
  };

  const handlePdfPathSubmit = (value: string) => {
    setPdfPath(value);
    setStep('uploading-pdf');
  };

  return (
    <Box flexDirection="column" padding={1}>
      <Header />

      {step === 'agent-endpoint' && (
        <Box flexDirection="column">
          <Box marginBottom={1}>
            <Text color="#eba1b5">Enter your agent's endpoint URL</Text>
          </Box>
          <Box marginBottom={1}>
            <Text dimColor>Examples:</Text>
            <Box paddingLeft={2} flexDirection="column">
                  <Text dimColor>- localhost:8000</Text>
                  <Text dimColor>- http://localhost:8000</Text>
                  <Text dimColor>- https://my-agent.vercel.app</Text>
            </Box>
          </Box>
          <TextInput
            label="Agent endpoint"
            placeholder="localhost:8000"
            onSubmit={handleAgentEndpointSubmit}
          />
        </Box>
      )}

      {step === 'testing-connection' && (
        <Box flexDirection="column">
          <Box marginBottom={1}>
            <Spinner label="Testing agent connection..." />
          </Box>
          {connectionTestResult && (
            <Box marginTop={1}>
              <Text color={connectionTestResult.success ? "green" : "red"}>
                {connectionTestResult.message}
              </Text>
            </Box>
          )}
        </Box>
      )}

      {step === 'connection-failed' && (
        <Box flexDirection="column">
          <Box marginBottom={1}>
            <Text color="red">Connection failed</Text>
          </Box>
          <Box marginBottom={1}>
            <Text>{connectionTestResult?.message || 'Could not connect to the agent'}</Text>
          </Box>
          <Box marginBottom={1}>
            <Text color="#eba1b5">What would you like to do?</Text>
          </Box>
          <SelectMenu
            title="Options"
            options={[
              { label: 'Try again with same endpoint', value: 'retry', description: 'Test connection again' },
              { label: 'Change endpoint URL', value: 'change', description: 'Enter a different endpoint' },
              { label: 'Configure custom parameters', value: 'custom', description: 'Add headers, auth, custom body format' },
              { label: 'Continue anyway', value: 'continue', description: 'Skip connection test (not recommended)' },
            ]}
            onSelect={(value) => {
              if (value === 'retry') {
                setStep('testing-connection');
              } else if (value === 'change') {
                setStep('agent-endpoint');
              } else if (value === 'custom') {
                setStep('custom-config');
              } else if (value === 'continue') {
                setStep('checking-knowledge');
              }
            }}
          />
        </Box>
      )}

      {step === 'custom-config' && (
        <Box flexDirection="column">
          <Box marginBottom={1}>
            <Text color="#eba1b5">Configure custom endpoint parameters</Text>
          </Box>
          <Box marginBottom={1}>
            <Text dimColor>Choose what to configure:</Text>
          </Box>
          <SelectMenu
            title="Configuration"
            options={[
              { label: 'Add custom headers', value: 'headers', description: 'Authorization, API keys, etc.' },
              { label: 'Custom request body', value: 'body', description: 'Define exact request structure' },
              { label: 'Custom response field', value: 'response', description: 'Specify where to find the answer' },
              { label: 'Test with current config', value: 'test', description: 'Try connection with custom settings' },
              { label: 'Back', value: 'back', description: 'Return to previous menu' },
            ]}
            onSelect={(value) => {
              if (value === 'headers') {
                setStep('custom-headers');
              } else if (value === 'body') {
                setStep('custom-body');
              } else if (value === 'response') {
                setStep('custom-response');
              } else if (value === 'test') {
                setStep('testing-connection');
              } else if (value === 'back') {
                setStep('connection-failed');
              }
            }}
          />
          {(customConfig.headers || customConfig.bodyTemplate || customConfig.responseField) && (
            <Box marginTop={1} flexDirection="column">
              <Text color="green">Current configuration:</Text>
              {customConfig.headers && Object.keys(customConfig.headers).length > 0 && (
                <Text dimColor>- Headers: {Object.keys(customConfig.headers).join(', ')}</Text>
              )}
              {customConfig.bodyTemplate && (
                <Text dimColor>- Custom body template configured</Text>
              )}
              {customConfig.responseField && (
                <Text dimColor>- Response field: {customConfig.responseField}</Text>
              )}
            </Box>
          )}
        </Box>
      )}

      {step === 'custom-headers' && (
        <Box flexDirection="column">
          <Box marginBottom={1}>
            <Text color="#eba1b5">Add custom headers</Text>
          </Box>
          <Box marginBottom={1}>
            <Text dimColor>Format: Header-Name: value (one per line, or comma-separated)</Text>
            <Text dimColor>Examples:</Text>
            <Box paddingLeft={2} flexDirection="column">
              <Text dimColor>- Authorization: Bearer sk-xxxxx</Text>
              <Text dimColor>- X-API-Key: your-api-key</Text>
              <Text dimColor>- Content-Type: application/json</Text>
            </Box>
          </Box>
          <TextInput
            label="Headers (press Enter when done)"
            placeholder="Authorization: Bearer token, X-API-Key: key"
            onSubmit={(value) => {
              const headers: Record<string, string> = {};
              const headerPairs = value.split(/[,\n]+/);
              headerPairs.forEach(pair => {
                const [key, ...valueParts] = pair.split(':');
                if (key && valueParts.length > 0) {
                  headers[key.trim()] = valueParts.join(':').trim();
                }
              });
              setCustomConfig(prev => ({ ...prev, headers }));
              setStep('custom-config');
            }}
          />
        </Box>
      )}

      {step === 'custom-body' && (
        <Box flexDirection="column">
          <Box marginBottom={1}>
            <Text color="#eba1b5">Define custom request body</Text>
          </Box>
          <Box marginBottom={1}>
            <Text dimColor>Use {'{question}'} as placeholder for the user's question</Text>
            <Text dimColor>Examples:</Text>
            <Box paddingLeft={2} flexDirection="column">
              <Text dimColor>- {'{"prompt": "{question}"}'}</Text>
              <Text dimColor>- {'{"messages": [{"role": "user", "content": "{question}"}]}'}</Text>
              <Text dimColor>- {'{"input": {"text": "{question}"}}'}</Text>
            </Box>
          </Box>
          <TextInput
            label="Body template (JSON format)"
            placeholder='{"prompt": "{question}"}'
            onSubmit={(value) => {
              setCustomConfig(prev => ({ ...prev, bodyTemplate: value }));
              setStep('custom-config');
            }}
          />
        </Box>
      )}

      {step === 'custom-response' && (
        <Box flexDirection="column">
          <Box marginBottom={1}>
            <Text color="#eba1b5">Specify response field</Text>
          </Box>
          <Box marginBottom={1}>
            <Text dimColor>Where to find the answer in the response?</Text>
            <Text dimColor>Examples:</Text>
            <Box paddingLeft={2} flexDirection="column">
              <Text dimColor>- answer</Text>
              <Text dimColor>- data.response</Text>
              <Text dimColor>- choices[0].message.content</Text>
              <Text dimColor>- result.text</Text>
            </Box>
          </Box>
          <TextInput
            label="Response field path"
            placeholder="answer"
            onSubmit={(value) => {
              setCustomConfig(prev => ({ ...prev, responseField: value }));
              setStep('custom-config');
            }}
          />
        </Box>
      )}

      {step === 'checking-knowledge' && (
        <Box flexDirection="column">
          <Spinner label="Checking for knowledge base in current folder..." />
        </Box>
      )}

      {step === 'select-source' && (
        <Box flexDirection="column">
          <Box marginBottom={1}>
            <Text bold color="#eba1b5">Choose your data source:</Text>
          </Box>
          {knowledgeFound && (
            <Box marginBottom={1}>
              <Text color="white">qanda.json found in current directory</Text>
            </Box>
          )}
          <SelectMenu
            title="Data Source"
            options={knowledgeSources}
            onSelect={handleSourceSelect}
          />
          <Box marginTop={1} flexDirection="column">
            <Text dimColor>Example qanda.json format:</Text>
            <Box paddingLeft={2} flexDirection="column">
              <Text dimColor>{`[ { "question": "What are your hours?", "answer": "Mon-Fri 9am-6pm" } ]`}</Text>
            </Box>
          </Box>
        </Box>
      )}

      {step === 'pinecone-url' && (
        <Box flexDirection="column">
          <TextInput
            label="Pinecone database URL"
            placeholder="https://sample-movies-02j22s8.svc.aped-4627-b74a.pinecone.io"
            onSubmit={handlePineconeUrlSubmit}
          />
        </Box>
      )}

      {step === 'pinecone-api-key' && (
        <Box flexDirection="column">
          <TextInput
            label="Pinecone API key"
            placeholder="pcsk_..."
            onSubmit={handlePineconeApiKeySubmit}
          />
        </Box>
      )}

      {step === 'fetching-pinecone' && (
        <Box flexDirection="column">
          <Spinner label={pineconeProgress || "Fetching Q&A from Pinecone..."} />
        </Box>
      )}

      {step === 'postgresql-config' && (
        <Box flexDirection="column">
          <Box marginBottom={1}>
            <Text color="#eba1b5">Enter PostgreSQL connection details</Text>
          </Box>
          <Box marginBottom={1}>
            <Text dimColor>Format 1: postgresql://user:password@host:port/database</Text>
          </Box>
          <Box marginBottom={1}>
            <Text dimColor>Format 2: host,port,database,user,password</Text>
          </Box>
          <TextInput
            label="PostgreSQL connection"
            placeholder="postgresql://postgres:password@localhost:5432/mydb"
            onSubmit={handlePostgresqlConnectionSubmit}
          />
        </Box>
      )}

      {step === 'fetching-postgresql' && (
        <Box flexDirection="column">
          <Spinner label={postgresqlProgress || "Analyzing PostgreSQL database..."} />
        </Box>
      )}

      {step === 'pdf-path-input' && (
        <Box flexDirection="column">
          <Box marginBottom={1}>
            <Text color="#eba1b5">Enter the path to your PDF file</Text>
          </Box>
          <Box marginBottom={1}>
            <Text dimColor>Examples:</Text>
            <Box paddingLeft={2} flexDirection="column">
              <Text dimColor>- ./docs/manual.pdf</Text>
              <Text dimColor>- /home/user/documents/guide.pdf</Text>
              <Text dimColor>- ../knowledge/faq.pdf</Text>
            </Box>
          </Box>
          <TextInput
            label="PDF path"
            placeholder="./document.pdf"
            onSubmit={handlePdfPathSubmit}
          />
        </Box>
      )}

      {step === 'uploading-pdf' && (
        <Box flexDirection="column">
          <Spinner label={pdfProgress || "Uploading PDF file..."} />
        </Box>
      )}

      {step === 'running-evaluation' && (
        <Box flexDirection="column">
          <Box marginBottom={2}>
            <Spinner label="Running evaluation" />
          </Box>
          
          <Box flexDirection="column" marginBottom={1}>
            <Box>
              <Box width={12}>
                <Text dimColor>Endpoint:</Text>
              </Box>
              <Text>{agentEndpoint}</Text>
            </Box>
            
            <Box>
              <Box width={12}>
                <Text dimColor>Data Source:</Text>
              </Box>
              <Text>{knowledgeSources.find(s => s.value === knowledgeSource)?.label || 'None'}</Text>
            </Box>
          </Box>

          <ProgressBar 
            progress={evaluationProgress}
          />
          
          {currentQuestion && (
            <Box flexDirection="column" marginTop={1} marginBottom={1}>
              <Box>
                <Text bold color="#eba1b5">Current Question:</Text>
              </Box>
              <Box paddingLeft={2}>
                <Text color="white">{currentQuestion}</Text>
              </Box>
            </Box>
          )}

          {currentLLMResponse && (
            <Box flexDirection="column" marginTop={1}>
              <Box>
                <Text bold color="green">Agent Response:</Text>
              </Box>
              <Box paddingLeft={2}>
                <Text>{currentLLMResponse.length > 200 ? currentLLMResponse.substring(0, 200) + '...' : currentLLMResponse}</Text>
              </Box>
            </Box>
          )}
        </Box>
      )}

      {step === 'complete' && evaluationResult && (
        <Box flexDirection="column">
          {evaluationResult.error ? (
            <Box flexDirection="column">
              <Text color="red">Error: Evaluation Failed</Text>
              <Text>{evaluationResult.error}</Text>
              <Box marginTop={1}>
                <Text dimColor>Run with --debug flag for more details</Text>
              </Box>
            </Box>
          ) : (
            <Summary
              totalTests={evaluationResult.totalTests}
              passed={evaluationResult.passed}
              failed={evaluationResult.failed}
              duration={evaluationResult.duration}
              evaluationUrl={evaluationResult.evaluationUrl}
            />
          )}
        </Box>
      )}
    </Box>
  );
};
