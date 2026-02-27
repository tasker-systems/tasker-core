# MCP Workflow Exercise: Content Publishing Pipeline

This guide walks through building a complete 7-step workflow from scratch using only MCP tools. Each step is replayable — you can follow along in Claude Code, Cursor, or any MCP-compatible client.

## Prerequisites

- `tasker-mcp` built and registered in your MCP client (see [MCP Setup Guide](mcp-setup.md))
- Verify 6 tools are visible: `template_generate`, `template_validate`, `template_inspect`, `handler_generate`, `schema_inspect`, `schema_compare`

## The Workflow: Content Publishing Pipeline

A 7-step double-diamond DAG — two fan-out/fan-in patterns chained together:

```
validate_content
   ├── check_plagiarism ──┐
   └── generate_metadata ─┤
                          ├── review_gate
                          │      ├── publish_to_cdn ──────┐
                          │      └── notify_subscribers ──┤
                          │                               └── update_analytics
```

## Exercise Steps

### Step 1: Generate the Template

**Prompt**: "Generate a task template for a content publishing pipeline with 7 steps..."

**Tool**: `template_generate`

Provide the structured spec with all 7 steps, their dependencies, and output fields:

| Step | Dependencies | Key Output Fields |
|------|-------------|-------------------|
| `validate_content` | (none — root) | `is_valid` (bool), `word_count` (int), `format_issues` (string[]) |
| `check_plagiarism` | validate_content | `plagiarism_score` (number), `flagged_passages` (string[]), `is_original` (bool) |
| `generate_metadata` | validate_content | `title` (string), `slug` (string), `seo_keywords` (string[]), `estimated_read_time` (int) |
| `review_gate` | check_plagiarism, generate_metadata | `approved` (bool), `reviewer_notes` (string), `quality_score` (number) |
| `publish_to_cdn` | review_gate | `published_url` (string), `cdn_id` (string), `cache_ttl` (int) |
| `notify_subscribers` | review_gate | `notifications_sent` (int), `channels` (string[]), `delivery_status` (string) |
| `update_analytics` | publish_to_cdn, notify_subscribers | `tracking_id` (string), `metrics_recorded` (bool), `dashboard_url` (string) |

**Expected**: Valid YAML with `name: publish_article`, `namespace_name: content_publishing`, 7 steps with `result_schema` definitions.

### Step 2: Validate the Template

**Prompt**: "Validate this template for correctness."

**Tool**: `template_validate`

Pass the YAML from Step 1.

**Expected**: `valid: true`, `step_count: 7`, no errors. May have info-level findings.

### Step 3: Inspect the DAG Structure

**Prompt**: "Show me the DAG structure and execution order."

**Tool**: `template_inspect`

**Expected**:
- `root_steps`: `["validate_content"]`
- `leaf_steps`: `["update_analytics"]`
- `execution_order`: validate_content → check_plagiarism, generate_metadata → review_gate → publish_to_cdn, notify_subscribers → update_analytics
- All 7 steps have `has_result_schema: true`

### Step 4: Evolve the Schema

**Prompt**: "Add a `quality_score` field (number, required) to the `review_gate` step's result schema."

**Tool**: `template_generate` (iterate on the spec)

**Expected**: Updated YAML with `quality_score` in `review_gate.result_schema`.

### Step 5: Break a Schema Contract

**Prompt**: "Compare the schema between `check_plagiarism` (producer) and `review_gate` (consumer) — what if we remove `is_original` from the producer?"

**Tool**: `schema_compare`

Deliberately modify the template to remove `is_original` from `check_plagiarism`, then compare.

**Expected**: Detects the missing field — incompatibility finding between producer and consumer.

### Step 6: Fix and Re-validate

**Prompt**: "Fix the schema and validate again."

**Tool**: `template_validate`

Restore the `is_original` field and validate.

**Expected**: `valid: true`, clean pass.

### Step 7: Inspect Field-Level Schema Details

**Prompt**: "Show me the schema details for the review_gate step."

**Tool**: `schema_inspect` with `step_filter: "review_gate"`

**Expected**: Shows `approved`, `reviewer_notes`, `quality_score` fields with types and required status. Shows `consumed_by` list (publish_to_cdn, notify_subscribers).

### Step 8: Generate Python Handlers

**Prompt**: "Generate Python handler code for this template."

**Tool**: `handler_generate` with `language: "python"`, `scaffold: true`

**Expected**:
- `types`: Pydantic model classes for each step's result schema
- `handlers`: Handler functions that import models and return typed results
- `tests`: Test stubs for each handler

### Step 9: Adapt for Other Languages

Try the same `handler_generate` call with different languages:
- `language: "typescript"` — generates TypeScript interfaces + handler functions
- `language: "ruby"` — generates Ruby classes + handler methods
- `language: "rust"` — generates Rust structs + handler functions

## Golden Fixture

The completed template is available as a test fixture:

```
tests/fixtures/task_templates/content_publishing_template.yaml
```

This fixture is validated by the tasker-mcp test suite.

## Prompt Engineering Tips

When using MCP tools with LLM agents:

1. **Be explicit about step dependencies** — list them by name, not by description
2. **Specify field types precisely** — use `string`, `integer`, `number`, `boolean`, `array:<type>`
3. **Start with `template_generate`** — let the tool create valid YAML structure, then iterate
4. **Use `schema_compare` early** — catch contract mismatches before generating handlers
5. **Use `step_filter`** — when generating handlers for a large template, filter to specific steps to keep responses focused

## Adapting This Exercise

To create a different workflow:

1. Replace the step names, dependencies, and output fields in Step 1
2. Follow the same tool sequence: generate → validate → inspect → generate handlers
3. Common DAG shapes:
   - **Linear**: A → B → C → D
   - **Fan-out**: A → [B, C, D] → E
   - **Diamond**: A → [B, C] → D (this exercise uses two diamonds)
   - **Complex**: Mix of fan-out, fan-in, and linear segments
