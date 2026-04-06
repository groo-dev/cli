use serde::Deserialize;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct LaunchpadConfig {
    pub name: String,
    pub root: String,
    pub description: String,
    pub domain: Option<String>,
    pub projects: Vec<ProjectConfig>,
    pub create_resources: bool,
    pub remote: bool,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub project_type: ProjectType,
    pub auth: Option<AuthProvider>,
    pub email: Option<EmailProvider>,
    #[serde(default)]
    pub resources: Vec<Resource>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectType {
    Web,
    ApiWorker,
    LightweightWorker,
    Ios,
    Android,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum AuthProvider {
    Clerk,
    BetterAuth,
    Simple,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum EmailProvider {
    Resend,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum Resource {
    D1,
    R2,
    Kv,
    Queues,
    AiGateway,
}
