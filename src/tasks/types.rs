use serde::{Deserialize, Serialize};

/// Project from tasks API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Task from tasks API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub tags: Option<Vec<String>>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Backlog,
    Open,
    InProgress,
    Done,
    Archived,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Backlog => write!(f, "backlog"),
            TaskStatus::Open => write!(f, "open"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Done => write!(f, "done"),
            TaskStatus::Archived => write!(f, "archived"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskPriority {
    Low,
    Medium,
    High,
}

impl std::fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskPriority::Low => write!(f, "low"),
            TaskPriority::Medium => write!(f, "medium"),
            TaskPriority::High => write!(f, "high"),
        }
    }
}

/// Comment on a task
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    pub task_id: String,
    pub content: String,
    pub author: Option<String>,
    pub created_at: String,
}

// API Response types

#[derive(Debug, Deserialize)]
pub struct ProjectsResponse {
    pub projects: Vec<Project>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ProjectResponse {
    pub project: Project,
    #[serde(rename = "taskCounts")]
    pub task_counts: Option<TaskCounts>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCounts {
    pub backlog: u32,
    pub open: u32,
    pub in_progress: u32,
    pub done: u32,
    pub archived: u32,
    pub total: u32,
}

#[derive(Debug, Deserialize)]
pub struct TasksResponse {
    pub tasks: Vec<Task>,
}

#[derive(Debug, Deserialize)]
pub struct TaskResponse {
    pub task: Task,
    pub comments: Option<Vec<Comment>>,
    pub project: Option<Project>,
}

#[derive(Debug, Deserialize)]
pub struct CommentResponse {
    pub comment: Comment,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SuccessResponse {
    pub success: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: String,
    #[serde(default)]
    pub code: Option<String>,
    pub project_name: Option<String>,
}

// Request types

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRequest {
    pub project_name: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct CreateCommentRequest {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
