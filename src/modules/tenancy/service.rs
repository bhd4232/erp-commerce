use serde_json::{json, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::dtos::{
    InviteRequest, PendingInvitation, TeamMember, TenantSummary, UpdateMemberRequest,
};
use super::models::{InvitationRow, MemberRow};
use crate::common::tenant::{TenantContext, TenantRole};
use crate::error::AppError;
use crate::state::AppState;

/// Creates the tenant, the owner membership and the first audit row inside the
/// caller's transaction. Auth registration calls this so signup is atomic.
pub async fn provision_owner(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    business_name: &str,
) -> Result<TenantSummary, AppError> {
    let name = business_name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(AppError::Validation(
            "business_name must be 1-120 characters".into(),
        ));
    }

    let slug = unique_slug(name);
    let tenant: (Uuid, String, String, String) = sqlx::query_as(
        r#"
        INSERT INTO tenants (name, slug)
        VALUES ($1, $2)
        RETURNING id, name, slug, status::text
        "#,
    )
    .bind(name)
    .bind(&slug)
    .fetch_one(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO memberships (tenant_id, user_id, role, status)
        VALUES ($1, $2, 'owner', 'active')
        "#,
    )
    .bind(tenant.0)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    insert_audit(
        tx,
        tenant.0,
        Some(user_id),
        "tenant",
        tenant.0,
        "tenant.created",
        None,
        Some(json!({ "name": tenant.1, "slug": tenant.2 })),
    )
    .await?;

    Ok(TenantSummary {
        id: tenant.0,
        name: tenant.1,
        slug: tenant.2,
        role: TenantRole::Owner,
        status: tenant.3,
    })
}

pub async fn list_tenants(state: &AppState, user_id: Uuid) -> Result<Vec<TenantSummary>, AppError> {
    let rows: Vec<(Uuid, String, String, String, String)> = sqlx::query_as(
        r#"
        SELECT t.id, t.name, t.slug, m.role::text, t.status::text
          FROM memberships m
          JOIN tenants t ON t.id = m.tenant_id
         WHERE m.user_id = $1
           AND m.status = 'active'
           AND t.status = 'active'
         ORDER BY t.created_at
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    rows.into_iter()
        .map(|(id, name, slug, role, status)| {
            Ok(TenantSummary {
                id,
                name,
                slug,
                role: parse_role(&role)?,
                status,
            })
        })
        .collect()
}

pub async fn current_tenant(ctx: &TenantContext) -> TenantSummary {
    TenantSummary {
        id: ctx.tenant_id,
        name: ctx.tenant_name.clone(),
        slug: ctx.tenant_slug.clone(),
        role: ctx.role,
        status: "active".into(),
    }
}

pub async fn list_members(
    state: &AppState,
    ctx: &TenantContext,
) -> Result<Vec<TeamMember>, AppError> {
    let rows: Vec<MemberRow> = sqlx::query_as(
        r#"
        SELECT m.user_id, u.email, u.full_name, m.role::text, m.status::text, m.created_at
          FROM memberships m
          JOIN users u ON u.id = m.user_id
         WHERE m.tenant_id = $1
         ORDER BY m.created_at
        "#,
    )
    .bind(ctx.tenant_id)
    .fetch_all(&state.db)
    .await?;

    rows.into_iter().map(member_from_row).collect()
}

pub async fn invite(
    state: &AppState,
    ctx: &TenantContext,
    payload: InviteRequest,
) -> Result<TeamMember, AppError> {
    ctx.require_manager()?;

    if payload.role == TenantRole::Owner {
        return Err(AppError::Unprocessable(
            "the owner role cannot be invited".into(),
        ));
    }

    let email = payload.email.trim().to_lowercase();
    if !email.contains('@') || email.len() > 255 {
        return Err(AppError::Validation("a valid email is required".into()));
    }

    let user: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM users WHERE email = $1 AND is_active = true")
            .bind(&email)
            .fetch_optional(&state.db)
            .await?;
    let user_id = user
        .map(|(id,)| id)
        .ok_or_else(|| AppError::Unprocessable("user must register before being invited".into()))?;

    let existing: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM memberships WHERE tenant_id = $1 AND user_id = $2")
            .bind(ctx.tenant_id)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?;
    if existing.is_some() {
        return Err(AppError::InvalidState(
            "user is already a member or invited".into(),
        ));
    }

    let mut tx = state.db.begin().await?;
    let membership_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO memberships (tenant_id, user_id, role, status)
        VALUES ($1, $2, $3::tenant_role, 'invited')
        RETURNING id
        "#,
    )
    .bind(ctx.tenant_id)
    .bind(user_id)
    .bind(payload.role.as_str())
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint() == Some("memberships_unique_user") => {
            AppError::InvalidState("user is already a member or invited".into())
        }
        _ => AppError::Db(e),
    })?;

    insert_audit(
        &mut tx,
        ctx.tenant_id,
        Some(ctx.user_id),
        "membership",
        membership_id,
        "membership.invited",
        None,
        Some(json!({ "user_id": user_id, "role": payload.role })),
    )
    .await?;
    tx.commit().await?;

    fetch_member(state, ctx.tenant_id, user_id).await
}

pub async fn pending_invitations(
    state: &AppState,
    user_id: Uuid,
) -> Result<Vec<PendingInvitation>, AppError> {
    let rows: Vec<InvitationRow> = sqlx::query_as(
        r#"
        SELECT m.id AS membership_id, m.tenant_id, t.name AS tenant_name,
               t.slug AS tenant_slug, m.role::text, m.created_at
          FROM memberships m
          JOIN tenants t ON t.id = m.tenant_id
         WHERE m.user_id = $1
           AND m.status = 'invited'
           AND t.status = 'active'
         ORDER BY m.created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(PendingInvitation {
                membership_id: row.membership_id,
                tenant_id: row.tenant_id,
                tenant_name: row.tenant_name,
                tenant_slug: row.tenant_slug,
                role: parse_role(&row.role)?,
                invited_at: row.created_at,
            })
        })
        .collect()
}

pub async fn accept_invitation(
    state: &AppState,
    user_id: Uuid,
    membership_id: Uuid,
) -> Result<TenantSummary, AppError> {
    let mut tx = state.db.begin().await?;

    let row: Option<(Uuid, String, String, String)> = sqlx::query_as(
        r#"
        SELECT m.tenant_id, m.role::text, t.name, t.slug
          FROM memberships m
          JOIN tenants t ON t.id = m.tenant_id
         WHERE m.id = $1
           AND m.user_id = $2
           AND m.status = 'invited'
           AND t.status = 'active'
        "#,
    )
    .bind(membership_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (tenant_id, role, tenant_name, tenant_slug) = row.ok_or(AppError::NotFound)?;

    sqlx::query("UPDATE memberships SET status = 'active', updated_at = NOW() WHERE id = $1")
        .bind(membership_id)
        .execute(&mut *tx)
        .await?;

    insert_audit(
        &mut tx,
        tenant_id,
        Some(user_id),
        "membership",
        membership_id,
        "membership.accepted",
        Some(json!({ "status": "invited" })),
        Some(json!({ "status": "active" })),
    )
    .await?;
    tx.commit().await?;

    Ok(TenantSummary {
        id: tenant_id,
        name: tenant_name,
        slug: tenant_slug,
        role: parse_role(&role)?,
        status: "active".into(),
    })
}

pub async fn update_member(
    state: &AppState,
    ctx: &TenantContext,
    member_id: Uuid,
    payload: UpdateMemberRequest,
) -> Result<TeamMember, AppError> {
    ctx.require_manager()?;

    if member_id == ctx.user_id {
        return Err(AppError::InvalidState(
            "you cannot change your own membership".into(),
        ));
    }

    let target: Option<(String, String)> = sqlx::query_as(
        "SELECT role::text, status::text FROM memberships WHERE tenant_id = $1 AND user_id = $2",
    )
    .bind(ctx.tenant_id)
    .bind(member_id)
    .fetch_optional(&state.db)
    .await?;
    let (current_role, current_status) = target.ok_or(AppError::NotFound)?;
    let current_role = parse_role(&current_role)?;

    if current_role == TenantRole::Owner {
        return Err(AppError::Forbidden);
    }
    if ctx.role == TenantRole::Admin && current_role == TenantRole::Admin {
        return Err(AppError::Forbidden);
    }

    if let Some(role) = payload.role {
        if role == TenantRole::Owner {
            return Err(AppError::Unprocessable(
                "ownership cannot be transferred here".into(),
            ));
        }
    }

    let new_status = match payload.status.as_deref() {
        None => None,
        Some("active") => Some("active"),
        Some("disabled") => Some("disabled"),
        Some(_) => {
            return Err(AppError::Validation(
                "status must be 'active' or 'disabled'".into(),
            ))
        }
    };

    if payload.role.is_none() && new_status.is_none() {
        return Err(AppError::Validation("nothing to update".into()));
    }

    let mut tx = state.db.begin().await?;
    sqlx::query(
        r#"
        UPDATE memberships
           SET role = COALESCE($3::tenant_role, role),
               status = COALESCE($4::membership_status, status),
               updated_at = NOW()
         WHERE tenant_id = $1 AND user_id = $2
        "#,
    )
    .bind(ctx.tenant_id)
    .bind(member_id)
    .bind(payload.role.map(|role| role.as_str()))
    .bind(new_status)
    .execute(&mut *tx)
    .await?;

    insert_audit(
        &mut tx,
        ctx.tenant_id,
        Some(ctx.user_id),
        "membership",
        member_id,
        "membership.updated",
        Some(json!({ "role": current_role, "status": current_status })),
        Some(json!({
            "role": payload.role.map(|role| role.as_str()),
            "status": new_status,
        })),
    )
    .await?;
    tx.commit().await?;

    fetch_member(state, ctx.tenant_id, member_id).await
}

async fn fetch_member(
    state: &AppState,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<TeamMember, AppError> {
    let row: MemberRow = sqlx::query_as(
        r#"
        SELECT m.user_id, u.email, u.full_name, m.role::text, m.status::text, m.created_at
          FROM memberships m
          JOIN users u ON u.id = m.user_id
         WHERE m.tenant_id = $1 AND m.user_id = $2
        "#,
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    member_from_row(row)
}

fn member_from_row(row: MemberRow) -> Result<TeamMember, AppError> {
    Ok(TeamMember {
        user_id: row.user_id,
        email: row.email,
        full_name: row.full_name,
        role: parse_role(&row.role)?,
        status: row.status,
        created_at: row.created_at,
    })
}

fn parse_role(raw: &str) -> Result<TenantRole, AppError> {
    TenantRole::parse(raw).ok_or_else(|| AppError::Internal(format!("unknown tenant role '{raw}'")))
}

#[allow(clippy::too_many_arguments)]
async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    entity_type: &str,
    entity_id: Uuid,
    action: &str,
    before: Option<Value>,
    after: Option<Value>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO audit_events
            (tenant_id, actor_id, entity_type, entity_id, action, before, after)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(tenant_id)
    .bind(actor_id)
    .bind(entity_type)
    .bind(entity_id)
    .bind(action)
    .bind(before)
    .bind(after)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn unique_slug(name: &str) -> String {
    let base: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let base = base.trim_matches('-');
    let base: String = if base.is_empty() {
        "business".into()
    } else {
        base.chars().take(48).collect()
    };
    let suffix = Uuid::new_v4().simple().to_string();
    format!("{base}-{}", &suffix[..8])
}
