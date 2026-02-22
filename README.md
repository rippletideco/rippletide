<img width="2000" height="491" alt="Rippletide CLI" src="./assets/banner.png" />

<p align="center">
  <strong>Rippletide CLI is a powerful evaluation tool for testing and validating AI agent responses with real-time feedback.</strong><br/>
  Interactive terminal interface, template support, and comprehensive testing reports.
</p>



<p align="center">
  <a href="https://eval.rippletide.com">Web Platform</a>
  ·
  <a href="https://github.com/rippletideco/starter">GitHub</a>
  ·
  <a href="https://docs.rippletide.com">Documentation</a>
  ·
  <a href="https://discord.gg/zUPTRH5eFv">Discord</a>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/rippletide"><img src="https://img.shields.io/npm/v/rippletide?style=flat-square&logo=npm" alt="npm" /></a>
  <a href="https://www.npmjs.com/package/rippletide"><img src="https://img.shields.io/npm/dm/rippletide?style=flat-square&logo=npm" alt="Downloads" /></a>
</p>

<p align="center">
  <a href="https://github.com/rippletideco/starter/stargazers"><img src="https://img.shields.io/github/stars/rippletideco/starter?style=flat-square&logo=github" alt="Stars" /></a>
  <a href="https://github.com/rippletideco/starter/issues"><img src="https://img.shields.io/github/issues/rippletideco/starter?style=flat-square&logo=github" alt="Issues" /></a>
  <a href="https://discord.gg/zUPTRH5eFv"><img src="https://img.shields.io/badge/Discord-Join_us-7289DA?style=flat-square&logo=discord" alt="Discord" /></a>
</p>
</p>


## What is Rippletide CLI?

Rippletide CLI is an interactive terminal tool that lets you evaluate AI agent endpoints directly from your command line. Test your agents against predefined questions, validate responses, and get instant feedback on performance.

Instead of manually testing API endpoints or writing custom test scripts, Rippletide CLI provides a beautiful terminal UI with real-time progress tracking, automatic evaluation, and detailed reports.

The result is a streamlined testing workflow that helps you validate AI agent quality before deployment.

<p align="center">
  <a href="https://github.com/rippletideco/starter">
    <picture>
      <img src="https://raw.githubusercontent.com/rippletideco/starter/main/assets/demo.gif" alt="Rippletide Preview" width="800">
    </picture>
  </a>
</p>

---

## Why Rippletide CLI?

Testing AI agents is complex. You need to verify responses, check for hallucinations, and ensure consistent quality. Rippletide CLI simplifies this process:

- **Interactive UI** with beautiful terminal interface built with React and Ink
- **Real-time feedback** showing test progress and results as they happen
- **Template support** for quick testing with pre-configured scenarios
- **Multiple data sources** including local files, Pinecone, and PostgreSQL
- **Custom headers & body** for testing any API endpoint format
- **Automatic evaluation** against expected answers
- **Detailed reports** with pass/fail statistics

This makes it perfect for:

- Testing AI agent endpoints during development
- Validating agent responses before production
- Running regression tests after updates
- Comparing different agent implementations
- Quality assurance workflows

---

## Installation

Install globally via npm:

```bash
npm install -g rippletide
```

Or use directly with npx:

```bash
npx rippletide
```

---

## Quick Start

### Interactive Mode (Default)

Simply run:

```bash
rippletide
```

You'll be prompted for:
1. **Agent endpoint** - Your API URL (e.g., `http://localhost:8000`)
2. **Knowledge source** - Choose between files, Pinecone, or PostgreSQL

The CLI will then:
- Load your test questions
- Send them to your agent
- Show real-time progress
- Display evaluation results

---

## Command Line Options

```bash
rippletide eval [options]
```

### Basic Options

| Option | Description | Example |
|--------|-------------|---------|
| `-t, --template <name>` | Use a pre-configured template | `rippletide eval -t banking_analyst` |
| `-a, --agent <url>` | Agent endpoint URL | `rippletide eval -a localhost:8000` |
| `-k, --knowledge <source>` | Knowledge source: files, pinecone, or postgresql | `rippletide eval -k pinecone` |
| `--debug` | Show detailed error information | `rippletide eval --debug` |
| `-h, --help` | Show help message | `rippletide --help` |

### Data Source Options

**Local Files (default):**
```bash
rippletide eval -a localhost:8000
```
Reads Q&A pairs from `qanda.json` in current directory.

**Pinecone:**
```bash
rippletide eval -a localhost:8000 -k pinecone \
  -pu https://db.pinecone.io \
  -pk pcsk_xxxxx
```

**PostgreSQL:**
```bash
rippletide eval -a localhost:8000 -k postgresql \
  -pg "postgresql://user:pass@localhost:5432/db"
```

### Custom Endpoint Options

For non-standard APIs:

```bash
rippletide eval -a localhost:8000 \
  -H "Authorization: Bearer token, X-API-Key: key" \
  -B '{"prompt": "{question}"}' \
  -rf "data.response"
```

- `-H, --headers` - Custom headers (comma-separated)
- `-B, --body` - Request body template (use `{question}` placeholder)
- `-rf, --response-field` - Path to response in JSON (dot notation)

---

## Development

### Build from Source

```bash
git clone https://github.com/rippletideco/starter.git
cd starter/cli
npm install
npm run build
```

### Run Development Version

```bash
npm run eval
```

### Project Structure

```
cli/
├── bin/
│   └── rippletide          # Entry point
├── src/
│   ├── api/                # API client
│   ├── components/         # UI components
│   │   ├── Header.tsx
│   │   ├── ProgressBar.tsx
│   │   ├── SelectMenu.tsx
│   │   ├── Spinner.tsx
│   │   ├── Summary.tsx
│   │   └── TextInput.tsx
│   ├── errors/            # Error handling
│   ├── utils/             # Utilities
│   └── App.tsx            # Main app
├── templates/             # Pre-built configs
├── dist/                  # Compiled output
└── package.json
```

---

## Support

Need help or found a bug?

- **Discord**: [Join our community](https://discord.gg/zUPTRH5eFv)
- **GitHub Issues**: [Report bugs](https://github.com/rippletideco/starter/issues)

---
Built with ❤️ by the [Rippletide](https://rippletide.com) team

