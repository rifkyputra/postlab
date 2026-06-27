use crate::core::models::DeploymentType;
use std::path::Path;

#[allow(dead_code)]
pub fn detect_deployment_type(dir: &Path) -> DeploymentType {
    if dir.join("docker-compose.yml").exists() || dir.join("compose.yaml").exists() {
        return DeploymentType::DockerCompose;
    }

    if dir.join("wadm.yaml").exists() || dir.join("wasmcloud.toml").exists() {
        return DeploymentType::WasmCloud;
    }

    DeploymentType::Unknown
}

#[cfg(test)]
mod tests {
    use super::detect_deployment_type;
    use crate::core::models::DeploymentType;
    use std::fs;

    fn tmp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "postlab-detector-{tag}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn detects_docker_compose_yml() {
        let dir = tmp_dir("dc-yml");
        fs::write(dir.join("docker-compose.yml"), "").unwrap();
        assert_eq!(detect_deployment_type(&dir), DeploymentType::DockerCompose);
    }

    #[test]
    fn detects_compose_yaml() {
        let dir = tmp_dir("compose-yaml");
        fs::write(dir.join("compose.yaml"), "").unwrap();
        assert_eq!(detect_deployment_type(&dir), DeploymentType::DockerCompose);
    }

    #[test]
    fn detects_wasmcloud_toml() {
        let dir = tmp_dir("wasmcloud-toml");
        fs::write(dir.join("wasmcloud.toml"), "").unwrap();
        assert_eq!(detect_deployment_type(&dir), DeploymentType::WasmCloud);
    }

    #[test]
    fn detects_wadm_yaml() {
        let dir = tmp_dir("wadm-yaml");
        fs::write(dir.join("wadm.yaml"), "").unwrap();
        assert_eq!(detect_deployment_type(&dir), DeploymentType::WasmCloud);
    }

    #[test]
    fn empty_dir_is_unknown() {
        let dir = tmp_dir("empty");
        assert_eq!(detect_deployment_type(&dir), DeploymentType::Unknown);
    }
}
