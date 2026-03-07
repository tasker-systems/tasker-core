#!/usr/bin/env python3
"""Generate DSL E2E test variants from existing E2E tests.

For each existing E2E test file, creates a _dsl variant with:
- Function names: append _dsl
- Namespace names: insert _dsl before lang suffix (or append _dsl)
- Template names: insert _dsl before lang suffix (or append _dsl)
- Step names in assertions: insert _dsl before lang suffix (or append _dsl)
- Helper function names: append _dsl
- Doc comments: updated to mention DSL variant
"""

import re
import sys
from pathlib import Path


# Language suffixes we recognize
LANG_SUFFIXES = {"_py", "_ts", "_rb"}


def insert_dsl(name: str) -> str:
    """Insert _dsl before language suffix, or append _dsl if no suffix."""
    for suffix in LANG_SUFFIXES:
        if name.endswith(suffix):
            return name[: -len(suffix)] + "_dsl" + suffix
    return name + "_dsl"


def build_name_maps(template_dir: Path, lang_suffix: str) -> dict[str, str]:
    """Build old->new name mappings from original templates in a language dir.

    Returns a dict mapping original names to DSL names for:
    - Template names (top-level name:)
    - Namespace names (namespace_name:)
    - Step names (- name: under steps:)
    """
    name_map = {}

    for yaml_file in sorted(template_dir.glob("*.yaml")):
        if "_dsl" in yaml_file.stem:
            continue

        content = yaml_file.read_text()
        for line in content.split("\n"):
            stripped = line.strip()

            # Top-level name
            if re.match(r"^name:\s+", line) and not line.startswith(" "):
                val = line.split(":", 1)[1].strip().strip('"').strip("'")
                name_map[val] = insert_dsl(val)

            # Namespace name
            elif re.match(r"^namespace_name:\s+", line):
                val = line.split(":", 1)[1].strip().strip('"').strip("'")
                name_map[val] = insert_dsl(val)

            # Step names (indented - name:)
            elif stripped.startswith("- name:") and line.startswith("  "):
                val = stripped.split(":", 1)[1].strip().strip('"').strip("'")
                if val != "ALL" and "." not in val:
                    name_map[val] = insert_dsl(val)

    return name_map


def transform_e2e_test(content: str, name_map: dict[str, str], lang: str) -> str:
    """Transform an E2E test Rust file to DSL variant.

    Strategy:
    1. Rename test functions by appending _dsl
    2. Rename helper functions by appending _dsl
    3. Replace namespace/template/step name string literals using name_map
    4. Update doc comments
    """
    result = content

    # Step 1: Rename all test functions by appending _dsl
    # Match any async fn test_*( pattern - covers all naming conventions
    result = re.sub(
        r"(async fn test_\w+?)(\s*\()",
        lambda m: m.group(1) + "_dsl" + m.group(2),
        result,
    )

    # Step 2: Rename helper functions
    # Common patterns: create_*_request_{lang_suffix}, create_*_task_request
    # Be specific - only rename declaration sites
    helper_patterns = [
        (r"(fn create_\w+?_request)_py(\()", r"\1_dsl_py\2"),
        (r"(fn create_\w+?_request)_ts(\()", r"\1_dsl_ts\2"),
        (r"(fn create_\w+?_request)_rb(\()", r"\1_dsl_rb\2"),
        # And call sites
        (r"(create_\w+?_request)_py(\()", r"\1_dsl_py\2"),
        (r"(create_\w+?_request)_ts(\()", r"\1_dsl_ts\2"),
        (r"(create_\w+?_request)_rb(\()", r"\1_dsl_rb\2"),
    ]
    for pattern, replacement in helper_patterns:
        result = re.sub(pattern, replacement, result)

    # Also handle helpers like create_domain_event_task_request
    result = re.sub(
        r"(fn create_domain_event_task_request)(\()",
        r"\1_dsl\2",
        result,
    )
    result = re.sub(
        r"(create_domain_event_task_request)(\()",
        r"\1_dsl\2",
        result,
    )

    # Helpers like create_approval_request, create_csv_processing_request, create_checkpoint_yield_request
    for helper_name in [
        "create_approval_request",
        "create_csv_processing_request",
        "create_checkpoint_yield_request",
    ]:
        result = re.sub(
            rf"(fn {helper_name})(\()",
            rf"\1_dsl\2",
            result,
        )
        result = re.sub(
            rf"({helper_name})(\()",
            rf"\1_dsl\2",
            result,
        )

    # Step 3: Replace string literals using name_map
    # Sort by length (longest first) to avoid partial replacements
    for old_name, new_name in sorted(name_map.items(), key=lambda x: -len(x[0])):
        # Replace in quoted strings: "old_name" -> "new_name"
        result = result.replace(f'"{old_name}"', f'"{new_name}"')
        # Replace in .to_string() patterns: "old_name".to_string()
        # Already handled by the above
        # Replace in contains patterns: &"old_name" -> &"new_name"
        result = result.replace(f'&"{old_name}"', f'&"{new_name}"')
        # Replace in starts_with patterns that reference step name prefixes
        # e.g., starts_with("process_csv_batch_")
        if old_name.endswith(("_py", "_ts", "_rb")):
            prefix = old_name
            new_prefix = new_name
            result = result.replace(f'starts_with("{prefix}', f'starts_with("{new_prefix}')

    # Also handle step name prefixes for batch workers (e.g., process_csv_batch_)
    # These use starts_with without full step name
    for old_name, new_name in name_map.items():
        if "batch" in old_name:
            # Handle starts_with("process_csv_batch_") patterns
            old_prefix = old_name.rstrip("_") + "_"
            if old_prefix != old_name + "_":
                new_prefix = new_name.rstrip("_") + "_"
                result = result.replace(f'"{old_prefix}"', f'"{new_prefix}"')

    # Step 4: Update module-level doc comment
    result = result.replace(
        "//! Python ",
        "//! Python DSL ",
        1,
    )
    result = result.replace(
        "//! TypeScript ",
        "//! TypeScript DSL ",
        1,
    )
    result = result.replace(
        "//! Ruby ",
        "//! Ruby DSL ",
        1,
    )
    result = result.replace(
        "//! TAS-93 Phase 5: Resolver",
        "//! TAS-294 DSL: Resolver",
        1,
    )
    result = result.replace(
        "//! TAS-125: Python",
        "//! TAS-294 DSL: Python",
        1,
    )
    result = result.replace(
        "//! TAS-125: TypeScript",
        "//! TAS-294 DSL: TypeScript",
        1,
    )
    result = result.replace(
        "//! TAS-125: Ruby",
        "//! TAS-294 DSL: Ruby",
        1,
    )

    # Step 5: Update println messages to mention DSL
    result = result.replace("Python linear workflow", "Python DSL linear workflow")
    result = result.replace("Python diamond workflow", "Python DSL diamond workflow")
    result = result.replace("Python method dispatch", "Python DSL method dispatch")
    result = result.replace("Python success scenario", "Python DSL success scenario")
    result = result.replace("Python permanent failure", "Python DSL permanent failure")
    result = result.replace("Python retryable failure", "Python DSL retryable failure")
    result = result.replace("Python small amount", "Python DSL small amount")
    result = result.replace("Python medium amount", "Python DSL medium amount")
    result = result.replace("Python large amount", "Python DSL large amount")
    result = result.replace("Python boundary", "Python DSL boundary")
    result = result.replace("Python very small amount", "Python DSL very small amount")
    result = result.replace("Python backward compatibility", "Python DSL backward compatibility")

    return result


def process_language(lang: str, e2e_dir: Path, template_dir: Path) -> list[str]:
    """Process all E2E tests for a language, generating DSL variants.

    Returns list of created module names for mod.rs.
    """
    lang_suffix_map = {"python": "_py", "typescript": "_ts", "ruby": "_rb"}
    lang_suffix = lang_suffix_map.get(lang, "")

    # Build name map from templates
    name_map = build_name_maps(template_dir, lang_suffix)

    if not name_map:
        print(f"  Warning: No name mappings found for {lang}")
        return []

    created_modules = []

    for rs_file in sorted(e2e_dir.glob("*.rs")):
        # Skip mod.rs, README, CLAUDE.md
        if rs_file.name in ("mod.rs", "README.md", "CLAUDE.md"):
            continue
        # Skip already-generated DSL variants
        if "_dsl" in rs_file.stem:
            continue

        content = rs_file.read_text()
        transformed = transform_e2e_test(content, name_map, lang)

        # Generate output filename
        new_stem = rs_file.stem + "_dsl"
        output_file = rs_file.parent / f"{new_stem}.rs"

        output_file.write_text(transformed)
        created_modules.append(new_stem)
        print(f"  Created: {output_file.name}")

    return created_modules


def update_mod_rs(e2e_dir: Path, new_modules: list[str]) -> None:
    """Add new DSL module declarations to mod.rs."""
    mod_file = e2e_dir / "mod.rs"
    if not mod_file.exists():
        return

    content = mod_file.read_text()

    # Add new module declarations at the end
    additions = []
    for module in sorted(new_modules):
        mod_line = f"mod {module};"
        if mod_line not in content:
            additions.append(f"mod {module}; // TAS-294 DSL variant")

    if additions:
        # Add after a blank line at the end
        if not content.endswith("\n"):
            content += "\n"
        content += "\n// TAS-294: DSL variant E2E tests\n"
        content += "\n".join(additions)
        content += "\n"
        mod_file.write_text(content)
        print(f"  Updated: mod.rs (+{len(additions)} modules)")


def main():
    e2e_base = Path("tests/e2e")
    templates_base = Path("tests/fixtures/task_templates")

    for lang in ["python", "typescript", "ruby"]:
        e2e_dir = e2e_base / lang
        template_dir = templates_base / lang

        if not e2e_dir.exists():
            print(f"Skipping {lang} (E2E directory not found)")
            continue

        if not template_dir.exists():
            print(f"Skipping {lang} (template directory not found)")
            continue

        print(f"\n=== {lang.upper()} E2E Tests ===")
        new_modules = process_language(lang, e2e_dir, template_dir)

        if new_modules:
            update_mod_rs(e2e_dir, new_modules)

    print("\nDone! DSL E2E test variants created.")


if __name__ == "__main__":
    main()
