use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct LaunchpadConfig {
    pub name: String,
    pub root: String,
    pub description: String,
    pub domain: Option<String>,
    pub projects: Vec<ProjectConfig>,
    pub create_resources: bool,
    pub remote: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub project_type: ProjectType,
    pub auth: Option<AuthProvider>,
    pub email: Option<EmailProvider>,
    #[serde(default)]
    pub resources: Vec<Resource>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectType {
    Web,
    ApiWorker,
    LightweightWorker,
    Ios,
    Android,
}

impl ProjectType {
    pub fn label(&self) -> &str {
        match self {
            ProjectType::Web => "web app",
            ProjectType::ApiWorker => "API worker",
            ProjectType::LightweightWorker => "lightweight worker",
            ProjectType::Ios => "iOS app",
            ProjectType::Android => "Android app",
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum AuthProvider {
    Clerk,
    BetterAuth,
    Simple,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum EmailProvider {
    Resend,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum Resource {
    D1,
    R2,
    Kv,
    Queues,
    AiGateway,
}

impl Resource {
    #[allow(dead_code)]
    pub fn label(&self) -> &str {
        match self {
            Resource::D1 => "D1",
            Resource::R2 => "R2",
            Resource::Kv => "KV",
            Resource::Queues => "Queues",
            Resource::AiGateway => "AI Gateway",
        }
    }
}

impl LaunchpadConfig {
    /// Derive the zone (root domain) from the application domain.
    /// e.g., "app.example.com" -> "example.com", "example.com" -> "example.com"
    pub fn zone(&self) -> Option<String> {
        self.domain.as_ref().map(|d| {
            let parts: Vec<&str> = d.split('.').collect();
            if parts.len() > 2 {
                parts[parts.len() - 2..].join(".")
            } else {
                d.clone()
            }
        })
    }

    pub fn has_api_worker(&self) -> bool {
        self.projects.iter().any(|p| p.project_type == ProjectType::ApiWorker)
    }

    pub fn api_worker_port(&self, ports: &[(String, u16)]) -> Option<u16> {
        self.projects.iter()
            .find(|p| p.project_type == ProjectType::ApiWorker)
            .and_then(|p| ports.iter().find(|(name, _)| name == &p.name).map(|(_, port)| *port))
    }
}

impl ProjectConfig {
    pub fn has_resource(&self, resource: &Resource) -> bool {
        self.resources.contains(resource)
    }

    pub fn is_worker(&self) -> bool {
        matches!(self.project_type, ProjectType::ApiWorker | ProjectType::LightweightWorker)
    }
}
