# MCP Memory

MCP Memory is a lightweight Model Context Protocol (MCP) server that stores a knowledge graph on disk and provides simple text search capabilities.

## Purpose

This server stores entities, observations, and relations on disk, exposing tools for manipulating the knowledge graph via MCP. It supports text search functionality, enabling LLMs to recall relevant context across sessions.

## Background

This project is based on the original [MCP Memory Server](https://github.com/modelcontextprotocol/servers/tree/main/src/memory) from the Model Context Protocol servers repository.

### Why This Fork Exists

- **Dependency-free distribution**: The original TypeScript implementation requires Node.js and npm dependencies to be installed. This Rust implementation provides a single, self-contained binary with no external dependencies.
- **Environment variable handling**: Addresses [the issue](https://github.com/modelcontextprotocol/servers/issues/1018) in the published npm package where the `MEMORY_FILE_PATH` environment variable was not respected, causing the server to use a hardcoded path instead of the user-specified configuration.
- **Performance**: Rust implementation with async Tokio runtime offers better performance and lower resource usage.

### Future Development

The following advanced search features are planned, and some initial work has already been done:
- **Embedding support**: Integration with Ollama and OpenAI for vector embeddings
- **Vector search**: Semantic search using vector similarity
- **Hybrid search**: Combining traditional text search with vector-based semantic search

These updates will become available once they have been thoroughly tested.

## Building

This project requires **Rust 1.86** or newer.

```bash
cargo build --release
```

## Development

For development, you can use the following commands:

```bash
# Run tests
cargo test

# Run clippy for linting
cargo clippy -- -D warnings

# Format code
cargo fmt

# Run all checks (useful before committing)
cargo test && cargo clippy -- -D warnings && cargo fmt --check
```

## Testing

Run the unit tests with:

```bash
cargo test
```

## Continuous Integration

This project uses GitHub Actions for automated testing and quality checks:

- **Tests**: Automated test execution on multiple Rust versions
- **Clippy**: Rust linting for code quality and best practices
- **Formatting**: Code formatting verification with `rustfmt`
- **Cross-platform builds**: Testing on Linux, macOS, and Windows

All pull requests are automatically validated through these CI pipelines.

## Installation

```bash
brew install aliev/tap/mcp_memory
```

Prebuilt binaries for all supported platforms are available on the [releases](https://github.com/aliev/mcp_memory/releases) page.

## Environment Variables

- `MEMORY_FILE_PATH` – Path to the JSONL file containing the knowledge graph (defaults to `memory.jsonl` in the same directory as the executable)

## Configuration Example for Claude Desktop

1. Open Claude Desktop → Settings → Developer → Edit Config
2. Add the following configuration to `claude_desktop_config.json` and restart the application:

```json
{
  "mcpServers": {
    "memory": {
      "command": "/path/to/your/mcp_memory",
      "env": {
        "MEMORY_FILE_PATH": "/Users/[your-username]/memory.jsonl"
      }
    }
  }
}
```

3. Create a new project and add the following prompt to the project instructions:

```
Follow these steps for each interaction:

1. User Identification:
   - You should assume that you are interacting with default_user
   - If you have not identified default_user, proactively try to do so.

2. Memory Retrieval:
   - Always begin your chat by saying only "Remembering..." and retrieve all relevant information from your knowledge graph
   - Always refer to your knowledge graph as your "memory"

3. Memory
   - While conversing with the user, be attentive to any new information that falls into these categories:
     a) Basic Identity (age, gender, location, job title, education level, etc.)
     b) Behaviors (interests, habits, etc.)
     c) Preferences (communication style, preferred language, etc.)
     d) Goals (goals, targets, aspirations, etc.)
     e) Relationships (personal and professional relationships up to 3 degrees of separation)

4. Memory Update:
   - If any new information was gathered during the interaction, update your memory as follows:
     a) Create entities for recurring organizations, people, and significant events
     b) Connect them to the current entities using relations
     c) Store facts about them as observations
```

## Available Tools

The `GraphService` provides the following MCP tools:

- `create_entities` / `create_relations` – Create new entities and relationships in the knowledge graph
- `add_observations` – Add factual observations about entities
- `search_nodes` – Search for entities and relationships using text queries
- `delete_entities`, `delete_relations`, `delete_observations` – Remove elements from the knowledge graph
- `open_nodes`, `read_graph` – Read and inspect the knowledge graph structure
- `get_stats` – Get statistics about the knowledge graph

Clients communicate using the MCP protocol, sending JSON-RPC requests through stdin/stdout. For protocol details, see [rmcp](https://crates.io/crates/rmcp).

## Releasing

This project uses [cargo-dist](https://github.com/axodotdev/cargo-dist) to build and publish release artifacts. Run the following command to generate archives for all supported targets:

```bash
cargo dist --release
```
