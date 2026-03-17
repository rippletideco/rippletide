import { logger } from '../utils/logger.js';

export function extractCustomResponseField(data: any, fieldPath: string): string {
  try {
    const parts = fieldPath.split(/[.\[\]]/).filter(p => p);
    let current = data;
    
    for (const part of parts) {
      if (current === null || current === undefined) {
        break;
      }
      
      if (/^\d+$/.test(part)) {
        current = current[parseInt(part)];
      } else {
        current = current[part];
      }
    }
    
    if (typeof current === 'string') {
      return current;
    } else if (current !== null && current !== undefined) {
      return JSON.stringify(current);
    }
  } catch (error) {
    logger.debug(`Could not extract field ${fieldPath}:`, error);
  }
  
  return extractResponseText(data);
}

export function extractResponseText(data: any): string {
  if (typeof data === 'string') {
    return data;
  }
  
  const responseFields = [
    'answer', 'response', 'message', 'text', 'result', 
    'output', 'content', 'reply', 'completion', 'data',
    'bot_message', 'assistant_message', 'ai_response'
  ];
  
  for (const field of responseFields) {
    if (data[field] && typeof data[field] === 'string') {
      return data[field];
    }
  }
  
  if (data.choices && Array.isArray(data.choices) && data.choices.length > 0) {
    const choice = data.choices[0];
    if (choice.message?.content) {
      return choice.message.content;
    }
    if (choice.text) {
      return choice.text;
    }
  }
  
  if (data.messages && Array.isArray(data.messages) && data.messages.length > 0) {
    const lastMessage = data.messages[data.messages.length - 1];
    if (lastMessage.content) {
      return lastMessage.content;
    }
  }
  
  if (typeof data === 'object' && Object.keys(data).length === 1) {
    const singleValue = Object.values(data)[0];
    if (typeof singleValue === 'string') {
      return singleValue;
    }
  }
  
  logger.debug('No standard field found, stringifying response');
  return JSON.stringify(data);
}


