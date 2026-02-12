# TAS-126 + TAS-127: Tasker Contrib Foundations & CLI Plugin Architecture

## Context

Tasker's developer bootstrap story is weak - users must figure out framework integration patterns themselves. The solution is making `tasker-ctl` an extensible developer tool that discovers plugins from `tasker-contrib` (and user-local paths) and generates framework-specific code from templates.

**Evolved vision**: No framework-specific packages (no tasker-contrib-rails gem, etc.). Instead:
- `tasker-contrib` = examples + CLI plugins (template manifests + Tera templates)
- `tasker-ctl` = the developer bootstrap tool (plugin discovery, template generation)
- Published FFI packages (`tasker-rb`, `tasker-py`, `@tasker-systems/tasker`) are the integration points

**Branch**: `jcoletaylor/tas-126-tasker-contrib-foundations` (combined TAS-126 + TAS-127)

---

## Implementation Plan

### Phase 1: Dependencies & CLI Config System

**Add workspace dependencies** to `Cargo.toml`:
```toml
tera = "1.20"        # Runtime template engine for plugins
heck = "0.5"         # Case conversion for Tera filters
```

**Add to `tasker-ctl/Cargo.toml`**:
```toml
tera = { workspace = true }
heck = { workspace = true }
```

**New module: `tasker-ctl/src/cli_config/`**
- `mod.rs` - `CliConfig` struct with `plugin_paths`, `default_language`, `default_output_dir`
- `loader.rs` - Config file discovery: `./.tasker-cli.toml` > `~/.config/tasker-cli.toml`

Config format (`.tasker-cli.toml`):
```toml
plugin-paths = [
    "./tasker-cli-plugins",
    "~/projects/tasker-systems/tasker-contrib",
]
default-language = "ruby"
default-output-dir = "./app/handlers"
```

No `directories` crate needed - just 2 hardcoded discovery paths with `$HOME` expansion.

### Phase 2: Plugin Discovery & Registry

**New module: `tasker-ctl/src/plugins/`**
- `mod.rs` - Public API
- `manifest.rs` - `PluginManifest`, `PluginMetadata`, `TemplateReference` structs (parse `tasker-plugin.toml`)
- `discovery.rs` - Smart 2-level scan: check path root, immediate subdirs, and `*/tasker-cli-plugin/` pattern
- `registry.rs` - `PluginRegistry` with `discover()`, `find_template()`, `list_templates()`

**Discovery logic** (handles tasker-contrib's `rails/tasker-cli-plugin/tasker-plugin.toml` layout):
1. Check if path itself contains `tasker-plugin.toml`
2. Scan immediate subdirs for `tasker-plugin.toml`
3. Scan `*/tasker-cli-plugin/` subdirs for `tasker-plugin.toml`

### Phase 3: Template Loading & Tera Engine

**New module: `tasker-ctl/src/template_engine/`** (named `template_engine` to avoid collision with existing `tasker-ctl/templates/` Askama dir)
- `mod.rs` - Public API
- `metadata.rs` - `TemplateMetadata`, `ParameterDef`, `OutputFile` (parse `template.toml`)
- `engine.rs` - Tera wrapper with custom filter registration
- `loader.rs` - Load `template.toml` + `*.tera` files from plugin template dirs
- `filters.rs` - Custom Tera filters: `snake_case`, `pascal_case`, `camel_case`, `kebab_case` (via `heck`)

Template directory structure (in plugin):
```
templates/step_handler/
├── template.toml          # Metadata, parameters, output files
├── handler.rb.tera        # Template content
└── handler_spec.rb.tera   # Optional additional outputs
```

### Phase 4: CLI Commands

**New command enums in `main.rs`**:
- `PluginCommands` - `List`, `Validate { path }`
- `TemplateCommands` - `List`, `Info { name }`, `Generate { template, params, language, framework, output }`

**New handler modules**:
- `tasker-ctl/src/commands/plugin.rs` - `handle_plugin_command()`
- `tasker-ctl/src/commands/template.rs` - `handle_template_command()`

**Commands**:
```bash
tasker-ctl plugin list                                    # Show discovered plugins
tasker-ctl plugin validate ./path/to/plugin               # Validate plugin manifest
tasker-ctl template list [--language ruby] [--framework rails]  # List templates
tasker-ctl template info step-handler [--plugin rails]    # Show template params
tasker-ctl template generate step-handler \               # Generate from template
    --param name=ProcessPayment \
    --param handler_type=api \
    --language ruby \
    --output ./app/handlers/
```

**Update `main.rs`**: Add `Plugin` and `Template` variants to `Commands` enum, add dispatch cases.
**Update `commands/mod.rs`**: Export new handlers.

### Phase 5: Tests

- **Unit tests** in each module: manifest parsing, config loading, filter correctness, registry lookup
- **Integration tests** in `tasker-ctl/tests/`: create temp plugin dirs, exercise full discovery + generation flow
- **Validation**: `cargo clippy --all-targets --all-features --workspace` must be zero warnings

---

## Files to Create

| File | Purpose |
|------|---------|
| `tasker-ctl/src/cli_config/mod.rs` | CliConfig struct, load() |
| `tasker-ctl/src/cli_config/loader.rs` | Config file discovery |
| `tasker-ctl/src/plugins/mod.rs` | Plugin module public API |
| `tasker-ctl/src/plugins/manifest.rs` | Parse tasker-plugin.toml |
| `tasker-ctl/src/plugins/discovery.rs` | Smart 2-level path scanning |
| `tasker-ctl/src/plugins/registry.rs` | PluginRegistry with filtering |
| `tasker-ctl/src/template_engine/mod.rs` | Template module public API |
| `tasker-ctl/src/template_engine/metadata.rs` | Parse template.toml |
| `tasker-ctl/src/template_engine/engine.rs` | Tera wrapper + filter registration |
| `tasker-ctl/src/template_engine/loader.rs` | Load .tera files from plugin dirs |
| `tasker-ctl/src/template_engine/filters.rs` | snake_case, pascal_case, etc. |
| `tasker-ctl/src/commands/plugin.rs` | Plugin CLI command handler |
| `tasker-ctl/src/commands/template.rs` | Template CLI command handler |

## Files to Modify

| File | Changes |
|------|---------|
| `Cargo.toml` (workspace root) | Add `tera = "1.20"`, `heck = "0.5"` to `[workspace.dependencies]` |
| `tasker-ctl/Cargo.toml` | Add `tera`, `heck` dependencies |
| `tasker-ctl/src/main.rs` | Add module declarations, `PluginCommands`/`TemplateCommands` enums, dispatch cases |
| `tasker-ctl/src/commands/mod.rs` | Export new plugin/template handlers |

## Key Design Decisions

1. **Hybrid template engines**: Askama (compile-time) stays for built-in docs, Tera (runtime) for plugin templates
2. **Separate config file**: `.tasker-cli.toml` for plugin paths (not in `.config/tasker-client.toml`)
3. **No feature gate**: Plugin system is core CLI functionality, Tera adds ~500KB
4. **No `directories` crate**: Simple 2-path discovery with `$HOME` expansion
5. **Smart 2-level scan**: Handles tasker-contrib's nested layout (`rails/tasker-cli-plugin/`)
6. **Module naming**: `template_engine/` (not `templates/`) to avoid collision with existing Askama `templates/` dir
7. **No profile system yet**: Start simple, add profiles in future iteration if needed
8. **Parameter validation in CLI**: Validate before rendering for clear error messages

## Verification

1. `cargo build --all-features` compiles cleanly
2. `cargo clippy --all-targets --all-features --workspace` zero warnings
3. `cargo test --all-features -p tasker-ctl` passes all tests
4. Manual test: point `.tasker-cli.toml` at tasker-contrib, run `tasker-ctl plugin list`
5. Manual test: create a test plugin with `.tera` templates, run `tasker-ctl template generate`
