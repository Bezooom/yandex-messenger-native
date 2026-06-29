# Contributing to Yandex Messenger Native

We are excited that you want to contribute to Yandex Messenger Native! This project is written in Rust using GTK4 and Libadwaita. Below are the basic guidelines and recommendations for developers.

## Environment Requirements

To build and test the project, you will need:
- **Rust Toolchain**: Install the stable version of Rust via [rustup](https://rustup.rs/).
- **GTK4 and Libadwaita**: System development libraries. On Ubuntu/Debian, install them with:
  ```bash
  sudo apt install -y libgtk-4-dev libadwaita-1-dev pkg-config
  ```
- Optional (for voice recording support): `libgstreamer1.0-dev`, `libgstreamer-plugins-base1.0-dev`.

## Development Process

### 1. Preparation
Fork the repository and clone it locally:
```bash
git clone https://github.com/your-username/yandex-messenger-native.git
cd yandex-messenger-native
```

### 2. Build and Run
You can use standard cargo commands or the Makefile:
```bash
make build  # Build the project
make run    # Run the application
```

### 3. Code Quality Checks
Before submitting your changes, ensure that your code passes all validation gates:
- **Formatting**:
  ```bash
  cargo fmt --check
  ```
- **Linting (Clippy)**:
  ```bash
  cargo clippy -- -D warnings
  ```
- **Tests**:
  ```bash
  cargo test
  ```

## Contribution Rules

### Branching
- Create a separate branch for each feature or bugfix:
  ```bash
  git checkout -b feature/your-feature-name
  # or
  git checkout -b fix/bug-description
  ```

### Commits
- Write clear commit messages.
- Avoid commits with generic names like "fix" or "update".

### Submitting a Pull Request
1. Make sure your branch is up to date with `main`.
2. Push your branch and open a Pull Request against the `main` branch of the original repository.
3. Describe the changes in detail in the PR description and list any issues they resolve.
4. Wait for the CI pipeline to pass and for the maintainers to review your code.
