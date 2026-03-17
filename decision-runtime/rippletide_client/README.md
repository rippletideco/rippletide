# Rippletide Client

Python Client for interacting with the Rippletide evaluation API.

## Installation

```bash
pip install -r requirements.txt
```

## Usage

### Basic Setup

```python
from rippletide_sdk import RippletideClient

# Initialize client with API key for authenticated requests
client = RippletideClient(api_key="your-api-key")
```

### 1. Create an Agent for Evaluation

```python
agent = client.create_agent(name="My Eval Agent")

agent_id = agent['id']
print(f"Created agent: {agent_id}")
```

### 2. Extract questions from your knowledge

```python
# Upload PDF and extract questions/expected answers
result = client.extract_questions_from_pdf(
    agent_id=agent_id,
    pdf_path="path/to/document.pdf"
)

print(f"Extracted {len(result.get('qaPairs', []))} Q&A pairs")

# Get all test prompts (questions and expected answers)
test_prompts = client.get_test_prompts(agent_id)
for prompt in test_prompts:
    print(f"Question: {prompt['prompt']}")
    print(f"Expected Answer: {prompt.get('expectedAnswer', 'N/A')}")
```

### 3. Evaluate Agent Response

```python
# Simple evaluation - just provide question and optional expected answer
report = client.evaluate(
    agent_id=agent_id,
    question="What is this document about?",
    expected_answer="Optional expected answer"
)

print(f"Label: {report['label']}")
print(f"Justification: {report['justification']}")
print(f"Facts evaluated: {len(report['facts'])}")
for fact in report['facts']:
    print(f"  - {fact['fact']}: {fact['label']}")
```

### Complete Example

```python
from rippletide_sdk import RippletideClient

# Initialize client (session ID auto-generated)
client = RippletideClient(api_key="your-api-key")

# 1. Create agent
agent = client.create_agent(name="Evaluation Agent")
agent_id = agent['id']

# 2. Extract questions from PDF
pdf_result = client.extract_questions_from_pdf(
    agent_id=agent_id,
    pdf_path="knowledge.pdf"
)

# 3. Get test prompts
test_prompts = client.get_test_prompts(agent_id)

# 4. Evaluate each prompt
for prompt in test_prompts:
    report = client.evaluate(
        agent_id=agent_id,
        question=prompt['prompt'],
        expected_answer=prompt.get('expectedAnswer')
    )
    print(f"Question: {prompt['prompt'][:50]}...")
    print(f"Label: {report['label']}")
```
```
