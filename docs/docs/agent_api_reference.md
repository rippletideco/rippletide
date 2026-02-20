---
title: SDK API Reference
description: Complete API endpoint reference for the Rippletide SDK
---

## Authentication

All API requests require an API key in the header:

```python
headers = {
    "x-api-key": "your-api-key-here",
    "Content-Type": "application/json"
}
```

## Base URL

```
https://agent.rippletide.com/api/sdk
```

## Agent Management

### Create Agent

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

### Get Agent

GET /agent/{agent_id}

### Update Agent

PUT /agent/{agent_id}

## Knowledge Management

### Create Q&A Pair

POST /q-and-a

**Request Body:**

```json
{
    "question": "User question",
    "answer": "Agent answer",
    "agent_id": "agent-uuid"
}
```

### Get Q&A Pairs

GET /q-and-a

### Update Q&A Pair

PUT /q-and-a/{q_and_a_id}

### Delete Q&A Pair

DELETE /q-and-a/{q_and_a_id}

## Tag Management

### Create Tag

POST /tag

**Request Body:**

```json
{
    "name": "tag-name",
    "description": "Tag description"
}
```

### Link Q&A to Tag

POST /q-and-a-tag

**Request Body:**

```json
{
    "q_and_a_id": "q-and-a-uuid",
    "tag_id": "tag-uuid"
}
```

## Action Management

### Create Action

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

## State Predicate Management

### Set State Predicate

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

## Guardrails Management

### Create Guardrail

POST /guardrail

**Request Body:**

```json
{
    "type": "action",
    "instruction": "Guardrail instruction",
    "agent_id": "agent-uuid"
}
```

## Chat Interface

### Send Message

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

## API Endpoints Summary

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
