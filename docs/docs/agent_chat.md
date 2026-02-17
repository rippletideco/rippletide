---
title: Chat & LangChain Integration
description: Send messages to your agent and use it as a drop-in LLM in LangChain
---

## Prerequisites

Before chatting, you need an agent with knowledge configured. Follow the [Create your Hypergraph](/docs/agent_setup) guide first to get your `agent_id`.

## Chat via the SDK API

Send a message and get a response:

```python
import os
import uuid
import requests

API_KEY = os.environ["RIPPLETIDE_API_KEY"]
BASE_URL = "https://agent.rippletide.com/api/sdk"
headers = {"x-api-key": API_KEY, "Content-Type": "application/json"}

agent_id = "your-agent-id"  # from the agent creation step
conversation_id = str(uuid.uuid4())  # one ID per conversation session

response = requests.post(f"{BASE_URL}/chat/{agent_id}", headers=headers, json={
    "user_message": "What products can I order?",
    "conversation_uuid": conversation_id
})

print(response.json()["answer"])
```

Use the same `conversation_uuid` for follow-up messages in the same conversation. Generate a new UUID to start a fresh session.

**API Reference**: [Chat API](/api-reference/introduction#tag/Chat)

## LangChain Integration

<Warning>This feature is experimental.</Warning>

You can use your Rippletide agent directly as a LLM in LangChain and LangGraph. This lets you replace your current LLM (e.g. ChatGPT) with a hallucination-free Rippletide agent in a few lines.

### Installation

```bash
pip install langchain langchain-openai
```

### Usage

Rippletide exposes an Azure OpenAI-compatible endpoint, so you can use `AzureChatOpenAI` from LangChain:

```python
import os
import uuid

from langchain_core.messages import SystemMessage, HumanMessage
from langchain_openai import AzureChatOpenAI

API_KEY = os.environ["RIPPLETIDE_API_KEY"]
agent_id = "your-agent-id"  # from the agent creation step
conversation_id = str(uuid.uuid4())

rippletide_llm = AzureChatOpenAI(
    model="v1",
    api_key=API_KEY,
    azure_endpoint="https://agent.rippletide.com",
    azure_deployment="v1",
    api_version="2024-12-01-preview",
    openai_api_type="azure",
    default_headers={
        "x-rippletide-agent-id": agent_id,
        "x-rippletide-conversation-id": str(conversation_id),
    },
)

messages = [
    SystemMessage(content="You are a helpful assistant."),
    HumanMessage(content="What products can I order?")
]

response = rippletide_llm.invoke(messages)
print(response.content)
```

### Use in a LangChain chain

Once initialized, `rippletide_llm` works like any other LangChain LLM:

```python
from langchain.prompts import ChatPromptTemplate

prompt = ChatPromptTemplate.from_messages([
    ("system", "You are a helpful assistant for an electronics store."),
    ("human", "{question}")
])

chain = prompt | rippletide_llm

result = chain.invoke({"question": "How long does delivery take?"})
print(result.content)
```

## Next Steps

<CardGroup cols={2}>
  <Card title="Evaluate your agent" icon="chart-line" href="/docs/evaluation_overview">
    Test for hallucinations before deploying to production
  </Card>

  <Card title="Full Developer Guide" icon="book" href="/docs/Agent">
    Complete API reference with advanced examples
  </Card>
</CardGroup>
