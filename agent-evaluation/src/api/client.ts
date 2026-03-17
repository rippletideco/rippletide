import { testAgentConnection, testAgentConnectionWithConfig } from './connection.js';
import { callLLMEndpoint } from './llm.js';
import { 
  normalizeEndpoint, 
  generatePayloadVariants,
  type CustomEndpointConfig 
} from './endpoint.js';
import { extractResponseText, extractCustomResponseField } from './response.js';
import {
  setBackendUrl,
  generateApiKey,
  createAgent,
  addTestPrompts,
  checkHallucination,
  runPromptEvaluation,
  runAllPromptEvaluations,
  type HallucinationCheckResult,
  type PromptEvaluationResult
} from './evaluation.js';
import {
  healthCheck,
  checkKnowledge,
  importKnowledge,
  getTestResults,
  runEvaluation,
  type EvaluationConfig,
  type EvaluationResult
} from './knowledge.js';

export type { 
  CustomEndpointConfig,
  HallucinationCheckResult,
  PromptEvaluationResult,
  EvaluationConfig,
  EvaluationResult
};

export const api = {
  setBaseUrl: setBackendUrl,
  testAgentConnection,
  testAgentConnectionWithConfig,
  normalizeEndpoint,
  generatePayloadVariants,
  extractResponseText,
  extractCustomResponseField,
  callLLMEndpoint,
  generateApiKey,
  healthCheck,
  checkKnowledge,
  createAgent,
  importKnowledge,
  addTestPrompts,
  checkHallucination,
  runPromptEvaluation,
  runAllPromptEvaluations,
  getTestResults,
  runEvaluation,
};