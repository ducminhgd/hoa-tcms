//! `ProjectService` — use case orchestration for project CRUD and member management.
//!
//! Each method checks system permissions via [`AuthorizationService`], delegates to
//! the repository layer, and returns domain entities or DTOs. This service is
//! agnostic to HTTP or database details.

use crate::application::repositories::metadata_seeder::MetadataSeeder;
use crate::application::repositories::project_member_repository::{
    ProjectMemberRepository, ProjectMemberRow,
};
use crate::application::repositories::project_repository::ProjectRepository;
use crate::application::services::authorization::AuthorizationService;
use crate::application::services::errors::ServiceError;
use crate::domain::entities::project::Project;
use crate::domain::value_objects::member_role::MemberRole;
use crate::domain::value_objects::project_status::ProjectStatus;

/// Orchestrates project and member management use cases.
pub struct ProjectService {
    project_repo: Box<dyn ProjectRepository>,
    member_repo: Box<dyn ProjectMemberRepository>,
    seeder: Box<dyn MetadataSeeder>,
    auth: std::sync::Arc<AuthorizationService>,
}

impl ProjectService {
    /// Create a new `ProjectService`.
    pub fn new(
        project_repo: Box<dyn ProjectRepository>,
        member_repo: Box<dyn ProjectMemberRepository>,
        seeder: Box<dyn MetadataSeeder>,
        auth: std::sync::Arc<AuthorizationService>,
    ) -> Self {
        Self {
            project_repo,
            member_repo,
            seeder,
            auth,
        }
    }

    // ------------------------------------------------------------------
    // Project CRUD
    // ------------------------------------------------------------------

    /// Create a new project and seed its metadata.
    ///
    /// The caller must hold the `project:create` system permission.
    /// The creator is automatically added as the Owner member.
    /// All operations run in the same conceptual transaction (caller
    /// is responsible for wrapping in a DB transaction if needed).
    pub async fn create_project(
        &self,
        name: String,
        description: Option<String>,
        status: ProjectStatus,
        created_by: i64,
    ) -> Result<Project, ServiceError> {
        // Permission check.
        if !self
            .auth
            .check_permission(created_by, "project:create")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // Duplicate name check.
        if self
            .project_repo
            .find_by_name(&name)
            .await
            .map_err(ServiceError::from)?
            .is_some()
        {
            return Err(ServiceError::Conflict(
                "a project with that name already exists".into(),
            ));
        }

        let project = Project::create(name, description, status, created_by);

        // Insert project.
        let saved = self
            .project_repo
            .insert(&project)
            .await
            .map_err(ServiceError::from)?;

        // Add creator as Owner.
        self.member_repo
            .add_member(saved.id, created_by, "Owner")
            .await
            .map_err(ServiceError::from)?;

        // Seed metadata from config.
        self.seeder
            .seed(saved.id, created_by)
            .await
            .map_err(ServiceError::from)?;

        Ok(saved)
    }

    /// List projects the user is a member of (or all projects for System Admin).
    pub async fn list_projects(
        &self,
        user_id: i64,
        page: u32,
        limit: u32,
        status_filter: Option<&str>,
    ) -> Result<(Vec<Project>, u64), ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "project:read_list")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // System Admin sees all non-deleted projects.
        let is_admin = self.auth.is_admin(user_id).await?;
        let result = if is_admin {
            self.project_repo.list_all(page, limit, status_filter).await
        } else {
            self.project_repo
                .list_by_user(user_id, page, limit, status_filter)
                .await
        };

        result.map_err(ServiceError::from)
    }

    /// Get a single project by ID, including its members.
    ///
    /// The caller must hold `project:read` AND be a member of the project
    /// (or be System Admin).
    pub async fn get_project(
        &self,
        project_id: i64,
        user_id: i64,
    ) -> Result<(Project, Vec<ProjectMemberRow>), ServiceError> {
        if !self.auth.check_permission(user_id, "project:read").await? {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        let project = self
            .project_repo
            .find_by_id(project_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        let is_admin = self.auth.is_admin(user_id).await?;
        if !is_admin {
            // Check the user is a member.
            let member = self.member_repo.get_member(project_id, user_id).await?;
            if member.is_none() {
                return Err(ServiceError::PermissionDenied("forbidden".into()));
            }
        }

        let members = self
            .member_repo
            .list_by_project(project_id)
            .await
            .map_err(ServiceError::from)?;

        Ok((project, members))
    }

    /// Update project fields.
    ///
    /// Requires `project:update` system permission AND the caller must be an
    /// Owner or Editor of the project (or be System Admin).
    pub async fn update_project(
        &self,
        project_id: i64,
        name: Option<String>,
        description: Option<String>,
        status: Option<String>,
        user_id: i64,
    ) -> Result<Project, ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "project:update")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        let mut project = self
            .project_repo
            .find_by_id(project_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        let is_admin = self.auth.is_admin(user_id).await?;
        if !is_admin {
            let member = self.member_repo.get_member(project_id, user_id).await?;
            match member {
                Some(m) => {
                    let role = MemberRole::from_str(&m.role)
                        .ok_or_else(|| ServiceError::Internal("invalid member role".into()))?;
                    if !matches!(role, MemberRole::Owner | MemberRole::Editor) {
                        return Err(ServiceError::PermissionDenied("forbidden".into()));
                    }
                }
                None => {
                    return Err(ServiceError::PermissionDenied(
                        "not a project member".into(),
                    ));
                }
            }
        }

        // Check for duplicate name if name is being changed.
        if let Some(ref new_name) = name {
            if !new_name.eq_ignore_ascii_case(&project.name)
                && self
                    .project_repo
                    .find_by_name(new_name)
                    .await
                    .map_err(ServiceError::from)?
                    .is_some()
                {
                    return Err(ServiceError::Conflict(
                        "a project with that name already exists".into(),
                    ));
                }
            project.name = new_name.clone();
        }

        if let Some(ref new_status) = status {
            project.status = ProjectStatus::from_str(new_status)
                .ok_or_else(|| ServiceError::Validation("invalid status".into()))?;
        }

        if description.is_some() {
            project.description = description;
        }

        project.updated_by = user_id;

        self.project_repo
            .update(&project)
            .await
            .map_err(ServiceError::from)
    }

    /// Soft-delete a project.
    ///
    /// Requires `project:delete` system permission AND the caller must be an
    /// Owner of the project (or be System Admin).
    pub async fn delete_project(&self, project_id: i64, user_id: i64) -> Result<(), ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "project:delete")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // Verify the project exists and is not already deleted.
        self.project_repo
            .find_by_id(project_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        let is_admin = self.auth.is_admin(user_id).await?;
        if !is_admin {
            let member = self.member_repo.get_member(project_id, user_id).await?;
            match member {
                Some(m) if m.role == "Owner" => {}
                _ => return Err(ServiceError::PermissionDenied("forbidden".into())),
            }
        }

        self.project_repo
            .soft_delete(project_id, user_id)
            .await
            .map_err(ServiceError::from)
    }

    // ------------------------------------------------------------------
    // Member management
    // ------------------------------------------------------------------

    /// List all members of a project.
    pub async fn list_members(
        &self,
        project_id: i64,
        user_id: i64,
    ) -> Result<Vec<ProjectMemberRow>, ServiceError> {
        if !self.auth.check_permission(user_id, "project:read").await? {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // Verify project exists.
        self.project_repo
            .find_by_id(project_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        let is_admin = self.auth.is_admin(user_id).await?;
        if !is_admin {
            let member = self.member_repo.get_member(project_id, user_id).await?;
            if member.is_none() {
                return Err(ServiceError::PermissionDenied("forbidden".into()));
            }
        }

        self.member_repo
            .list_by_project(project_id)
            .await
            .map_err(ServiceError::from)
    }

    /// Bulk add and/or remove members from a project.
    ///
    /// Requires `project:update` system permission AND Owner role on the project.
    /// Removals are processed before additions.
    pub async fn manage_members(
        &self,
        project_id: i64,
        add_entries: &[crate::application::dto::project::AddMemberEntry],
        remove_ids: &[i64],
        user_id: i64,
    ) -> Result<ManageMembersOutcome, ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "project:update")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // Verify project exists.
        self.project_repo
            .find_by_id(project_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        let is_admin = self.auth.is_admin(user_id).await?;
        if !is_admin {
            let member = self.member_repo.get_member(project_id, user_id).await?;
            match member {
                Some(m) if m.role == "Owner" => {}
                _ => return Err(ServiceError::PermissionDenied("forbidden".into())),
            }
        }

        // Process removals first.
        let mut removed = Vec::new();
        for &uid in remove_ids {
            self.member_repo.remove_member(project_id, uid).await?;
            removed.push(uid);
        }

        // Process additions.
        let mut added = Vec::new();
        for entry in add_entries {
            self.member_repo
                .add_member(project_id, entry.user_id, &entry.role)
                .await?;
            if let Some(row) = self
                .member_repo
                .get_member(project_id, entry.user_id)
                .await?
            {
                added.push(row);
            }
        }

        Ok(ManageMembersOutcome { added, removed })
    }

    /// Change a single member's role.
    ///
    /// Requires `project:update` system permission AND Owner role on the project.
    /// Cannot downgrade the last Owner.
    pub async fn change_member_role(
        &self,
        project_id: i64,
        target_user_id: i64,
        new_role: &str,
        user_id: i64,
    ) -> Result<ProjectMemberRow, ServiceError> {
        if !self
            .auth
            .check_permission(user_id, "project:update")
            .await?
        {
            return Err(ServiceError::PermissionDenied("forbidden".into()));
        }

        // Verify project exists.
        self.project_repo
            .find_by_id(project_id)
            .await
            .map_err(ServiceError::from)?
            .ok_or(ServiceError::NotFound)?;

        let is_admin = self.auth.is_admin(user_id).await?;
        if !is_admin {
            let member = self.member_repo.get_member(project_id, user_id).await?;
            match member {
                Some(m) if m.role == "Owner" => {}
                _ => return Err(ServiceError::PermissionDenied("forbidden".into())),
            }
        }

        // If changing away from Owner, check it's not the last Owner.
        let current = self
            .member_repo
            .get_member(project_id, target_user_id)
            .await?
            .ok_or(ServiceError::NotFound)?;

        if current.role == "Owner" && new_role != "Owner" {
            let owner_count = self.member_repo.count_by_role(project_id, "Owner").await?;
            if owner_count <= 1 {
                return Err(ServiceError::Conflict(
                    "cannot change the role of the last project owner".into(),
                ));
            }
        }

        self.member_repo
            .change_role(project_id, target_user_id, new_role)
            .await?;

        self.member_repo
            .get_member(project_id, target_user_id)
            .await?
            .ok_or(ServiceError::Internal(
                "member disappeared after role change".into(),
            ))
    }
}

/// Outcome of a bulk member management operation.
#[derive(Debug)]
pub struct ManageMembersOutcome {
    pub added: Vec<ProjectMemberRow>,
    pub removed: Vec<i64>,
}
