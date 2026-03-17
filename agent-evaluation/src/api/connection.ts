import axios from 'axios';
import { logger } from '../utils/logger.js';
import { 
  normalizeEndpoint, 
  generatePayloadVariants, 
  buildCustomPayload,
  type CustomEndpointConfig 
} from './endpoint.js';
import { extractResponseText, extractCustomResponseField } from './response.js';
import { transformNetworkError } from '../errors/transform.js';

export async function testAgentConnection(
  endpoint: string
): Promise<{ success: boolean; message: string; details?: any }> {
  try {
    const normalizedEndpoint = normalizeEndpoint(endpoint);
    logger.debug(`Testing connection to: ${normalizedEndpoint}`);
    
    const testClient = axios.create({
      timeout: 10000,
      validateStatus: () => true
    });
    
    const testQuestion = 'Hello, are you there?';
    const payloadVariants = generatePayloadVariants(testQuestion);
    
    for (const [index, payload] of payloadVariants.entries()) {
      try {
        logger.debug(`Testing with payload variant ${index + 1}`);
        const response = await testClient.post(normalizedEndpoint, payload);
        
        if (response.status < 400) {
          const responseText = extractResponseText(response.data);
          if (responseText && responseText !== '{}') {
            return {
              success: true,
              message: 'Connection successful! Agent is responding.',
              details: {
                payloadFormat: index + 1,
                sampleResponse: responseText.substring(0, 100)
              }
            };
          }
        }
      } catch (error: any) {
        const transformedError = transformNetworkError(error, normalizedEndpoint);
        
        if (error.code === 'ECONNREFUSED' || error.code === 'ENOTFOUND') {
          return {
            success: false,
            message: transformedError.userMessage,
            details: { error: error.code }
          };
        }
        logger.debug(`Payload variant ${index + 1} failed:`, transformedError.message);
      }
    }
    
    return {
      success: false,
      message: 'Could not establish a valid connection. The endpoint might not be compatible.',
      details: { triedFormats: payloadVariants.length }
    };
  } catch (error: any) {
    const transformedError = transformNetworkError(error, endpoint);
    return {
      success: false,
      message: transformedError.userMessage,
      details: { error: error.message }
    };
  }
}

export async function testAgentConnectionWithConfig(
  endpoint: string, 
  config: CustomEndpointConfig
): Promise<{ success: boolean; message: string; details?: any }> {
  try {
    const normalizedEndpoint = normalizeEndpoint(endpoint);
    logger.debug(`Testing connection with custom config to: ${normalizedEndpoint}`);
    logger.debug('Custom config:', config);
    
    const testClient = axios.create({
      timeout: 10000,
      validateStatus: () => true,
      headers: {
        'Content-Type': 'application/json',
        ...config.headers
      }
    });
    
    const testQuestion = 'Hello, are you there?';
    let payload: any;
    
    if (config.bodyTemplate) {
      payload = buildCustomPayload(config.bodyTemplate, testQuestion);
    } else {
      payload = { message: testQuestion };
    }
    
    logger.debug('Testing with custom payload:', payload);
    
    const method = config.method?.toLowerCase() || 'post';
    const response = method === 'get' 
      ? await testClient.get(normalizedEndpoint)
      : await testClient.post(normalizedEndpoint, payload);
    
    if (response.status < 400) {
      const responseText = config.responseField 
        ? extractCustomResponseField(response.data, config.responseField)
        : extractResponseText(response.data);
        
      if (responseText && responseText !== '{}') {
        return {
          success: true,
          message: 'Connection successful with custom configuration!',
          details: {
            customConfig: true,
            sampleResponse: responseText.substring(0, 100)
          }
        };
      }
    }
    
    return {
      success: false,
      message: `Server returned ${response.status}: ${response.statusText}`,
      details: { status: response.status, data: response.data }
    };
  } catch (error: any) {
    const transformedError = transformNetworkError(error, endpoint);
    return {
      success: false,
      message: transformedError.userMessage,
      details: { error: error.message }
    };
  }
}

