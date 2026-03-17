export enum ErrorCode {
  CONNECTION_REFUSED = 'CONNECTION_REFUSED',
  CONNECTION_TIMEOUT = 'CONNECTION_TIMEOUT',
  CONNECTION_NOT_FOUND = 'CONNECTION_NOT_FOUND',
  INVALID_ENDPOINT = 'INVALID_ENDPOINT',
  INVALID_RESPONSE = 'INVALID_RESPONSE',
  API_ERROR = 'API_ERROR',
  AUTH_ERROR = 'AUTH_ERROR',
  NETWORK_ERROR = 'NETWORK_ERROR',
  VALIDATION_ERROR = 'VALIDATION_ERROR',
  FILE_NOT_FOUND = 'FILE_NOT_FOUND',
  PARSE_ERROR = 'PARSE_ERROR',
  DATABASE_ERROR = 'DATABASE_ERROR',
  PINECONE_ERROR = 'PINECONE_ERROR',
  POSTGRESQL_ERROR = 'POSTGRESQL_ERROR',
  PDF_UPLOAD_ERROR = 'PDF_UPLOAD_ERROR',
  PDF_VALIDATION_ERROR = 'PDF_VALIDATION_ERROR',
  EVALUATION_ERROR = 'EVALUATION_ERROR',
  UNKNOWN_ERROR = 'UNKNOWN_ERROR'
}

export abstract class BaseError extends Error {
  public readonly code: ErrorCode;
  public readonly details?: any;
  public readonly userMessage: string;
  public readonly isRetryable: boolean;

  constructor(
    code: ErrorCode,
    message: string,
    userMessage: string,
    isRetryable: boolean = false,
    details?: any
  ) {
    super(message);
    this.name = this.constructor.name;
    this.code = code;
    this.userMessage = userMessage;
    this.isRetryable = isRetryable;
    this.details = details;
    Error.captureStackTrace(this, this.constructor);
  }
}

export class ConnectionError extends BaseError {
  constructor(
    code: ErrorCode,
    endpoint: string,
    originalError?: any
  ) {
    const messages: Partial<Record<ErrorCode, { message: string; userMessage: string }>> = {
      [ErrorCode.CONNECTION_REFUSED]: {
        message: `Connection refused to ${endpoint}`,
        userMessage: `Cannot connect to agent at ${endpoint}. Please ensure your agent is running and accessible.`
      },
      [ErrorCode.CONNECTION_TIMEOUT]: {
        message: `Connection timeout to ${endpoint}`,
        userMessage: `Connection timed out after 60 seconds. The agent might be slow to respond or unreachable.`
      },
      [ErrorCode.CONNECTION_NOT_FOUND]: {
        message: `Host not found: ${endpoint}`,
        userMessage: `Could not find the agent at ${endpoint}. Please check the URL and try again.`
      },
      [ErrorCode.INVALID_ENDPOINT]: {
        message: `Invalid endpoint format: ${endpoint}`,
        userMessage: `The endpoint URL "${endpoint}" is not valid. Please use format like "localhost:8000" or "https://my-agent.com".`
      },
      [ErrorCode.NETWORK_ERROR]: {
        message: `Network error connecting to ${endpoint}`,
        userMessage: `Network error occurred. Please check your internet connection and try again.`
      },
      [ErrorCode.API_ERROR]: {
        message: `API error at ${endpoint}`,
        userMessage: `The agent returned an error. Please check the agent logs for details.`
      }
    };

    const errorInfo = messages[code] || messages[ErrorCode.NETWORK_ERROR] || {
      message: `Error connecting to ${endpoint}`,
      userMessage: `An error occurred while connecting to the agent.`
    };
    super(
      code,
      errorInfo.message,
      errorInfo.userMessage,
      [ErrorCode.CONNECTION_REFUSED, ErrorCode.CONNECTION_TIMEOUT, ErrorCode.NETWORK_ERROR].includes(code),
      { endpoint, originalError }
    );
  }
}

export class ApiError extends BaseError {
  public readonly statusCode?: number;

  constructor(
    message: string,
    userMessage: string,
    statusCode?: number,
    details?: any
  ) {
    super(
      ErrorCode.API_ERROR,
      message,
      userMessage,
      statusCode === 503 || statusCode === 502,
      details
    );
    this.statusCode = statusCode;
  }
}

export class ValidationError extends BaseError {
  constructor(
    field: string,
    value: any,
    requirement: string
  ) {
    super(
      ErrorCode.VALIDATION_ERROR,
      `Validation failed for ${field}: ${requirement}`,
      `Invalid ${field}: ${requirement}`,
      false,
      { field, value, requirement }
    );
  }
}

export class FileError extends BaseError {
  constructor(
    filePath: string,
    operation: 'read' | 'write' | 'parse'
  ) {
    const userMessages = {
      read: `Cannot read file "${filePath}". Please ensure it exists and you have permission to access it.`,
      write: `Cannot write to file "${filePath}". Please check file permissions.`,
      parse: `Cannot parse file "${filePath}". Please ensure it contains valid JSON.`
    };

    super(
      operation === 'parse' ? ErrorCode.PARSE_ERROR : ErrorCode.FILE_NOT_FOUND,
      `File ${operation} error: ${filePath}`,
      userMessages[operation],
      false,
      { filePath, operation }
    );
  }
}

export class DatabaseError extends BaseError {
  constructor(
    dbType: 'pinecone' | 'postgresql',
    operation: string,
    originalError?: any
  ) {
    const code = dbType === 'pinecone' ? ErrorCode.PINECONE_ERROR : ErrorCode.POSTGRESQL_ERROR;
    const dbName = dbType === 'pinecone' ? 'Pinecone' : 'PostgreSQL';
    
    super(
      code,
      `${dbName} ${operation} failed: ${originalError?.message || 'Unknown error'}`,
      `Failed to ${operation} with ${dbName}. Please check your connection details and try again.`,
      true,
      { dbType, operation, originalError }
    );
  }
}

export class PdfUploadError extends BaseError {
  constructor(
    operation: 'upload' | 'process' | 'extract',
    reason: string,
    filePath?: string,
    originalError?: any
  ) {
    const userMessages = {
      upload: `Failed to upload PDF: ${reason}`,
      process: `Failed to process PDF: ${reason}`,
      extract: `Failed to extract Q&A from PDF: ${reason}`
    };

    super(
      ErrorCode.PDF_UPLOAD_ERROR,
      `PDF ${operation} error: ${reason}`,
      userMessages[operation],
      operation === 'upload',
      { filePath, operation, originalError }
    );
  }
}

export class PdfValidationError extends BaseError {
  constructor(
    filePath: string,
    reason: 'not_found' | 'too_large' | 'invalid_format' | 'no_permission',
    details?: any
  ) {
    const userMessages = {
      not_found: `PDF file not found: ${filePath}`,
      too_large: `PDF file is too large (max 10MB): ${filePath}`,
      invalid_format: `File is not a valid PDF: ${filePath}`,
      no_permission: `No permission to read PDF file: ${filePath}`
    };

    super(
      ErrorCode.PDF_VALIDATION_ERROR,
      `PDF validation error: ${reason} - ${filePath}`,
      userMessages[reason],
      false,
      { filePath, reason, ...details }
    );
  }
}

export class EvaluationError extends BaseError {
  constructor(
    stage: string,
    reason: string,
    details?: any
  ) {
    super(
      ErrorCode.EVALUATION_ERROR,
      `Evaluation failed at ${stage}: ${reason}`,
      `Evaluation could not complete: ${reason}`,
      false,
      { stage, ...details }
    );
  }
}
