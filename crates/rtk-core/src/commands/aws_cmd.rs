use crate::{
    commands::CommandModule,
    filter::{ErrorOnly, GroupingByPattern, Strategy},
    Context,
};
use anyhow::Result;

pub struct AwsModule {
    error_strategy: ErrorOnly,
    grouping_strategy: GroupingByPattern,
}

impl AwsModule {
    pub fn new() -> Self {
        Self {
            error_strategy: ErrorOnly,
            grouping_strategy: GroupingByPattern,
        }
    }

    /// Check if output is JSON
    fn is_json_output(&self, output: &str) -> bool {
        let trimmed = output.trim();
        trimmed.starts_with('{') || trimmed.starts_with('[')
    }

    /// Check if output is a table (ASCII table format)
    fn is_table_output(&self, output: &str) -> bool {
        let lines: Vec<&str> = output.lines().take(5).collect();
        lines
            .iter()
            .any(|line| line.contains("||") || (line.contains("|") && line.split('|').count() > 2))
    }

    /// Format JSON output - condense and prettify
    fn format_json_output(&self, output: &str) -> Result<String> {
        let trimmed = output.trim();

        // Try to parse as JSON and extract summary
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
            return self.summarize_json(&json);
        }

        // Fallback: just clean up whitespace
        Ok(trimmed.to_string())
    }

    /// Summarize JSON structure
    fn summarize_json(&self, json: &serde_json::Value) -> Result<String> {
        match json {
            serde_json::Value::Object(map) => {
                let mut parts = Vec::new();

                // Check for common AWS response patterns
                if let Some(items) = map.get("Items") {
                    if let serde_json::Value::Array(arr) = items {
                        parts.push(format!("Items: {} records", arr.len()));
                        if let Some(serde_json::Value::Object(first)) = arr.first() {
                            parts.push(format!(
                                "Fields: {}",
                                first.keys().take(5).cloned().collect::<Vec<_>>().join(", ")
                            ));
                        }
                    }
                } else if let Some(buckets) = map.get("Buckets") {
                    if let serde_json::Value::Array(arr) = buckets {
                        parts.push(format!("Buckets: {}", arr.len()));
                    }
                } else if let Some(instances) = map.get("Reservations") {
                    if let serde_json::Value::Array(arr) = instances {
                        let total_instances: usize = arr
                            .iter()
                            .filter_map(|r| r.get("Instances"))
                            .filter_map(|i| i.as_array())
                            .map(|a| a.len())
                            .sum();
                        parts.push(format!("Instances: {}", total_instances));
                    }
                } else if let Some(functions) = map.get("Functions") {
                    if let serde_json::Value::Array(arr) = functions {
                        parts.push(format!("Functions: {}", arr.len()));
                    }
                } else if let Some(topics) = map.get("Topics") {
                    if let serde_json::Value::Array(arr) = topics {
                        parts.push(format!("Topics: {}", arr.len()));
                    }
                } else if let Some(queues) = map.get("QueueUrls") {
                    if let serde_json::Value::Array(arr) = queues {
                        parts.push(format!("Queues: {}", arr.len()));
                    }
                } else {
                    // Generic object - show keys
                    let keys: Vec<&str> = map.keys().take(10).map(|s| s.as_str()).collect();
                    parts.push(format!("Fields: {}", keys.join(", ")));
                }

                // Check for NextToken (pagination)
                if map.contains_key("NextToken") {
                    parts.push("(paginated - more results available)".to_string());
                }

                if parts.is_empty() {
                    parts.push("(empty response)".to_string());
                }

                Ok(parts.join("\n"))
            }
            serde_json::Value::Array(arr) => Ok(format!("Array: {} items", arr.len())),
            _ => Ok("(simple value)".to_string()),
        }
    }

    /// Compress table output
    fn compress_table_output(&self, output: &str) -> Result<String> {
        let lines: Vec<&str> = output.lines().collect();

        if lines.is_empty() {
            return Ok("(no output)".to_string());
        }

        // Find header and data rows
        let mut header: Option<String> = None;
        let mut data_rows = 0;
        let mut truncated = false;

        for line in lines.iter() {
            let trimmed = line.trim();

            // Skip empty lines and separator lines
            if trimmed.is_empty() || trimmed.starts_with("+-") || trimmed.starts_with("--") {
                continue;
            }

            // First non-separator line is likely the header
            if header.is_none() && trimmed.contains('|') {
                let cols: Vec<&str> = trimmed
                    .split('|')
                    .filter(|s| !s.trim().is_empty())
                    .collect();
                if !cols.is_empty() {
                    header = Some(
                        cols.iter()
                            .map(|s| s.trim())
                            .collect::<Vec<_>>()
                            .join(" | "),
                    );
                }
            } else if trimmed.contains('|') {
                data_rows += 1;
                if data_rows > 10 {
                    truncated = true;
                }
            }
        }

        let mut result = Vec::new();

        if let Some(h) = header {
            result.push(format!("Columns: {}", h));
        }

        result.push(format!(
            "Rows: {}{}",
            data_rows,
            if truncated { " (truncated)" } else { "" }
        ));

        Ok(result.join("\n"))
    }

    /// Extract errors from AWS output
    fn extract_errors(&self, output: &str) -> Option<String> {
        let mut errors = Vec::new();

        // First check if entire output is JSON with error
        if output.trim().starts_with('{') && output.contains("\"Error\"") {
            // Try to parse as JSON to extract error details
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(output) {
                if let Some(error_obj) = json.get("Error") {
                    if let Some(code) = error_obj.get("Code") {
                        if let Some(message) = error_obj.get("Message") {
                            return Some(format!("Error: {} - {}", code, message));
                        }
                        return Some(format!("Error: {}", code));
                    }
                }
            }
            // If JSON parsing fails, return the whole output as error
            return Some(output.to_string());
        }

        for line in output.lines() {
            let trimmed = line.trim();

            // AWS CLI error patterns
            if trimmed.contains("An error occurred")
                || trimmed.contains("Error:")
                || (trimmed.starts_with('{') && trimmed.contains("\"Error\""))
            {
                errors.push(trimmed.to_string());
            }
        }

        if errors.is_empty() {
            None
        } else {
            Some(errors.join("\n"))
        }
    }
}

impl Default for AwsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for AwsModule {
    fn name(&self) -> &str {
        "aws"
    }

    fn strategy(&self) -> &str {
        "multi_strategy"
    }

    fn compress(&self, output: &str, context: &Context) -> Result<String> {
        // Check for errors first
        if let Some(errors) = self.extract_errors(output) {
            return Ok(errors);
        }

        // On non-zero exit, use error strategy
        if context.exit_code != 0 {
            return self.error_strategy.compress(output);
        }

        if output.is_empty() {
            return Ok("(no output)".to_string());
        }

        // Detect output format
        if self.is_json_output(output) {
            return self.format_json_output(output);
        }

        if self.is_table_output(output) {
            return self.compress_table_output(output);
        }

        // Default: use grouping for repetitive content
        self.grouping_strategy.compress(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context(command: &str, exit_code: i32) -> Context {
        Context {
            cwd: "/tmp".to_string(),
            exit_code,
            tool: "bash".to_string(),
            session_id: None,
            command: Some(command.to_string()),
        }
    }

    #[test]
    fn test_aws_s3_ls_json() {
        let module = AwsModule::new();
        let input = r#"{
    "Buckets": [
        {"Name": "bucket1", "CreationDate": "2024-01-01"},
        {"Name": "bucket2", "CreationDate": "2024-01-02"},
        {"Name": "bucket3", "CreationDate": "2024-01-03"}
    ]
}"#;
        let result = module
            .compress(input, &make_context("aws s3api list-buckets", 0))
            .unwrap();

        assert!(result.contains("Buckets: 3"));
    }

    #[test]
    fn test_aws_error() {
        let module = AwsModule::new();
        let input = r#"{
    "Error": {
        "Code": "NoSuchBucket",
        "Message": "The specified bucket does not exist"
    }
}"#;
        let result = module
            .compress(
                input,
                &make_context("aws s3api get-bucket --bucket nonexistent", 254),
            )
            .unwrap();

        assert!(result.contains("Error") || result.contains("NoSuchBucket"));
    }

    #[test]
    fn test_aws_dynamodb_scan() {
        let module = AwsModule::new();
        let input = r#"{
    "Items": [
        {"id": "1", "name": "Item 1"},
        {"id": "2", "name": "Item 2"},
        {"id": "3", "name": "Item 3"}
    ],
    "Count": 3
}"#;
        let result = module
            .compress(
                input,
                &make_context("aws dynamodb scan --table-name MyTable", 0),
            )
            .unwrap();

        assert!(result.contains("Items: 3 records"));
    }

    #[test]
    fn test_aws_paginated_response() {
        let module = AwsModule::new();
        let input = r#"{
    "Items": [
        {"id": "1"}
    ],
    "NextToken": "eyJ..."
}"#;
        let result = module
            .compress(
                input,
                &make_context("aws dynamodb scan --table-name MyTable", 0),
            )
            .unwrap();

        assert!(result.contains("paginated"));
    }

    #[test]
    fn test_aws_table_output() {
        let module = AwsModule::new();
        let input = r#"---------------------------------------------
|              ListBuckets                   |
---------------------------------------------
|            Name            |   CreationDate|
---------------------------------------------
|  bucket1                   |  2024-01-01   |
|  bucket2                   |  2024-01-02   |
|  bucket3                   |  2024-01-03   |
---------------------------------------------
"#;
        let result = module
            .compress(
                input,
                &make_context("aws s3api list-buckets --output table", 0),
            )
            .unwrap();

        assert!(result.contains("Rows: 3") || result.contains("Columns:"));
    }

    #[test]
    fn test_aws_empty_output() {
        let module = AwsModule::new();
        let result = module.compress("", &make_context("aws", 0)).unwrap();

        assert_eq!(result, "(no output)");
    }

    #[test]
    fn test_aws_lambda_list() {
        let module = AwsModule::new();
        let input = r#"{
    "Functions": [
        {"FunctionName": "func1", "Runtime": "python3.9"},
        {"FunctionName": "func2", "Runtime": "nodejs18.x"}
    ]
}"#;
        let result = module
            .compress(input, &make_context("aws lambda list-functions", 0))
            .unwrap();

        assert!(result.contains("Functions: 2"));
    }

    #[test]
    fn test_aws_ec2_describe() {
        let module = AwsModule::new();
        let input = r#"{
    "Reservations": [
        {
            "Instances": [
                {"InstanceId": "i-123", "State": {"Name": "running"}}
            ]
        },
        {
            "Instances": [
                {"InstanceId": "i-456", "State": {"Name": "stopped"}}
            ]
        }
    ]
}"#;
        let result = module
            .compress(input, &make_context("aws ec2 describe-instances", 0))
            .unwrap();

        assert!(result.contains("Instances: 2"));
    }

    #[test]
    fn test_aws_sns_list() {
        let module = AwsModule::new();
        let input = r#"{
    "Topics": [
        {"TopicArn": "arn:aws:sns:us-east-1:123:topic1"},
        {"TopicArn": "arn:aws:sns:us-east-1:123:topic2"}
    ]
}"#;
        let result = module
            .compress(input, &make_context("aws sns list-topics", 0))
            .unwrap();

        assert!(result.contains("Topics: 2"));
    }

    #[test]
    fn test_aws_sqs_list() {
        let module = AwsModule::new();
        let input = r#"{
    "QueueUrls": [
        "https://sqs.us-east-1.amazonaws.com/123/queue1",
        "https://sqs.us-east-1.amazonaws.com/123/queue2",
        "https://sqs.us-east-1.amazonaws.com/123/queue3"
    ]
}"#;
        let result = module
            .compress(input, &make_context("aws sqs list-queues", 0))
            .unwrap();

        assert!(result.contains("Queues: 3"));
    }

    #[test]
    fn test_aws_json_array() {
        let module = AwsModule::new();
        let input = r#"["item1", "item2", "item3"]"#;
        let result = module
            .compress(input, &make_context("aws some-command", 0))
            .unwrap();

        assert!(result.contains("Array: 3 items"));
    }

    #[test]
    fn test_aws_generic_object() {
        let module = AwsModule::new();
        let input = r#"{
    "SomeField": "value",
    "AnotherField": 123,
    "ThirdField": true
}"#;
        let result = module
            .compress(input, &make_context("aws some-command", 0))
            .unwrap();

        assert!(result.contains("Fields:"));
    }
}
