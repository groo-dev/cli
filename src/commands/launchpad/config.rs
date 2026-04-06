use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct LaunchpadConfig {
    pub name: String,
    pub root: String,
    pub description: String,
    pub domain: Option<String>,
    #[serde(default)]
    pub resources: Vec<Resource>,
    pub projects: Vec<ProjectConfig>,
    pub create_resources: bool,
    pub remote: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub project_type: ProjectType,
    #[serde(default)]
    pub features: Vec<Feature>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectType {
    Web,
    Worker,
    Ios,
    Android,
}

impl ProjectType {
    pub fn label(&self) -> &str {
        match self {
            ProjectType::Web => "web app",
            ProjectType::Worker => "worker",
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
#[serde(tag = "type")]
pub enum Feature {
    // Web features
    #[serde(rename = "tailwind")]
    Tailwind,
    #[serde(rename = "shadcn")]
    Shadcn,
    #[serde(rename = "tanstack-router")]
    TanstackRouter,
    #[serde(rename = "tanstack-query")]
    TanstackQuery,
    #[serde(rename = "axios")]
    Axios,

    // Worker features
    #[serde(rename = "hono")]
    Hono,
    #[serde(rename = "drizzle")]
    Drizzle,

    // Shared features (web + worker)
    #[serde(rename = "auth")]
    Auth { provider: AuthProvider },
    #[serde(rename = "email")]
    Email { provider: EmailProvider },
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

#[allow(dead_code)]
impl Resource {
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

    /// True if any worker has the hono feature (i.e. serves API routes)
    pub fn has_hono_worker(&self) -> bool {
        self.projects.iter().any(|p| p.has_feature_type("hono"))
    }

    /// Get the port of the first worker with hono feature
    pub fn hono_worker_port(&self, ports: &[(String, u16)]) -> Option<u16> {
        self.projects
            .iter()
            .find(|p| p.has_feature_type("hono"))
            .and_then(|p| {
                ports
                    .iter()
                    .find(|(name, _)| name == &p.name)
                    .map(|(_, port)| *port)
            })
    }

    pub fn has_resource(&self, resource: &Resource) -> bool {
        self.resources.contains(resource)
    }
}

impl ProjectConfig {
    pub fn is_worker(&self) -> bool {
        self.project_type == ProjectType::Worker
    }

    pub fn has_feature_type(&self, feature_type: &str) -> bool {
        self.features.iter().any(|f| {
            matches!(
                (f, feature_type),
                (Feature::Tailwind, "tailwind")
                    | (Feature::Shadcn, "shadcn")
                    | (Feature::TanstackRouter, "tanstack-router")
                    | (Feature::TanstackQuery, "tanstack-query")
                    | (Feature::Axios, "axios")
                    | (Feature::Hono, "hono")
                    | (Feature::Drizzle, "drizzle")
                    | (Feature::Auth { .. }, "auth")
                    | (Feature::Email { .. }, "email")
            )
        })
    }

    pub fn auth_provider(&self) -> Option<&AuthProvider> {
        self.features.iter().find_map(|f| match f {
            Feature::Auth { provider } => Some(provider),
            _ => None,
        })
    }

    pub fn email_provider(&self) -> Option<&EmailProvider> {
        self.features.iter().find_map(|f| match f {
            Feature::Email { provider } => Some(provider),
            _ => None,
        })
    }
}
