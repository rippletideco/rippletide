const isDebugMode = process.argv.includes('--debug');

export const logger = {
  debug: (...args: any[]) => {
    if (isDebugMode) {
      console.log('[DEBUG]', ...args);
    }
  },
  
  error: (...args: any[]) => {
    if (isDebugMode) {
      console.error('[ERROR]', ...args);
    }
  },
  
  warn: (...args: any[]) => {
    if (isDebugMode) {
      console.warn('[WARN]', ...args);
    }
  },
  
  info: (...args: any[]) => {
    if (isDebugMode) {
      console.log(...args);
    }
  },
};
