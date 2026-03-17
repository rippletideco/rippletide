import { Pinecone } from '@pinecone-database/pinecone';
import { DatabaseError } from '../errors/types.js';
import { logger } from './logger.js';

const API_BASE_URL = process.env.API_BASE_URL || 'http://localhost:3001';
const MAX_CHUNKS_LIMIT = parseInt(process.env.MAX_CHUNKS_LIMIT || '10', 10);

export interface QAPair {
  question: string;
  answer: string;
}

export async function getPineconeQAndA(
  pineconeUrl: string,
  apiKey: string,
  onProgress?: (message: string) => void
): Promise<QAPair[]> {
  try {
    const urlMatch = pineconeUrl.match(/https?:\/\/([^\/]+)/);
    if (!urlMatch) {
      throw new DatabaseError('pinecone', 'validate URL', new Error('Invalid Pinecone URL format'));
    }
    
    const hostname = urlMatch[1];
    const indexName = hostname.split('.')[0];
    
    if (!apiKey) {
      throw new DatabaseError('pinecone', 'authenticate', new Error('Pinecone API key is required'));
    }

    if (onProgress) onProgress('Connecting to Pinecone...');
    const pc = new Pinecone({
      apiKey: apiKey
    });

    const indexHost = pineconeUrl.replace('https://', '').replace('http://', '');
    const index = pc.index(indexName, indexHost);

    const indexDescription = await index.describeIndexStats();
    const dimensions = indexDescription.dimension || 1536;
    
    if (onProgress) onProgress('Fetching sample records...');
    const queryResult = await index.query({
      vector: new Array(dimensions).fill(0.01),
      topK: 10,
      includeMetadata: true
    });
    
    if (!queryResult.matches || queryResult.matches.length === 0) {
      throw new DatabaseError('pinecone', 'query index', new Error('No records found in Pinecone index'));
    }

    const records = queryResult.matches
      .filter(match => match.metadata)
      .map(match => match.metadata);
    
    if (records.length === 0) {
      throw new DatabaseError('pinecone', 'fetch metadata', new Error('No records with metadata found'));
    }

    if (onProgress) onProgress('Identifying chunk field...');
    const apiUrl = `${API_BASE_URL}/api/chunks/identify-chunk-field`;
    const response = await fetch(apiUrl, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({ records })
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new DatabaseError('pinecone', 'identify chunk field', new Error(`API request failed: ${response.status} ${response.statusText} - ${errorText}`));
    }
    
    const result = await response.json() as { fieldName: string };
    const chunkFieldName = result.fieldName;

    if (onProgress) onProgress('Fetching all records from Pinecone...');
    const totalRecords = indexDescription.totalRecordCount || 0;
    const chunkFieldValues: string[] = [];
    
    try {
      let allIds: string[] = [];
      let paginationToken: string | null = null;
      
      do {
        const listResponse: any = paginationToken 
          ? await index.listPaginated({ limit: 100, paginationToken })
          : await index.listPaginated({ limit: 100 });
        
        if (listResponse.vectors) {
          allIds = allIds.concat(listResponse.vectors.map((v: any) => v.id));
        }
        
        paginationToken = listResponse.pagination?.next || null;
      } while (paginationToken);
      
      if (onProgress) onProgress(`Fetching metadata for ${allIds.length} records...`);
      
      const BATCH_SIZE = 100;
      for (let i = 0; i < allIds.length; i += BATCH_SIZE) {
        const batchIds = allIds.slice(i, i + BATCH_SIZE);
        const fetchResult = await index.fetch(batchIds);
        
        if (fetchResult.records) {
          Object.values(fetchResult.records).forEach(record => {
            if (record.metadata && record.metadata[chunkFieldName]) {
              const fieldValue = record.metadata[chunkFieldName];
              if (typeof fieldValue === 'string' && fieldValue.trim()) {
                chunkFieldValues.push(fieldValue.trim());
              }
            }
          });
        }
      }
    } catch (fetchError: any) {
      if (onProgress) onProgress('Trying alternative method with query...');
      
      const queryResult = await index.query({
        vector: new Array(dimensions).fill(0.01),
        topK: Math.min(10000, totalRecords),
        includeMetadata: true
      });
      
      if (queryResult.matches) {
        queryResult.matches.forEach(match => {
          if (match.metadata && match.metadata[chunkFieldName]) {
            const fieldValue = match.metadata[chunkFieldName];
            if (typeof fieldValue === 'string' && fieldValue.trim()) {
              chunkFieldValues.push(fieldValue.trim());
            }
          }
        });
      }
    }
    
    if (chunkFieldValues.length === 0) {
      throw new DatabaseError('pinecone', 'extract chunks', new Error('No chunks extracted from Pinecone'));
    }

    const limitedChunks = chunkFieldValues.slice(0, MAX_CHUNKS_LIMIT);
    if (chunkFieldValues.length > MAX_CHUNKS_LIMIT && onProgress) {
      onProgress(`Limiting to ${MAX_CHUNKS_LIMIT} chunks (${chunkFieldValues.length - MAX_CHUNKS_LIMIT} skipped)`);
    }
    
    if (limitedChunks.length === 0) {
      throw new DatabaseError('pinecone', 'process chunks', new Error('No chunks to process'));
    }

    if (onProgress) onProgress(`Generating Q&A pairs from ${limitedChunks.length} chunks...`);
    const qaApiUrl = `${API_BASE_URL}/api/chunks/generate-qa-from-chunks`;
    const BATCH_SIZE = 5;
    let allQAPairs: QAPair[] = [];
    
    for (let i = 0; i < limitedChunks.length; i += BATCH_SIZE) {
      const batch = limitedChunks.slice(i, i + BATCH_SIZE);
      
      if (onProgress) {
        onProgress(`Processing batch ${Math.floor(i / BATCH_SIZE) + 1} (chunks ${i + 1}-${Math.min(i + BATCH_SIZE, limitedChunks.length)}/${limitedChunks.length})...`);
      }
      
      try {
        const qaResponse = await fetch(qaApiUrl, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json'
          },
          body: JSON.stringify({ chunks: batch })
        });
        
        if (!qaResponse.ok) {
          const errorText = await qaResponse.text();
          throw new DatabaseError('pinecone', 'generate Q&A', new Error(`API request failed: ${qaResponse.status} ${qaResponse.statusText} - ${errorText}`));
        }
        
        const qaResult = await qaResponse.json() as { qaPairs: QAPair[] };
        allQAPairs.push(...qaResult.qaPairs);
      } catch (qaError: any) {
        logger.error(`Error processing batch: ${qaError.message}`);
      }
    }
    
    if (allQAPairs.length === 0) {
      throw new DatabaseError('pinecone', 'generate Q&A pairs', new Error('No Q&A pairs generated from chunks'));
    }

    return allQAPairs;
  } catch (error: any) {
    if (error instanceof DatabaseError) {
      throw error;
    }
    throw new DatabaseError('pinecone', 'fetch Q&A', error);
  }
}
