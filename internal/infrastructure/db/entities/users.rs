//! SeaORM entity for the `users` table.
//!
//! NOTE: The trigger_set_updated_at trigger does NOT apply to `users` due to
//! the chicken-and-egg problem with the self-referencing FK. The application
//! sets updated_at and updated_by explicitly.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub fullname: String,
    pub status: String,
    pub created_by: Option<i64>,
    pub created_at: DateTime,
    pub updated_by: Option<i64>,
    pub updated_at: DateTime,
    pub deleted_by: Option<i64>,
    pub deleted_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
