import axios, { AxiosInstance } from 'axios';
import { logger } from '../utils/logger.js';

export interface CustomEndpointConfig {
  headers?: Record<string, string>;
  bodyTemplate?: string;
  responseField?: string;
  method?: string;
}

export function normalizeEndpoint(endpoint: string): string {
  let normalized = endpoint.trim();
  
  if (!normalized.match(/^https?:\/\//)) {
    const hasPort = normalized.match(/:\d+/);
    if (hasPort || normalized.includes('localhost') || normalized.match(/^\d+\.\d+\.\d+\.\d+/)) {
      normalized = `http://${normalized}`;
    } else {
      normalized = `https://${normalized}`;
    }
  }
  
  try {
    const url = new URL(normalized);
    if (!url.pathname || url.pathname === '/') {
      return normalized;
    }
    return normalized;
  } catch (error) {
    logger.warn(`Could not parse URL: ${normalized}, using as-is`);
    return normalized;
  }
}

export function generatePayloadVariants(question: string): any[] {
  const variants = [];
  
  variants.push({ 
    messages: [{ role: 'user', content: question }]
  });
  
  variants.push({ message: question });
  
  variants.push({ inputs: question });
  
  variants.push({ 
    message: question,
    query: question,
    question: question,
    prompt: question
  });
  
  variants.push({ query: question });
  variants.push({ question: question });
  variants.push({ prompt: question });
  variants.push({ input: question });
  variants.push({ text: question });
  variants.push({ user_message: question });
  
  variants.push({ data: question });
  variants.push({ content: question });
  
  variants.push(question);
  
  return variants;
}

export function buildCustomPayload(template: string, question: string): any {
  try {
    let replaced = template
      .replace(/\[eval-question\]/g, question)
      .replace(/\{\{question\}\}/g, question)
      .replace(/\{question\}/g, question);
    return JSON.parse(replaced);
  } catch (e) {
    return template
      .replace(/\[eval-question\]/g, question)
      .replace(/\{\{question\}\}/g, question)
      .replace(/\{question\}/g, question);
  }
}

export function createAxiosClient(customConfig?: CustomEndpointConfig): AxiosInstance {
  const config: any = {
    timeout: 60000,
    validateStatus: () => true
  };
  
  if (customConfig?.headers) {
    config.headers = {
      'Content-Type': 'application/json',
      ...customConfig.headers
    };
  } else {
    config.headers = {
      'Content-Type': 'application/json'
    };
  }
  
  return axios.create(config);
}


