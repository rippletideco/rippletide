# Rippletide Evaluation CLI

CLI tool to quickly run an evaluation of your Rippletide agent from the terminal.

## Installation

```bash
npm install -g rippletide
```

## Usage

Run the evaluation UI:

```bash
rippletide
# or
rippletide eval
```

You will be asked for:

- **Agent endpoint** (e.g. `http://localhost:8000`)
- **Knowledge source** (currently `qanda.json` in the current directory)

The CLI will:

1. Generate an API key for the evaluation
2. Create a temporary agent
3. Run a small set of test prompts against your endpoint
4. Show progress and a summary of passed/failed tests

For more details and source code, see the main repository:

- GitHub: https://github.com/rippletideco/starter
