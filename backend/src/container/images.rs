use anyhow::Result;
use bollard::image::BuildImageOptions;
use bollard::Docker;
use futures_util::StreamExt;
use std::io::Write;

use crate::config::PlatformConfig;

// Embedded build context files
const BASE_DOCKERFILE: &str = include_str!("../../../images/base/Dockerfile");
const CC_DOCKERFILE: &str = include_str!("../../../images/claude-code/Dockerfile");
const CODEX_DOCKERFILE: &str = include_str!("../../../images/codex/Dockerfile");
const SYSTEM_RULES: &str = include_str!("../../../scripts/system-rules.md");
const ENTRYPOINT_SH: &str = include_str!("../../../scripts/entrypoint.sh");
const TAVILY_SEARCH: &str = include_str!("../../../scripts/tavily-search");

#[derive(Debug, Clone, serde::Serialize)]
pub struct ImageStatus {
    pub name: String,
    pub ready: bool,
}

pub async fn check_image_status(
    docker: &Docker,
    config: &PlatformConfig,
) -> Result<Vec<ImageStatus>> {
    let images = vec![
        "ccodebox-base:latest".to_string(),
        config.cc_image.clone(),
        config.codex_image.clone(),
    ];

    let mut statuses = Vec::new();
    for name in images {
        let ready = image_exists(docker, &name).await.unwrap_or(false);
        statuses.push(ImageStatus { name, ready });
    }

    Ok(statuses)
}

pub async fn image_exists(docker: &Docker, image: &str) -> Result<bool> {
    match docker.inspect_image(image).await {
        Ok(_) => Ok(true),
        Err(bollard::errors::Error::DockerResponseServerError { status_code: 404, .. }) => {
            Ok(false)
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn ensure_images(docker: &Docker, config: &PlatformConfig) -> Result<()> {
    let specs = [
        ("ccodebox-base:latest", BuildSpec::Base),
        (&config.cc_image, BuildSpec::ClaudeCode),
        (&config.codex_image, BuildSpec::Codex),
    ];

    for (image, spec) in &specs {
        if !image_exists(docker, image).await? {
            tracing::info!("Image {image} not found, building...");
            build_image(docker, image, spec).await?;
            tracing::info!("Image {image} built successfully");
        }
    }

    Ok(())
}

pub async fn build_all_images(docker: &Docker, config: &PlatformConfig) -> Result<()> {
    let specs = [
        ("ccodebox-base:latest", BuildSpec::Base),
        (&config.cc_image, BuildSpec::ClaudeCode),
        (&config.codex_image, BuildSpec::Codex),
    ];

    for (image, spec) in &specs {
        tracing::info!("Building {image}...");
        build_image(docker, image, spec).await?;
        tracing::info!("Image {image} built successfully");
    }

    Ok(())
}

enum BuildSpec {
    Base,
    ClaudeCode,
    Codex,
}

async fn build_image(docker: &Docker, tag: &str, spec: &BuildSpec) -> Result<()> {
    let tar_bytes = create_build_context(spec)?;

    let options = BuildImageOptions {
        t: tag.to_string(),
        rm: true,
        ..Default::default()
    };

    let mut stream = docker.build_image(options, None, Some(tar_bytes.into()));

    while let Some(result) = stream.next().await {
        match result {
            Ok(info) => {
                if let Some(stream_msg) = info.stream {
                    let trimmed = stream_msg.trim();
                    if !trimmed.is_empty() {
                        tracing::debug!("[build {tag}] {trimmed}");
                    }
                }
                if let Some(err) = info.error {
                    return Err(anyhow::anyhow!("Build error for {tag}: {err}"));
                }
            }
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}

fn create_build_context(spec: &BuildSpec) -> Result<Vec<u8>> {
    let buf = Vec::new();
    let mut archive = tar::Builder::new(buf);

    let dockerfile = match spec {
        BuildSpec::Base => BASE_DOCKERFILE,
        BuildSpec::ClaudeCode => CC_DOCKERFILE,
        BuildSpec::Codex => CODEX_DOCKERFILE,
    };

    add_file_to_tar(&mut archive, "Dockerfile", dockerfile.as_bytes())?;
    add_file_to_tar(&mut archive, "scripts/system-rules.md", SYSTEM_RULES.as_bytes())?;
    add_file_to_tar(&mut archive, "scripts/entrypoint.sh", ENTRYPOINT_SH.as_bytes())?;
    add_file_to_tar(&mut archive, "scripts/tavily-search", TAVILY_SEARCH.as_bytes())?;

    archive.into_inner().map_err(Into::into)
}

fn add_file_to_tar<W: Write>(
    archive: &mut tar::Builder<W>,
    path: &str,
    data: &[u8],
) -> Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o755);
    header.set_cksum();
    archive.append_data(&mut header, path, data)?;
    Ok(())
}
