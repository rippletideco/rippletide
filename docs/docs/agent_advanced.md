---
title: Advanced Agent Guide
description: Configuration, integrations, best practices, and complete examples for Rippletide agents
---

## Agent Configuration

### System Prompt

The system prompt defines the agent's personality and behavior:

```python
agent_data = {
    "name": "customer-support-agent",
    "prompt": """You are a professional customer support agent for an e-commerce platform.
    Your role is to help customers with their orders, answer product questions, and resolve issues.
    Always be polite, helpful, and accurate in your responses. Use only the information provided
    in your knowledge base to answer questions."""
}
```

### Best Practices for Prompts

1. **Be Specific**: Clearly define the agent's role and responsibilities
2. **Set Boundaries**: Specify what the agent should and shouldn't do
3. **Include Context**: Provide relevant background information
4. **Define Tone**: Specify the communication style (professional, friendly, etc.)

## Knowledge Management

### Q&A Structure

Each Q&A pair should be:
- **Specific**: Address a particular question or scenario
- **Accurate**: Provide correct, up-to-date information
- **Complete**: Include all necessary details
- **Tagged**: Organized with relevant tags for better retrieval

### Example Q&A Setup

```python
def setup_knowledge_base(agent_id):
    q_and_a_pairs = [
        {
            "question": "What are your business hours?",
            "answer": "We are open Monday through Friday from 9 AM to 6 PM EST.",
            "tags": ["business_hours", "contact_info"]
        },
        {
            "question": "What is your return policy?",
            "answer": "We offer a 30-day return policy for most items. Items must be in original condition.",
            "tags": ["returns", "policy"]
        }
    ]

    for qa in q_and_a_pairs:
        qa_response = requests.post(
            f"{BASE_URL}/q-and-a", headers=headers,
            json={"question": qa["question"], "answer": qa["answer"], "agent_id": agent_id}
        )
        qa_id = qa_response.json()["id"]

        for tag_name in qa["tags"]:
            tag_response = requests.post(
                f"{BASE_URL}/tag", headers=headers,
                json={"name": tag_name, "description": f"Tag for {tag_name}"}
            )
            requests.post(
                f"{BASE_URL}/q-and-a-tag", headers=headers,
                json={"q_and_a_id": qa_id, "tag_id": tag_response.json()["id"]}
            )
```

### Tag Organization

```python
TAG_CATEGORIES = {
    "product_info": ["pricing", "specifications", "availability"],
    "customer_service": ["returns", "shipping", "support"],
    "account_management": ["login", "profile", "orders"],
    "technical": ["troubleshooting", "installation", "compatibility"]
}
```

## State Management

### State Predicates

State predicates define conversation flow based on the current state:

```python
def create_order_flow_state_predicate():
    return {
        "transition_kind": "branch",
        "question_to_evaluate": "What is the user trying to do?",
        "possible_values": ["place_order", "track_order", "return_item", "get_support"],
        "re_evaluate": True,
        "value_to_node": {
            "place_order": {
                "transition_kind": "branch",
                "question_to_evaluate": "What product are they interested in?",
                "possible_values": ["product_selected", "need_recommendation"],
                "value_to_node": {
                    "product_selected": {
                        "transition_kind": "end",
                        "question_to_evaluate": "Proceeding to checkout..."
                    },
                    "need_recommendation": {
                        "transition_kind": "end",
                        "question_to_evaluate": "Let me recommend some products."
                    }
                }
            },
            "track_order": {
                "transition_kind": "end",
                "question_to_evaluate": "Please provide your order number."
            },
            "return_item": {
                "transition_kind": "end",
                "question_to_evaluate": "I'll help with the return. What's your order number?"
            },
            "get_support": {
                "transition_kind": "end",
                "question_to_evaluate": "What issue are you experiencing?"
            }
        }
    }
```

### Setting State Predicates

```python
def set_agent_state_predicate(agent_id, state_predicate):
    response = requests.put(
        f"{BASE_URL}/state-predicate/{agent_id}",
        headers=headers,
        json={"state_predicate": state_predicate}
    )
    response.raise_for_status()
    return response.json()
```

## Chat Integration

### Basic Chat Implementation

```python
class RippletideChat:
    def __init__(self, agent_id, api_key):
        self.agent_id = agent_id
        self.api_key = api_key
        self.conversation_id = str(uuid.uuid4())
        self.headers = {"x-api-key": api_key, "Content-Type": "application/json"}

    def send_message(self, message):
        url = f"{BASE_URL}/chat/{self.agent_id}"
        data = {"user_message": message, "conversation_uuid": self.conversation_id}
        try:
            response = requests.post(url, headers=self.headers, json=data)
            response.raise_for_status()
            return response.json()["answer"]
        except requests.exceptions.RequestException as e:
            return f"Error: {e}"

    def start_new_conversation(self):
        self.conversation_id = str(uuid.uuid4())
```

## LangChain Integration

Rippletide provides a LangChain-compatible Azure OpenAI endpoint:

```python
from langchain_core.messages import SystemMessage, HumanMessage
from langchain_openai import AzureChatOpenAI

rippletide_llm = AzureChatOpenAI(
    model="v1",
    api_key=RIPPLETIDE_API_KEY,
    azure_endpoint="https://agent.rippletide.com",
    azure_deployment="v1",
    api_version="2024-12-01-preview",
    openai_api_type="azure",
    default_headers={
        "x-rippletide-agent-id": agent_id,
        "x-rippletide-conversation-id": conversation_id,
    },
)

messages = [
    SystemMessage(content="You are a helpful assistant."),
    HumanMessage(content="Hello, how can you help me?")
]
response = rippletide_llm.invoke(messages)
print(response.content)
```

## Best Practices

### 1. Knowledge Base Design
- **Granular Q&A**: Break complex topics into specific, focused Q&A pairs
- **Consistent Formatting**: Use consistent question and answer formats
- **Regular Updates**: Keep knowledge base current and accurate
- **Tag Organization**: Use a clear, hierarchical tagging system

### 2. Agent Configuration
- **Clear Prompts**: Write specific, actionable system prompts
- **Appropriate Guardrails**: Set boundaries without being overly restrictive
- **State Management**: Design logical conversation flows
- **Error Handling**: Implement robust error handling and fallbacks

### 3. Performance Optimization
- **Efficient Queries**: Use specific questions to get relevant answers
- **Rate Limiting**: Implement proper rate limiting for API calls
- **Caching**: Cache frequently accessed knowledge when appropriate

### 4. Security
- **API Key Management**: Store API keys securely (environment variables)
- **Input Validation**: Validate all user inputs
- **Content Filtering**: Implement content filtering for sensitive topics

## Error Handling

### Common Issues

**Authentication Errors:**

```python
if not os.environ.get("RIPPLETIDE_API_KEY"):
    raise ValueError("RIPPLETIDE_API_KEY environment variable not set")
```

**Rate Limiting with Retry:**

```python
import time

def make_request_with_retry(url, headers, json_data, max_retries=3):
    for attempt in range(max_retries):
        try:
            response = requests.post(url, headers=headers, json=json_data)
            if response.status_code == 429:
                time.sleep(2 ** attempt)
                continue
            response.raise_for_status()
            return response
        except requests.exceptions.RequestException as e:
            if attempt == max_retries - 1:
                raise e
            time.sleep(1)
```

**Enable Request Logging:**

```python
import logging
logging.basicConfig(level=logging.DEBUG)
requests_log = logging.getLogger("requests.packages.urllib3")
requests_log.setLevel(logging.DEBUG)
```

## Quick Reference Templates

### Minimal Agent Setup

```python
import os, uuid, requests

RIPPLETIDE_API_KEY = os.environ["RIPPLETIDE_API_KEY"]
BASE_URL = "https://agent.rippletide.com/api/sdk"
headers = {"x-api-key": RIPPLETIDE_API_KEY, "Content-Type": "application/json"}

# Create agent
agent_response = requests.post(f"{BASE_URL}/agent", headers=headers, json={
    "name": "my-agent", "prompt": "You are a helpful assistant."
})
agent_id = agent_response.json()["id"]

# Add knowledge
requests.post(f"{BASE_URL}/q-and-a", headers=headers, json={
    "question": "What is your purpose?",
    "answer": "I help users with their questions.",
    "agent_id": agent_id
})

# Chat
conversation_id = str(uuid.uuid4())
chat_response = requests.post(f"{BASE_URL}/chat/{agent_id}", headers=headers, json={
    "user_message": "Hello!",
    "conversation_uuid": conversation_id
})
print(chat_response.json()["answer"])
```

### Conversation Manager Class

```python
class AgentChat:
    def __init__(self, agent_id, api_key):
        self.agent_id = agent_id
        self.headers = {"x-api-key": api_key, "Content-Type": "application/json"}
        self.conversation_id = str(uuid.uuid4())

    def send_message(self, message):
        response = requests.post(f"{BASE_URL}/chat/{self.agent_id}",
                               headers=self.headers,
                               json={"user_message": message,
                                     "conversation_uuid": self.conversation_id})
        return response.json()["answer"]

    def new_conversation(self):
        self.conversation_id = str(uuid.uuid4())
```
