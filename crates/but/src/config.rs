use anyhow::{Context, Result};
use but_settings::AppSettings;
use gitbutler_repo::Config;
use gitbutler_secret::secret;
use gitbutler_user;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInfo {
    pub repository_path: String,
    pub user_info: UserInfo,
    pub gitbutler_info: GitButlerInfo,
    pub ai_tooling: AiToolingInfo,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitButlerInfo {
    pub username: Option<String>,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiToolingInfo {
    pub openai: AiProviderInfo,
    pub anthropic: AiProviderInfo,
    pub ollama: AiProviderInfo,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderInfo {
    pub configured: bool,
    pub model: Option<String>,
}

pub fn handle(current_dir: &Path, app_settings: &AppSettings, json: bool, key: Option<&str>, value: Option<&str>) -> Result<()> {
    match (key, value) {
        // Set configuration value
        (Some(key), Some(value)) => {
            set_config_value(current_dir, key, value)?;
            if !json {
                println!("Set {} = {}", key, value);
            }
            Ok(())
        }
        // Get specific configuration value
        (Some(key), None) => {
            let config_value = get_config_value(current_dir, key)?;
            if json {
                let result = serde_json::json!({
                    "key": key,
                    "value": config_value
                });
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                match config_value {
                    Some(val) => println!("{}", val),
                    None => println!("{} is not set", key),
                }
            }
            Ok(())
        }
        // Show all configuration (existing behavior)
        (None, None) => show(current_dir, app_settings, json),
        // Invalid: value without key
        (None, Some(_)) => {
            Err(anyhow::anyhow!("Cannot set a value without specifying a key"))
        }
    }
}

pub fn show(current_dir: &Path, app_settings: &AppSettings, json: bool) -> Result<()> {
    let config_info = gather_config_info(current_dir, app_settings)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&config_info)?);
    } else {
        print_formatted_config(&config_info);
    }

    Ok(())
}

fn gather_config_info(current_dir: &Path, _app_settings: &AppSettings) -> Result<ConfigInfo> {
    let user_info = get_git_user_info(current_dir)?;
    let gitbutler_info = get_gitbutler_info()?;
    let ai_tooling = get_ai_tooling_info()?;

    Ok(ConfigInfo {
        repository_path: current_dir.display().to_string(),
        user_info,
        gitbutler_info,
        ai_tooling,
    })
}

fn get_git_user_info(current_dir: &Path) -> Result<UserInfo> {
    let git_repo =
        git2::Repository::discover(current_dir).context("Failed to find Git repository")?;
    let config = Config::from(&git_repo);

    let name = config.user_name().unwrap_or(None);
    let email = config.user_email().unwrap_or(None);

    Ok(UserInfo { name, email })
}

fn get_gitbutler_info() -> Result<GitButlerInfo> {
    let (username, status) = match gitbutler_user::get_user() {
        Ok(Some(user)) => {
            let username = user.login.clone();
            let status = if user.access_token().is_ok() {
                "Connected ✓".to_string()
            } else {
                "Not connected ✗".to_string()
            };
            (username, status)
        }
        _ => (None, "Not configured ✗".to_string()),
    };

    Ok(GitButlerInfo { username, status })
}

fn get_ai_tooling_info() -> Result<AiToolingInfo> {
    let openai = check_openai_config();
    let anthropic = check_anthropic_config();
    let ollama = check_ollama_config();

    Ok(AiToolingInfo {
        openai,
        anthropic,
        ollama,
    })
}

fn check_openai_config() -> AiProviderInfo {
    let has_env_key = std::env::var("OPENAI_API_KEY").is_ok();
    let has_stored_key = secret::retrieve("openai_api_key", secret::Namespace::BuildKind).is_ok();
    let has_gb_token =
        secret::retrieve("gitbutler_access_token", secret::Namespace::BuildKind).is_ok();

    let configured = has_env_key || has_stored_key || has_gb_token;
    let model = if configured {
        Some("gpt-4".to_string())
    } else {
        None
    };

    AiProviderInfo { configured, model }
}

fn check_anthropic_config() -> AiProviderInfo {
    let has_key = secret::retrieve("anthropic_api_key", secret::Namespace::BuildKind).is_ok();

    let model = if has_key {
        Some("claude-3-5-sonnet".to_string())
    } else {
        None
    };

    AiProviderInfo {
        configured: has_key,
        model,
    }
}

fn check_ollama_config() -> AiProviderInfo {
    let configured = std::process::Command::new("curl")
        .arg("-s")
        .arg("--connect-timeout")
        .arg("2")
        .arg("http://localhost:11434/api/tags")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);

    let model = if configured {
        Some("llama2:7b".to_string())
    } else {
        None
    };

    AiProviderInfo { configured, model }
}

fn print_formatted_config(config: &ConfigInfo) {
    println!("Configuration for {}", config.repository_path);
    println!("==================================================");
    println!();

    println!("👤 User Information:");
    println!(
        "  Name  (user.name):  {}",
        config.user_info.name.as_deref().unwrap_or("Not configured")
    );
    println!(
        "  Email (user.email): {}",
        config
            .user_info
            .email
            .as_deref()
            .unwrap_or("Not configured")
    );
    println!();

    println!("🚀 GitButler:");
    println!(
        "  Username (user.login): {}",
        config
            .gitbutler_info
            .username
            .as_deref()
            .unwrap_or("Not configured")
    );
    println!("  Status:   {}", config.gitbutler_info.status);
    println!();

    println!("🤖 AI Tooling:");
    print_ai_provider("OpenAI", &config.ai_tooling.openai);
    print_ai_provider("Anthropic", &config.ai_tooling.anthropic);
    print_ai_provider("Ollama", &config.ai_tooling.ollama);
}

fn print_ai_provider(name: &str, provider: &AiProviderInfo) {
    let status = if provider.configured { "✓" } else { "✗" };
    let model_info = match &provider.model {
        Some(model) => format!(" ({})", model),
        None => String::new(),
    };

    println!(
        "  {:10} {}{}{}",
        format!("{}:", name),
        if provider.configured {
            "Configured"
        } else {
            "Not configured"
        },
        if provider.configured { " " } else { " " },
        if provider.configured { status } else { status }
    );
    if !model_info.is_empty() && provider.configured {
        println!("             {}", model_info.trim_start());
    }
}

fn set_config_value(current_dir: &Path, key: &str, value: &str) -> Result<()> {
    let git_repo = git2::Repository::discover(current_dir)
        .context("Failed to find Git repository")?;
    let config = Config::from(&git_repo);
    
    config.set_local(key, value)
        .with_context(|| format!("Failed to set {} = {}", key, value))
}

fn get_config_value(current_dir: &Path, key: &str) -> Result<Option<String>> {
    let git_repo = git2::Repository::discover(current_dir)
        .context("Failed to find Git repository")?;
    let config = Config::from(&git_repo);
    
    // For getting values, use the same logic as the existing code
    // which checks the full git config hierarchy (local, global, system)
    match key {
        "user.name" => config.user_name(),
        "user.email" => config.user_email(),
        _ => {
            // For other keys, try to get the value from git config
            let git_config = git_repo.config()?;
            match git_config.get_string(key) {
                Ok(value) => Ok(Some(value)),
                Err(err) => match err.code() {
                    git2::ErrorCode::NotFound => Ok(None),
                    _ => Err(err.into()),
                },
            }
        }
    }
}
