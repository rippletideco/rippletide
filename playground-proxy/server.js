const express = require('express');
const cors = require('cors');
const { AzureOpenAI } = require('openai');
const { Client } = require("@modelcontextprotocol/sdk/client/index.js");
const { StreamableHTTPClientTransport } = require("@modelcontextprotocol/sdk/client/streamableHttp.js");
require('dotenv').config();

const app = express();
const PORT = process.env.PORT || 3042;

// CORS: allow docs site + local dev
app.use(cors({
    origin: [
        'https://docs.rippletide.com',
        'http://localhost:3000',
        'http://localhost:3333',
        'http://localhost:3334'
    ]
}));
app.use(express.json());

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
        const { message, sessionId, history } = req.body;
        if (!message) return res.status(400).json({ error: 'Missing message' });
        if (!sessionId) return res.status(400).json({ error: 'Missing sessionId' });

        // 1. Connect to Rippletide MCP server for this session
        const mcpUrl = new URL(`https://mcp.rippletide.com/mcp?agentId=${sessionId}`);
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

app.listen(PORT, () => {
    console.log(`\n🚀 Playground Proxy running on port ${PORT}`);
    console.log(`   POST /api/chat-playground/vanilla  →  Azure OpenAI (no memory)`);
    console.log(`   POST /api/chat-playground/mcp      →  Azure OpenAI + Context Graph\n`);
});
