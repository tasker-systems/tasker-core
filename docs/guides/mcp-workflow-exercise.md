# MCP Workflow Exercise: Content Publishing Pipeline

This guide walks through building a complete 7-step workflow from scratch using only MCP tools. Each step is replayable — you can follow along in Claude Code, Cursor, or any MCP-compatible client.

## Prerequisites

- `tasker-mcp` built and registered in your MCP client (see [MCP Setup Guide](mcp-setup.md))
- Verify 7 tools are visible: `template_generate`, `template_validate`, `template_inspect`, `handler_generate`, `schema_inspect`, `schema_compare`, `schema_diff`

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

## Tool Reference

| Tool | Purpose | When to Use |
|------|---------|-------------|
| `template_generate` | Create template YAML from a structured spec | Starting a new workflow |
| `template_validate` | Check structural correctness and detect cycles | After any template change |
| `template_inspect` | Show DAG structure, execution order, root/leaf steps | Understanding workflow shape |
| `handler_generate` | Generate typed code (types + handlers + tests) | Ready to implement handlers |
| `schema_inspect` | Show field-level details per step | Reviewing data contracts |
| `schema_compare` | Compare two steps' schemas within a template | Checking producer/consumer compatibility |
| `schema_diff` | Compare two versions of the same template | Detecting breaking changes across versions |

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
| `review_gate` | check_plagiarism, generate_metadata | `approved` (bool), `reviewer_notes` (string) |
| `publish_to_cdn` | review_gate | `published_url` (string), `cdn_id` (string), `cache_ttl` (int) |
| `notify_subscribers` | review_gate | `notifications_sent` (int), `channels` (string[]), `delivery_status` (string) |
| `update_analytics` | publish_to_cdn, notify_subscribers | `tracking_id` (string), `metrics_recorded` (bool), `dashboard_url` (string) |

**Expected**: Valid YAML with `name: publish_article`, `namespace_name: content_publishing`, 7 steps with `result_schema` definitions.

**Save this YAML** — you'll use it as the "v1" baseline for `schema_diff` in Step 6.

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
- Parallel-eligible steps may appear in any order — both orderings are valid topological sorts (e.g., `generate_metadata` before `check_plagiarism`, or `notify_subscribers` before `publish_to_cdn`)
- All 7 steps have `has_result_schema: true`

### Step 4: Evolve the Schema

**Prompt**: "Add a `quality_score` field (number, required) to the `review_gate` step's result schema."

**Tool**: `template_generate` (iterate on the spec)

Re-generate the template with the updated `review_gate` step. Keep all other steps identical.

**Expected**: Updated YAML with `quality_score` in `review_gate.result_schema.properties` and added to `required`.

**Save this YAML** — this is the "v2" version for `schema_diff` in Step 6.

### Step 5: Compare Schemas for Structural Compatibility

**Prompt**: "Check whether `review_gate`'s output is compatible with what `publish_to_cdn` expects."

**Tool**: `schema_compare`

Use `schema_compare` with `producer_step: "review_gate"` and `consumer_step: "publish_to_cdn"` to check whether the producer's output covers the consumer's input contract. This is most useful for steps in a direct dependency relationship — `publish_to_cdn` depends on `review_gate`, so any missing required fields or type mismatches represent real data flow problems.

**Expected**: A compatibility report showing `EXTRA_PRODUCER_FIELD` findings for `review_gate` fields that `publish_to_cdn` doesn't reference (e.g., `approved`, `reviewer_notes`). Since these are non-breaking, the report shows `compatible_with_warnings`. If `publish_to_cdn` required a field that `review_gate` doesn't produce, you'd see `MISSING_REQUIRED_FIELD` with `breaking: true`.

> **Tip**: `schema_compare` is most valuable between directly connected steps. Comparing unrelated steps (e.g., `check_plagiarism` vs `notify_subscribers`) shows complete structural divergence — every field appears as missing or extra — which is technically correct but not actionable.

### Step 6: Diff Template Versions for Breaking Changes

This step uses `schema_diff` — the versioning tool that detects field-level changes between two versions of the same template. Unlike `schema_compare` (which compares two *different* steps within a single template), `schema_diff` compares *before and after* versions of the entire template to detect temporal changes.

#### 6a: Detect a Non-Breaking Change

**Prompt**: "Compare v1 and v2 of my template to detect breaking changes."

**Tool**: `schema_diff`

**Parameters**:
- `before_yaml`: The original template YAML from Step 1 (v1)
- `after_yaml`: The evolved template YAML from Step 4 (v2, with `quality_score` added)
- `step_filter`: `"review_gate"` (optional — focuses the diff on just this step)

**Expected**:
```json
{
  "compatibility": "compatible_with_warnings",
  "step_diffs": [
    {
      "step_name": "review_gate",
      "status": "modified",
      "findings": [
        {
          "code": "FIELD_ADDED",
          "breaking": false,
          "field_path": "quality_score",
          "after_type": "number",
          "message": "Field 'quality_score' was added"
        }
      ]
    }
  ]
}
```

Adding a new field is always non-breaking from a diff perspective — the field didn't exist before, so there's no prior contract to violate. Note that `OPTIONAL_TO_REQUIRED` only fires when a field exists in *both* versions and changes status (see 6c below).

#### 6b: Detect a Breaking Change

Now create a "v3" by removing the `is_original` field from `check_plagiarism` (delete it from both `properties` and `required`). Diff v1 against v3:

**Parameters**:
- `before_yaml`: Original template from Step 1
- `after_yaml`: Modified template with `is_original` removed
- `step_filter`: `"check_plagiarism"`

**Expected**:
```json
{
  "compatibility": "incompatible",
  "step_diffs": [
    {
      "step_name": "check_plagiarism",
      "status": "modified",
      "findings": [
        {
          "code": "FIELD_REMOVED",
          "breaking": true,
          "field_path": "is_original",
          "before_type": "boolean",
          "message": "Required field 'is_original' was removed"
        }
      ]
    }
  ]
}
```

Removing a *required* field is always a breaking change. Removing an *optional* field reports `breaking: false`.

#### 6c: Detect a Required Status Change

To see `OPTIONAL_TO_REQUIRED` in action, create a "v4" where `reviewer_notes` (optional in v1) becomes required. Diff v1 against v4:

**Parameters**:
- `before_yaml`: Original template from Step 1
- `after_yaml`: Modified template with `reviewer_notes` added to `review_gate.result_schema.required`
- `step_filter`: `"review_gate"`

**Expected**:
```json
{
  "compatibility": "incompatible",
  "step_diffs": [
    {
      "step_name": "review_gate",
      "status": "modified",
      "findings": [
        {
          "code": "OPTIONAL_TO_REQUIRED",
          "breaking": true,
          "field_path": "reviewer_notes",
          "before_type": "string",
          "after_type": "string",
          "message": "Field 'reviewer_notes' changed from optional to required"
        }
      ]
    }
  ]
}
```

Tightening a field from optional to required is breaking — existing consumers may not provide it. The reverse (`REQUIRED_TO_OPTIONAL`) is non-breaking because it relaxes the contract.

#### Change Codes Reference

| Code | Meaning | Breaking? |
|------|---------|-----------|
| `FIELD_ADDED` | Field present in after but not before | No |
| `FIELD_REMOVED` | Field present in before but not after | Yes if required, No if optional |
| `TYPE_CHANGED` | Field type differs between versions | Yes |
| `REQUIRED_TO_OPTIONAL` | Was required, now optional | No (relaxing) |
| `OPTIONAL_TO_REQUIRED` | Was optional, now required | Yes (tightening) |
| `STEP_ADDED` | Step exists in after but not before | No |
| `STEP_REMOVED` | Step exists in before but not after | Yes |
| `SCHEMA_ADDED` | Step gained a result_schema | No |
| `SCHEMA_REMOVED` | Step lost its result_schema | Yes |

### Step 7: Validate the Final Template

**Prompt**: "Validate the final v2 template."

**Tool**: `template_validate`

Pass the v2 template (with `quality_score` added). This confirms the evolved template is still structurally valid.

**Expected**: `valid: true`, clean pass.

### Step 8: Inspect Field-Level Schema Details

**Prompt**: "Show me the schema details for the review_gate step."

**Tool**: `schema_inspect` with `step_filter: "review_gate"`

**Expected**: Shows `approved`, `reviewer_notes`, `quality_score` fields with types and required status. Shows `consumed_by` list (publish_to_cdn, notify_subscribers).

### Step 9: Generate Python Handlers

**Prompt**: "Generate Python handler code for this template."

**Tool**: `handler_generate` with `language: "python"`, `scaffold: true`

**Expected**:
- `types`: Pydantic model classes for each step's result schema
- `handlers`: Handler functions that import models and return typed results
- `tests`: Test stubs for each handler

### Step 10: Adapt for Other Languages

Try the same `handler_generate` call with different languages:
- `language: "typescript"` — generates Zod schemas + inferred types, `defineHandler` functions, and `bun:test` stubs
- `language: "ruby"` — generates `Dry::Struct` types, handler blocks, and RSpec stubs
- `language: "rust"` — generates serde structs, plain function handlers, `#[test]` stubs, and a `handler_registry` module

> **Rust note**: Rust codegen produces a fourth output field — `handler_registry` — in addition to `types`, `handlers`, and `tests`. This is a `StepHandlerRegistry` bridge module that wraps plain handler functions as `StepHandler` trait objects, which is the dispatch interface required by `tasker-worker`. Other languages don't need this because their FFI bindings handle dispatch registration differently.

## Golden Fixture

The completed template (v2, with `quality_score`) is available as a test fixture:

```
tests/fixtures/task_templates/content_publishing_template.yaml
```

This fixture is validated by the tasker-mcp test suite.

## Prompt Engineering Tips

When using MCP tools with LLM agents:

1. **Be explicit about step dependencies** — list them by name, not by description
2. **Specify field types precisely** — use `string`, `integer`, `number`, `boolean`, `array:<type>`
3. **Start with `template_generate`** — let the tool create valid YAML structure, then iterate
4. **Use `schema_diff` for versioning** — always diff before and after when evolving a template to catch breaking changes before they reach production
5. **Use `schema_compare` for data flow** — check producer/consumer compatibility between steps that pass data
6. **Use `step_filter`** — when generating handlers or diffing a large template, filter to specific steps to keep responses focused

## Adapting This Exercise

To create a different workflow:

1. Replace the step names, dependencies, and output fields in Step 1
2. Follow the same tool sequence: generate → validate → inspect → evolve → diff → generate handlers
3. Common DAG shapes:
   - **Linear**: A → B → C → D
   - **Fan-out**: A → [B, C, D] → E
   - **Diamond**: A → [B, C] → D (this exercise uses two diamonds)
   - **Complex**: Mix of fan-out, fan-in, and linear segments
