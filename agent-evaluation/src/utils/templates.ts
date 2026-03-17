import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';
import axios from 'axios';

export interface TemplateConfig {
  endpoint_url: string;
  type?: string;
  knowledge_source?: string;
  pinecone_url?: string;
  pinecone_api_key?: string;
  postgresql_connection?: string;
  headers?: Record<string, string>;
  body_template?: string;
  response_field?: string;
  name?: string;
  description?: string;
}

export interface Template {
  name: string;
  path: string;
  config: TemplateConfig;
  isRemote?: boolean;
}

const GITHUB_BASE_URL = 'https://raw.githubusercontent.com/rippletideco/rippletide/refs/heads/main/agent-evaluation/templates';

const REMOTE_TEMPLATES = [
  'banking_analyst',
  'blog_to_linkedin',
  'customer_service',
  'local_dev',
  'luxe_concierge',
  'openai_compatible',
  'project_manager'
];

const getTemplatesDir = (): string | null => {
  const currentDir = process.cwd();
  const cliDir = path.resolve(currentDir);
  const templatesDir = path.join(cliDir, 'templates');
  
  if (fs.existsSync(templatesDir)) {
    const configFiles = fs.readdirSync(templatesDir, { withFileTypes: true })
      .filter(dirent => dirent.isDirectory())
      .some(dirent => fs.existsSync(path.join(templatesDir, dirent.name, 'config.json')));
    if (configFiles) {
      return templatesDir;
    }
  }
  
  const altTemplatesDir = path.join(path.dirname(cliDir), 'cli', 'templates');
  if (fs.existsSync(altTemplatesDir)) {
    const configFiles = fs.readdirSync(altTemplatesDir, { withFileTypes: true })
      .filter(dirent => dirent.isDirectory())
      .some(dirent => fs.existsSync(path.join(altTemplatesDir, dirent.name, 'config.json')));
    if (configFiles) {
      return altTemplatesDir;
    }
  }
  
  return null;
};

const loadLocalTemplates = (): Template[] => {
  const templates: Template[] = [];
  const templatesDir = getTemplatesDir();
  
  if (!templatesDir || !fs.existsSync(templatesDir)) {
    return templates;
  }
  
  const dirs = fs.readdirSync(templatesDir, { withFileTypes: true })
    .filter(dirent => dirent.isDirectory())
    .map(dirent => dirent.name);
  
  for (const dir of dirs) {
    const configPath = path.join(templatesDir, dir, 'config.json');
    if (fs.existsSync(configPath)) {
      try {
        const configContent = fs.readFileSync(configPath, 'utf-8');
        const config = JSON.parse(configContent) as TemplateConfig;
        templates.push({
          name: dir,
          path: path.join(templatesDir, dir),
          config: {
            ...config,
            name: config.name || dir,
            description: config.description || `Template: ${dir}`
          },
          isRemote: false
        });
      } catch (error) {
      }
    }
  }
  
  return templates;
};

const loadRemoteTemplate = async (templateName: string): Promise<Template | null> => {
  try {
    const configUrl = `${GITHUB_BASE_URL}/${templateName}/config.json`;
    const response = await axios.get(configUrl, { timeout: 5000 });
    const config = response.data as TemplateConfig;
    
    return {
      name: templateName,
      path: `${GITHUB_BASE_URL}/${templateName}`,
      config: {
        ...config,
        name: config.name || templateName,
        description: config.description || `Template: ${templateName}`
      },
      isRemote: true
    };
  } catch (error) {
    return null;
  }
};

export const listTemplates = async (): Promise<Template[]> => {
  const localTemplates = loadLocalTemplates();
  
  if (localTemplates.length > 0) {
    return localTemplates;
  }
  
  const remoteTemplates: Template[] = [];
  for (const templateName of REMOTE_TEMPLATES) {
    const template = await loadRemoteTemplate(templateName);
    if (template) {
      remoteTemplates.push(template);
    }
  }
  
  return remoteTemplates;
};

export const loadTemplate = async (templateName: string): Promise<Template | null> => {
  const localTemplates = loadLocalTemplates();
  const localTemplate = localTemplates.find(t => 
    t.name.toLowerCase() === templateName.toLowerCase() ||
    t.config.name?.toLowerCase() === templateName.toLowerCase()
  );
  
  if (localTemplate) {
    return localTemplate;
  }
  
  return await loadRemoteTemplate(templateName);
};

export const getTemplateOptions = async (template: Template): Promise<any> => {
  const config = template.config;
  
  let templatePath = template.path;
  if (template.isRemote) {
    templatePath = `${template.path}`;
  }
  
  return {
    agentEndpoint: config.endpoint_url,
    knowledgeSource: config.knowledge_source || 'files',
    pineconeUrl: config.pinecone_url,
    pineconeApiKey: config.pinecone_api_key,
    postgresqlConnection: config.postgresql_connection,
    headers: config.headers,
    bodyTemplate: config.body_template,
    responseField: config.response_field,
    nonInteractive: true,
    templatePath,
    isRemoteTemplate: template.isRemote
  };
};

export const loadRemoteQAndA = async (templateName: string): Promise<any> => {
  try {
    const qandaUrl = `${GITHUB_BASE_URL}/${templateName}/qanda.json`;
    const response = await axios.get(qandaUrl, { timeout: 5000 });
    return response.data;
  } catch (error) {
    return null;
  }
};
