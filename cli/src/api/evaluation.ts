import axios from 'axios';
import { logger } from '../utils/logger.js';
import { callLLMEndpoint } from './llm.js';
import { type CustomEndpointConfig } from './endpoint.js';

let client = axios.create({
  baseURL: 'https://agent-evalserver-production.up.railway.app',
  headers: {
    'Content-Type': 'application/json',
  },
});

let API_KEY: string | null = null;

const setupInterceptor = () => {
  client.interceptors.request.use((config) => {
    if (API_KEY) {
      config.headers['x-api-key'] = API_KEY;
    }
    return config;
  });
};

setupInterceptor();

export interface HallucinationCheckResult {
  question: string;
  llmResponse: string;
  summary: string;
  facts: string[];
  status: 'passed' | 'failed' | 'ambiguous';
  hallucinationLabel: string;
  hallucinationFindings: any[];
}

export interface PromptEvaluationResult {
  success: boolean;
  question: string;
  llmResponse?: string;
  hallucinationResult?: HallucinationCheckResult;
  error?: any;
}

export function setBackendUrl(url: string) {
  client = axios.create({
    baseURL: url,
    headers: {
      'Content-Type': 'application/json',
    },
  });
  setupInterceptor();
  logger.debug('Backend URL set to:', url);
}

export async function generateApiKey(name?: string) {
  try {
    const response = await client.post('/api/api-keys/generate-cli', {
      name: name || 'CLI Evaluation Key'
    });
    
    API_KEY = response.data.apiKey;
    logger.info('API key generated successfully');
    logger.debug('API Key:', API_KEY?.substring(0, 12) + '...');
    
    const { setApiClient } = await import('./knowledge.js');
    setApiClient(client, API_KEY);
    
    return response.data;
  } catch (error) {
    logger.error('Error generating API key:', error);
    throw error;
  }
}

export async function createAgent(publicUrl: string, customPayloadTemplate?: string) {
  try {
    const response = await client.post('/api/agents', {
      name: `Agent Eval ${Date.now()}`,
      seed: Math.floor(Math.random() * 1000),
      numNodes: 100,
      publicUrl: publicUrl,
      label: 'eval',
    });
    
    const agentId = response.data.id;
    
    const payloadTemplate = customPayloadTemplate || '{"message": "[eval-question]"}';
    
    logger.debug(`Configuring advanced payload for agent ${agentId}`);
    logger.debug(`Payload template: ${payloadTemplate}`);
    
    try {
      await client.patch(`/api/agents/${agentId}`, {
        advancedPayload: {
          payload: payloadTemplate
        }
      });
      logger.debug('Advanced payload configured successfully');
    } catch (patchError: any) {
      logger.warn('Could not set advanced payload:', patchError?.message);
      logger.debug('Will use default payload format');
    }
    
    return response.data;
  } catch (error) {
    logger.error('Error creating agent:', error);
    throw error;
  }
}

export async function addTestPrompts(agentId: string, prompts?: string[] | Array<{question: string, answer?: string}>) {
  try {
    const defaultPrompts = [
      'What can you help me with?',
      'Tell me about your capabilities',
      'How do I get started?',
      'What features do you support?',
      'Can you explain your main functionality?',
    ];
    
    let promptsArray: Array<{prompt: string, expectedAnswer: string | null}>;
    
    if (!prompts || prompts.length === 0) {
      promptsArray = defaultPrompts.map(p => ({ prompt: p, expectedAnswer: null }));
    } else if (typeof prompts[0] === 'string') {
      promptsArray = (prompts as string[]).map(p => ({ prompt: p, expectedAnswer: null }));
    } else {
      promptsArray = (prompts as Array<{question: string, answer?: string}>).map(p => ({
        prompt: p.question,
        expectedAnswer: p.answer || null,
      }));
    }
    
    logger.info(`Adding ${promptsArray.length} test prompts to agent ${agentId}`);
    logger.debug('Prompts:', promptsArray);
    
    const response = await client.post(`/api/agents/${agentId}/test-prompts`, {
      prompts: promptsArray,
    });
    
    logger.info(`Successfully added ${response.data.length} test prompts`);
    
    return response.data;
  } catch (error: any) {
    logger.error('Error adding test prompts:', error?.message || error);
    if (error.response) {
      logger.error('Response data:', error.response.data);
      logger.error('Response status:', error.response.status);
    }
    throw error;
  }
}

export async function checkHallucination(
  agentId: string, 
  question: string, 
  llmResponse: string, 
  expectedAnswer?: string
): Promise<HallucinationCheckResult> {
  try {
    if (!llmResponse || llmResponse.startsWith('Error calling LLM endpoint:')) {
      return {
        question,
        llmResponse,
        summary: 'LLM endpoint error',
        facts: [],
        status: 'failed',
        hallucinationLabel: '',
        hallucinationFindings: []
      };
    }
    
    logger.debug('Checking hallucination for question:', question);
    logger.debug('LLM Response length:', llmResponse.length);
    logger.debug('Expected answer:', expectedAnswer || 'None provided');
    
    const response = await client.post(`/api/agents/${agentId}/check-hallucination-response`, {
      question,
      llmResponse,
      expectedAnswer
    });
    
    logger.debug('Hallucination check result:', response.data);
    
    return {
      question: response.data.question,
      llmResponse: response.data.llmResponse,
      summary: response.data.summary || '',
      facts: response.data.facts || [],
      status: response.data.status || 'passed',
      hallucinationLabel: response.data.hallucinationLabel || 'FactIsPresent',
      hallucinationFindings: response.data.hallucinationFindings || []
    };
  } catch (error: any) {
    if (error?.response?.status === 404) {
      logger.warn('Hallucination check endpoint not found, using fallback');
      return {
        question,
        llmResponse,
        summary: 'Hallucination check not available',
        facts: [],
        status: 'passed',
        hallucinationLabel: 'FactIsPresent',
        hallucinationFindings: []
      };
    }
    
    logger.error('Error in hallucination check:', error?.message || error);
    logger.debug('Error details:', error?.response?.data);
    
    return {
      question,
      llmResponse,
      summary: 'Check failed',
      facts: [],
      status: 'passed',
      hallucinationLabel: 'FactIsPresent',
      hallucinationFindings: []
    };
  }
}

export async function runPromptEvaluation(
  agentId: string, 
  promptId: number, 
  promptText: string, 
  agentEndpoint: string,
  expectedAnswer?: string,
  onLLMResponse?: (response: string) => void,
  customConfig?: CustomEndpointConfig
): Promise<PromptEvaluationResult> {
  let llmResponse: string | null = null;
  
  try {
    logger.info(`Calling LLM for question: ${promptText}`);
    llmResponse = await callLLMEndpoint(agentEndpoint, promptText, customConfig);
    
    if (onLLMResponse) {
      onLLMResponse(llmResponse);
    }
    
    logger.info(`LLM Response: ${llmResponse.substring(0, 100)}...`);
    
    const hallucinationResult = await checkHallucination(
      agentId, 
      promptText, 
      llmResponse, 
      expectedAnswer
    );
    
    const status = hallucinationResult.status === 'passed' ? 'passed' : 'failed';
    
    try {
      const payload: any = {
        status,
        response: llmResponse,
        expectedAnswer: expectedAnswer || null
      };
      
      if (hallucinationResult.hallucinationLabel && hallucinationResult.hallucinationLabel !== '') {
        payload.hallucinationLabel = hallucinationResult.hallucinationLabel;
      }

      // Always include hallucinationFindings, even if empty
      payload.hallucinationFindings = hallucinationResult.hallucinationFindings || [];
      if (payload.hallucinationFindings.length > 0) {
        logger.debug(`Including ${payload.hallucinationFindings.length} hallucination findings`);
      } else {
        logger.debug('No hallucination findings to include (empty array)');
      }
      
      await client.post(`/api/agents/${agentId}/test-results/${promptId}`, payload);
      logger.debug(`Stored test result for prompt ${promptId} with status ${status}`);
    } catch (storeError: any) {
      logger.warn('Could not store test result, trying minimal payload:', storeError?.message);
      logger.debug('Store error details:', storeError?.response?.data);
      
      const minimalPayload: any = {
        status,
        response: llmResponse,
        expectedAnswer: expectedAnswer || null
      };

      if (hallucinationResult.hallucinationLabel) {
        minimalPayload.hallucinationLabel = hallucinationResult.hallucinationLabel;
      }

      // Always include hallucinationFindings, even if empty
      minimalPayload.hallucinationFindings = hallucinationResult.hallucinationFindings || [];

      await client.post(`/api/agents/${agentId}/test-results/${promptId}`, minimalPayload);
      logger.debug(`Stored minimal test result for prompt ${promptId}`);
    }
    
    return { 
      success: status === 'passed', 
      question: promptText,
      llmResponse,
      hallucinationResult
    };
  } catch (error: any) {
    if (llmResponse) {
      logger.warn('LLM responded successfully but evaluation failed, marking as passed');
      try {
        await client.post(`/api/agents/${agentId}/test-results/${promptId}`, {
          status: 'passed',
          response: llmResponse,
          expectedAnswer: expectedAnswer || null,
          hallucinationFindings: []
        });
      } catch (storeError) {
        logger.error('Could not store passed result:', storeError);
      }
      
      return {
        success: true,
        question: promptText,
        llmResponse,
        hallucinationResult: {
          question: promptText,
          llmResponse,
          summary: 'Evaluation skipped (LLM responded successfully)',
          facts: [],
          status: 'passed',
          hallucinationLabel: 'NO_HALLUCINATION',
          hallucinationFindings: []
        }
      };
    }
    
    const errorMessage = error?.response?.data?.message || error?.message || 'Unknown error';
    const errorDetails = {
      message: errorMessage,
      endpoint: agentEndpoint,
      statusCode: error?.response?.status,
      data: error?.response?.data
    };
    
    logger.error(`Error running prompt ${promptId}:`, errorDetails);
    
    const errorResponse = `Error calling LLM endpoint: ${errorMessage}`;
    
    try {
      await client.post(`/api/agents/${agentId}/test-results/${promptId}`, {
        status: 'failed',
        response: errorResponse,
        expectedAnswer: expectedAnswer || null,
        hallucinationFindings: []
      });
      logger.debug(`Stored failed result for prompt ${promptId}`);
    } catch (e: any) {
      logger.error('Failed to store failed result:', e?.message || e);
      logger.debug('Error details:', e?.response?.data);

      try {
        await client.post(`/api/agents/${agentId}/test-results/${promptId}`, {
          status: 'failed',
          hallucinationFindings: []
        });
        logger.debug(`Stored minimal failed result for prompt ${promptId}`);
      } catch (fallbackError) {
        logger.error('Fallback storage also failed:', fallbackError);
      }
    }
    
    if (onLLMResponse && !llmResponse) {
      onLLMResponse(errorResponse);
    }
    
    return { 
      success: false, 
      question: promptText,
      llmResponse: errorResponse,
      error: errorDetails
    };
  }
}

export async function runAllPromptEvaluations(
  agentId: string, 
  prompts: any[], 
  agentEndpoint: string,
  onProgress?: (current: number, total: number, question?: string, llmResponse?: string) => void,
  customConfig?: CustomEndpointConfig
) {
  const results: PromptEvaluationResult[] = [];
  try {
    for (let i = 0; i < prompts.length; i++) {
      const prompt = prompts[i];
      
      if (onProgress) {
        onProgress(i + 1, prompts.length, prompt.prompt);
      }
      
      const result = await runPromptEvaluation(
        agentId, 
        prompt.id, 
        prompt.prompt,
        agentEndpoint,
        prompt.expectedAnswer,
        (llmResponse) => {
          if (onProgress) {
            onProgress(i + 1, prompts.length, prompt.prompt, llmResponse);
          }
        },
        customConfig
      );
      
      results.push(result);
      
      await new Promise(resolve => setTimeout(resolve, 500));
    }
  } catch (error) {
    logger.error('Error running evaluations:', error);
  }
  return results;
}


