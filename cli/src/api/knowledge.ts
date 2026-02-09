import * as fs from 'fs';
import * as path from 'path';
import axios from 'axios';
import { logger } from '../utils/logger.js';
import { PdfUploadError, PdfValidationError } from '../errors/types.js';

let client = axios.create({
  baseURL: 'https://agent-evalserver-production.up.railway.app',
  headers: {
    'Content-Type': 'application/json',
  },
});

let API_KEY: string | null = null;

export function setApiClient(newClient: any, apiKey: string | null) {
  client = newClient;
  API_KEY = apiKey;
}

export async function healthCheck() {
  const response = await client.get('/health');
  return response.data;
}

export async function checkKnowledge(folderPath: string = '.') {
  try {
    const knowledgeFiles = [
      'knowledge-base/qanda.json',
      'qanda.json',
      'knowledge.json',
    ];
    
    for (const file of knowledgeFiles) {
      const filePath = path.join(folderPath, file);
      if (fs.existsSync(filePath)) {
        return { found: true, path: filePath };
      }
    }
    
    return { found: false };
  } catch (error) {
    logger.error('Error checking knowledge:', error);
    return { found: false };
  }
}

export async function importKnowledge(agentId: string, knowledgeData: any) {
  try {
    logger.debug('Importing knowledge for agent:', agentId);
    logger.debug('Knowledge data type:', Array.isArray(knowledgeData) ? 'array' : typeof knowledgeData);
    logger.debug('Knowledge data length:', Array.isArray(knowledgeData) ? knowledgeData.length : 'N/A');
    
    if (!Array.isArray(knowledgeData)) {
      logger.warn('Knowledge data is not an array, skipping import');
      return null;
    }
    
    let importedCount = 0;
    for (const item of knowledgeData) {
      try {
        const question = item.question || item.prompt || item.input;
        const answer = item.answer || item.response || item.expectedAnswer;
        
        if (!question || !answer) {
          logger.debug('Skipping item without question or answer:', item);
          continue;
        }
        
        const response = await client.post(`/api/agents/${agentId}/config`, {
          label: question,
          description: answer,
          type: 'knowledge'
        });
        
        importedCount++;
        logger.debug(`Imported Q&A ${importedCount}/${knowledgeData.length}: ${question.substring(0, 50)}...`);
      } catch (error: any) {
        logger.warn(`Failed to import Q&A pair: ${error?.message || error}`);
        logger.debug('Q&A import error details:', error?.response?.data);
        logger.debug('Q&A import error status:', error?.response?.status);
      }
    }
    
    logger.info(`Knowledge imported successfully: ${importedCount}/${knowledgeData.length} Q&A pairs`);
    
    return { imported: importedCount, total: knowledgeData.length };
  } catch (error: any) {
    logger.error('Error importing knowledge:', error?.message || error);
    logger.debug('Error details:', error?.response?.data);
    logger.debug('Error status:', error?.response?.status);
    throw error;
  }
}

export async function getTestResults(agentId: string) {
  try {
    const response = await client.get(`/api/agents/${agentId}/test-results`);
    return response.data;
  } catch (error) {
    console.error('Error getting test results:', error);
    return [];
  }
}

export interface EvaluationConfig {
  agentEndpoint: string;
  knowledgeSource?: string;
  customConfig?: {
    headers?: Record<string, string>;
    bodyTemplate?: string;
    responseField?: string;
    method?: string;
  };
}

export interface EvaluationResult {
  totalTests: number;
  passed: number;
  failed: number;
  duration: string;
  evaluationUrl: string;
  agentId?: string;
}

export async function uploadPdfToAgent(
  agentId: string,
  pdfPath: string,
  onProgress?: (message: string) => void
): Promise<{ qaPairs: Array<{question: string, answer: string}> }> {
  try {
    if (!fs.existsSync(pdfPath)) {
      throw new PdfValidationError(pdfPath, 'not_found');
    }

    const stats = fs.statSync(pdfPath);
    const MAX_SIZE = 10 * 1024 * 1024;
    
    if (stats.size > MAX_SIZE) {
      throw new PdfValidationError(pdfPath, 'too_large', { 
        fileSize: stats.size, 
        maxSize: MAX_SIZE 
      });
    }

    const fileName = path.basename(pdfPath);
    const fileBuffer = fs.readFileSync(pdfPath);
    const fileSizeKB = Math.round(fileBuffer.length / 1024);
    
    if (onProgress) {
      onProgress(`Uploading ${fileName} (${fileSizeKB}KB) - this may take a few minutes...`);
    }

    const formData = new FormData();
    const blob = new Blob([fileBuffer], { type: 'application/pdf' });
    formData.append('file', blob, fileName);

    const response = await client.post(
      `/api/agents/${agentId}/upload-pdf`,
      formData,
      {
        headers: API_KEY ? { 'x-api-key': API_KEY } : {},
        maxBodyLength: Infinity,
        maxContentLength: Infinity,
        timeout: 300000
      }
    );

    if (!response.data.success) {
      throw new PdfUploadError(
        'upload',
        response.data.message || 'Upload failed',
        pdfPath
      );
    }

    if (onProgress) {
      onProgress('Processing PDF and generating Q&A pairs...');
    }

    const qaPairs = response.data.processed?.qaPairs || [];
    
    if (qaPairs.length === 0) {
      throw new PdfUploadError(
        'extract',
        'No Q&A pairs generated from PDF',
        pdfPath
      );
    }

    if (onProgress) {
      onProgress(`Generated ${qaPairs.length} Q&A pairs`);
    }

    return {
      qaPairs: qaPairs.map((pair: any) => ({
        question: pair.question,
        answer: pair.answer
      }))
    };
  } catch (error: any) {
    if (error instanceof PdfValidationError || error instanceof PdfUploadError) {
      throw error;
    }
    
    logger.error('Error uploading PDF:', error);
    throw new PdfUploadError(
      'upload',
      error.message || 'Unknown error',
      pdfPath,
      error
    );
  }
}

export async function runEvaluation(
  config: EvaluationConfig, 
  onProgress?: (progress: number) => void
) {
  const { 
    generateApiKey, 
    createAgent, 
    addTestPrompts, 
    runAllPromptEvaluations 
  } = await import('./evaluation.js');
  
  try {
    const startTime = Date.now();
    
    if (onProgress) onProgress(10);
    const payloadTemplate = config.customConfig?.bodyTemplate || '{"message": "[eval-question]"}';
    const agent = await createAgent(config.agentEndpoint, payloadTemplate);
    const agentId = agent.id;
    
    if (onProgress) onProgress(30);
    if (config.knowledgeSource === 'files') {
      const knowledgeResult = await checkKnowledge();
      if (knowledgeResult.found && knowledgeResult.path) {
        const knowledgeData = JSON.parse(fs.readFileSync(knowledgeResult.path, 'utf-8'));
        await importKnowledge(agentId, knowledgeData);
      }
    }
    
    if (onProgress) onProgress(40);
    
    let testPrompts: string[] = [];
    if (config.knowledgeSource === 'files') {
      const knowledgeResult = await checkKnowledge();
      if (knowledgeResult.found && knowledgeResult.path) {
        try {
          const knowledgeData = JSON.parse(fs.readFileSync(knowledgeResult.path, 'utf-8'));
          if (Array.isArray(knowledgeData)) {
            testPrompts = knowledgeData.slice(0, 5).map((item: any) => 
              item.question || item.prompt || item.input || 'Test question'
            );
          } else if (knowledgeData.questions) {
            testPrompts = knowledgeData.questions.slice(0, 5);
          }
        } catch (error) {
          logger.error('Error loading prompts from knowledge:', error);
        }
      }
    }
    
    const createdPrompts = await addTestPrompts(agentId, testPrompts);
    const promptIds = createdPrompts.map((p: any) => p.id);
    const promptCount = promptIds.length;
    
    if (onProgress) onProgress(50);
    const evaluationResults = await runAllPromptEvaluations(
      agentId, 
      createdPrompts,
      config.agentEndpoint,
      (current, total) => {
        const progress = 50 + Math.round((current / total) * 40);
        if (onProgress) onProgress(progress);
      }
    );
    
    if (onProgress) onProgress(100);
    
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
    
    return {
      totalTests: promptCount || 5,
      passed,
      failed,
      duration: durationStr,
      evaluationUrl: `http://localhost:5173/eval/${agentId}`,
      agentId,
    };
  } catch (error) {
    logger.error('Evaluation error:', error);
    throw error;
  }
}


