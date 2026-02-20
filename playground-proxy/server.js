const express = require('express');
const cors = require('cors');
const { AzureOpenAI } = require('openai');
const { Client } = require("@modelcontextprotocol/sdk/client/index.js");
const { StreamableHTTPClientTransport } = require("@modelcontextprotocol/sdk/client/streamableHttp.js");
require('dotenv').config();

const app = express();
const PORT = process.env.PORT || 3042;
const APP_VERSION = process.env.APP_VERSION || '1.0.0';

function extractAgentIdFromChatUrl(chatUrl) {
    if (!chatUrl) return null;
    const match = String(chatUrl).match(/\/api\/agents\/([^/]+)\/chat/i);
    return match ? match[1] : null;
}

async function callAgentChatEndpoint(chatUrl, payload) {
    const response = await fetch(chatUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
    });

    let data;
    try {
        data = await response.json();
    } catch (error) {
        throw new Error(`Invalid JSON from agent endpoint (${response.status})`);
    }

    if (!response.ok || (data && data.error)) {
        throw new Error((data && data.error) ? data.error : `Agent endpoint error (${response.status})`);
    }

    return data;
}

async function runRecallProbe(agentId, message) {
    let transport;
    try {
        const mcpUrl = new URL(`https://mcp.rippletide.com/mcp?agentId=${agentId}`);
        transport = new StreamableHTTPClientTransport(mcpUrl);
        const mcpClient = new Client({ name: 'playground-docs', version: '1.0.0' });
        await mcpClient.connect(transport);

        const recallResult = await mcpClient.callTool({
            name: 'recall',
            arguments: { query: message, limit: 3 }
        });

        const toolResultText = Array.isArray(recallResult.content)
            ? recallResult.content.map(c => c.text).filter(Boolean).join('\n')
            : '';

        return [{
            name: 'recall',
            params: message,
            result: toolResultText
        }];
    } catch (error) {
        return [{
            name: 'recall_probe',
            params: message,
            result: `probe_failed: ${error.message}`
        }];
    } finally {
        if (transport) {
            await transport.close().catch(() => {});
        }
    }
}

function getAllowedOrigins() {
    const configured = (process.env.CORS_ORIGINS || '')
        .split(',')
        .map(s => s.trim())
        .filter(Boolean);

    const defaults = [
        'https://docs.rippletide.com',
        'https://rippletide.com',
        'https://www.rippletide.com',
        'http://localhost:3000',
        'http://localhost:3333',
        'http://localhost:3334'
    ];

    // Merge + dedupe while preserving order.
    const merged = [...defaults, ...configured];
    const seen = new Set();
    const result = [];
    for (const origin of merged) {
        if (!seen.has(origin)) {
            seen.add(origin);
            result.push(origin);
        }
    }
    return result;
}

const allowedOrigins = getAllowedOrigins();

// CORS: allow docs site + local dev
app.use(cors({
    origin: (origin, callback) => {
        // Allow non-browser clients (curl, server-side calls)
        if (!origin) return callback(null, true);

        // Allow configured explicit origins.
        if (allowedOrigins.includes(origin)) return callback(null, true);

        // Allow Vercel preview docs domains if needed.
        if (/^https:\/\/.*\.vercel\.app$/.test(origin)) return callback(null, true);

        return callback(null, false);
    },
    methods: ['GET', 'POST', 'OPTIONS'],
    allowedHeaders: ['Content-Type'],
    optionsSuccessStatus: 200
}));
app.use(express.json({ limit: '1mb' }));

// Azure OpenAI client
function getOpenAIClient() {
    return new AzureOpenAI({
        apiKey: process.env.AZURE_API_KEY,
        endpoint: process.env.AZURE_OPENAI_ENDPOINT,
        apiVersion: process.env.AZURE_API_VERSION || '2024-08-01-preview',
        deployment: process.env.AZURE_OPENAI_DEPLOYMENT || 'gpt-4o-mini'
    });
}

// Health check
app.get('/health', (req, res) => res.json({ status: 'ok' }));
app.get('/', (req, res) => {
    res.json({
        service: 'playground-proxy',
        status: 'ok',
        version: APP_VERSION,
        routes: [
            'GET /health',
            'POST /api/chat-playground/vanilla',
            'POST /api/chat-playground/mcp'
        ]
    });
});

// ─── VANILLA ROUTE (No Context Graph) ───────────────────────────────
app.post('/api/chat-playground/vanilla', async (req, res) => {
    try {
        const { message, history } = req.body;
        if (!message) return res.status(400).json({ error: 'Missing message' });

        const client = getOpenAIClient();

        // Build messages with conversation history
        const messages = [
            { role: 'system', content: 'You are a helpful assistant. You have no long-term memory and no tools. You can only see the current conversation. Keep responses concise (2-3 sentences max).' },
        ];

        // Add conversation history
        if (history && Array.isArray(history)) {
            for (const h of history) {
                messages.push({ role: 'user', content: h.user });
                messages.push({ role: 'assistant', content: h.assistant });
            }
        }

        // Add current message
        messages.push({ role: 'user', content: message });

        const response = await client.chat.completions.create({
            model: process.env.AZURE_OPENAI_DEPLOYMENT || 'gpt-4o-mini',
            temperature: 0,
            messages
        });

        res.json({
            response: response.choices[0].message.content,
            error: null
        });
    } catch (error) {
        console.error('Vanilla error:', error.message);
        res.status(500).json({ error: 'API Error (Vanilla): ' + error.message });
    }
});

// ─── MCP ROUTE (With Context Graph) ─────────────────────────────────
app.post('/api/chat-playground/mcp', async (req, res) => {
    let transport;
    try {
        const { message, sessionId, history, chatAgentUrl, agentId } = req.body;
        if (!message) return res.status(400).json({ error: 'Missing message' });
        if (!sessionId) return res.status(400).json({ error: 'Missing sessionId' });

        const selectedChatUrl = chatAgentUrl || process.env.MCP_AGENT_CHAT_URL || '';
        if (selectedChatUrl) {
            const selectedAgentId = agentId || extractAgentIdFromChatUrl(selectedChatUrl) || sessionId;
            const toolLogs = await runRecallProbe(selectedAgentId, message);
            const upstream = await callAgentChatEndpoint(selectedChatUrl, {
                message,
                sessionId,
                history: Array.isArray(history) ? history : []
            });

            const responseText = upstream.message
                || upstream.response
                || upstream.output
                || (typeof upstream === 'string' ? upstream : JSON.stringify(upstream));

            return res.json({
                response: responseText,
                tools: toolLogs,
                error: null
            });
        }

        const effectiveAgentId = agentId || sessionId;

        // 1. Connect to Rippletide MCP server for this session
        const mcpUrl = new URL(`https://mcp.rippletide.com/mcp?agentId=${effectiveAgentId}`);
        transport = new StreamableHTTPClientTransport(mcpUrl);
        const mcpClient = new Client({ name: "playground-docs", version: "1.0.0" });

        await mcpClient.connect(transport);

        // 2. Get available tools from MCP server
        const toolsResponse = await mcpClient.listTools();
        const openaiTools = toolsResponse.tools.map(tool => ({
            type: "function",
            function: {
                name: tool.name,
                description: tool.description,
                parameters: tool.inputSchema
            }
        }));

        const client = getOpenAIClient();

        // Build messages with conversation history
        const messages = [
            { role: 'system', content: 'You are an AI assistant with persistent memory via the Rippletide Context Graph. ALWAYS use the recall tool at the start of each conversation to check for previously stored information. Use remember to store new facts, preferences and decisions. Use relate to link entities together. Keep responses concise (2-3 sentences max).' },
        ];

        // Add conversation history
        if (history && Array.isArray(history)) {
            for (const h of history) {
                messages.push({ role: 'user', content: h.user });
                messages.push({ role: 'assistant', content: h.assistant });
            }
        }

        // Add current message
        messages.push({ role: 'user', content: message });

        // 3. First call — let the model decide if tools are needed
        let response = await client.chat.completions.create({
            model: process.env.AZURE_OPENAI_DEPLOYMENT || 'gpt-4o-mini',
            temperature: 0,
            messages,
            tools: openaiTools
        });

        let finalMessage = response.choices[0].message;
        let toolLogs = [];

        // 4. Execute tool calls in a loop (multiple rounds possible)
        let maxRounds = 5;
        while (finalMessage.tool_calls && maxRounds > 0) {
            maxRounds--;
            messages.push(finalMessage);

            for (const toolCall of finalMessage.tool_calls) {
                const { name, arguments: args } = toolCall.function;
                const parsedArgs = JSON.parse(args);

                // Call the actual MCP tool on Rippletide
                const result = await mcpClient.callTool({
                    name,
                    arguments: parsedArgs
                });

                const toolResultText = result.content.map(c => c.text).join('\n');

                toolLogs.push({
                    name,
                    params: typeof parsedArgs === 'object' ? Object.values(parsedArgs).join(', ') : String(parsedArgs),
                    result: toolResultText
                });

                messages.push({
                    role: "tool",
                    tool_call_id: toolCall.id,
                    content: toolResultText
                });
            }

            // Next round — synthesize or call more tools
            const followUp = await client.chat.completions.create({
                model: process.env.AZURE_OPENAI_DEPLOYMENT || 'gpt-4o-mini',
                temperature: 0,
                messages,
                tools: openaiTools
            });
            finalMessage = followUp.choices[0].message;
        }

        await transport.close();

        res.json({
            response: finalMessage.content,
            tools: toolLogs,
            error: null
        });

    } catch (error) {
        console.error('MCP error:', error.message);
        if (transport) await transport.close().catch(() => {});
        res.status(500).json({ error: 'API Error (MCP): ' + error.message });
    }
});

module.exports = app;

if (require.main === module) {
    app.listen(PORT, () => {
        console.log(`\n🚀 Playground Proxy running on port ${PORT}`);
        console.log(`   POST /api/chat-playground/vanilla  →  Azure OpenAI (no memory)`);
        console.log(`   POST /api/chat-playground/mcp      →  Azure OpenAI + Context Graph\n`);
    });
}
