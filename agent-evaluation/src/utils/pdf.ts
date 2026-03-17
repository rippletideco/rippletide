import * as fs from 'fs';
import * as path from 'path';
import axios from 'axios';
import { PdfUploadError, PdfValidationError } from '../errors/types.js';
import { logger } from './logger.js';

const MAX_FILE_SIZE = 10 * 1024 * 1024;

export interface QAPair {
  question: string;
  answer: string;
}

export interface PdfUploadResponse {
  success: boolean;
  message: string;
  file?: {
    originalName: string;
    size: number;
    mimetype: string;
  };
  processed?: {
    totalChunks: number;
    chunks: Array<{
      text: string;
      type: string;
    }>;
    qaPairs: Array<{
      question: string;
      answer: string;
      sourceChunk?: string;
      chunkType?: string;
    }>;
    totalQAPairs: number;
  };
}

function validatePdfFile(filePath: string): void {
  if (!fs.existsSync(filePath)) {
    throw new PdfValidationError(filePath, 'not_found');
  }

  const stats = fs.statSync(filePath);
  
  if (!stats.isFile()) {
    throw new PdfValidationError(filePath, 'invalid_format', { reason: 'Path is not a file' });
  }

  if (stats.size > MAX_FILE_SIZE) {
    throw new PdfValidationError(filePath, 'too_large', { 
      fileSize: stats.size, 
      maxSize: MAX_FILE_SIZE 
    });
  }

  try {
    fs.accessSync(filePath, fs.constants.R_OK);
  } catch (error) {
    throw new PdfValidationError(filePath, 'no_permission');
  }

  const ext = path.extname(filePath).toLowerCase();
  if (ext !== '.pdf') {
    throw new PdfValidationError(filePath, 'invalid_format', { 
      extension: ext,
      expected: '.pdf' 
    });
  }
}

export async function getPdfQAndA(
  pdfPath: string,
  agentId: string,
  backendUrl: string,
  onProgress?: (message: string) => void
): Promise<QAPair[]> {
  try {
    if (onProgress) onProgress('Validating PDF file...');
    validatePdfFile(pdfPath);

    const fileName = path.basename(pdfPath);
    const fileBuffer = fs.readFileSync(pdfPath);
    
    if (onProgress) onProgress(`Uploading ${fileName} (${Math.round(fileBuffer.length / 1024)}KB)...`);

    const formData = new FormData();
    const blob = new Blob([fileBuffer], { type: 'application/pdf' });
    formData.append('file', blob, fileName);

    const uploadUrl = `${backendUrl}/api/agents/${agentId}/upload-pdf`;
    
    logger.debug(`Uploading PDF to: ${uploadUrl}`);
    logger.debug(`File: ${fileName}, Size: ${fileBuffer.length} bytes`);

    const response = await axios.post<PdfUploadResponse>(uploadUrl, formData, {
      maxBodyLength: Infinity,
      maxContentLength: Infinity,
      timeout: 300000
    });

    if (!response.data.success) {
      throw new PdfUploadError(
        'upload',
        response.data.message || 'Upload failed',
        pdfPath
      );
    }

    if (onProgress) onProgress('Processing PDF content...');

    if (!response.data.processed || !response.data.processed.qaPairs) {
      throw new PdfUploadError(
        'extract',
        'No Q&A pairs generated from PDF',
        pdfPath
      );
    }

    const qaPairs = response.data.processed.qaPairs;
    
    if (onProgress) {
      onProgress(`Generated ${qaPairs.length} Q&A pairs from ${response.data.processed.totalChunks} chunks`);
    }

    return qaPairs.map(pair => ({
      question: pair.question,
      answer: pair.answer
    }));

  } catch (error: any) {
    if (error instanceof PdfValidationError || error instanceof PdfUploadError) {
      throw error;
    }

    if (error.response) {
      const status = error.response.status;
      const message = error.response.data?.message || error.response.statusText;
      
      if (status === 413) {
        throw new PdfValidationError(pdfPath, 'too_large', { 
          serverMessage: message 
        });
      }
      
      throw new PdfUploadError(
        'upload',
        `Server error (${status}): ${message}`,
        pdfPath,
        error
      );
    }

    if (error.code === 'ECONNREFUSED') {
      throw new PdfUploadError(
        'upload',
        'Cannot connect to backend server',
        pdfPath,
        error
      );
    }

    if (error.code === 'ETIMEDOUT') {
      throw new PdfUploadError(
        'upload',
        'Upload timed out - file may be too large',
        pdfPath,
        error
      );
    }

    throw new PdfUploadError(
      'upload',
      error.message || 'Unknown error',
      pdfPath,
      error
    );
  }
}
