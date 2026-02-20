---
title: Agent Development Guide
description: Comprehensive guide for building conversational AI agents with the Rippletide SDK
---

> **For AI Code Assistants (Claude, Cursor, Codex)**: This document contains complete, executable code examples for building conversational AI agents with the Rippletide SDK. All code blocks are ready to copy-paste and run.

## Quick Reference for AI Assistants

**Base URL**: `https://agent.rippletide.com/api/sdk`
**Authentication**: `x-api-key` header with your API key
**Key Endpoints**:
- `POST /agent` - Create agent
- `POST /q-and-a` - Add knowledge
- `POST /chat/{agent_id}` - Chat with agent
- `POST /action` - Define agent actions
- `PUT /state-predicate/{agent_id}` - Set conversation flow

## Quick Start

### Prerequisites

```bash
# Install required packages
pip install requests langchain-openai

# Set your API key
export RIPPLETIDE_API_KEY="your-api-key-here"
```

### Environment Setup

```python
import os
import uuid
import requests
```

```python
# Required environment variables
RIPPLETIDE_API_KEY = os.environ["RIPPLETIDE_API_KEY"]
BASE_URL = "https://agent.rippletide.com/api/sdk"
headers = {
    "x-api-key": RIPPLETIDE_API_KEY,
    "Content-Type": "application/json"
}
```

### Basic Agent Setup

```python
# 1. Create an agent
def create_agent():
    url = f"{BASE_URL}/agent"
    data = {
        "name": "my-agent",
        "prompt": "You are a helpful assistant that provides accurate information based on your knowledge base."
    }
    response = requests.post(url, headers=headers, json=data)
    response.raise_for_status()
    return response.json()

# 2. Add knowledge (Q&A pairs)
def add_knowledge(agent_id, question, answer):
    url = f"{BASE_URL}/q-and-a"
    data = {
        "question": question,
        "answer": answer,
        "agent_id": agent_id
    }
    response = requests.post(url, headers=headers, json=data)
    response.raise_for_status()
    return response.json()

# 3. Chat with the agent
def chat(agent_id, message, conversation_id):
    url = f"{BASE_URL}/chat/{agent_id}"
    data = {
        "user_message": message,
        "conversation_uuid": conversation_id
    }
    response = requests.post(url, headers=headers, json=data)
    response.raise_for_status()
    return response.json()
```

```python
# Complete working example
def main():
    # Create agent
    agent = create_agent()
    agent_id = agent["id"]
    print(f"Created agent: {agent_id}")

    # Add knowledge
    add_knowledge(agent_id, "What is Rippletide?", "Rippletide is a platform for building reliable AI agents with minimal hallucinations.")
    print("Added knowledge")

    # Start conversation
    conversation_id = str(uuid.uuid4())
    response = chat(agent_id, "What is Rippletide?", conversation_id)
    print(f"Agent response: {response['answer']}")

if __name__ == "__main__":
    main()
```

## Core Concepts

### Hypergraph Architecture

Rippletide uses a hypergraph-based knowledge representation system:

- **Entities**: Unique identifiers (UUIDs) representing concepts
- **Relations**: Directed connections between entities
- **Tags**: Labels for organizing and categorizing content
- **Data**: Typed values stored on entities
- **Commits**: Version control for all changes

### Key Components

1. **Agents**: The conversational AI entities that interact with users
2. **Q&A Pairs**: The knowledge base that agents use to answer questions
3. **Tags**: Organizational labels for categorizing knowledge
4. **Actions**: Functions that agents can perform
5. **State Predicates**: Rules that govern agent behavior and state transitions
6. **Guardrails**: Safety constraints that prevent inappropriate responses

## Next Steps

<CardGroup cols={2}>
  <Card title="SDK API Reference" icon="code" href="/docs/agent_api_reference">
    All SDK endpoints for agent, knowledge, action, guardrail, and chat management
  </Card>

  <Card title="Advanced Guide" icon="book" href="/docs/agent_advanced">
    Configuration, state management, LangChain integration, complete examples, and best practices
  </Card>
</CardGroup>
