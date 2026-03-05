use crate::core::models::{Deployment, DeploymentStatus, DeploymentType};
use anyhow::Result;
use sqlx::{SqlitePool, Row};

pub async fn list_deployments(pool: &SqlitePool) -> Result<Vec<Deployment>> {
    let rows = sqlx::query("SELECT id, repo_url, path, deploy_type, status, last_updated FROM deployments")
        .fetch_all(pool)
        .await?;
        
    let mut deployments = Vec::new();
    for row in rows {
        let deploy_type_str: String = row.get("deploy_type");
        let deploy_type = match deploy_type_str.as_str() {
            "DockerCompose" => DeploymentType::DockerCompose,
            "WasmCloud" => DeploymentType::WasmCloud,
            _ => DeploymentType::Unknown,
        };
        let status_str: String = row.get("status");
        let status = match status_str.as_str() {
            "Cloning" => DeploymentStatus::Cloning,
            "Deploying" => DeploymentStatus::Deploying,
            "Running" => DeploymentStatus::Running,
            "Stopped" => DeploymentStatus::Stopped,
            s if s.starts_with("Failed:") => DeploymentStatus::Failed(s.replace("Failed:", "").trim().to_string()),
            _ => DeploymentStatus::Failed("Unknown".to_string()),
        };
        
        deployments.push(Deployment {
            id: row.get("id"),
            repo_url: row.get("repo_url"),
            path: row.get("path"),
            deploy_type,
            status,
            last_updated: row.get("last_updated"),
        });
    }
    
    Ok(deployments)
}

pub async fn add_deployment(pool: &SqlitePool, deployment: &Deployment) -> Result<()> {
    let deploy_type = match deployment.deploy_type {
        DeploymentType::DockerCompose => "DockerCompose",
        DeploymentType::WasmCloud => "WasmCloud",
        DeploymentType::Unknown => "Unknown",
    };
    
    let status = match &deployment.status {
        DeploymentStatus::Cloning => "Cloning".to_string(),
        DeploymentStatus::Deploying => "Deploying".to_string(),
        DeploymentStatus::Running => "Running".to_string(),
        DeploymentStatus::Stopped => "Stopped".to_string(),
        DeploymentStatus::Failed(e) => format!("Failed: {}", e),
    };

    sqlx::query("INSERT INTO deployments (id, repo_url, path, deploy_type, status, last_updated) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&deployment.id)
        .bind(&deployment.repo_url)
        .bind(&deployment.path)
        .bind(deploy_type)
        .bind(status)
        .bind(&deployment.last_updated)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn update_deployment_status(pool: &SqlitePool, id: &str, new_type: Option<&DeploymentType>, new_status: &DeploymentStatus) -> Result<()> {
    let status_str = match new_status {
        DeploymentStatus::Cloning => "Cloning".to_string(),
        DeploymentStatus::Deploying => "Deploying".to_string(),
        DeploymentStatus::Running => "Running".to_string(),
        DeploymentStatus::Stopped => "Stopped".to_string(),
        DeploymentStatus::Failed(e) => format!("Failed: {}", e),
    };
    let now = chrono::Utc::now().to_rfc3339();

    if let Some(t) = new_type {
        let type_str = match t {
            DeploymentType::DockerCompose => "DockerCompose",
            DeploymentType::WasmCloud => "WasmCloud",
            DeploymentType::Unknown => "Unknown",
        };
        sqlx::query("UPDATE deployments SET status = ?, deploy_type = ?, last_updated = ? WHERE id = ?")
            .bind(status_str)
            .bind(type_str)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
    } else {
        sqlx::query("UPDATE deployments SET status = ?, last_updated = ? WHERE id = ?")
            .bind(status_str)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
    }

    Ok(())
}

pub async fn delete_deployment(pool: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM deployments WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
