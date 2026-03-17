#!/usr/bin/env node
import React from 'react';
import { render } from 'ink';
import { App } from './App.js';
import { ErrorHandler } from './errors/handler.js';
import { ValidationError } from './errors/types.js';
import { listTemplates, loadTemplate, getTemplateOptions } from './utils/templates.js';
import { analytics } from './utils/analytics.js';

const parseArgs = async () => {
  const args = process.argv.slice(2);
  
  if (args[0] === 'list-templates' || args[0] === 'templates') {
    const templates = await listTemplates();
    console.log('\nAvailable Templates:\n');
    if (templates.length === 0) {
      console.log('No templates found');
    } else {
      templates.forEach(template => {
        console.log(`  ${template.name}`);
        if (template.config.description) {
          console.log(`    ${template.config.description}`);
        }
        console.log(`    Endpoint: ${template.config.endpoint_url}`);
        if (template.config.type) {
          console.log(`    Type: ${template.config.type}`);
        }
        console.log();
      });
    }
    process.exit(0);
  }
  
  const options: any = {
    backendUrl: 'https://agent-evalserver-production.up.railway.app',
    dashboardUrl: 'https://app.rippletide.com/eval',
    debug: false,
    nonInteractive: false
  };

  for (let i = 0; i < args.length; i++) {
    if ((args[i] === '--template' || args[i] === '-t') && args[i + 1]) {
      const template = await loadTemplate(args[i + 1]);
      if (!template) {
        console.error(`Template '${args[i + 1]}' not found.`);
        console.log('\nAvailable templates:');
        const templates = await listTemplates();
        templates.forEach(t => console.log(`  - ${t.name}`));
        process.exit(1);
      }
      const templateOptions = await getTemplateOptions(template);
      Object.assign(options, templateOptions);
      i++;
    } else if ((args[i] === '--backend-url' || args[i] === '-b') && args[i + 1]) {
      options.backendUrl = args[i + 1];
      i++;
    } else if ((args[i] === '--dashboard-url' || args[i] === '-d') && args[i + 1]) {
      options.dashboardUrl = args[i + 1];
      i++;
    } else if ((args[i] === '--agent' || args[i] === '-a') && args[i + 1]) {
      options.agentEndpoint = args[i + 1];
      options.nonInteractive = true;
      i++;
    } else if ((args[i] === '--knowledge' || args[i] === '-k') && args[i + 1]) {
      options.knowledgeSource = args[i + 1];
      i++;
    } else if ((args[i] === '--pdf-path' || args[i] === '-pp') && args[i + 1]) {
      options.pdfPath = args[i + 1];
      i++;
    } else if ((args[i] === '--pinecone-url' || args[i] === '-pu') && args[i + 1]) {
      options.pineconeUrl = args[i + 1];
      i++;
    } else if ((args[i] === '--pinecone-key' || args[i] === '-pk') && args[i + 1]) {
      options.pineconeApiKey = args[i + 1];
      i++;
    } else if ((args[i] === '--postgresql' || args[i] === '-pg') && args[i + 1]) {
      options.postgresqlConnection = args[i + 1];
      i++;
    } else if ((args[i] === '--headers' || args[i] === '-H') && args[i + 1]) {
      if (!options.headers) options.headers = {};
      const headerPairs = args[i + 1].split(',');
      headerPairs.forEach(pair => {
        const [key, ...valueParts] = pair.split(':');
        if (key && valueParts.length > 0) {
          options.headers[key.trim()] = valueParts.join(':').trim();
        }
      });
      i++;
    } else if ((args[i] === '--body' || args[i] === '-B') && args[i + 1]) {
      options.bodyTemplate = args[i + 1];
      i++;
    } else if ((args[i] === '--response-field' || args[i] === '-rf') && args[i + 1]) {
      options.responseField = args[i + 1];
      i++;
    } else if (args[i] === '--debug') {
      options.debug = true;
    } else if (args[i] === '--no-analytics') {
      options.noAnalytics = true;
      analytics.setAnalyticsEnabled(false);
    } else if (args[i] === '--help' || args[i] === '-h') {
      console.log(`
Rippletide CLI

Usage:
  rippletide eval [options]
  rippletide list-templates
  rippletide templates

Commands:
  eval                        Run evaluation (default)
  list-templates, templates   List available templates

Options:
  -t, --template <name>       Use a pre-configured template
  -a, --agent <url>           Agent endpoint URL (e.g., localhost:8000)
  -k, --knowledge <source>    Knowledge source: files, pinecone, postgresql, or pdf (default: files)
  -b, --backend-url <url>     Backend API URL (default: https://agent-evalserver-production.up.railway.app)
  -d, --dashboard-url <url>   Dashboard URL (default: https://app.rippletide.com/eval)
  
  Pinecone options:
  -pu, --pinecone-url <url>   Pinecone database URL
  -pk, --pinecone-key <key>   Pinecone API key
  
  PostgreSQL options:
  -pg, --postgresql <conn>    PostgreSQL connection string or comma-separated values
  
  PDF options:
  -pp, --pdf-path <path>      Path to PDF file for knowledge extraction
  
  Custom endpoint options:
  -H, --headers <headers>     Custom headers (e.g., "Authorization: Bearer token, X-API-Key: key")
  -B, --body <template>       Custom body template (use {question} as placeholder)
  -rf, --response-field <path> Response field path (e.g., "data.response")
  
  Other options:
  --debug                     Show detailed error information
  --no-analytics              Disable usage analytics
  -h, --help                  Show this help message

Examples:
  # Interactive mode (default)
  rippletide eval
  
  # List available templates
  rippletide list-templates
  
  # Run with a template
  rippletide eval -t customer_service
  
  # Direct evaluation with local files
  rippletide eval -a localhost:8000
  
  # Direct evaluation with Pinecone
  rippletide eval -a localhost:8000 -k pinecone -pu https://db.pinecone.io -pk pcsk_xxxxx
  
  # Direct evaluation with PostgreSQL
  rippletide eval -a localhost:8000 -k postgresql -pg "postgresql://user:pass@localhost:5432/db"
  
  # Direct evaluation with PDF
  rippletide eval -a localhost:8000 -k pdf -pp ./docs/manual.pdf
  
  # With custom headers and body
  rippletide eval -a localhost:8000 -H "Authorization: Bearer token" -B '{"prompt": "{question}"}'
`);
      process.exit(0);
    } else if (!args[i].startsWith('-')) {
      throw new ValidationError(
        'command',
        args[i],
        `Unknown argument: ${args[i]}`
      );
    }
  }

  return options;
};

async function run() {
  try {
    const options = await parseArgs();
    
    const fs = await import('fs');
    const path = await import('path');
    const os = await import('os');
    const configPath = path.join(os.homedir(), '.rippletide', 'config.json');
    const isNewUser = !fs.existsSync(configPath);
    
    analytics.track('cli_started', {
      command: process.argv[2] || 'eval',
      has_template: !!options.templatePath,
      knowledge_source: options.knowledgeSource || 'files',
      is_non_interactive: options.nonInteractive,
      is_new_user: isNewUser,
    });
    
    if (isNewUser) {
      analytics.track('new_user_activated', {
        source: process.argv[2] || 'eval',
        activation_date: new Date().toISOString(),
      });
    }
    
    await new Promise(resolve => setTimeout(resolve, 100));
    
    process.stdout.write('\x1Bc');
    
    const { waitUntilExit } = render(
      <App 
        backendUrl={options.backendUrl} 
        dashboardUrl={options.dashboardUrl}
        nonInteractive={options.nonInteractive}
        agentEndpoint={options.agentEndpoint}
        knowledgeSource={options.knowledgeSource}
        pineconeUrl={options.pineconeUrl}
        pineconeApiKey={options.pineconeApiKey}
        postgresqlConnection={options.postgresqlConnection}
        customHeaders={options.headers}
        customBodyTemplate={options.bodyTemplate}
        customResponseField={options.responseField}
        templatePath={options.templatePath}
        isRemoteTemplate={options.isRemoteTemplate}
        pdfPath={options.pdfPath}
      />
    );
    
    await waitUntilExit();
    
    await analytics.shutdown();
    await new Promise(resolve => setTimeout(resolve, 500));
  } catch (error) {
    analytics.track('cli_error', {
      error_type: error instanceof Error ? error.constructor.name : 'Unknown',
      error_message: error instanceof Error ? error.message : String(error),
    });
    await analytics.shutdown();
    await new Promise(resolve => setTimeout(resolve, 500));
    ErrorHandler.handle(error);
    process.exit(1);
  }
}

process.on('uncaughtException', (error) => {
  ErrorHandler.handle(error);
  process.exit(1);
});

process.on('unhandledRejection', (error) => {
  ErrorHandler.handle(error);
  process.exit(1);
});

run();
