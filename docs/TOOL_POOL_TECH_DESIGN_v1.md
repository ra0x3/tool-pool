# MCP Tool Marketplace: Specification & Design Rationale

## Executive Summary

A marketplace for MCP (Model Context Protocol) tools where users can discover, download, and install portable tool binaries that work across any compatible MCP server. Tools are compiled to WebAssembly (WASM) using WASI for portability, requiring no custom FFI—only standard system interfaces.

**Core thesis**: Tools are shared, reusable, *opinionated executables*. In the early days of MCP, everyone is writing their own tool to do the same thing—duplication. These tools are largely deterministic, non-differentiating, and expensive to maintain relative to their value. As AI increases leverage, differentiation moves to the **domain layer**, not the integration layer. This makes a community-driven tool registry rational, inevitable infrastructure.

---

## Core Principles

- Tools are **executables**, not libraries
- Tools are **atomic**—one tool, one operation
- Tool authors **own semantics**
- Users **opt into behavior** via tool choice + versioning
- The marketplace distributes **binaries + metadata**, not services
- Execution happens **locally**, on the user's MCP server
- Composition happens at the **agent layer**, not inside tools
- Determinism + explicit I/O is sufficient for reuse

---

## Objections & Responses

### Objection 1: The portability assumption is wrong

**Objection**: The vision assumes tools are interchangeable (e.g. swap `gcal.create_event` for `outlook.create_event`).

**Response**: Interchangeability is *not* the goal. The goal is **reproducibility**, not fungibility. User A and User B installing `gcal.create_event@1.2.0` should get identical behavior. This mirrors GitHub Actions and Docker images: same artifact, same semantics. Package-manager semantics, not universal abstraction.

---

### Objection 2: Capability specs become OpenAPI-level complex

**Response**: The capability spec is **not the product**. It is minimal metadata for discovery and invocation. The executable defines behavior. Specs evolve *from usage*, not up-front design. This avoids SOAP/WSDL failure modes.

---

### Objection 3: Who runs the code?

**Response**: The MCP server owner runs the code. Tools are WASM binaries executed locally via WASI. The marketplace hosts files and metadata only. No hosting, no execution, no runtime lock-in.

---

### Objection 4: Trust isn't solved by signing

**Response**: This follows the npm / GitHub Actions trust model:
- Community trust
- Explicit versioning
- Visible maintainers
- Reproducible artifacts

The alternative (everyone writes everything) is strictly worse. WASM + explicit effects *constrain blast radius* even if trust fails.

---

### Objection 5: Incentives are misaligned (vendors want lock-in)

**Response**: Vendors are not the customer. Users running MCP servers are. This externalizes undifferentiated integration risk away from domain teams. This is the same economic force that made open source and cloud infrastructure inevitable.

---

### Objection 6: This has been tried before (UDDI, RapidAPI, etc.)

**Response**: Those systems failed because they distributed *interfaces*, not *executables*. MCP tools are opinionated binaries with owned semantics. The correct analogy is GitHub Actions or Homebrew—not service registries.

---

### Objection 7: MCP itself is unproven

**Response**: True—but this system degrades gracefully. Even if MCP fragments, a WASM tool format + registry remains valuable. This is infrastructure *for capability execution*, not a bet on a single vendor.

---

### Objection 8: Composio already exists

**Response**: Composio is an SDK and hosted execution layer for developers. This proposal defines a **binary tool format and registry**. Composio could build *on top* of this. This operates one layer lower.

---

### Objection 9: Non-technical users can't handle OAuth

**Response**: Credential acquisition is handled by the MCP server's credential provider. Tools declare requirements; servers resolve them. Users configure OAuth providers once at the server level, not per-tool.

---

### Objection 10: WASM FFI doesn't scale

**Response**: Correct—and irrelevant. Tool authors target `wasm32-wasip2` and use standard libraries. WASI provides filesystem, networking (HTTP, sockets), env vars, subprocesses. No custom FFI layer is introduced.

---

## Additional Concerns & Resolutions

### Concern 11: Pagination and large result sets

**Problem**: How does a CLI-style tool handle pagination without streaming or sessions?

**Resolution**:
- Tools MUST be stateless across invocations
- Pagination state (cursor, offset) is returned explicitly in the response
- The caller (agent or MCP server) controls iteration

Example:

**Input**
```json
{
  "cursor": null,
  "page_size": 50
}
```

**Output**
```json
{
  "items": [...],
  "next_cursor": "abc123",
  "has_more": true
}
```

This matches REST, AWS SDKs, and agent planning models. No streaming required.

---

### Concern 12: Semantic divergence (timezone handling, retries, strictness)

**Problem**: Different users want different behavior.

**Resolution**:
- Tools are **opinionated**
- Tool authors decide semantics
- Users choose tools + versions accordingly

If one tool is flexible and another strict, that is **ecosystem selection**, not a failure. This mirrors CLI tools, Docker images, and GitHub Actions.

---

### Concern 13: "Why not just build my own?"

**Problem**: Teams could write custom glue code.

**Resolution**:
This is the same argument against open source and cloud infra.

As AI increases leverage:
- Writing glue code gets cheaper
- **Owning failure modes gets more expensive**

Shared tools externalize undifferentiated integration risk. Domain teams optimize for outcomes, not integration craftsmanship.

---

### Concern 14: Tool quality variance harms agents

**Problem**: Inconsistent tool behavior confuses LLMs.

**Resolution**:
- Strict stdin/stdout JSON contract
- Versioned behavior
- Explicit success/error shapes

This is sufficient. Agents already operate in stepwise loops and adapt to tool variance via planning.

---

### Concern 15: Tool composition

**Problem**: How do tools compose (e.g., `normalize_address` → `geocode` → `create_event`)?

**Resolution**:
- Tools are **atomic**—one tool, one operation
- Composition happens at the **agent layer**
- The agent orchestrates: call A, receive output, call B with output, etc.

Tools do not invoke other tools. Tools do not declare pipelines. This keeps tools simple, portable, and independently testable. The agent (or MCP server acting on behalf of an agent) is the orchestrator.

This is the Unix model: tools are filters, the shell composes them.

---

## Technical Architecture

### Execution Model

- Tools compile to `wasm32-wasip2`
- Executed via wasmtime/wasmer
- Communicate via stdin/stdout JSON
- No persistent state across invocations
- WASI capabilities: filesystem, HTTP (`wasi:http`), sockets (`wasi:sockets`), env vars, clocks

**Rule**:
> All continuation state (pagination, retries, offsets) MUST be explicit in responses.

---

### Input / Output Contract

**stdin**: JSON request
**stdout**: JSON response
**stderr**: ignored or logged by server

Success:
```json
{ "success": true, "result": {...} }
```

Error:
```json
{ "success": false, "error": { "code": "...", "message": "..." } }
```

---

### Credential Management

Tools declare credential requirements. The MCP server resolves and injects them.

#### Tool Manifest Declaration

```toml
[tool]
name = "gcal.create_event"
version = "1.2.0"

[credentials.google]
type = "oauth2"
provider = "google"
scopes = ["https://www.googleapis.com/auth/calendar.events"]

[credentials.custom_api]
type = "api_key"
env_var = "ACME_API_KEY"
```

#### Credential Types

| Type | Required Fields | Injected As |
|------|-----------------|-------------|
| `oauth2` | `provider`, `scopes` | `{PROVIDER}_ACCESS_TOKEN` |
| `api_key` | `env_var` | User-specified env var |
| `basic_auth` | `env_var_user`, `env_var_pass` | Two env vars |

#### Server-Side Provider Configuration

Servers configure OAuth2 providers independently. Tools reference providers by name; servers resolve them.

```toml
[oauth2_providers.google]
client_id = "..."
client_secret = "..."
auth_url = "https://accounts.google.com/o/oauth2/v2/auth"
token_url = "https://oauth2.googleapis.com/token"
```

#### Credential Provider Interface

The server implements (or delegates to) a credential provider:

```rust
trait CredentialProvider {
    /// Resolve a credential requirement to an injectable value.
    /// Handles refresh internally if needed.
    fn resolve(&mut self, requirement: &CredentialRequirement) -> Result<ResolvedCredential, CredentialError>;

    /// Persist updated tokens after refresh.
    fn store(&mut self, provider_id: &str, tokens: &TokenSet) -> Result<(), CredentialError>;
}

struct ResolvedCredential {
    env_var: String,
    value: SecretString,
    expires_at: Option<DateTime<Utc>>,
}

struct TokenSet {
    access_token: SecretString,
    refresh_token: Option<SecretString>,
    expires_at: Option<DateTime<Utc>>,
    scopes: Vec<String>,
}
```

#### Injection Flow

1. Server receives tool invocation request
2. Server reads tool manifest → extracts credential requirements
3. For each requirement:
   - Call `provider.resolve(requirement)`
   - If OAuth2 and expired → refresh → store new tokens
   - Get back env_var + value
4. Build environment map
5. Spawn WASM tool with environment injected
6. Tool reads credentials from env, executes, exits

#### Error Handling

If a required provider is not configured:

```json
{
  "success": false,
  "error": {
    "code": "credential_provider_not_configured",
    "message": "OAuth2 provider 'google' is not configured on this server"
  }
}
```

---

### Non-Goals (Explicit)

- Tools are not long-running services
- Tools must terminate after producing output
- Tools must not expose inbound network listeners
- Tools must not maintain hidden state
- Tools do not invoke other tools
- Tools do not declare pipelines or dependencies on other tools

---

## Marketplace Design

- Static file hosting for `.wasm` binaries
- Metadata index for discovery
- Semver versioning
- Permissionless publication (initially)
- Delisting allowed; binaries are immutable

---

## Prior Art & Differentiation

| System | Model | Why This Is Different |
|--------|-------|----------------------|
| GitHub Actions | Executable actions | Same model, new domain |
| npm | Libraries | This ships executables |
| Docker Hub | Images | CLI-level granularity |
| RapidAPI | APIs | No execution portability |
| Composio | SDK | Requires code |

---

## First Moves

1. Define v0.1 I/O contract
2. Embed WASM runtime in MCP server
3. Build 3–5 real tools
4. Prove portability across environments
5. Publish registry + spec
6. Measure adoption

---

## Bottom Line

This system treats tools as **opinionated, atomic, deterministic executables**.
Credentials are server-managed; tools declare requirements, servers resolve them.
Composition happens at the agent layer, not inside tools.
Coordination costs are minimized by versioning and opt-in semantics.
As AI shifts value to domain expertise, shared integration tools become inevitable infrastructure.
