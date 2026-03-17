import { Text, Box } from 'ink';
import React from 'react';
import { BaseError, ErrorCode } from './types.js';

interface ErrorDisplayOptions {
  showDebug: boolean;
  showRetryHint: boolean;
}

export class ErrorHandler {
  public static isDebugMode(): boolean {
    return process.argv.includes('--debug') || process.env.DEBUG === 'true';
  }

  public static handle(error: unknown): void {
    const isDebug = this.isDebugMode();
    
    if (error instanceof BaseError) {
      this.displayError(error, { showDebug: isDebug, showRetryHint: error.isRetryable });
      
      if (isDebug) {
        console.error('\n[DEBUG] Full error details:', error.details);
        console.error('[DEBUG] Stack trace:', error.stack);
      }
    } else if (error instanceof Error) {
      const userMessage = this.getUserFriendlyMessage(error);
      console.error(`\nError: ${userMessage}`);
      
      if (isDebug) {
        console.error('\n[DEBUG] Original error:', error.message);
        console.error('[DEBUG] Stack trace:', error.stack);
      }
    } else {
      console.error('\nError: An unexpected error occurred');
      
      if (isDebug) {
        console.error('[DEBUG] Unknown error:', error);
      }
    }
    
    if (!isDebug) {
      console.error('\nRun with --debug flag for more details');
    }
  }

  private static displayError(error: BaseError, options: ErrorDisplayOptions): void {
    console.error(`\nError: ${error.userMessage}`);
    
    if (options.showRetryHint) {
      console.error('\nNote: This error might be temporary. Please try again.');
    }
    
    const suggestions = this.getSuggestions(error.code);
    if (suggestions.length > 0) {
      console.error('\nSuggestions:');
      suggestions.forEach(suggestion => {
        console.error(`  - ${suggestion}`);
      });
    }
  }

  private static getUserFriendlyMessage(error: Error): string {
    const message = error.message.toLowerCase();
    
    if (message.includes('econnrefused')) {
      return 'Cannot connect to the service. Please ensure it is running and accessible.';
    }
    if (message.includes('etimedout') || message.includes('timeout')) {
      return 'The request timed out. The service might be slow or unreachable.';
    }
    if (message.includes('enotfound') || message.includes('getaddrinfo')) {
      return 'Could not find the specified host. Please check the URL.';
    }
    if (message.includes('eacces') || message.includes('permission')) {
      return 'Permission denied. Please check your access rights.';
    }
    if (message.includes('enoent')) {
      return 'File or directory not found.';
    }
    if (message.includes('invalid json') || message.includes('unexpected token')) {
      return 'Invalid data format received. Please check the configuration.';
    }
    if (message.includes('network')) {
      return 'Network error occurred. Please check your internet connection.';
    }
    
    return error.message;
  }

  private static getSuggestions(code: ErrorCode): string[] {
    const suggestions: Partial<Record<ErrorCode, string[]>> = {
      [ErrorCode.CONNECTION_REFUSED]: [
        'Check if your agent is running',
        'Verify the port number is correct',
        'Ensure no firewall is blocking the connection'
      ],
      [ErrorCode.CONNECTION_TIMEOUT]: [
        'Check if the agent URL is correct',
        'Verify your network connection',
        'The agent might be under heavy load'
      ],
      [ErrorCode.CONNECTION_NOT_FOUND]: [
        'Double-check the URL spelling',
        'Ensure the protocol (http/https) is correct',
        'Verify the domain exists'
      ],
      [ErrorCode.INVALID_ENDPOINT]: [
        'Use format like "localhost:8000"',
        'Or full URL like "https://my-agent.com"',
        'Do not include paths after the domain'
      ],
      [ErrorCode.AUTH_ERROR]: [
        'Check your API key or credentials',
        'Ensure you have the necessary permissions',
        'Verify the authentication method'
      ],
      [ErrorCode.FILE_NOT_FOUND]: [
        'Check if the file exists in the current directory',
        'Verify the file path is correct',
        'Ensure you have read permissions'
      ],
      [ErrorCode.PARSE_ERROR]: [
        'Check the file contains valid JSON',
        'Look for missing commas or brackets',
        'Validate the JSON structure'
      ],
      [ErrorCode.PINECONE_ERROR]: [
        'Verify your Pinecone API key',
        'Check the index URL is correct',
        'Ensure the index exists and is accessible'
      ],
      [ErrorCode.POSTGRESQL_ERROR]: [
        'Check database connection string',
        'Verify database credentials',
        'Ensure the database server is running'
      ],
      [ErrorCode.VALIDATION_ERROR]: [
        'Review the input requirements',
        'Check for typos or formatting issues'
      ],
      [ErrorCode.API_ERROR]: [],
      [ErrorCode.NETWORK_ERROR]: [
        'Check your internet connection',
        'Try again in a few moments'
      ],
      [ErrorCode.DATABASE_ERROR]: [
        'Check database connection',
        'Verify credentials are correct'
      ],
      [ErrorCode.EVALUATION_ERROR]: [],
      [ErrorCode.UNKNOWN_ERROR]: [],
      [ErrorCode.INVALID_RESPONSE]: []
    };

    return suggestions[code] || [];
  }
}

export function createErrorComponent(error: BaseError): React.ReactElement {
  return React.createElement(
    Box,
    { flexDirection: 'column', marginTop: 1 },
    React.createElement(Text, { color: 'red' }, `Error: ${error.userMessage}`),
    error.isRetryable && React.createElement(
      Text,
      { color: 'yellow', dimColor: true },
      'Note: This error might be temporary. Please try again.'
    ),
    !ErrorHandler.isDebugMode() && React.createElement(
      Text,
      { dimColor: true },
      'Run with --debug flag for more details'
    )
  );
}

export function wrapAsync<T extends (...args: any[]) => Promise<any>>(
  fn: T,
  errorTransform?: (error: unknown) => BaseError
): T {
  return (async (...args: Parameters<T>) => {
    try {
      return await fn(...args);
    } catch (error) {
      if (errorTransform) {
        throw errorTransform(error);
      }
      throw error;
    }
  }) as T;
}
