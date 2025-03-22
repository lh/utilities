use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use tokio::process::Command;
use std::env::consts::OS;

#[derive(Debug)]
enum FondaError {
    Io(#[allow(dead_code)] io::Error),
    Yaml(#[allow(dead_code)] serde_yaml::Error),
}

impl From<io::Error> for FondaError {
    fn from(err: io::Error) -> FondaError {
        FondaError::Io(err)
    }
}

impl From<serde_yaml::Error> for FondaError {
    fn from(err: serde_yaml::Error) -> FondaError {
        FondaError::Yaml(err)
    }
}

#[derive(Deserialize, Serialize)]
struct CondaEnv {
    name: String,
    dependencies: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), FondaError> {
    let args: Vec<String> = std::env::args().collect();
    let default_command = String::from("");
    let command = args.get(1).unwrap_or(&default_command);

    match command.as_str() {
        "-r" => run_requirements().await?,
        "-w" => write_requirements().await?,
        _ => create_and_run().await?,
    }

    Ok(())
}

async fn run_requirements() -> Result<(), FondaError> {
    let requirements_path = Path::new("requirements.txt");
    if !requirements_path.exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "requirements.txt not found").into());
    }

    let output = Command::new("pip")
        .arg("install")
        .arg("-r")
        .arg(requirements_path)
        .output()
        .await?;

    if !output.status.success() {
        eprintln!("Failed to install requirements: {}", String::from_utf8_lossy(&output.stderr));
        return Err(io::Error::new(io::ErrorKind::Other, "pip install failed").into());
    }

    println!("Requirements installed successfully.");
    Ok(())
}

async fn write_requirements() -> Result<(), FondaError> {
    let path = Path::new("environment.yaml");
    let file = File::open(path)?;
    let env: CondaEnv = serde_yaml::from_reader(file)?;

    let requirements_path = Path::new("requirements.txt");
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

async fn create_and_run() -> Result<(), FondaError> {
    // Read the .yaml file
    let path = Path::new("environment.yaml");
    let file = File::open(path)?;
    let env: CondaEnv = serde_yaml::from_reader(file)?;

    // Convert dependencies to requirements.txt
    let requirements_path = Path::new("requirements.txt");
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
    let venv_path = env_name.to_string();  // Remove the "venv_" prefix

    // Try uv first, fall back to pip if not available
    let uv_result = Command::new("uv")
        .arg("venv")
        .arg(&venv_path)
        .output()
        .await;

    match uv_result {
        Ok(output) if output.status.success() => {
            println!("Environment created successfully using uv");
        }
        _ => {
            println!("uv not found or failed, falling back to python venv...");
            // Use python -m venv to create the environment
            let output = Command::new("python")
                .arg("-m")
                .arg("venv")
                .arg(&venv_path)
                .output()
                .await?;

            if !output.status.success() {
                eprintln!("Failed to create environment with python -m venv: {}", 
                    String::from_utf8_lossy(&output.stderr));
                return Err(io::Error::new(io::ErrorKind::Other, 
                    "python -m venv command failed").into());
            }
        }
    }

    // Install requirements using pip
    let output = Command::new("pip")
        .arg("install")
        .arg("-r")
        .arg(requirements_path)
        .output()
        .await?;

    if !output.status.success() {
        eprintln!("Failed to install requirements: {}", String::from_utf8_lossy(&output.stderr));
        return Err(io::Error::new(io::ErrorKind::Other, "pip install failed").into());
    }

    println!("Environment '{}' created and requirements installed successfully.", env_name);

    let activation_command = if OS == "windows" {
        format!("{}\\Scripts\\activate.bat", venv_path)
    } else {
        format!("source {}/bin/activate", venv_path)
    };

    println!("\nTo activate your environment, run:");
    println!("{}", activation_command);
    println!("\nTo deactivate your environment, run:");
    println!("deactivate");
    Ok(())
}