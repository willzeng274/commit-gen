# commit-gen

A CLI tool that uses Ollama to generate meaningful git commit messages based on your changes. It analyzes your staged and unstaged changes, selects the most relevant files, and generates a well-structured commit message using local LLMs.

## Features

- Uses Ollama to generate commit messages with a local LLM
- Smart file selection with detailed change statistics
  - Shows number of lines changed per file
  - Prioritizes files with significant changes
  - Provides clear overview of modifications
- Supports conventional commits format with emojis
- Shows detailed diffs with configurable preview sizes
- Highly configurable behaviour through TOML
- Works completely offline with your local Ollama instance
- Supports custom commit dates for time-traveling commits

## Prerequisites

1. Install [Rust](https://rustup.rs/)
2. Install [Ollama](https://ollama.ai/)
3. Pull your preferred model:
   ```bash
   # Recommended models (lightweight & good performance):
   ollama pull codellama
   ollama pull llama3.2
   ```

## Installation

```bash
cargo install --path .
```

## Usage

1. Make some changes to your git repository
2. Run the tool:
   ```bash
   commit-gen

   # With custom date
   commit-gen -d "2 days ago"
   commit-gen --date "2024-03-20 15:30:00"
   
   # With separate author/committer dates
   commit-gen --author-date "3 hours ago" --committer-date "1 hour ago"
   ```

### Command Line Options

- `-c, --config <PATH>`: Use custom config file (searches in order: ./config/default.toml, ~/.config/commit-gen/config.toml, ~/.commit-gen/config.toml, ~/.commit-gen.toml)
- `-y, --yes`: Skip confirmation and commit directly (NOT recommended unless you trust the LLM)
- `-d, --diff`: Show full diff while generating (NOT recommended for large diffs)
- `-v, --verbose`: Show debug information
- `-x, --xml`: Show raw XML response from LLM (useful for using the CLI as a library)
- `-i, --issue <NUMBER>`: Reference an issue number
- `-p, --pr <NUMBER>`: Reference a PR number
- `-d, --date <DATE>`: Set both author and committer dates
- `--author-date <DATE>`: Set author date specifically
- `--committer-date <DATE>`: Set committer date specifically

Date formats supported:
- Exact: "YYYY-MM-DD HH:MM:SS" (e.g., "2024-03-20 15:30:00")
- Relative: "X units ago" where units can be: minute(s), hour(s), day(s), week(s), month(s), year(s)

## Configuration

The tool is highly configurable through a TOML file. Here's the default configuration with explanations:

```toml
[model]
# Name of the Ollama model to use
name = "codellama"  # "llama3.2" is good as well
# Controls randomness in output (0.0 = deterministic, 1.0 = random)
file_selection_temperature = 0.2  # Low for consistent file selection
commit_temperature = 0.5          # Higher for creative commit messages
# Nucleus sampling threshold (0.0 to 1.0)
top_p = 0.9
# Maximum tokens in the response
max_tokens = 500

[commit]
# Enable conventional commit format (feat:, fix:, etc.)
conventional = true
# Add commit type emojis (✨, 🐛, etc.)
emoji = true
# Maximum length of the commit message's first line
max_message_length = 50

[git]
# Which changes to analyze
include_staged = true
include_unstaged = true
# Patterns to exclude from analysis
exclude_patterns = [
    "*.lock",
    "target/",
    "dist/",
    "node_modules/"
]

[selection]
# File selection parameters
min_files = 2      # Minimum files to analyze
max_files = 10     # Maximum files to analyze
prioritize_src = true     # Prefer files in src/ directory
exclude_tests = true      # Skip test files unless crucial
min_changes = 5          # Minimum line changes to consider a file significant

[formatting]
# Maximum lines to show in full diff view
max_diff_lines = 15
# Number of lines to show at start of large diffs
preview_lines = 10
# Number of lines to show in summaries
summary_lines = 5
# XML indentation size
indent_size = 2
# Show line count statistics for each file
show_file_stats = true
```

## How It Works

1. **File Selection**: 
   - Analyzes all changed files and shows detailed statistics
   - Displays number of lines changed for each file
   - Uses LLM to select 2-10 most relevant files
   - Prioritizes src/ directory and non-test files
   - Excludes files matching exclude_patterns

2. **Change Analysis**:
   - Shows full diff for small changes (≤15 lines)
   - For larger changes, shows first 10 and last 5 lines
   - Summarizes other files with first 5 lines
   - Includes line count statistics for all files
   - Clearly indicates added and deleted lines

3. **Commit Generation**:
   - Uses LLM to analyze selected changes
   - Generates conventional commit message
   - Adds emoji based on commit type
   - Includes detailed bullet-point description
   - References issues/PRs if specified
   - Supports custom commit dates for time travel

4. **XML Processing**:
   - Uses structured XML format for reliable parsing
   - Validates and fixes common XML issues
   - Ensures consistent formatting

## Model Selection

We tested codellama and llama3.2. Both models typically produce decently good results within 1-3 iterations.

## License

MIT 