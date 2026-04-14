# GraphRAG-MCP 🕸️🧠

<p align="center">
  <img src="https://img.shields.io/badge/Rust-1.84+-00ADD8?style=flat&logo=rust&logoColor=white" alt="Rust Version" />
  <img src="https://img.shields.io/badge/MCP-Ready-blue.svg?style=flat&logo=abstract" alt="MCP Ready" />
  <img src="https://img.shields.io/badge/Zero_LLM_Cost-100%25-brightgreen.svg?style=flat" alt="Zero LLM Cost" />
  <img src="https://img.shields.io/badge/Storage-SQLite5(FTS%20%7C%20Vec)-003B57?style=flat&logo=sqlite&logoColor=white" alt="SQLite Config" />
  <img src="https://img.shields.io/badge/Test_Coverage-100%25-success.svg?style=flat" alt="Test Coverage" />
</p>

> A blazing-fast, standalone **Graph Retrieval-Augmented Generation (GraphRAG)** implementation exposed as a **Model Context Protocol (MCP)** server. Provide your AI agents with complete codebase topology, structural contexts, and semantic graphs—fully locally, with ZERO LLM API overhead.

---

## 🚀 Why GraphRAG-MCP?

Modern AI coding assistants struggle with massive codebases. They lose context, hallucinate architectural decisions, and struggle to trace deeply nested relationships. Passing thousands of code files to Large Language Models (LLMs) to rebuild relationships is both slow and incredibly expensive.

**GraphRAG-MCP solves this.** It parses your codebase locally using deterministic AST analysis, maps the relationships into a graph database, calculates community topologies via the Leiden algorithm, and generates local semantic vectors mathematically. It then serves these highly contextual structural clusters directly to your AI agents via standard MCP queries.

## ✨ Key Features

- **Zero-LLM Cost Ingestion**: We abandoned expensive LLM-based entity extraction. Instead, we use `tree-sitter` AST multi-language parsing (Rust, Go, Python, JS/TS, Java, C/C++, C#, Ruby) to automatically deduce precise chunks, classes, methods, and relation structures.
- **Native Leiden Community Detection**: Out-of-the-box native graph clustering! We use a robust Rust implementation of the Leiden algorithm to assemble semantic communities without heavy external dependencies.
- **Hybrid Search Engine**:
  - **Local/Semantic**: Runs text-embedding models entirely locally via native `ort` (ONNX runtime) integrations. Embeddings are stored and searched using extreme-performance `sqlite-vec`.
  - **Global/Structural**: Utilizes optimized SQLite `FTS5` virtual mapping for high-speed textual queries and exact structural pinpointing.
- **Standardized MCP Interface**: Integrates flawlessly into any agentic IDE environment (e.g., Cline, Claude Desktop) seamlessly exposing internal functions like `local_search`, `global_search`, and `get_graph_topology`.

## 🏗️ Architecture Stack

1. **Indexer / Extractor Engine**: `tree-sitter` parsers recursively map AST syntax trees to an entity/relationship model.
2. **Local AI Engine**: Extracts embeddings exclusively in local memory using `ort` ONNX models (CPU/GPU acceleration available).
3. **Storage Engine**: Consolidated everything strictly under SQLite. By exploiting `sqlite-vec` + `fts5`, we achieve high concurrency throughput without requiring complex external Vector databases.
4. **Agent Server**: Handles STDIO MCP interactions conforming to specification limits seamlessly.

## 📦 Installation & Setup

### Requirements
- **Rust**: `1.84+` installed via `rustup`.
- **Cargo**: Rust's package manager.

1. **Clone the repository:**
   ```bash
   git clone https://github.com/kim-nam-jung/GraphRAG-mcp.git
   cd GraphRAG-mcp
   ```

2. **Build the Server:**
   ```bash
   cargo build --release
   ```

---

## 💻 Usage & MCP Integration

To add this server to your agent's MCP configurations (e.g., for Cline or Claude Desktop), append the configuration JSON:

```json
{
  "mcpServers": {
    "graphrag-mcp": {
      "command": "/path/to/GraphRAG-mcp/target/release/graphrag_mcp",
      "args": ["-c", "/path/to/GraphRAG-mcp/configs/default.yaml"]
    }
  }
}
```

### Available AI Agent Tools
Once connected, the AI agent gains access to the following dynamic protocols:

* `search_local_neighborhood`: Given a keyword/node entity, maps structural graph edges, vectors, and source code logic in a bounded radius.
* `search_global_topology`: Executes sweeping full-text queries merged with FTS5 triggers to fetch clusters and community reports across unrelated code layers.

## 🧪 Running Tests
The framework is aggressively tested. Run the integration suite:

```bash
cargo test
```

## 📜 License
This project is open-source under the **MIT License**.

---
*Built with ❤️ for true localized Agentic autonomy.*
