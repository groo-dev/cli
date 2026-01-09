use anyhow::{Context, Result};
use reqwest::Client;

use super::types::*;

const TASKS_API_URL: &str = "https://tasks.groo.dev";
#[allow(dead_code)]
const TASKS_API_DEV_URL: &str = "http://localhost:29985";

pub struct TasksClient {
    client: Client,
    token: String,
    base_url: String,
}

impl TasksClient {
    pub fn new(token: String) -> Self {
        Self {
            client: Client::new(),
            token,
            base_url: TASKS_API_URL.to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn with_dev_mode(token: String) -> Self {
        Self {
            client: Client::new(),
            token,
            base_url: TASKS_API_DEV_URL.to_string(),
        }
    }

    // ==========================================================================
    // Projects
    // ==========================================================================

    /// List all projects
    pub async fn list_projects(&self) -> Result<Vec<Project>> {
        let response = self
            .client
            .get(format!("{}/v1/projects", self.base_url))
            .header("Cookie", format!("session={}", self.token))
            .send()
            .await
            .context("Failed to fetch projects")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        let data: ProjectsResponse = response.json().await?;
        Ok(data.projects)
    }

    /// Get a single project by ID
    #[allow(dead_code)]
    pub async fn get_project(&self, project_id: &str) -> Result<ProjectResponse> {
        let response = self
            .client
            .get(format!("{}/v1/projects/{}", self.base_url, project_id))
            .header("Cookie", format!("session={}", self.token))
            .send()
            .await
            .context("Failed to fetch project")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        response.json().await.context("Failed to parse project")
    }

    /// Create a new project
    #[allow(dead_code)]
    pub async fn create_project(&self, request: CreateProjectRequest) -> Result<Project> {
        let response = self
            .client
            .post(format!("{}/v1/projects", self.base_url))
            .header("Cookie", format!("session={}", self.token))
            .json(&request)
            .send()
            .await
            .context("Failed to create project")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        let data: ProjectResponse = response.json().await?;
        Ok(data.project)
    }

    // ==========================================================================
    // Tasks
    // ==========================================================================

    /// List tasks with optional filters
    pub async fn list_tasks(
        &self,
        project_id: Option<&str>,
        status: Option<&str>,
        priority: Option<&str>,
        include_archived: bool,
    ) -> Result<Vec<Task>> {
        let mut url = format!("{}/v1/tasks", self.base_url);
        let mut params = Vec::new();

        if let Some(p) = project_id {
            params.push(format!("project={}", p));
        }
        if let Some(s) = status {
            params.push(format!("status={}", s));
        }
        if let Some(p) = priority {
            params.push(format!("priority={}", p));
        }
        if include_archived {
            params.push("all=true".to_string());
        }

        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }

        let response = self
            .client
            .get(&url)
            .header("Cookie", format!("session={}", self.token))
            .send()
            .await
            .context("Failed to fetch tasks")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        let data: TasksResponse = response.json().await?;
        Ok(data.tasks)
    }

    /// Search tasks by query
    pub async fn search_tasks(&self, query: &str) -> Result<Vec<Task>> {
        let response = self
            .client
            .get(format!("{}/v1/tasks/search?q={}", self.base_url, urlencoding::encode(query)))
            .header("Cookie", format!("session={}", self.token))
            .send()
            .await
            .context("Failed to search tasks")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        let data: TasksResponse = response.json().await?;
        Ok(data.tasks)
    }

    /// Get a single task with comments
    pub async fn get_task(&self, task_id: &str) -> Result<TaskResponse> {
        let response = self
            .client
            .get(format!("{}/v1/tasks/{}", self.base_url, task_id))
            .header("Cookie", format!("session={}", self.token))
            .send()
            .await
            .context("Failed to fetch task")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        response.json().await.context("Failed to parse task")
    }

    /// Create a new task
    pub async fn create_task(&self, request: CreateTaskRequest) -> Result<Task> {
        let response = self
            .client
            .post(format!("{}/v1/tasks", self.base_url))
            .header("Cookie", format!("session={}", self.token))
            .json(&request)
            .send()
            .await
            .context("Failed to create task")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        let data: TaskResponse = response.json().await?;
        Ok(data.task)
    }

    /// Update a task
    #[allow(dead_code)]
    pub async fn update_task(&self, task_id: &str, request: UpdateTaskRequest) -> Result<Task> {
        let response = self
            .client
            .put(format!("{}/v1/tasks/{}", self.base_url, task_id))
            .header("Cookie", format!("session={}", self.token))
            .json(&request)
            .send()
            .await
            .context("Failed to update task")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        let data: TaskResponse = response.json().await?;
        Ok(data.task)
    }

    /// Delete a task
    #[allow(dead_code)]
    pub async fn delete_task(&self, task_id: &str) -> Result<()> {
        let response = self
            .client
            .delete(format!("{}/v1/tasks/{}", self.base_url, task_id))
            .header("Cookie", format!("session={}", self.token))
            .send()
            .await
            .context("Failed to delete task")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        Ok(())
    }

    // ==========================================================================
    // Quick status changes
    // ==========================================================================

    /// Start a task (set to in_progress)
    pub async fn start_task(&self, task_id: &str) -> Result<Task> {
        let response = self
            .client
            .post(format!("{}/v1/tasks/{}/start", self.base_url, task_id))
            .header("Cookie", format!("session={}", self.token))
            .send()
            .await
            .context("Failed to start task")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        let data: TaskResponse = response.json().await?;
        Ok(data.task)
    }

    /// Complete a task (set to done)
    pub async fn complete_task(&self, task_id: &str) -> Result<Task> {
        let response = self
            .client
            .post(format!("{}/v1/tasks/{}/done", self.base_url, task_id))
            .header("Cookie", format!("session={}", self.token))
            .send()
            .await
            .context("Failed to complete task")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        let data: TaskResponse = response.json().await?;
        Ok(data.task)
    }

    /// Archive a task
    pub async fn archive_task(&self, task_id: &str) -> Result<Task> {
        let response = self
            .client
            .post(format!("{}/v1/tasks/{}/archive", self.base_url, task_id))
            .header("Cookie", format!("session={}", self.token))
            .send()
            .await
            .context("Failed to archive task")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        let data: TaskResponse = response.json().await?;
        Ok(data.task)
    }

    // ==========================================================================
    // Comments
    // ==========================================================================

    /// Add a comment to a task
    pub async fn add_comment(&self, task_id: &str, request: CreateCommentRequest) -> Result<Comment> {
        let response = self
            .client
            .post(format!("{}/v1/tasks/{}/comments", self.base_url, task_id))
            .header("Cookie", format!("session={}", self.token))
            .json(&request)
            .send()
            .await
            .context("Failed to add comment")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            anyhow::bail!("{}", error.error);
        }

        let data: CommentResponse = response.json().await?;
        Ok(data.comment)
    }
}
