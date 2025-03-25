use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tokio::process::Command as TokioCommand;
use std::env::consts::OS;
use std::time::Instant;

const REQUIREMENTS_FILE: &str = "requirements.txt";
const ENVIRONMENT_FILE: &str = "environment.yaml";
const PYTHON_COMMANDS: [&str; 3] = ["python", "python3", "py"];
const DEBUG_FILE: &str = "fonda_debug.log";
static mut VERBOSE_MODE: bool = false;

/// Print debug information if verbose mode is enabled
macro_rules! debug_println {
    ($($arg:tt)*) => {
        if unsafe { VERBOSE_MODE } {
            println!($($arg)*);
        }
    };
}

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
    /// Python version requirement (optional)
    #[serde(default)]
    python_version: Option<String>,
    /// List of conda channels to use (optional)
    #[serde(default)]
    channels: Option<Vec<String>>,
    /// List of dependencies to install
    dependencies: Vec<String>,
    /// List of pip packages to install (optional)
    #[serde(default)]
    pip: Option<Vec<String>>,
}

#[derive(Debug)]
enum FondaCommand {
    RunRequirements,
    WriteRequirements,
    WriteRequirementsCustomFile(String),
    CreateAndRun,
    CustomFile(String),
}

impl From<&str> for FondaCommand {
    fn from(s: &str) -> Self {
        match s {
            "-r" => FondaCommand::RunRequirements,
            "-w" => FondaCommand::WriteRequirements,
            "-f" => FondaCommand::CustomFile(String::new()), // Will be populated with the file path later
            _ => FondaCommand::CreateAndRun,
        }
    }
}

fn log_debug(message: &str) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(DEBUG_FILE)?;
    
    writeln!(file, "{}", message)
}

// Helper function to ensure debug log is created and writable
fn ensure_debug_log() -> io::Result<()> {
    // Create the debug log file if it doesn't exist
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(DEBUG_FILE)?;
    
    // Close the file handle
    drop(file);
    
    // Log initial message
    log_debug(&format!("Debug log initialized. OS: {}", OS))
}

#[tokio::main]
async fn main() -> Result<(), FondaError> {
    let args: Vec<String> = std::env::args().collect();
    
    // Ensure debug log is created and writable
    if let Err(e) = ensure_debug_log() {
        eprintln!("Warning: Failed to create debug log: {}", e);
    }
    
    // Check for verbose mode flag
    if args.contains(&"-v".to_string()) {
        unsafe { VERBOSE_MODE = true; }
        println!("Verbose mode enabled");
    }
    
    // Find the first non-verbose flag to determine the command
    let command_arg = args.iter().skip(1)
        .find(|&arg| arg != "-v")
        .map(String::as_str)
        .unwrap_or("");
    
    // Parse command and optional file path
    let mut command = FondaCommand::from(command_arg);
    
    // Check for -w -f combination
    let w_index = args.iter().position(|arg| arg == "-w");
    let f_index = args.iter().position(|arg| arg == "-f");
    
    if let (Some(_w_index), Some(f_index)) = (w_index, f_index) {
        // Get the file path after -f
        if let Some(file_path) = args.get(f_index + 1) {
            // Validate that the file exists and has a .yaml or .yml extension
            let path = Path::new(file_path);
            if !path.exists() {
                eprintln!("Error: File not found: {}", file_path);
                eprintln!("Usage: fonda -w -f <environment_file.yaml>");
                std::process::exit(1);
            }
            
            let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
            if extension != "yaml" && extension != "yml" {
                eprintln!("Warning: File does not have .yaml or .yml extension: {}", file_path);
                let _ = log_debug(&format!("Warning: File does not have .yaml or .yml extension: {}", file_path));
            }
            
            command = FondaCommand::WriteRequirementsCustomFile(file_path.clone());
            let _ = log_debug(&format!("Using -w -f with file: {}", file_path));
        } else {
            eprintln!("Error: -w -f flags require a file path argument");
            eprintln!("Usage: fonda -w -f <environment_file.yaml>");
            std::process::exit(1);
        }
    }
    // If using -f flag without -w, get the file path from the next argument
    else if let (Some(f_index), None) = (f_index, w_index) {
        if let Some(file_path) = args.get(f_index + 1) {
            // Validate that the file exists and has a .yaml or .yml extension
            let path = Path::new(file_path);
            if !path.exists() {
                eprintln!("Error: File not found: {}", file_path);
                eprintln!("Usage: fonda -f <environment_file.yaml>");
                std::process::exit(1);
            }
            
            let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
            if extension != "yaml" && extension != "yml" {
                eprintln!("Warning: File does not have .yaml or .yml extension: {}", file_path);
                let _ = log_debug(&format!("Warning: File does not have .yaml or .yml extension: {}", file_path));
            }
            
            command = FondaCommand::CustomFile(file_path.clone());
            let _ = log_debug(&format!("Using -f with file: {}", file_path));
        } else {
            eprintln!("Error: -f flag requires a file path argument");
            eprintln!("Usage: fonda -f <environment_file.yaml>");
            std::process::exit(1);
        }
    }

    match command {
        FondaCommand::RunRequirements => run_requirements().await,
        FondaCommand::WriteRequirements => write_requirements().await,
        FondaCommand::WriteRequirementsCustomFile(file_path) => {
            println!("Writing requirements from custom file: {}", file_path);
            let _ = log_debug(&format!("Writing requirements from custom file: {}", file_path));
            write_requirements_from_file(&file_path).await
        },
        FondaCommand::CreateAndRun => create_and_run().await,
        FondaCommand::CustomFile(file_path) => create_and_run_with_file(&file_path).await,
    }
}

async fn run_command(command: &str, args: &[&str]) -> Result<std::process::Output, FondaError> {
    let start = Instant::now();
    println!("Running command: {} {}", command, args.join(" "));
    let _ = log_debug(&format!("Running command: {} {}", command, args.join(" ")));
    
    let result = TokioCommand::new(command)
        .args(args)
        .output()
        .await
        .map_err(|e| FondaError::CommandFailed {
            command: format!("{} {}", command, args.join(" ")),
            error: e.to_string(),
        });

    println!("Command completed in {:?}", start.elapsed());
    let _ = log_debug(&format!("Command completed in {:?}", start.elapsed()));
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
    let _ = log_debug("Requirements installed successfully.");
    Ok(())
}

async fn write_requirements() -> Result<(), FondaError> {
    println!("Writing requirements from default environment file: {}", ENVIRONMENT_FILE);
    let _ = log_debug(&format!("Writing requirements from default environment file: {}", ENVIRONMENT_FILE));
    write_requirements_from_file(ENVIRONMENT_FILE).await
}

async fn write_requirements_from_file(env_file: &str) -> Result<(), FondaError> {
    debug_println!("DEBUG: Starting write_requirements_from_file with file: {}", env_file);
    let path = Path::new(env_file);
    if !path.exists() {
        return Err(FondaError::ConfigNotFound(format!("{} not found", env_file)));
    }

    // First, parse the YAML file to get the basic structure (for validation)
    let file = File::open(path)?;
    let _env: CondaEnv = serde_yaml::from_reader(file)?;
    debug_println!("DEBUG: Successfully parsed YAML file structure");

    // Now, read the file as raw text to preserve comments
    let file_content = std::fs::read_to_string(path)?;
    debug_println!("DEBUG: Read raw file content");

    let requirements_path = Path::new(REQUIREMENTS_FILE);
    let mut requirements_file = File::create(requirements_path)?;
    debug_println!("DEBUG: Created requirements.txt file");
    
    // Process dependencies from the raw file content
    debug_println!("DEBUG: Processing dependencies from raw file content");
    
    // Find the dependencies section
    let mut in_dependencies = false;
    let mut in_pip = false;
    
    for line in file_content.lines() {
        let trimmed_line = line.trim();
        
        // Skip empty lines and comments at the beginning of lines
        if trimmed_line.is_empty() || trimmed_line.starts_with('#') {
            continue;
        }
        
        // Check if we're entering the dependencies section
        if trimmed_line == "dependencies:" {
            in_dependencies = true;
            in_pip = false;
            debug_println!("DEBUG: Found dependencies section");
            continue;
        }
        
        // Check if we're entering the pip section
        if trimmed_line == "pip:" {
            in_dependencies = false;
            in_pip = true;
            debug_println!("DEBUG: Found pip section");
            continue;
        }
        
        // If we're not in either section, skip
        if !in_dependencies && !in_pip {
            continue;
        }
        
        // Check if we're exiting the current section (indentation level change)
        if !trimmed_line.starts_with('-') && !trimmed_line.starts_with(' ') {
            in_dependencies = false;
            in_pip = false;
            continue;
        }
        
            // Process dependency line
            if trimmed_line.starts_with('-') {
                let dep_line = trimmed_line.trim_start_matches('-').trim();
                debug_println!("DEBUG: Processing raw dependency line: '{}'", dep_line);
                
                // Handle pip: prefix in dependencies section
                if in_dependencies && dep_line.starts_with("pip:") {
                    let packages = dep_line.trim_start_matches("pip:").split(',');
                    for package in packages {
                        let package_spec = package.trim();
                        if !package_spec.is_empty() {
                            debug_println!("DEBUG: Adding pip package from dependencies section: {}", package_spec);
                            writeln!(requirements_file, "{}", package_spec)?;
                        }
                    }
                    continue;
                }
                
                // Check for platform-specific dependencies
                if let Some(comment_idx) = dep_line.find('#') {
                    let package_spec = dep_line[0..comment_idx].trim();
                    let comment = dep_line[comment_idx..].trim();
                    
                    debug_println!("DEBUG: Found comment in dependency: '{}'", comment);
                    debug_println!("DEBUG: Package spec: '{}'", package_spec);
                    
                    // Check if this is a platform-specific dependency
                    let comment_lower = comment.to_lowercase();
                    debug_println!("DEBUG: Comment lowercase: '{}'", comment_lower);
                    debug_println!("DEBUG: Current OS: '{}'", OS);
                    
                    let section = if in_dependencies { "dependency" } else { "pip dependency" };
                    debug_println!("PROCESSING - {}: {}, Comment: {}, Current OS: {}", section, package_spec, comment, OS);
                    
                    // Skip Windows-only dependencies on non-Windows platforms
                    debug_println!("DEBUG: Checking for [win] marker: {}", comment_lower.contains("[win]"));
                    if comment_lower.contains("[win]") {
                        debug_println!("FOUND Windows marker in: {}", comment);
                        if OS != "windows" {
                            debug_println!("SKIPPING Windows-only {}: {}", section, package_spec);
                            continue;
                        } else {
                            debug_println!("KEEPING Windows-only {} (on Windows): {}", section, package_spec);
                        }
                    }
                    
                    // Skip Linux-only dependencies on non-Linux platforms
                    debug_println!("DEBUG: Checking for [linux] marker: {}", comment_lower.contains("[linux]"));
                    if comment_lower.contains("[linux]") {
                        debug_println!("FOUND Linux marker in: {}", comment);
                        if OS != "linux" {
                            debug_println!("SKIPPING Linux-only {}: {}", section, package_spec);
                            continue;
                        } else {
                            debug_println!("KEEPING Linux-only {} (on Linux): {}", section, package_spec);
                        }
                    }
                    
                    // Skip macOS-only dependencies on non-macOS platforms
                    debug_println!("DEBUG: Checking for [osx] marker: {}", comment_lower.contains("[osx]"));
                    debug_println!("DEBUG: Checking for [darwin] marker: {}", comment_lower.contains("[darwin]"));
                    if comment_lower.contains("[osx]") || comment_lower.contains("[darwin]") {
                        debug_println!("FOUND macOS marker in: {}", comment);
                        if OS != "macos" {
                            debug_println!("SKIPPING macOS-only {}: {}", section, package_spec);
                            continue;
                        } else {
                            debug_println!("KEEPING macOS-only {} (on macOS): {}", section, package_spec);
                        }
                    }
                    
                    debug_println!("ADDING {} to requirements.txt: {}", section, package_spec);
                    
                    if !package_spec.is_empty() {
                        writeln!(requirements_file, "{}", package_spec)?;
                    }
                } else {
                    // No platform marker, include the dependency
                    let package_spec = dep_line.trim();
                    if !package_spec.is_empty() {
                        // Handle Git/URL dependencies and editable installs
                        if package_spec.starts_with("git+") || 
                           package_spec.starts_with("http://") || 
                           package_spec.starts_with("https://") || 
                           package_spec.starts_with("-e ") {
                            debug_println!("DEBUG: Adding special dependency: {}", package_spec);
                            writeln!(requirements_file, "{}", package_spec)?;
                        } else {
                            debug_println!("DEBUG: Adding regular dependency: {}", package_spec);
                            writeln!(requirements_file, "{}", package_spec)?;
                        }
                    }
                }
            }
    }

    debug_println!("DEBUG: Finished processing all dependencies");
    println!("requirements.txt created successfully.");
    let _ = log_debug("requirements.txt created successfully.");
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

/// Creates a new virtual environment and installs dependencies using the default environment file
///
/// # Errors
/// Returns `FondaError` if:
/// - The environment already exists
/// - Python is not found
/// - Virtual environment creation fails
/// - Package installation fails
async fn create_and_run() -> Result<(), FondaError> {
    create_and_run_with_file(ENVIRONMENT_FILE).await
}

/// Creates a new virtual environment and installs dependencies using a specified environment file
///
/// # Errors
/// Returns `FondaError` if:
/// - The environment already exists
/// - Python is not found
/// - Virtual environment creation fails
/// - Package installation fails
async fn create_and_run_with_file(env_file: &str) -> Result<(), FondaError> {
    // Read the .yaml file
    let path = Path::new(env_file);
    if !path.exists() {
        return Err(FondaError::ConfigNotFound(format!("{} not found", env_file)));
    }

    // First, parse the YAML file to get the basic structure (for validation)
    let file = File::open(path)?;
    let env: CondaEnv = serde_yaml::from_reader(file)?;

    // Generate requirements.txt using our platform-specific filtering
    // We'll reuse the write_requirements_from_file function to ensure consistent behavior
    write_requirements_from_file(env_file).await?;
    
    // Read the requirements.txt file that was just created
    let requirements_path = Path::new(REQUIREMENTS_FILE);
    if !requirements_path.exists() {
        return Err(FondaError::RequirementsNotFound(format!("{} not found", REQUIREMENTS_FILE)));
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
