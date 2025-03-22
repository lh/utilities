# README.md

# Fonda

Fonda is a Rust-based CLI tool that manages Python virtual environments and dependencies from YAML configuration files.

## Features

- Creates Python virtual environments using both `uv` and standard `venv`
- Converts conda-style environment.yaml files to requirements.txt
- Installs Python package dependencies
- Supports multiple commands for different operations

## Installation

`
### From Source

```sh
# Clone the repository
git clone https://github.com/yourusername/fonda.git
cd fonda

# Build the project
cargo build --release

# The binary will be available at ./target/release/fonda
```

Usage
The tool supports three main operations:


# Create environment and install dependencies
fonda

# Install from existing requirements.txt
fonda -r

# Generate requirements.txt from environment.yaml
fonda -w


Configuration
Create an environment.yaml file in your project root:

name: myenv
dependencies:
  - "pip:numpy,pandas"
  - "pip:scikit-learn"

## Requirements

Rust 1.67 or higher
Python 3.x
pip
uv (Python package installer)
Building from Source


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