import { drizzle } from 'drizzle-orm/node-postgres';
import { sql } from 'drizzle-orm';
import pg from 'pg';
import { DatabaseError } from '../errors/types.js';

const { Client } = pg;

export interface QAPair {
  question: string;
  answer: string;
}

export interface PostgreSQLConfig {
  host: string;
  port: number;
  user: string;
  password: string;
  database: string;
  ssl?: boolean | { rejectUnauthorized: boolean };
}

async function exploreDatabaseStructure(db: any, sampleSize: number = 10): Promise<string> {
  let reportContent = '';
  
  const log = (text: string = '') => {
    reportContent += text + '\n';
  };
  
  log('=== DATABASE STRUCTURE REPORT ===');
  log();
  
  const databaseInfo = await db.execute(sql`
    SELECT current_database() as db_name, 
           version() as db_version,
           pg_database_size(current_database()) as db_size
  `);
  
  const dbName = databaseInfo.rows[0].db_name;
  const dbVersion = databaseInfo.rows[0].db_version.split(' ')[1];
  const dbSizeMB = (parseInt(databaseInfo.rows[0].db_size) / 1024 / 1024).toFixed(2);
  
  log('DATABASE METADATA:');
  log(`Database Name: ${dbName}`);
  log(`PostgreSQL Version: ${dbVersion}`);
  log(`Database Size: ${dbSizeMB} MB`);
  log();
  
  const tables = await db.execute(sql`
    SELECT 
      table_schema,
      table_name,
      (SELECT COUNT(*) FROM information_schema.columns c 
       WHERE c.table_schema = t.table_schema 
       AND c.table_name = t.table_name) as column_count
    FROM information_schema.tables t
    WHERE table_schema = 'public'
    ORDER BY table_name
  `);
  
  log(`TABLES OVERVIEW:`);
  log(`Total tables in public schema: ${tables.rows.length}`);
  log();
  
  if (tables.rows.length === 0) {
    log('No tables found in the database.');
    log();
  } else {
    for (const table of tables.rows) {
      log(`TABLE: ${table.table_name}`);
      
      const columns = await db.execute(sql`
        SELECT 
          column_name,
          data_type,
          character_maximum_length,
          is_nullable,
          column_default,
          (
            SELECT COUNT(*) 
            FROM information_schema.key_column_usage k 
            WHERE k.table_name = c.table_name 
              AND k.column_name = c.column_name 
              AND k.constraint_name LIKE '%_pkey'
          ) as is_primary
        FROM information_schema.columns c
        WHERE table_schema = ${table.table_schema}
          AND table_name = ${table.table_name}
        ORDER BY ordinal_position
      `);
      
      const countResult = await db.execute(sql`
        SELECT COUNT(*) as row_count 
        FROM ${sql.identifier(table.table_name)}
      `);
      
      const rowCount = countResult.rows[0].row_count;
      
      log(`Number of rows: ${rowCount}`);
      log(`Number of columns: ${columns.rows.length}`);
      log();
      log('Columns:');
      
      columns.rows.forEach((col: any) => {
        let type = col.data_type;
        if (col.character_maximum_length) {
          type = `${col.data_type}(${col.character_maximum_length})`;
        }
        const nullable = col.is_nullable === 'YES' ? 'nullable' : 'not null';
        const isPrimary = col.is_primary > 0 ? ', primary key' : '';
        const hasDefault = col.column_default ? ', has default value' : '';
        
        log(`- ${col.column_name}: ${type}, ${nullable}${isPrimary}${hasDefault}`);
      });
      
      const sampleData = await db.execute(sql`
        SELECT * FROM ${sql.identifier(table.table_name)} 
        LIMIT ${sampleSize}
      `);
      
      if (sampleData.rows.length > 0) {
        log();
        log(`Sample data (${sampleData.rows.length} rows):`);
        sampleData.rows.forEach((row: any, idx: number) => {
          const rowStr = Object.entries(row)
            .map(([key, val]) => {
              if (val === null) return `${key}=null`;
              if (typeof val === 'string' && val.length > 50) {
                return `${key}="${val.substring(0, 47)}..."`;
              }
              if (val instanceof Date) {
                return `${key}="${val.toISOString()}"`;
              }
              return `${key}="${val}"`;
            })
            .join(', ');
          log(`Row ${idx + 1}: ${rowStr}`);
        });
      }
      
      log();
    }
  }
  
  const foreignKeys = await db.execute(sql`
    SELECT 
      tc.table_name,
      kcu.column_name,
      ccu.table_name AS referenced_table,
      ccu.column_name AS referenced_column
    FROM information_schema.table_constraints tc
    JOIN information_schema.key_column_usage kcu 
      ON tc.constraint_name = kcu.constraint_name
    JOIN information_schema.constraint_column_usage ccu 
      ON ccu.constraint_name = tc.constraint_name
    WHERE tc.constraint_type = 'FOREIGN KEY'
      AND tc.table_schema = 'public'
  `);
  
  if (foreignKeys.rows.length > 0) {
    log('RELATIONSHIPS (Foreign Keys):');
    foreignKeys.rows.forEach((fk: any) => {
      log(`- ${fk.table_name}.${fk.column_name} references ${fk.referenced_table}.${fk.referenced_column}`);
    });
    log();
  }
  
  const indexes = await db.execute(sql`
    SELECT 
      tablename,
      indexname,
      indexdef
    FROM pg_indexes
    WHERE schemaname = 'public'
      AND indexname NOT LIKE '%_pkey'
    ORDER BY tablename, indexname
  `);
  
  if (indexes.rows.length > 0) {
    log('CUSTOM INDEXES:');
    indexes.rows.forEach((idx: any) => {
      log(`- ${idx.tablename}: ${idx.indexname}`);
    });
    log();
  }
  
  log('SUMMARY:');
  const tableStats = await db.execute(sql`
    SELECT 
      (SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public') as table_count,
      (SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = 'public') as total_columns,
      (SELECT COUNT(*) FROM pg_indexes WHERE schemaname = 'public') as index_count,
      (SELECT SUM(n_live_tup) FROM pg_stat_user_tables) as total_rows
  `);
  
  log(`This database contains ${tableStats.rows[0].table_count} tables with a total of ${tableStats.rows[0].total_columns} columns.`);
  log(`There are ${tableStats.rows[0].index_count} indexes defined.`);
  if (tableStats.rows[0].total_rows) {
    log(`Total estimated rows across all tables: ${tableStats.rows[0].total_rows}`);
  }
  
  log();
  log('=== END OF REPORT ===');
  
  return reportContent;
}

async function generateQAFromReport(report: string, backendUrl: string): Promise<QAPair[]> {
  const apiUrl = `${backendUrl}/api/database-qa/generate`;
  
  const response = await fetch(apiUrl, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      databaseReport: report,
      sampleCount: 15
    })
  });
  
  if (!response.ok) {
    let errorMessage = response.statusText;
    try {
      const errorData = await response.json() as { error?: string };
      errorMessage = errorData.error || response.statusText;
    } catch {
      errorMessage = response.statusText;
    }
    throw new DatabaseError('postgresql', 'generate Q&A', new Error(`API Error (${response.status}): ${errorMessage}`));
  }
  
  const result = await response.json() as { 
    success?: boolean; 
    qa_pairs?: Array<{
      question: string;
      answer?: string;
      expected_behavior?: string;
      sql_query?: string;
    }> 
  };
  
  if (result.success && result.qa_pairs) {
    return result.qa_pairs.map((qa) => ({
      question: qa.question,
      answer: qa.expected_behavior || qa.sql_query || qa.answer || 'See database documentation'
    }));
  }
  
  throw new DatabaseError('postgresql', 'parse API response', new Error('No Q&A pairs in API response'));
}

export async function getPostgreSQLQAndA(
  config: PostgreSQLConfig,
  backendUrl: string,
  onProgress?: (message: string) => void
): Promise<QAPair[]> {
  const client = new Client({
    host: config.host,
    port: config.port,
    user: config.user,
    password: config.password,
    database: config.database,
    ssl: config.ssl !== undefined ? config.ssl : { rejectUnauthorized: false }
  });

  try {
    if (onProgress) onProgress('Connecting to PostgreSQL database...');
    await client.connect();
    
    const db = drizzle(client);
    
    if (onProgress) onProgress('Analyzing database structure...');
    const report = await exploreDatabaseStructure(db, 10);
    
    if (onProgress) onProgress('Generating Q&A pairs from database schema...');
    const qaPairs = await generateQAFromReport(report, backendUrl);
    
    if (onProgress) onProgress(`Generated ${qaPairs.length} Q&A pairs`);
    
    return qaPairs;
  } catch (error: any) {
    if (error instanceof DatabaseError) {
      throw error;
    }
    throw new DatabaseError('postgresql', 'connect and generate Q&A', error);
  } finally {
    await client.end();
  }
}

export function parsePostgreSQLConnectionString(connectionString: string): PostgreSQLConfig {
  const url = new URL(connectionString);
  
  return {
    host: url.hostname,
    port: parseInt(url.port) || 5432,
    user: url.username,
    password: decodeURIComponent(url.password),
    database: url.pathname.substring(1),
    ssl: url.searchParams.get('sslmode') !== 'disable' ? { rejectUnauthorized: false } : false
  };
}