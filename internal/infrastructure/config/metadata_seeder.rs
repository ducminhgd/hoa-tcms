//! `ConfigFileSeeder` — seeds per-project metadata from a YAML configuration file.
//!
//! Reads the default metadata configuration at startup and copies it into
//! per-project metadata tables when a new project is created.

use async_trait::async_trait;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use serde::Deserialize;

use crate::application::repositories::metadata_seeder::MetadataSeeder;
use crate::application::repositories::{RepositoryError, RepositoryResult};
use crate::infrastructure::db::entities::{test_case_templates, test_categories};

/// YAML structure for a single category.
#[derive(Debug, Clone, Deserialize)]
struct CategoryConfig {
    name: String,
    #[serde(default)]
    description: Option<String>,
}

/// YAML structure for a single template.
#[derive(Debug, Clone, Deserialize)]
struct TemplateConfig {
    name: String,
    #[serde(default)]
    content: Option<String>,
}

/// Root structure of the default-metadata.yaml file.
#[derive(Debug, Clone, Deserialize)]
struct MetadataConfig {
    #[serde(default)]
    categories: Vec<CategoryConfig>,
    #[serde(default)]
    templates: Vec<TemplateConfig>,
}

/// Seeds per-project metadata from a YAML configuration file.
///
/// The config file is loaded once at construction time. Each call to
/// `seed()` inserts the pre-loaded defaults into the given project's
/// per-project metadata tables.
pub struct ConfigFileSeeder {
    db: DatabaseConnection,
    config: MetadataConfig,
}

impl ConfigFileSeeder {
    /// Load the metadata config from `config_path` and create a new seeder.
    ///
    /// If the config file is missing or malformed, the seeder is created
    /// with an empty configuration (a warning is logged).
    pub async fn new(db: DatabaseConnection, config_path: &str) -> Self {
        let config = match Self::load_config(config_path) {
            Ok(cfg) => {
                tracing::info!(
                    categories = cfg.categories.len(),
                    templates = cfg.templates.len(),
                    "loaded default metadata configuration from {}",
                    config_path
                );
                cfg
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    path = config_path,
                    "failed to load metadata configuration; seeding will be empty"
                );
                MetadataConfig {
                    categories: Vec::new(),
                    templates: Vec::new(),
                }
            }
        };

        Self { db, config }
    }

    fn load_config(path: &str) -> Result<MetadataConfig, String> {
        let contents =
            std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {}", path, e))?;

        serde_yaml::from_str::<MetadataConfig>(&contents)
            .map_err(|e| format!("invalid YAML in {}: {}", path, e))
    }
}

#[async_trait]
impl MetadataSeeder for ConfigFileSeeder {
    async fn seed(&self, project_id: i64, created_by: i64) -> RepositoryResult<()> {
        // Seed categories.
        for cat in &self.config.categories {
            test_categories::ActiveModel {
                project_id: Set(project_id),
                name: Set(cat.name.clone()),
                description: Set(cat.description.clone()),
                created_by: Set(created_by),
                updated_by: Set(created_by),
                ..Default::default()
            }
            .insert(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        }

        // Seed templates.
        for tmpl in &self.config.templates {
            test_case_templates::ActiveModel {
                project_id: Set(project_id),
                name: Set(tmpl.name.clone()),
                template_content: Set(tmpl.content.clone()),
                created_by: Set(created_by),
                updated_by: Set(created_by),
                ..Default::default()
            }
            .insert(&self.db)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        }

        tracing::info!(
            project_id,
            categories = self.config.categories.len(),
            templates = self.config.templates.len(),
            "seeded default metadata for project"
        );

        Ok(())
    }
}
