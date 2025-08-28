//! Provider별 Webhook 페이로드 파서
//!
//! 각 외부 시스템(Linear, Jira, GitHub 등)의 webhook 페이로드를
//! 표준 TaskData 형식으로 변환합니다.

use super::dto::TaskData;
use serde_json::Value;
use std::collections::HashMap;

/// Provider별 파서 트레이트
pub trait ProviderParser {
    fn parse(&self, payload: &Value) -> Result<TaskData, String>;
    fn provider_name(&self) -> &'static str;
}

/// Linear 파서
pub struct LinearParser;

impl ProviderParser for LinearParser {
    fn provider_name(&self) -> &'static str {
        "linear"
    }

    fn parse(&self, payload: &Value) -> Result<TaskData, String> {
        // Linear webhook 구조: { "data": { "issue": { ... } } }
        let issue = payload
            .get("data")
            .and_then(|d| d.get("issue"))
            .or_else(|| payload.get("task")) // 테스트용 직접 task 구조도 지원
            .ok_or("Missing issue data in Linear payload")?;

        let id = issue
            .get("id")
            .or_else(|| issue.get("identifier"))
            .and_then(|v| v.as_str())
            .ok_or("Missing issue ID")?
            .to_string();

        let title = issue
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or("Missing issue title")?
            .to_string();

        let body = issue
            .get("description")
            .or_else(|| issue.get("body"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let url = issue
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Linear labels 처리
        let labels = issue
            .get("labels")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|label| {
                        label
                            .get("name")
                            .and_then(|v| v.as_str())
                            .or_else(|| label.as_str())
                    })
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        // Linear assignee 처리
        let assignees = issue
            .get("assignee")
            .and_then(|v| v.get("email"))
            .and_then(|v| v.as_str())
            .map(|email| vec![email.to_string()])
            .or_else(|| {
                issue
                    .get("assignees")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|assignee| {
                                assignee
                                    .get("email")
                                    .and_then(|v| v.as_str())
                                    .or_else(|| assignee.as_str())
                            })
                            .map(|s| s.to_string())
                            .collect()
                    })
            })
            .unwrap_or_default();

        Ok(TaskData {
            id,
            title,
            body,
            url,
            labels,
            assignees,
        })
    }
}

/// GitHub 파서
pub struct GitHubParser;

impl ProviderParser for GitHubParser {
    fn provider_name(&self) -> &'static str {
        "github"
    }

    fn parse(&self, payload: &Value) -> Result<TaskData, String> {
        // GitHub webhook 구조: { "issue": { ... } } 또는 { "pull_request": { ... } }
        let item = payload
            .get("issue")
            .or_else(|| payload.get("pull_request"))
            .or_else(|| payload.get("task")) // 테스트용 직접 task 구조도 지원
            .ok_or("Missing issue or pull_request data in GitHub payload")?;

        let id = item
            .get("number")
            .and_then(|v| v.as_u64())
            .map(|n| format!("#{}", n))
            .or_else(|| {
                item.get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .ok_or("Missing issue/PR number")?;

        let title = item
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or("Missing issue/PR title")?
            .to_string();

        let body = item
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let url = item
            .get("html_url")
            .or_else(|| item.get("url"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // GitHub labels 처리
        let labels = item
            .get("labels")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|label| {
                        label
                            .get("name")
                            .and_then(|v| v.as_str())
                            .or_else(|| label.as_str())
                    })
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        // GitHub assignees 처리
        let assignees = item
            .get("assignees")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|assignee| {
                        assignee
                            .get("login")
                            .and_then(|v| v.as_str())
                            .or_else(|| assignee.as_str())
                    })
                    .map(|s| s.to_string())
                    .collect()
            })
            .or_else(|| {
                item.get("assignee")
                    .and_then(|v| v.get("login"))
                    .and_then(|v| v.as_str())
                    .map(|login| vec![login.to_string()])
            })
            .unwrap_or_default();

        Ok(TaskData {
            id,
            title,
            body,
            url,
            labels,
            assignees,
        })
    }
}

/// Jira 파서
pub struct JiraParser;

impl ProviderParser for JiraParser {
    fn provider_name(&self) -> &'static str {
        "jira"
    }

    fn parse(&self, payload: &Value) -> Result<TaskData, String> {
        // Jira webhook 구조: { "issue": { "key": "...", "fields": { ... } } }
        let issue = payload
            .get("issue")
            .or_else(|| payload.get("task")) // 테스트용 직접 task 구조도 지원
            .ok_or("Missing issue data in Jira payload")?;

        let id = issue
            .get("key")
            .or_else(|| issue.get("id"))
            .and_then(|v| v.as_str())
            .ok_or("Missing issue key")?
            .to_string();

        let fields = issue.get("fields").unwrap_or(issue);

        let title = fields
            .get("summary")
            .or_else(|| fields.get("title"))
            .and_then(|v| v.as_str())
            .ok_or("Missing issue summary")?
            .to_string();

        let body = fields
            .get("description")
            .or_else(|| fields.get("body"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let url = issue
            .get("self")
            .or_else(|| issue.get("url"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Jira labels 처리
        let labels = fields
            .get("labels")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        // Jira assignee 처리
        let assignees = fields
            .get("assignee")
            .and_then(|v| v.get("emailAddress"))
            .and_then(|v| v.as_str())
            .map(|email| vec![email.to_string()])
            .or_else(|| {
                fields
                    .get("assignees")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|assignee| {
                                assignee
                                    .get("emailAddress")
                                    .and_then(|v| v.as_str())
                                    .or_else(|| assignee.as_str())
                            })
                            .map(|s| s.to_string())
                            .collect()
                    })
            })
            .unwrap_or_default();

        Ok(TaskData {
            id,
            title,
            body,
            url,
            labels,
            assignees,
        })
    }
}

/// 기본 파서 (테스트용)
pub struct DefaultParser;

impl ProviderParser for DefaultParser {
    fn provider_name(&self) -> &'static str {
        "default"
    }

    fn parse(&self, payload: &Value) -> Result<TaskData, String> {
        // 직접 TaskData 구조를 기대
        let task = payload.get("task").ok_or("Missing task data in payload")?;

        serde_json::from_value(task.clone())
            .map_err(|e| format!("Failed to parse task data: {}", e))
    }
}

/// Provider 파서 팩토리
pub struct ProviderParserFactory {
    parsers: HashMap<String, Box<dyn ProviderParser>>,
}

impl ProviderParserFactory {
    pub fn new() -> Self {
        let mut parsers: HashMap<String, Box<dyn ProviderParser>> = HashMap::new();

        parsers.insert("linear".to_string(), Box::new(LinearParser));
        parsers.insert("github".to_string(), Box::new(GitHubParser));
        parsers.insert("jira".to_string(), Box::new(JiraParser));
        parsers.insert("default".to_string(), Box::new(DefaultParser));

        Self { parsers }
    }

    pub fn get_parser(&self, provider: &str) -> Option<&dyn ProviderParser> {
        self.parsers.get(provider).map(|p| p.as_ref())
    }

    pub fn parse_payload(&self, provider: &str, payload: &Value) -> Result<TaskData, String> {
        let parser = self
            .get_parser(provider)
            .or_else(|| self.get_parser("default"))
            .ok_or_else(|| format!("No parser available for provider: {}", provider))?;

        parser.parse(payload)
    }
}

impl Default for ProviderParserFactory {
    fn default() -> Self {
        Self::new()
    }
}
