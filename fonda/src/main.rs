use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tokio::process::Command as TokioCommand;
use std::env::consts::OS;
use std::time::Instant;

const REQUIREMENTS_FILE: &str = "requirements.txt";
const ENVIRONMENT_FILE: &str = "environment.yaml";
const PYTHON_COMMANDS: [&str; 3] = ["python", "python3", "py"];

#[derive(Debug)]
enum FondaError {
    Io(io::Error),
    Yaml(serde_yaml::Error),
    PythonNotFound(String),
    VenvCreationFailed(String),
    EnvironmentExists(String),
    ConfigNotFound(String),
    RequirementsNotFound(String),
    CommandFailed { command: String, error: String },
}

impl From<io::Error> for FondaError {
    fn from(err: io::Error) -> FondaError {
        match err.kind() {
            io::ErrorKind::NotFound => {
                FondaError::ConfigNotFound(err.to_string())
            }
            _ => FondaError::Io(err)
        }
    }
}

impl From<serde_yaml::Error> for FondaError {
    fn from(err: serde_yaml::Error) -> FondaError {
        FondaError::Yaml(err)
    }
}

impl std::fmt::Display for FondaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "IO error: {}", err),
            Self::Yaml(err) => write!(f, "YAML parsing error: {}", err),
            Self::PythonNotFound(msg) => write!(f, "Python not found: {}", msg),
            Self::VenvCreationFailed(msg) => write!(f, "Failed to create virtual environment: {}", msg),
            Self::EnvironmentExists(name) => write!(f, "Environment already exists: {}", name),
            Self::ConfigNotFound(msg) => write!(f, "Configuration file not found: {}", msg),
            Self::RequirementsNotFound(msg) => write!(f, "Requirements file not found: {}", msg),
            Self::CommandFailed { command, error } => write!(f, "Command '{}' failed: {}", command, error),
        }
    }
}

impl std::error::Error for FondaError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Yaml(err) => Some(err),
            _ => None,
        }
    }
}

/// Configuration for a conda-style environment
#[derive(Deserialize, Serialize)]
struct CondaEnv {
    /// Name of the environment
    name: String,
    /// List of dependencies to install
    dependencies: Vec<String>,
}

#[derive(Debug)]
enum FondaCommand {
    RunRequirements,
    WriteRequirements,
    CreateAndRun,
}

impl From<&str> for FondaCommand {
    fn from(s: &str) -> Self {
        match s {
            "-r" => FondaCommand::RunRequirements,
            "-w" => FondaCommand::WriteRequirements,
            _ => FondaCommand::CreateAndRun,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), FondaError> {
    let args: Vec<String> = std::env::args().collect();
    let command = FondaCommand::from(args.get(1).map(String::as_str).unwrap_or(""));

    match command {
        FondaCommand::RunRequirements => run_requirements().await,
        FondaCommand::WriteRequirements => write_requirements().await,
        FondaCommand::CreateAndRun => create_and_run().await,
    }
}

async fn run_command(command: &str, args: &[&str]) -> Result<std::process::Output, FondaError> {
    let start = Instant::now();
    println!("Running command: {} {}", command, args.join(" "));
    
    let result = TokioCommand::new(command)
        .args(args)
        .output()
        .await
        .map_err(|e| FondaError::CommandFailed {
            command: format!("{} {}", command, args.join(" ")),
            error: e.to_string(),
        });

    println!("Command completed in {:?}", start.elapsed());
    result
}

async fn run_requirements() -> Result<(), FondaError> {
    let requirements_path = Path::new(REQUIREMENTS_FILE);
    if !requirements_path.exists() {
        return Err(FondaError::RequirementsNotFound(format!("{} not found", REQUIREMENTS_FILE)));
    }

    if OS == "windows" {
        run_command(
            "python",
            &["-m", "pip", "install", "-r", sanitize_path(requirements_path)?]
        ).await?;
    } else {
        run_command(
            "pip",
            &["install", "-r", sanitize_path(requirements_path)?]
        ).await?;
    }

    println!("Requirements installed successfully.");
    Ok(())
}

async fn write_requirements() -> Result<(), FondaError> {
    let path = Path::new(ENVIRONMENT_FILE);
    if !path.exists() {
        return Err(FondaError::ConfigNotFound(format!("{} not found", ENVIRONMENT_FILE)));
    }

    let file = File::open(path)?;
    let env: CondaEnv = serde_yaml::from_reader(file)?;

    let requirements_path = Path::new(REQUIREMENTS_FILE);
    let mut requirements_file = File::create(requirements_path)?;
    for dep in &env.dependencies {
        if dep.starts_with("pip:") {
            let packages = dep.split(':').nth(1).unwrap_or("").split(',');
            for package in packages {
                writeln!(requirements_file, "{}", package.trim())?;
            }
        }
    }

    println!("requirements.txt created successfully.");
    Ok(())
}

async fn get_python_command() -> Result<&'static str, FondaError> {
    for cmd in PYTHON_COMMANDS {
        if let Ok(output) = TokioCommand::new(cmd)
            .arg("--version")
            .output()
            .await
        {
            if output.status.success() {
                return Ok(cmd);
            }
        }
    }
    Err(FondaError::PythonNotFound("No Python installation found".to_string()))
}

/// Creates a new virtual environment and installs dependencies
///
/// # Errors
/// Returns `FondaError` if:
/// - The environment already exists
/// - Python is not found
/// - Virtual environment creation fails
/// - Package installation fails
async fn create_and_run() -> Result<(), FondaError> {
    // Read the .yaml file
    let path = Path::new(ENVIRONMENT_FILE);
    if !path.exists() {
        return Err(FondaError::ConfigNotFound(format!("{} not found", ENVIRONMENT_FILE)));
    }

    let file = File::open(path)?;
    let env: CondaEnv = serde_yaml::from_reader(file)?;

    // Convert dependencies to requirements.txt
    let requirements_path = Path::new(REQUIREMENTS_FILE);
    let mut requirements_file = File::create(requirements_path)?;
    for dep in &env.dependencies {
        if dep.starts_with("pip:") {
            let packages = dep.split(':').nth(1).unwrap_or("").split(',');
            for package in packages {
                writeln!(requirements_file, "{}", package.trim())?;
            }
        }
    }

    // Create the virtual environment
    let env_name = &env.name;
    validate_env_name(env_name)?;

    let venv_path = PathBuf::from(env_name);
    if venv_path.exists() {
        return Err(FondaError::EnvironmentExists(env_name.clone()));
    }

    // Try uv first, fall back to pip if not available
    let env_creation_result = match run_command("uv", &["venv", sanitize_path(&venv_path)?]).await {
        Ok(_) => {
            println!("Environment created successfully using uv");
            Ok(())
        }
        Err(_) => {
            println!("uv not found or failed, falling back to python venv...");
            let python_command = get_python_command().await?;
            match run_command(
                python_command,
                &["-m", "venv", sanitize_path(&venv_path)?]
            ).await {
                Ok(_) => {
                    println!("Environment created successfully using python venv");
                    Ok(())
                }
                Err(e) => Err(FondaError::VenvCreationFailed(e.to_string()))
            }
        }
    };

    env_creation_result?;

    // Install requirements using pip
    let python_cmd = get_python_command().await?;
    run_command(
        python_cmd,
        &["-m", "pip", "install", "-r", sanitize_path(requirements_path)?]
    ).await?;

    println!("Environment '{}' created and requirements installed successfully.", env_name);
    println!("\nTo use your new environment:");
    
    if OS == "windows" {
        println!("  Activate:   .\\{}\\Scripts\\activate.bat", env_name);
        println!("  Deactivate: deactivate");
    } else {
        println!("  Activate:   source ./{}/bin/activate", env_name);
        println!("  Deactivate: deactivate");
    }
    
    println!("\nNote: You may need to restart your terminal for the environment to be available.");
    Ok(())
}

fn sanitize_path(path: &Path) -> Result<&str, FondaError> {
    path.to_str().ok_or_else(|| FondaError::CommandFailed {
        command: "path conversion".to_string(),
        error: "Invalid path encoding".to_string(),
    })
}

fn validate_env_name(name: &str) -> Result<(), FondaError> {
    if name.is_empty() || name.contains(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-') {
        return Err(FondaError::CommandFailed {
            command: "validate_env_name".to_string(),
            error: "Environment name must only contain alphanumeric characters, underscores, or hyphens".to_string(),
        });
    }
    Ok(())
}