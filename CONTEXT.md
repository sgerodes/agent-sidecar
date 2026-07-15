# AI Sidecar

This context defines the language for a restricted AI execution boundary used by application backends.

## Language

**AI Sidecar**:
An isolated AI execution service that receives application requests and runs subscription CLI-based AI tooling with constrained access to files, network, and credentials. A sidecar is usually paired one-to-one with an application, but the term describes the isolation boundary rather than a permanent deployment cardinality.
_Avoid_: AI gateway, AI sandbox, bot worker

**Policy Workspace**:
A tiny filesystem context intentionally exposed to the AI Sidecar's provider runtime. It contains only guidance needed for safe operation, such as provider rules and schema context.
_Avoid_: project workspace, app repository, mounted source

**Schema Manifest**:
A sidecar-owned description of the database shape available for AI prompting. It is convenience context only; database credentials and roles define actual access.
_Avoid_: database policy, authorization schema, source of truth

**Security AI**:
A stateless AI gate that reviews the current user message before executor execution. It returns a concise allow/block judgement and reason, but does not get database access or sticky session context.
_Avoid_: firewall, authorization service, audit database

**Executor AI**:
The front-facing AI execution module that answers approved user requests. It may receive database access when configured, and its output is returned through the REST API after validation and egress filtering.
_Avoid_: security reviewer, policy engine

**Egress Secret Filter**:
A fail-closed output check that blocks provider output when it appears to contain protected secrets or configured secret fragments. It is a defense-in-depth control, not a proof that secrets are unreachable.
_Avoid_: purificator, sanitizer, redactor

**Protected Secret**:
A value that must not leave the AI Sidecar in provider output, logs, or API responses. Provider auth material, database credentials, and canary secrets are protected secrets.
_Avoid_: token only, credential only

## Example Dialogue

Developer: "Can the AI Sidecar read the NestJS project files?"

Domain expert: "No. The Policy Workspace is not the app repository; it only contains provider rules and the Schema Manifest."

Developer: "Does the Schema Manifest stop the model from querying a table?"

Domain expert: "No. The Schema Manifest helps the model write useful SQL. The database role is the permission boundary."

Developer: "What happens if Codex tries to return a database password?"

Domain expert: "The Egress Secret Filter detects the Protected Secret and the request fails closed."

Developer: "Does the Security AI keep chat history?"

Domain expert: "No. It is pure per request: stored security prompt plus current user message only."
