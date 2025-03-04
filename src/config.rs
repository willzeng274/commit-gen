use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub model: ModelConfig,
    pub commit: CommitConfig,
    pub git: GitConfig,
    pub selection: FileSelectionConfig,
    pub formatting: FormattingConfig,
    pub prompts: PromptsConfig,
}

#[derive(Debug, Deserialize)]
pub struct ModelConfig {
    pub name: String,
    pub top_p: f32,
    pub max_tokens: u32,
    pub file_selection_temperature: f32,
    pub commit_temperature: f32,
}

#[derive(Debug, Deserialize)]
pub struct CommitConfig {
    pub conventional: bool,
    pub emoji: bool,
    pub max_message_length: u32,
}

#[derive(Debug, Deserialize)]
pub struct GitConfig {
    pub include_staged: bool,
    pub include_unstaged: bool,
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct FileSelectionConfig {
    pub min_files: usize,
    pub max_files: usize,
    pub prioritize_src: bool,
    pub exclude_tests: bool,
    pub min_changes: usize,
}

#[derive(Debug, Deserialize)]
pub struct FormattingConfig {
    pub max_diff_lines: usize,
    pub preview_lines: usize,
    pub summary_lines: usize,
    pub indent_size: usize,
    pub show_file_stats: bool,
}

#[derive(Debug, Deserialize)]
pub struct PromptsConfig {
    pub file_selection_system: String,
    pub file_selection_context: String,
    pub commit_system: String,
    pub commit_context: String,
    pub placeholders: PromptPlaceholders,
}

#[derive(Debug, Deserialize)]
pub struct PromptPlaceholders {
    pub changes_summary: String,
    pub changes_text: String,
    pub indent_size: String,
    pub max_message_length: String,
    pub min_files: String,
    pub max_files: String,
}