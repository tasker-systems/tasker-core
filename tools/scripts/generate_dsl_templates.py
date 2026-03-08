#!/usr/bin/env python3
"""Generate DSL task template variants from existing templates.

For each existing template, creates a _dsl variant with:
- Template name: insert _dsl before lang suffix (or append _dsl)
- Namespace name: insert _dsl before lang suffix (or append _dsl)
- Step names: insert _dsl before lang suffix (or append _dsl)
- Handler callables: point to DSL handler names
- Dependencies: updated to reference _dsl step names
- Everything else: unchanged
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


def transform_handler_callable(callable_name: str) -> str:
    """Transform handler callable to DSL equivalent.

    E.g., 'linear_workflow.step_handlers.LinearStep1Handler'
       -> 'linear_workflow_dsl.step_handlers.linear_step_1'

    For callables without clear step_handlers component, insert _dsl in first segment.
    """
    parts = callable_name.split(".")

    if len(parts) >= 2 and "step_handlers" in parts:
        # Find position of step_handlers
        sh_idx = parts.index("step_handlers")
        # Insert _dsl in the namespace part (everything before step_handlers)
        for i in range(sh_idx):
            parts[i] = insert_dsl(parts[i])
        # Convert class name to snake_case function name for parts after step_handlers
        for i in range(sh_idx + 1, len(parts)):
            parts[i] = camel_to_snake(parts[i])
        return ".".join(parts)
    elif len(parts) >= 2:
        # For task handlers and other callables, just insert _dsl in first part
        parts[0] = insert_dsl(parts[0])
        return ".".join(parts)
    else:
        return insert_dsl(callable_name)


def camel_to_snake(name: str) -> str:
    """Convert CamelCase to snake_case, removing Handler suffix."""
    # Remove Handler suffix
    if name.endswith("Handler"):
        name = name[: -len("Handler")]
    # Insert underscores before uppercase letters
    s1 = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", name)
    result = re.sub(r"([a-z\d])([A-Z])", r"\1_\2", s1).lower()
    return result


def transform_template(content: str) -> str:
    """Transform a template YAML content to DSL variant.

    Uses line-by-line processing to handle YAML without a parser,
    preserving comments and formatting exactly.
    """
    lines = content.split("\n")
    output_lines = []

    # First pass: collect all step names for dependency remapping
    step_names = []
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("- name:"):
            step_name = stripped.split(":", 1)[1].strip().strip('"').strip("'")
            # Skip special keywords like ALL and event names (contain dots)
            if step_name != "ALL" and "." not in step_name:
                step_names.append(step_name)

    # Build step name mapping
    step_map = {name: insert_dsl(name) for name in step_names}

    # Track context for multi-line processing
    in_dependencies = False
    in_env_steps = False

    for line in lines:
        stripped = line.strip()

        # Transform top-level 'name:' field (not step names)
        if re.match(r"^name:\s+", line) and not line.startswith(" "):
            val = line.split(":", 1)[1].strip().strip('"').strip("'")
            new_val = insert_dsl(val)
            output_lines.append(f"name: {new_val}")
            continue

        # Transform namespace_name
        if re.match(r"^namespace_name:\s+", line):
            val = line.split(":", 1)[1].strip().strip('"').strip("'")
            new_val = insert_dsl(val)
            output_lines.append(f"namespace_name: {new_val}")
            continue

        # Transform step name (indented '- name:')
        if stripped.startswith("- name:") and line.startswith("  "):
            val = stripped.split(":", 1)[1].strip().strip('"').strip("'")
            indent = line[: len(line) - len(line.lstrip())]
            # Skip special keywords like ALL and event names (contain dots)
            if val == "ALL" or "." in val:
                output_lines.append(line)
            else:
                new_val = insert_dsl(val)
                output_lines.append(f"{indent}- name: {new_val}")
            continue

        # Transform handler callable
        if stripped.startswith("callable:"):
            val = stripped.split(":", 1)[1].strip().strip('"').strip("'")
            indent = line[: len(line) - len(line.lstrip())]
            new_val = transform_handler_callable(val)
            output_lines.append(f"{indent}callable: {new_val}")
            continue

        # Transform dependencies (list items that reference step names)
        if stripped.startswith("- ") and not stripped.startswith("- name:"):
            dep_val = stripped[2:].strip().strip('"').strip("'")
            if dep_val in step_map:
                indent = line[: len(line) - len(line.lstrip())]
                output_lines.append(f"{indent}- {step_map[dep_val]}")
                continue

        # Transform namespace tags
        if "namespace:" in stripped and stripped.startswith("- namespace:"):
            ns_val = stripped.split("namespace:", 1)[1].strip()
            indent = line[: len(line) - len(line.lstrip())]
            new_ns = insert_dsl(ns_val)
            output_lines.append(f"{indent}- namespace:{new_ns}")
            continue

        # Transform cross_namespace tags
        if "cross_namespace:" in stripped and stripped.startswith("- cross_namespace:"):
            ns_val = stripped.split("cross_namespace:", 1)[1].strip()
            indent = line[: len(line) - len(line.lstrip())]
            new_ns = insert_dsl(ns_val)
            output_lines.append(f"{indent}- cross_namespace:{new_ns}")
            continue

        # Transform expected_results keys (they reference step names)
        for old_name, new_name in step_map.items():
            if stripped.startswith(f"{old_name}:"):
                indent = line[: len(line) - len(line.lstrip())]
                rest = stripped[len(old_name) :]
                output_lines.append(f"{indent}{new_name}{rest}")
                line = None
                break

        if line is not None:
            # Transform worker_template references
            if "worker_template" in stripped and ":" in stripped:
                for old_name, new_name in step_map.items():
                    if f'"{old_name}"' in line or f"'{old_name}'" in line or f" {old_name}" in line.rstrip():
                        line = line.replace(old_name, new_name)

            # Transform publisher references (domain events)
            if "publisher:" in stripped:
                val = stripped.split(":", 1)[1].strip().strip('"').strip("'")
                if "." in val:
                    indent = line[: len(line) - len(line.lstrip())]
                    new_val = transform_handler_callable(val)
                    output_lines.append(f"{indent}publisher: {new_val}")
                    continue

            output_lines.append(line)

    result = "\n".join(output_lines)

    # Update comment header to mention DSL
    result = result.replace(
        "# TaskTemplate Configuration",
        "# TaskTemplate Configuration - DSL Variant",
        1,
    )

    return result


def process_directory(template_dir: Path) -> None:
    """Process all YAML templates in a language directory."""
    for yaml_file in sorted(template_dir.glob("*.yaml")):
        # Skip already-generated DSL variants
        if "_dsl" in yaml_file.stem:
            continue
        content = yaml_file.read_text()
        transformed = transform_template(content)

        # Generate output filename: insert _dsl before .yaml
        stem = yaml_file.stem
        new_stem = insert_dsl(stem)
        output_file = yaml_file.parent / f"{new_stem}.yaml"

        output_file.write_text(transformed)
        print(f"  Created: {output_file.name}")


def main():
    templates_dir = Path("tests/fixtures/task_templates")

    for lang in ["python", "typescript", "ruby"]:
        lang_dir = templates_dir / lang
        if not lang_dir.exists():
            print(f"Skipping {lang} (directory not found)")
            continue

        print(f"\n=== {lang.upper()} ===")
        process_directory(lang_dir)

    print("\nDone! DSL template variants created.")


if __name__ == "__main__":
    main()
