# Fonda

Fonda is a Rust-based CLI tool that manages Python virtual environments and dependencies from YAML configuration files.

## Features

- Creates Python virtual environments using both `uv` and standard `venv`
- Converts conda-style environment.yaml files to requirements.txt
- Installs Python package dependencies
- Supports multiple commands for different operations
- Handles platform-specific dependencies with conda-style markers (`# [win]`, `# [linux]`, `# [osx]`)
- Supports Git/URL dependencies and development/editable installs
- Provides verbose mode for detailed debugging information

## Installation

### From Source

```sh
# Clone the repository
git clone https://github.com/yourusername/fonda.git
cd fonda

# Build the project
cargo build --release

# The binary will be available at ./target/release/fonda
```

## Usage

The tool supports the following operations:

```sh
# Create environment and install dependencies from environment.yaml
fonda

# Create environment and install dependencies from a custom YAML file
fonda -f custom-environment.yaml

# Install from existing requirements.txt
fonda -r

# Generate requirements.txt from environment.yaml
fonda -w

# Generate requirements.txt from a custom YAML file
fonda -w -f custom-environment.yaml

# Enable verbose mode (can be combined with any command)
fonda -v
fonda -v -w -f custom-environment.yaml
```

### Command Flags

- `-f <file>`: Use a custom YAML file instead of the default environment.yaml
- `-r`: Install packages from an existing requirements.txt file
- `-w`: Generate requirements.txt from environment.yaml without creating an environment
- `-v`: Enable verbose mode for detailed debugging information


## Configuration

Create an environment.yaml file in your project root:

```yaml
name: myenv
dependencies:
  - "numpy>=1.24.0"
  - "pandas>=1.3.0"
  - "pywin32>=300       # [win]"    # Windows-only dependency
  - "pyobjc>=8.0        # [osx]"    # macOS-only dependency
  - "python-xlib>=0.30  # [linux]"  # Linux-only dependency
  - "pip:requests>=2.28.0"          # Regular pip dependency
  - "pip:winreg>=0.3.1      # [win]"    # Windows-only pip dependency
  - "pip:pyobjc-framework-Cocoa>=8.0  # [osx]"    # macOS-only pip dependency
  - "pip:dbus-python>=1.2.18  # [linux]"  # Linux-only pip dependency
  
  # Git repository dependencies
  - "git+https://github.com/user/repo.git"                # Basic Git repo
  - "git+https://github.com/user/repo.git@branch"         # Git repo with branch
  - "git+https://github.com/user/repo.git@v1.0.0"         # Git repo with tag
  
  # URL dependencies
  - "https://example.com/packages/some-package.tar.gz"    # Direct URL to package
  
  # Editable installs
  - "-e ."                                                # Current directory
  - "-e ./path/to/local/package"                          # Local path
  - "-e git+https://github.com/user/dev-repo.git"         # Editable Git repo
```

Platform-specific dependencies are automatically filtered based on the current operating system. The following markers are supported:
- `# [win]`: Windows-only dependency
- `# [linux]`: Linux-only dependency
- `# [osx]` or `# [darwin]`: macOS-only dependency

### Special Dependency Types

Fonda supports several special dependency types:

1. **Git Repository Dependencies**:
   - Basic format: `git+https://github.com/user/repo.git`
   - With branch: `git+https://github.com/user/repo.git@branch`
   - With tag: `git+https://github.com/user/repo.git@v1.0.0`
   - With commit hash: `git+https://github.com/user/repo.git@a1b2c3d`
   - SSH format: `git+ssh://git@github.com/user/private-repo.git`

2. **URL Dependencies**:
   - Direct URL to package: `https://example.com/packages/some-package.tar.gz`

3. **Development/Editable Installs**:
   - Current directory: `-e .`
   - Local path: `-e ./path/to/local/package`
   - Editable Git repo: `-e git+https://github.com/user/dev-repo.git`

## Requirements

- Rust 1.67 or higher
- Python 3.x
- pip
- uv (Python package installer) (optional)


## Project Structure

```
fonda
├── src
│   └── main.rs
├── Cargo.toml
├── .gitignore
└── README.md
```

## License

This project is licensed under the MIT License. See the LICENSE file for details.
