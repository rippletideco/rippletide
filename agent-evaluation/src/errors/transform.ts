import { 
  BaseError, 
  ConnectionError, 
  ApiError, 
  DatabaseError, 
  FileError,
  ValidationError,
  ErrorCode 
} from './types.js';

export function transformNetworkError(error: any, endpoint: string): BaseError {
  if (error?.code === 'ECONNREFUSED') {
    return new ConnectionError(ErrorCode.CONNECTION_REFUSED, endpoint, error);
  }
  if (error?.code === 'ETIMEDOUT' || error?.message?.includes('timeout')) {
    return new ConnectionError(ErrorCode.CONNECTION_TIMEOUT, endpoint, error);
  }
  if (error?.code === 'ENOTFOUND' || error?.code === 'ENOENT') {
    return new ConnectionError(ErrorCode.CONNECTION_NOT_FOUND, endpoint, error);
  }
  if (error?.response?.status) {
    const status = error.response.status;
    const message = error.response.data?.message || error.response.statusText || 'API request failed';
    
    if (status === 401 || status === 403) {
      return new ApiError(
        `Authentication failed: ${message}`,
        'Authentication failed. Please check your credentials.',
        status,
        error.response.data
      );
    }
    if (status >= 400 && status < 500) {
      return new ApiError(
        `Client error: ${message}`,
        `Request failed: ${message}`,
        status,
        error.response.data
      );
    }
    if (status >= 500) {
      return new ApiError(
        `Server error: ${message}`,
        'The server encountered an error. Please try again later.',
        status,
        error.response.data
      );
    }
  }
  
  return new ConnectionError(ErrorCode.NETWORK_ERROR, endpoint, error);
}

export function transformFileError(error: any, filePath: string): BaseError {
  if (error?.code === 'ENOENT') {
    return new FileError(filePath, 'read');
  }
  if (error?.code === 'EACCES' || error?.code === 'EPERM') {
    return new FileError(filePath, 'write');
  }
  if (error?.message?.includes('JSON') || error instanceof SyntaxError) {
    return new FileError(filePath, 'parse');
  }
  
  return new FileError(filePath, 'read');
}

export function transformDatabaseError(
  error: any, 
  dbType: 'pinecone' | 'postgresql',
  operation: string
): BaseError {
  return new DatabaseError(dbType, operation, error);
}

export function transformValidationError(
  field: string,
  value: any,
  requirement: string
): ValidationError {
  return new ValidationError(field, value, requirement);
}

export function isNetworkError(error: any): boolean {
  return !!(
    error?.code?.startsWith('E') ||
    error?.response?.status ||
    error?.message?.includes('network') ||
    error?.message?.includes('timeout') ||
    error?.message?.includes('connection')
  );
}

export function isRetryableError(error: any): boolean {
  if (error instanceof BaseError) {
    return error.isRetryable;
  }
  
  const retryableCodes = ['ETIMEDOUT', 'ECONNRESET', 'EPIPE'];
  const retryableStatuses = [502, 503, 504];
  
  return !!(
    retryableCodes.includes(error?.code) ||
    retryableStatuses.includes(error?.response?.status)
  );
}

