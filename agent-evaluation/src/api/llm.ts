import axios from 'axios';
import { logger } from '../utils/logger.js';
import { 
  normalizeEndpoint, 
  generatePayloadVariants, 
  buildCustomPayload,
  createAxiosClient,
  type CustomEndpointConfig 
} from './endpoint.js';
import { extractResponseText, extractCustomResponseField } from './response.js';
import { ConnectionError, ErrorCode } from '../errors/types.js';

export async function callLLMEndpoint(
  agentEndpoint: string, 
  question: string, 
  customConfig?: CustomEndpointConfig
): Promise<string> {
  try {
    logger.debug(`Calling LLM endpoint: ${agentEndpoint}`);
    logger.debug(`Question: ${question}`);
    
    const normalizedEndpoint = normalizeEndpoint(agentEndpoint);
    logger.debug(`Normalized endpoint: ${normalizedEndpoint}`);
    
    let response: any = null;
    
    if (customConfig && (customConfig.headers || customConfig.bodyTemplate)) {
      logger.debug('Using custom configuration');
      
      const llmClient = createAxiosClient(customConfig);
      
      let payload: any;
      if (customConfig.bodyTemplate) {
        payload = buildCustomPayload(customConfig.bodyTemplate, question);
      } else {
        payload = { message: question };
      }
      
      logger.debug('Custom payload:', payload);
      const method = customConfig.method?.toLowerCase() || 'post';
      response = method === 'get'
        ? await llmClient.get(normalizedEndpoint)
        : await llmClient.post(normalizedEndpoint, payload);
      
      if (response.status >= 400) {
        throw new ConnectionError(ErrorCode.API_ERROR, normalizedEndpoint, {
          status: response.status,
          statusText: response.statusText
        });
      }
    } else {
      const llmClient = axios.create({
        timeout: 60000,
        validateStatus: () => true
      });
      
      const payloadVariants = generatePayloadVariants(question);
      let lastError: any = null;
      
      for (const [index, payload] of payloadVariants.entries()) {
        try {
          logger.debug(`Trying payload variant ${index + 1}/${payloadVariants.length}:`, payload);
          response = await llmClient.post(normalizedEndpoint, payload);
          
          if (response.status < 400) {
            logger.debug(`Success with payload variant ${index + 1}`);
            break;
          }
          
          lastError = new Error(`HTTP ${response.status}: ${response.statusText}`);
        } catch (error) {
          lastError = error;
          logger.debug(`Payload variant ${index + 1} failed:`, error);
        }
      }
      
      if (!response || response.status >= 400) {
        if (lastError) {
          throw lastError;
        }
        const errorMsg = `LLM endpoint returned error: HTTP ${response?.status} - ${response?.statusText}`;
        logger.error(errorMsg);
        throw new Error(errorMsg);
      }
    }
    
    logger.debug(`Response status: ${response.status}`);
    logger.debug('Response headers:', response.headers);
    
    const llmResponse = customConfig?.responseField 
      ? extractCustomResponseField(response.data, customConfig.responseField)
      : extractResponseText(response.data);
    
    if (!llmResponse || llmResponse === '{}') {
      logger.warn('Empty or invalid response from LLM endpoint');
      logger.debug('Full response:', response.data);
    }
    
    logger.debug(`Extracted response: ${llmResponse.substring(0, 100)}...`);
    
    return llmResponse;
  } catch (error: any) {
    const errorDetails = {
      message: error?.message || 'Unknown error',
      code: error?.code,
      endpoint: agentEndpoint,
      response: error?.response?.data,
      status: error?.response?.status
    };
    
    logger.error('Error calling LLM endpoint:', errorDetails);
    
    if (error.code === 'ECONNREFUSED') {
      throw new ConnectionError(ErrorCode.CONNECTION_REFUSED, agentEndpoint, error);
    } else if (error.code === 'ETIMEDOUT') {
      throw new ConnectionError(ErrorCode.CONNECTION_TIMEOUT, agentEndpoint, error);
    } else if (error.code === 'ENOTFOUND') {
      throw new ConnectionError(ErrorCode.CONNECTION_NOT_FOUND, agentEndpoint, error);
    }
    
    throw error;
  }
}

