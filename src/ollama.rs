use anyhow::{Context, Result};
use ollama_rs::{
    generation::{
        completion::request::GenerationRequest,
        options::GenerationOptions,
    },
    Ollama,
};
use std::collections::HashSet;

use crate::{config::Config, git::GitChanges};

fn format_prompt(template: &str, replacements: &[(&str, &str)]) -> String {
    let mut result = template.to_string();
    for (placeholder, value) in replacements {
        if *placeholder == "indent" {
            result = result.replace("{indent}", value);
        } else {
            result = result.replace(placeholder, value);
        }
    }
    result
}

async fn get_files_to_examine(ollama: &Ollama, config: &Config, changes: &GitChanges, verbose: bool) -> Result<HashSet<String>> {
    let indent = " ".repeat(config.formatting.indent_size);
    
    let mut changes_summary = changes.summary.clone();
    changes_summary.push_str("\nDetailed file statistics:\n");
    for (path, change) in &changes.files {
        let mut total_changes = 0;
        
        for line in change.diff.lines() {
            match line.chars().next() {
                Some('+') => total_changes += 1,
                Some('-') => total_changes += 1,
                _ => {}
            }
        }
        
        if total_changes > 0 {
            changes_summary.push_str(&format!("  {} ({}) - {} lines changed\n", path, change.status, total_changes));
        }
    }

    let changes_summary = changes_summary.as_str();
    let indent_size = config.formatting.indent_size.to_string();
    let min_files = config.selection.min_files.to_string();
    let max_files = config.selection.max_files.to_string();
    
    let replacements = [
        (config.prompts.placeholders.changes_summary.as_str(), changes_summary),
        (config.prompts.placeholders.indent_size.as_str(), indent_size.as_str()),
        (config.prompts.placeholders.min_files.as_str(), min_files.as_str()),
        (config.prompts.placeholders.max_files.as_str(), max_files.as_str()),
        ("indent", indent.as_str()),
    ];
    
    let context = format_prompt(&config.prompts.file_selection_context, &replacements);

    if verbose {
        println!("\n=== Debug: File selection context ===\n{}\n===\n", context);
    }

    let options = GenerationOptions::default()
        .temperature(config.model.file_selection_temperature)
        .top_p(config.model.top_p)
        .num_predict(config.model.max_tokens as i32)
        .stop(vec!["</files>".to_string()]);

    let request = GenerationRequest::new(
        config.model.name.to_string(),
        context,
    )
    .system(config.prompts.file_selection_system.clone())
    .options(options);

    let response = ollama
        .generate(request)
        .await
        .context("Failed to get file selection")?;

    let mut response_text = response.response.trim().to_string();
    
    if !response_text.starts_with("<files>") {
        response_text = format!("<files>\n{}", response_text);
    }
    if !response_text.ends_with("</files>") {
        response_text.push_str("\n</files>");
    }

    if verbose {
        println!("=== Debug: Raw LLM Response ===\n{}\n===\n", response_text);
    }

    let mut files = HashSet::new();

    if let Some(start) = response_text.find("<files>") {
        if let Some(end) = response_text.find("</files>") {
            let files_content = &response_text[start + 7..end];
            for line in files_content.lines() {
                let trimmed = line.trim();
                if let Some(file_path) = trimmed
                    .strip_prefix("<file>")
                    .and_then(|s| s.strip_suffix("</file>"))
                {
                    files.insert(file_path.trim().to_string());
                }
            }
        }
    }

    if files.len() < config.selection.min_files {
        let mut available_files: Vec<_> = changes.files
            .iter()
            .filter(|(path, change)| {
                !files.contains(*path) && 
                change.diff.is_empty() && 
                change.line_count >= config.selection.min_changes &&
                !(config.selection.exclude_tests && path.contains("test")) &&
                !config.git.exclude_patterns.iter().any(|pattern| path.contains(pattern))
            })
            .collect();
        
        if config.selection.prioritize_src {
            available_files.sort_by_key(|(path, _)| !path.starts_with("src/"));
        }
        
        available_files.sort_by_key(|(_, change)| change.diff.len());
        available_files.reverse();
        
        for (path, _) in available_files.iter().take(config.selection.min_files - files.len()) {
            files.insert((*path).clone());
        }
    }

    if verbose {
        println!("=== Debug: Selected files for detailed examination ===");
        for file in &files {
            println!("  - {}", file);
        }
        println!("===\n");
    }

    Ok(files)
}

pub async fn generate_commit_message(config: &Config, changes: &GitChanges, verbose: bool) -> Result<(String, String)> {
    let ollama = Ollama::default();
    
    let files_to_examine = get_files_to_examine(&ollama, config, changes, verbose).await?;
    
    let mut changes_text = String::new();
    
    let mut has_diffs = false;
    for (path, change) in &changes.files {
        if files_to_examine.contains(path) && !change.diff.is_empty() {
            if !has_diffs {
                changes_text.push_str("Detailed changes in selected files:\n");
                has_diffs = true;
            }
            if config.formatting.show_file_stats {
                changes_text.push_str(&format!("\nIn {} ({}) - {} lines changed:\n```diff\n", path, change.status, change.line_count));
            } else {
                changes_text.push_str(&format!("\nIn {} ({}):\n```diff\n", path, change.status));
            }
            
            if change.line_count > config.formatting.max_diff_lines {
                let lines: Vec<_> = change.diff.lines().collect();
                let first_lines = lines.iter().take(config.formatting.preview_lines).cloned().collect::<Vec<_>>().join("\n");
                let last_lines = lines.iter().rev().take(config.formatting.summary_lines).cloned().collect::<Vec<_>>().join("\n");
                changes_text.push_str(&format!("{}\n[...{} lines skipped...]\n{}\n", 
                    first_lines, 
                    change.line_count - config.formatting.preview_lines - config.formatting.summary_lines,
                    last_lines
                ));
            } else {
                changes_text.push_str(&change.diff);
            }
            changes_text.push_str("```\n");
        }
    }
    
    let mut other_changes = false;
    for (path, change) in &changes.files {
        if !files_to_examine.contains(path) && !change.diff.is_empty() {
            if !other_changes {
                changes_text.push_str("\nOther changes (summarized):\n");
                other_changes = true;
            }
            if config.formatting.show_file_stats {
                changes_text.push_str(&format!("\nIn {} ({}) - {} lines changed:\n```diff\n", path, change.status, change.line_count));
            } else {
                changes_text.push_str(&format!("\nIn {} ({}):\n```diff\n", path, change.status));
            }
            
            let first_lines = change.diff.lines().take(config.formatting.summary_lines).collect::<Vec<_>>().join("\n");
            if change.line_count > config.formatting.summary_lines {
                changes_text.push_str(&format!("{}\n[...{} additional lines not shown...]\n", 
                    first_lines, 
                    change.line_count - config.formatting.summary_lines
                ));
            } else {
                changes_text.push_str(&first_lines);
            }
            changes_text.push_str("```\n");
        }
    }

    let indent = " ".repeat(config.formatting.indent_size);
    let replacements = [
        (config.prompts.placeholders.changes_summary.as_str(), changes.summary.as_str()),
        (config.prompts.placeholders.changes_text.as_str(), &changes_text),
        (config.prompts.placeholders.indent_size.as_str(), &config.formatting.indent_size.to_string()),
        (config.prompts.placeholders.max_message_length.as_str(), &config.commit.max_message_length.to_string()),
        ("indent", &indent),
    ];
    
    let context = format_prompt(&config.prompts.commit_context, &replacements);
    
    if verbose {
        println!("\n=== Debug: Context sent to LLM ===\n{}\n===\n", context);
    }

    let options = GenerationOptions::default()
        .temperature(config.model.commit_temperature)
        .top_p(config.model.top_p)
        .num_predict(config.model.max_tokens as i32)
        .stop(vec!["</commit>".to_string()]);

    let request = GenerationRequest::new(
        config.model.name.to_string(),
        context,
    )
    .system(config.prompts.commit_system.clone())
    .options(options);
    
    let response = ollama
        .generate(request)
        .await
        .context("Failed to generate commit message")?;
    
    let mut commit_message = response.response.trim().to_string();
    
    if !commit_message.starts_with("<commit>") {
        commit_message = format!("<commit>\n{}", commit_message);
    }
    if !commit_message.ends_with("</commit>") {
        commit_message.push_str("\n</commit>");
    }

    // some old edge case cleanup
    // commit_message = commit_message
    //     .replace("  message:", "  <message>")
    //     .replace("  description:", "  <description>")
    //     .replace("</message\n", "</message>\n")
    //     .replace("</description\n", "</description>\n");
    
    if verbose {
        println!("=== Debug: Raw LLM Response ===\n{}\n===\n", commit_message);
    }

    let message = if let Some(start) = commit_message.find("<message>") {
        if let Some(end) = commit_message.find("</message>") {
            if verbose {
                println!("=== Debug: Found message tags at positions {} to {} ===\n", start, end);
            }
            commit_message[start + 9..end].trim().to_string()
        } else {
            if verbose {
                println!("=== Debug: Found opening <message> but no closing tag ===\n");
            }
            commit_message.trim().to_string()
        }
    } else {
        if verbose {
            println!("=== Debug: No message tags found ===\n");
        }
        commit_message.trim().to_string()
    };

    if verbose {
        println!("=== Debug: Extracted message ===\n{}\n===\n", message);
    }
    let mut final_message = message;
    
    if config.commit.conventional {
        if !final_message.contains("feat:") 
            && !final_message.contains("fix:") 
            && !final_message.contains("docs:") 
            && !final_message.contains("style:") 
            && !final_message.contains("refactor:") 
            && !final_message.contains("test:") 
            && !final_message.contains("chore:") {
            let message_lower = final_message.to_lowercase();
            let commit_type = if message_lower.contains("fix") || message_lower.contains("bug") {
                "fix"
            } else if message_lower.contains("add") || message_lower.contains("new") || message_lower.contains("feat") {
                "feat"
            } else if message_lower.contains("doc") {
                "docs"
            } else if message_lower.contains("style") {
                "style"
            } else if message_lower.contains("refactor") {
                "refactor"
            } else if message_lower.contains("test") {
                "test"
            } else {
                "chore"
            };
            final_message = format!("{}: {}", commit_type, final_message);
            if verbose {
                println!("=== Debug: Added conventional commit type ===\n{}\n===\n", final_message);
            }
        }
    }
    
    if config.commit.emoji {
        let emoji = match final_message.split(':').next().unwrap_or("") {
            "feat" => "✨",
            "fix" => "🐛",
            "docs" => "📚",
            "style" => "💄",
            "refactor" => "♻️",
            "test" => "✅",
            "chore" => "🔨",
            _ => "🔨",
        };
        final_message = format!("{} {}", emoji, final_message);
        if verbose {
            println!("=== Debug: Added emoji ===\n{}\n===\n", final_message);
        }
    }

    if let Some(start) = commit_message.find("<description>") {
        if let Some(end) = commit_message.find("</description>") {
            if verbose {
                println!("=== Debug: Found description tags at positions {} to {} ===\n", start, end);
            }
            let description = commit_message[start + 13..end].trim();
            if !description.is_empty() {
                final_message = format!("{}\n\n{}", final_message, description);
                if verbose {
                    println!("=== Debug: Added description ===\n{}\n===\n", final_message);
                }
            }
        }
    }
    
    Ok((final_message, commit_message))
} 