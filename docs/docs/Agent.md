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

## Table of Contents

1. [Quick Start](#quick-start)
2. [Core Concepts](#core-concepts)
3. [SDK API Reference](#sdk-api-reference)
4. [Agent Configuration](#agent-configuration)
5. [Knowledge Management](#knowledge-management)
6. [State Management](#state-management)
7. [Chat Integration](#chat-integration)
8. [LangChain Integration](#langchain-integration)
9. [Best Practices](#best-practices)
10. [Complete Examples](#complete-examples)
11. [Quick Reference](#quick-reference)

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

## SDK API Reference

### Authentication

All API requests require an API key in the header:

```python
headers = {
    "x-api-key": "your-api-key-here",
    "Content-Type": "application/json"
}
```

### Base URL

```
https://agent.rippletide.com/api/sdk
```

### Agent Management

#### Create Agent

POST /agent

**Request Body:**

```json
{
    "name": "agent-name",
    "prompt": "Agent system prompt"
}
```

**Response:**

```json
{
    "id": "agent-uuid",
    "name": "agent-name",
    "prompt": "Agent system prompt"
}
```

#### Get Agent

GET /agent/{agent_id}

#### Update Agent

PUT /agent/{agent_id}

### Knowledge Management

#### Create Q&A Pair

POST /q-and-a

**Request Body:**

```json
{
    "question": "User question",
    "answer": "Agent answer",
    "agent_id": "agent-uuid"
}
```

#### Get Q&A Pairs

GET /q-and-a

#### Update Q&A Pair

PUT /q-and-a/{q_and_a_id}

#### Delete Q&A Pair

DELETE /q-and-a/{q_and_a_id}

### Tag Management

#### Create Tag

POST /tag

**Request Body:**

```json
{
    "name": "tag-name",
    "description": "Tag description"
}
```

#### Link Q&A to Tag

POST /q-and-a-tag

**Request Body:**

```json
{
    "q_and_a_id": "q-and-a-uuid",
    "tag_id": "tag-uuid"
}
```

### Action Management

#### Create Action

POST /action

**Request Body:**

```json
{
    "name": "action-name",
    "description": "Action description",
    "what_to_do": "Detailed action instructions",
    "agent_id": "agent-uuid"
}
```

### State Predicate Management

#### Set State Predicate

PUT /state-predicate/{agent_id}

**Request Body:**

```json
{
    "state_predicate": {
        "transition_kind": "branch",
        "question_to_evaluate": "Current state question",
        "possible_values": ["option1", "option2"],
        "value_to_node": {
            "option1": {
                "transition_kind": "end",
                "question_to_evaluate": "End state message"
            }
        }
    }
}
```

### Guardrails Management

#### Create Guardrail

POST /guardrail

**Request Body:**

```json
{
    "type": "action",
    "instruction": "Guardrail instruction",
    "agent_id": "agent-uuid"
}
```

### Chat Interface

#### Send Message

POST /chat/{agent_id}

**Request Body:**

```json
{
    "user_message": "User message",
    "conversation_uuid": "conversation-uuid"
}
```

**Response:**

```json
{
    "answer": "Agent response",
    "conversation_uuid": "conversation-uuid"
}
```

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
            "answer": "We are open Monday through Friday from 9 AM to 6 PM EST, and Saturday from 10 AM to 4 PM EST. We are closed on Sundays.",
            "tags": ["business_hours", "contact_info"]
        },
        {
            "question": "How can I track my order?",
            "answer": "You can track your order by logging into your account and going to the 'My Orders' section, or by using the tracking number sent to your email.",
            "tags": ["orders", "tracking", "account"]
        },
        {
            "question": "What is your return policy?",
            "answer": "We offer a 30-day return policy for most items. Items must be in original condition with tags attached. Electronics have a 14-day return window.",
            "tags": ["returns", "policy", "electronics"]
        }
    ]

    for qa in q_and_a_pairs:
        # Create Q&A
        qa_response = requests.post(
            f"{BASE_URL}/q-and-a",
            headers=headers,
            json={
                "question": qa["question"],
                "answer": qa["answer"],
                "agent_id": agent_id
            }
        )
        qa_id = qa_response.json()["id"]

        # Create and link tags
        for tag_name in qa["tags"]:
            # Create tag if it doesn't exist
            tag_response = requests.post(
                f"{BASE_URL}/tag",
                headers=headers,
                json={
                    "name": tag_name,
                    "description": f"Tag for {tag_name} related content"
                }
            )
            tag_id = tag_response.json()["id"]

            # Link Q&A to tag
            requests.post(
                f"{BASE_URL}/q-and-a-tag",
                headers=headers,
                json={
                    "q_and_a_id": qa_id,
                    "tag_id": tag_id
                }
            )
```

### Tag Organization

Organize your knowledge with a clear tagging strategy:

```python
# Define your tag hierarchy
TAG_CATEGORIES = {
    "product_info": ["pricing", "specifications", "availability"],
    "customer_service": ["returns", "shipping", "support"],
    "account_management": ["login", "profile", "orders"],
    "technical": ["troubleshooting", "installation", "compatibility"]
}
```

## State Management

### State Predicates

State predicates define how your agent should behave based on the current conversation state:

```
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
                        "transition_kind": "branch",
                        "question_to_evaluate": "Ready to checkout?",
                        "possible_values": ["yes", "no"],
                        "value_to_node": {
                            "yes": {
                                "transition_kind": "end",
                                "question_to_evaluate": "Proceeding to checkout..."
                            },
                            "no": {
                                "transition_kind": "end",
                                "question_to_evaluate": "What else can I help you with?"
                            }
                        }
                    },
                    "need_recommendation": {
                        "transition_kind": "end",
                        "question_to_evaluate": "Let me recommend some products based on your needs."
                    }
                }
            },
            "track_order": {
                "transition_kind": "end",
                "question_to_evaluate": "Please provide your order number or email address."
            },
            "return_item": {
                "transition_kind": "end",
                "question_to_evaluate": "I'll help you with the return process. What's your order number?"
            },
            "get_support": {
                "transition_kind": "end",
                "question_to_evaluate": "I'm here to help. What issue are you experiencing?"
            }
        }
    }
```

### Setting State Predicates

```
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

```
class RippletideChat:
    def __init__(self, agent_id, api_key):
        self.agent_id = agent_id
        self.api_key = api_key
        self.conversation_id = str(uuid.uuid4())
        self.headers = {
            "x-api-key": api_key,
            "Content-Type": "application/json"
        }

    def send_message(self, message):
        """Send a message to the agent and get a response."""
        url = f"{BASE_URL}/chat/{self.agent_id}"
        data = {
            "user_message": message,
            "conversation_uuid": self.conversation_id
        }

        try:
            response = requests.post(url, headers=self.headers, json=data)
            response.raise_for_status()
            return response.json()["answer"]
        except requests.exceptions.RequestException as e:
            return f"Error: {e}"

    def start_new_conversation(self):
        """Start a new conversation session."""
        self.conversation_id = str(uuid.uuid4())
```

# Usage
```
chat = RippletideChat(agent_id, RIPPLETIDE_API_KEY)
response = chat.send_message("Hello, I need help with my order")
print(response)
```

### Advanced Chat with Context

```
class AdvancedRippletideChat:
    def __init__(self, agent_id, api_key):
        self.agent_id = agent_id
        self.api_key = api_key
        self.conversation_id = str(uuid.uuid4())
        self.conversation_history = []
        self.headers = {
            "x-api-key": api_key,
            "Content-Type": "application/json"
        }

    def send_message(self, message, include_context=True):
        """Send a message with optional conversation context."""
        if include_context and self.conversation_history:
            # Add context to the message
            context = "\n".join([f"Previous: {msg}" for msg in self.conversation_history[-3:]])
            full_message = f"Context: {context}\n\nCurrent message: {message}"
        else:
            full_message = message

        url = f"{BASE_URL}/chat/{self.agent_id}"
        data = {
            "user_message": full_message,
            "conversation_uuid": self.conversation_id
        }

        try:
            response = requests.post(url, headers=self.headers, json=data)
            response.raise_for_status()
            answer = response.json()["answer"]

            # Store in conversation history
            self.conversation_history.append(f"User: {message}")
            self.conversation_history.append(f"Agent: {answer}")

            return answer
        except requests.exceptions.RequestException as e:
            return f"Error: {e}"

    def get_conversation_summary(self):
        """Get a summary of the current conversation."""
        return "\n".join(self.conversation_history)
```

## LangChain Integration

### Azure OpenAI Integration

Rippletide provides a LangChain-compatible Azure OpenAI endpoint:

```python
from langchain_core.messages import SystemMessage, HumanMessage
from langchain_openai import AzureChatOpenAI

# Initialize the Rippletide LLM
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

# Use with LangChain
messages = [
    SystemMessage(content="You are a helpful assistant."),
    HumanMessage(content="Hello, how can you help me?")
]

response = rippletide_llm.invoke(messages)
print(response.content)
```

### Custom LangChain Chain

```python
from langchain.chains import LLMChain
from langchain.prompts import PromptTemplate

# Create a custom prompt template
prompt = PromptTemplate(
    input_variables=["user_input", "context"],
    template="""Context: {context}

User: {user_input}

Assistant:"""
)

# Create a chain with Rippletide LLM
chain = LLMChain(llm=rippletide_llm, prompt=prompt)

# Use the chain
result = chain.run(
    user_input="What are your business hours?",
    context="Customer is asking about store hours"
)
print(result)
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
- **Conversation Management**: Maintain conversation context appropriately
- **Rate Limiting**: Implement proper rate limiting for API calls
- **Caching**: Cache frequently accessed knowledge when appropriate

### 4. Security Considerations

- **API Key Management**: Store API keys securely
- **Input Validation**: Validate all user inputs
- **Content Filtering**: Implement content filtering for sensitive topics
- **Access Control**: Implement proper access controls for different user types

## Complete Examples

### E-commerce Customer Support Agent

```python
import os
import uuid
import requests
from typing import List, Dict

class EcommerceSupportAgent:
    def __init__(self, api_key: str):
        self.api_key = api_key
        self.base_url = "https://agent.rippletide.com/api/sdk"
        self.headers = {
            "x-api-key": api_key,
            "Content-Type": "application/json"
        }
        self.agent_id = None
        self.conversation_id = str(uuid.uuid4())

    def create_agent(self) -> str:
        """Create the customer support agent."""
        url = f"{self.base_url}/agent"
        data = {
            "name": "ecommerce-support",
            "prompt": """You are a professional customer support agent for an e-commerce platform.
            Your role is to help customers with orders, product questions, returns, and general inquiries.
            Always be polite, helpful, and accurate. Use only information from your knowledge base.
            If you cannot answer a question, politely direct the customer to human support."""
        }

        response = requests.post(url, headers=self.headers, json=data)
        response.raise_for_status()
        self.agent_id = response.json()["id"]
        return self.agent_id

    def setup_knowledge_base(self):
        """Set up the complete knowledge base."""
        knowledge_items = [
            {
                "question": "What are your business hours?",
                "answer": "We are open Monday through Friday from 9 AM to 6 PM EST, and Saturday from 10 AM to 4 PM EST. We are closed on Sundays and major holidays.",
                "tags": ["business_hours", "contact_info"]
            },
            {
                "question": "How can I track my order?",
                "answer": "You can track your order in three ways: 1) Log into your account and go to 'My Orders', 2) Use the tracking number sent to your email, or 3) Contact us with your order number.",
                "tags": ["orders", "tracking", "account"]
            },
            {
                "question": "What is your return policy?",
                "answer": "We offer a 30-day return policy for most items. Items must be in original condition with tags attached. Electronics have a 14-day return window. Returns are free for defective items.",
                "tags": ["returns", "policy", "electronics"]
            },
            {
                "question": "How long does shipping take?",
                "answer": "Standard shipping takes 3-5 business days. Express shipping takes 1-2 business days. International shipping takes 7-14 business days depending on the destination.",
                "tags": ["shipping", "delivery", "international"]
            },
            {
                "question": "Do you offer international shipping?",
                "answer": "Yes, we ship to most countries worldwide. International shipping takes 7-14 business days and additional customs fees may apply. Check our shipping page for specific country restrictions.",
                "tags": ["shipping", "international", "customs"]
            }
        ]

        # Create tags first
        tag_ids = {}
        for item in knowledge_items:
            for tag_name in item["tags"]:
                if tag_name not in tag_ids:
                    tag_response = requests.post(
                        f"{self.base_url}/tag",
                        headers=self.headers,
                        json={
                            "name": tag_name,
                            "description": f"Tag for {tag_name.replace('_', ' ')} related content"
                        }
                    )
                    tag_ids[tag_name] = tag_response.json()["id"]

        # Create Q&A pairs and link to tags
        for item in knowledge_items:
            qa_response = requests.post(
                f"{self.base_url}/q-and-a",
                headers=self.headers,
                json={
                    "question": item["question"],
                    "answer": item["answer"],
                    "agent_id": self.agent_id
                }
            )
            qa_id = qa_response.json()["id"]

            # Link to tags
            for tag_name in item["tags"]:
                requests.post(
                    f"{self.base_url}/q-and-a-tag",
                    headers=self.headers,
                    json={
                        "q_and_a_id": qa_id,
                        "tag_id": tag_ids[tag_name]
                    }
                )

    def setup_actions(self):
        """Set up available actions for the agent."""
        actions = [
            {
                "name": "create_support_ticket",
                "description": "Create a support ticket for complex issues",
                "what_to_do": "Collect user details and create a support ticket for human review"
            },
            {
                "name": "process_return",
                "description": "Process a return request",
                "what_to_do": "Guide user through return process and generate return label"
            },
            {
                "name": "escalate_to_manager",
                "description": "Escalate complex issues to management",
                "what_to_do": "Transfer conversation to a manager for resolution"
            }
        ]

        for action in actions:
            requests.post(
                f"{self.base_url}/action",
                headers=self.headers,
                json={
                    "agent_id": self.agent_id,
                    **action
                }
            )

    def setup_guardrails(self):
        """Set up safety guardrails."""
        guardrails = [
            {
                "type": "action",
                "instruction": "Always maintain a professional and helpful tone"
            },
            {
                "type": "action",
                "instruction": "Never provide personal opinions or make promises about delivery times"
            },
            {
                "type": "action",
                "instruction": "Always direct customers to official policies for legal matters"
            }
        ]

        for guardrail in guardrails:
            requests.post(
                f"{self.base_url}/guardrail",
                headers=self.headers,
                json={
                    "agent_id": self.agent_id,
                    **guardrail
                }
            )

    def setup_state_predicate(self):
        """Set up conversation flow states."""
        state_predicate = {
            "transition_kind": "branch",
            "question_to_evaluate": "What type of help does the customer need?",
            "possible_values": ["order_help", "product_question", "return_request", "general_inquiry"],
            "re_evaluate": True,
            "value_to_node": {
                "order_help": {
                    "transition_kind": "branch",
                    "question_to_evaluate": "What specific order issue are they having?",
                    "possible_values": ["tracking", "modification", "cancellation", "billing"],
                    "value_to_node": {
                        "tracking": {
                            "transition_kind": "end",
                            "question_to_evaluate": "I'll help you track your order. What's your order number?"
                        },
                        "modification": {
                            "transition_kind": "end",
                            "question_to_evaluate": "I'll help you modify your order. What changes do you need?"
                        },
                        "cancellation": {
                            "transition_kind": "end",
                            "question_to_evaluate": "I'll help you cancel your order. What's your order number?"
                        },
                        "billing": {
                            "transition_kind": "end",
                            "question_to_evaluate": "I'll help you with billing questions. What's your order number?"
                        }
                    }
                },
                "product_question": {
                    "transition_kind": "end",
                    "question_to_evaluate": "I'll help you with product information. What would you like to know?"
                },
                "return_request": {
                    "transition_kind": "end",
                    "question_to_evaluate": "I'll help you with your return. What's your order number and reason for return?"
                },
                "general_inquiry": {
                    "transition_kind": "end",
                    "question_to_evaluate": "I'm here to help with any questions you have. What can I assist you with?"
                }
            }
        }

        requests.put(
            f"{self.base_url}/state-predicate/{self.agent_id}",
            headers=self.headers,
            json={"state_predicate": state_predicate}
        )

    def chat(self, message: str) -> str:
        """Send a message to the agent."""
        url = f"{self.base_url}/chat/{self.agent_id}"
        data = {
            "user_message": message,
            "conversation_uuid": self.conversation_id
        }

        try:
            response = requests.post(url, headers=self.headers, json=data)
            response.raise_for_status()
            return response.json()["answer"]
        except requests.exceptions.RequestException as e:
            return f"I'm sorry, I'm experiencing technical difficulties. Please try again later. Error: {e}"

    def initialize(self):
        """Initialize the complete agent setup."""
        print("Creating agent...")
        self.create_agent()

        print("Setting up knowledge base...")
        self.setup_knowledge_base()

        print("Setting up actions...")
        self.setup_actions()

        print("Setting up guardrails...")
        self.setup_guardrails()

        print("Setting up state predicate...")
        self.setup_state_predicate()

        print(f"Agent ready! ID: {self.agent_id}")

# Usage example
def main():
    api_key = os.environ["RIPPLETIDE_API_KEY"]
    agent = EcommerceSupportAgent(api_key)
    agent.initialize()

    # Interactive chat
    print("\nAgent is ready! Type 'quit' to exit.")
    while True:
        user_input = input("\nCustomer: ")
        if user_input.lower() in ['quit', 'exit', 'bye']:
            break

        response = agent.chat(user_input)
        print(f"Agent: {response}")

if __name__ == "__main__":
    main()
```

### Technical Support Agent

```python
class TechnicalSupportAgent:
    def __init__(self, api_key: str):
        self.api_key = api_key
        self.base_url = "https://agent.rippletide.com/api/sdk"
        self.headers = {
            "x-api-key": api_key,
            "Content-Type": "application/json"
        }
        self.agent_id = None
        self.conversation_id = str(uuid.uuid4())

    def create_agent(self) -> str:
        """Create the technical support agent."""
        url = f"{self.base_url}/agent"
        data = {
            "name": "technical-support",
            "prompt": """You are a technical support specialist for a software company.
            Your role is to help users troubleshoot technical issues, provide installation guidance,
            and resolve software problems. Always ask for specific error messages and system details.
            Provide step-by-step solutions and escalate complex issues when necessary."""
        }

        response = requests.post(url, headers=self.headers, json=data)
        response.raise_for_status()
        self.agent_id = response.json()["id"]
        return self.agent_id

    def setup_technical_knowledge(self):
        """Set up technical knowledge base."""
        technical_qa = [
            {
                "question": "How do I install the software?",
                "answer": "To install our software: 1) Download the installer from our website, 2) Run the installer as administrator, 3) Follow the setup wizard, 4) Restart your computer if prompted. Minimum system requirements: Windows 10/11, 4GB RAM, 2GB free disk space.",
                "tags": ["installation", "system_requirements", "setup"]
            },
            {
                "question": "The software won't start, what should I do?",
                "answer": "If the software won't start: 1) Check if your antivirus is blocking it, 2) Run as administrator, 3) Check Windows Event Viewer for error messages, 4) Try reinstalling the software, 5) Contact support with the specific error message.",
                "tags": ["troubleshooting", "startup", "errors"]
            },
            {
                "question": "How do I update the software?",
                "answer": "To update: 1) Open the software, 2) Go to Help > Check for Updates, 3) Download and install the update, 4) Restart the software. Automatic updates can be enabled in Settings > Preferences > Updates.",
                "tags": ["updates", "maintenance", "settings"]
            },
            {
                "question": "I'm getting a license error, what does this mean?",
                "answer": "License errors usually mean: 1) Your license has expired, 2) You've exceeded the maximum number of installations, 3) There's a network connectivity issue. Check your license status in Help > About, and contact support if the issue persists.",
                "tags": ["licensing", "errors", "authentication"]
            }
        ]

        # Implementation similar to ecommerce example...
        # (Create tags, Q&A pairs, and link them)

    def setup_technical_actions(self):
        """Set up technical support actions."""
        actions = [
            {
                "name": "collect_system_info",
                "description": "Collect system information for troubleshooting",
                "what_to_do": "Ask user to provide system specs, OS version, and error messages"
            },
            {
                "name": "schedule_remote_session",
                "description": "Schedule a remote support session",
                "what_to_do": "Collect user availability and schedule a remote support session"
            },
            {
                "name": "escalate_to_engineering",
                "description": "Escalate complex technical issues to engineering team",
                "what_to_do": "Document the issue and escalate to the engineering team for investigation"
            }
        ]

        for action in actions:
            requests.post(
                f"{self.base_url}/action",
                headers=self.headers,
                json={
                    "agent_id": self.agent_id,
                    **action
                }
            )

    def chat(self, message: str) -> str:
        """Send a message to the technical support agent."""
        url = f"{self.base_url}/chat/{self.agent_id}"
        data = {
            "user_message": message,
            "conversation_uuid": self.conversation_id
        }

        try:
            response = requests.post(url, headers=self.headers, json=data)
            response.raise_for_status()
            return response.json()["answer"]
        except requests.exceptions.RequestException as e:
            return f"I'm experiencing technical difficulties. Please try again later. Error: {e}"
```

## Error Handling and Troubleshooting

### Common Issues

1. **Authentication Errors**

```python
# Check API key
if not os.environ.get("RIPPLETIDE_API_KEY"):
    raise ValueError("RIPPLETIDE_API_KEY environment variable not set")
```

2. **Rate Limiting**

```python
import time

def make_request_with_retry(url, headers, json_data, max_retries=3):
    for attempt in range(max_retries):
        try:
            response = requests.post(url, headers=headers, json=json_data)
            if response.status_code == 429:  # Rate limited
                time.sleep(2 ** attempt)  # Exponential backoff
                continue
            response.raise_for_status()
            return response
        except requests.exceptions.RequestException as e:
            if attempt == max_retries - 1:
                raise e
            time.sleep(1)
```

3. **Invalid Agent ID**

```python
def validate_agent_id(agent_id):
    try:
        uuid.UUID(agent_id)
        return True
    except ValueError:
        return False
```

### Debugging Tips

1. **Enable Request Logging**

```python
import logging

logging.basicConfig(level=logging.DEBUG)
requests_log = logging.getLogger("requests.packages.urllib3")
requests_log.setLevel(logging.DEBUG)
requests_log.propagate = True
```

2. **Test API Connectivity**

```python
def test_api_connection():
    try:
        response = requests.get(f"{BASE_URL}/health", headers=headers)
        return response.status_code == 200
    except:
        return False
```

## Quick Reference

### Essential Code Templates

#### 1. Minimal Agent Setup

```python
import os, uuid, requests

# Setup
RIPPLETIDE_API_KEY = os.environ["RIPPLETIDE_API_KEY"]
BASE_URL = "https://agent.rippletide.com/api/sdk"
headers = {"x-api-key": RIPPLETIDE_API_KEY, "Content-Type": "application/json"}

# Create agent
agent_response = requests.post(f"{BASE_URL}/agent", headers=headers, json={
    "name": "my-agent",
    "prompt": "You are a helpful assistant."
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

#### 2. Agent with Actions

```python
# Add action to agent
requests.post(f"{BASE_URL}/action", headers=headers, json={
    "name": "get_weather",
    "description": "Get current weather",
    "what_to_do": "Call weather API and return current conditions",
    "agent_id": agent_id
})
```

#### 3. Agent with State Management

```python
# Set conversation flow
state_predicate = {
    "transition_kind": "branch",
    "question_to_evaluate": "What does the user need?",
    "possible_values": ["help", "info", "support"],
    "value_to_node": {
        "help": {"transition_kind": "end", "question_to_evaluate": "How can I help?"},
        "info": {"transition_kind": "end", "question_to_evaluate": "What information do you need?"},
        "support": {"transition_kind": "end", "question_to_evaluate": "I'll connect you with support."}
    }
}

requests.put(f"{BASE_URL}/state-predicate/{agent_id}", headers=headers, json={
    "state_predicate": state_predicate
})
```

#### 4. LangChain Integration

```python
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

### Common Patterns

#### Error Handling

```python
def safe_api_call(url, headers, json_data):
    try:
        response = requests.post(url, headers=headers, json=json_data)
        response.raise_for_status()
        return response.json()
    except requests.exceptions.RequestException as e:
        print(f"API Error: {e}")
        return None
```

#### Batch Knowledge Addition

```python
def add_knowledge_batch(agent_id, qa_pairs):
    for question, answer in qa_pairs:
        requests.post(f"{BASE_URL}/q-and-a", headers=headers, json={
            "question": question,
            "answer": answer,
            "agent_id": agent_id
        })
```

#### Conversation Management

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

### API Endpoints Summary

| Method | Endpoint | Purpose |
|--------|----------|---------|
| POST | `/agent` | Create agent |
| GET | `/agent/{id}` | Get agent details |
| PUT | `/agent/{id}` | Update agent |
| POST | `/q-and-a` | Add knowledge |
| GET | `/q-and-a` | Get knowledge |
| PUT | `/q-and-a/{id}` | Update knowledge |
| DELETE | `/q-and-a/{id}` | Delete knowledge |
| POST | `/tag` | Create tag |
| POST | `/q-and-a-tag` | Link knowledge to tag |
| POST | `/action` | Create agent action |
| PUT | `/state-predicate/{id}` | Set conversation flow |
| POST | `/guardrail` | Add safety guardrail |
| POST | `/chat/{id}` | Send message to agent |

## Conclusion

The Rippletide SDK provides a powerful platform for building reliable, hallucination-free conversational AI agents. By following this guide and implementing the best practices outlined, you can create sophisticated agents that provide accurate, helpful responses while maintaining conversation context and following predefined business logic.

For additional support and updates, visit the [Rippletide documentation](https://docs.rippletide.com) or contact our support team.

---

*This guide is designed to be comprehensive and self-contained. Save this file and use it as a reference when building your Rippletide agents. All code examples are ready to copy-paste and execute.*
