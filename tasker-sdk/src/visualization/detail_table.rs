//! Markdown detail table generation from TaskTemplate.

use std::collections::HashMap;
use std::fmt::Write;

use tasker_shared::models::core::task_template::TaskTemplate;

pub(super) fn generate_detail_table(template: &TaskTemplate) -> String {
    let mut out = String::new();

    // Header
    writeln!(
        out,
        "| Step | Type | Handler | Dependencies | Schema Fields | Retry |"
    )
    .unwrap();
    writeln!(
        out,
        "|------|------|---------|--------------|---------------|-------|"
    )
    .unwrap();

    // Build step lookup for topological ordering
    let order = topological_order(template);

    for name in &order {
        let Some(step) = template.steps.iter().find(|s| &s.name == name) else {
            continue;
        };

        let step_type = format!("{:?}", step.step_type);
        let handler = &step.handler.callable;
        let deps = if step.dependencies.is_empty() {
            "\u{2014}".to_string()
        } else {
            step.dependencies.join(", ")
        };
        let schema_fields = extract_schema_field_names(&step.result_schema);
        let retry = format_retry(&step.retry);

        writeln!(
            out,
            "| {name} | {step_type} | {handler} | {deps} | {schema_fields} | {retry} |"
        )
        .unwrap();
    }

    out
}

fn topological_order(template: &TaskTemplate) -> Vec<String> {
    use std::collections::VecDeque;

    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();

    for step in &template.steps {
        in_degree.entry(&step.name).or_insert(0);
        for dep in &step.dependencies {
            adj.entry(dep.as_str()).or_default().push(&step.name);
            *in_degree.entry(&step.name).or_insert(0) += 1;
        }
    }

    let mut queue: VecDeque<&str> = {
        let mut roots: Vec<&str> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&name, _)| name)
            .collect();
        roots.sort();
        roots.into_iter().collect()
    };

    let mut result = Vec::new();
    while let Some(node) = queue.pop_front() {
        result.push(node.to_string());
        if let Some(neighbors) = adj.get(node) {
            let mut next: Vec<&str> = Vec::new();
            for &neighbor in neighbors {
                if let Some(deg) = in_degree.get_mut(neighbor) {
                    *deg -= 1;
                    if *deg == 0 {
                        next.push(neighbor);
                    }
                }
            }
            next.sort();
            queue.extend(next);
        }
    }

    result
}

fn extract_schema_field_names(schema: &Option<serde_json::Value>) -> String {
    let Some(schema) = schema else {
        return "\u{2014}".to_string();
    };
    let Some(props) = schema.get("properties").and_then(|p| p.as_object()) else {
        return "\u{2014}".to_string();
    };
    let mut names: Vec<&str> = props.keys().map(|k| k.as_str()).collect();
    names.sort();
    if names.is_empty() {
        "\u{2014}".to_string()
    } else {
        names.join(", ")
    }
}

fn format_retry(retry: &tasker_shared::models::core::task_template::RetryConfiguration) -> String {
    if !retry.retryable {
        return "\u{2014}".to_string();
    }
    // Default retry config (retryable=true, 3 attempts, exponential) is the norm —
    // only show non-default configs
    if retry.max_attempts == 3
        && retry.backoff == tasker_shared::models::core::task_template::BackoffStrategy::Exponential
    {
        return "\u{2014}".to_string();
    }
    format!("{}x {:?}", retry.max_attempts, retry.backoff).to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template_parser::parse_template_str;

    #[test]
    fn test_detail_table_has_header_row() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let result = generate_detail_table(&template);

        assert!(result.contains("| Step | Type | Handler | Dependencies | Schema Fields | Retry |"));
        assert!(result.contains("|------|------|---------|"));
    }

    #[test]
    fn test_detail_table_rows_in_topological_order() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let result = generate_detail_table(&template);

        // validate_order should appear before enrich_order (it's a dependency)
        let validate_pos = result.find("validate_order").unwrap();
        let enrich_pos = result.find("enrich_order").unwrap();
        assert!(validate_pos < enrich_pos);
    }

    #[test]
    fn test_detail_table_shows_schema_fields() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let result = generate_detail_table(&template);

        // validate_order has result_schema with properties
        let lines: Vec<&str> = result.lines().collect();
        let validate_line = lines
            .iter()
            .find(|l| l.contains("| validate_order"))
            .unwrap();
        // Should contain schema field names from result_schema
        assert!(validate_line.contains("validated"));
        assert!(validate_line.contains("order_total"));
    }

    #[test]
    fn test_detail_table_root_step_shows_dash_for_deps() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(yaml).unwrap();
        let result = generate_detail_table(&template);

        let lines: Vec<&str> = result.lines().collect();
        let validate_line = lines
            .iter()
            .find(|l| l.contains("| validate_order"))
            .unwrap();
        // Root step has no dependencies
        assert!(validate_line.contains("| \u{2014} |"));
    }

    #[test]
    fn test_detail_table_shows_retry_config() {
        let yaml = r#"
name: retry_test
namespace_name: test
version: "1.0.0"
steps:
  - name: step_with_retry
    handler:
      callable: test.retry_step
    retry:
      retryable: true
      max_attempts: 5
      backoff: exponential
  - name: step_no_retry
    handler:
      callable: test.no_retry
    retry:
      retryable: false
    depends_on: [step_with_retry]
"#;
        let template = parse_template_str(yaml).unwrap();
        let result = generate_detail_table(&template);

        assert!(result.contains("5x exponential"));
        assert!(result.contains("\u{2014}")); // no-retry step
    }
}
