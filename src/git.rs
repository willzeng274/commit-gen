use anyhow::{Context, Result};
use git2::{Repository, Status, StatusOptions, DiffOptions, Time};
use std::fmt;
use std::collections::HashMap;
use chrono::{NaiveDateTime, Duration, Local, TimeZone};
use regex::Regex;

use crate::config::GitConfig;

#[derive(Default)]
pub struct FileChange {
    pub status: String,
    pub diff: String,
    pub line_count: usize,
}

pub struct GitChanges {
    pub staged: Vec<String>,
    pub unstaged: Vec<String>,
    pub files: HashMap<String, FileChange>,
    pub summary: String,
}

impl GitChanges {
    pub fn is_empty(&self) -> bool {
        self.staged.is_empty() && self.unstaged.is_empty()
    }
}

impl fmt::Display for GitChanges {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.summary)?;
        
        for (path, change) in &self.files {
            if !change.diff.is_empty() {
                writeln!(f, "\nChanges in {} ({}):", path, change.status)?;
                writeln!(f, "{}", change.diff)?;
            }
        }
        Ok(())
    }
}

pub fn get_changes(config: &GitConfig) -> Result<GitChanges> {
    let repo = Repository::open_from_env()
        .context("Failed to open git repository")?;
    
    let mut options = StatusOptions::new();
    options.include_untracked(true);
    
    let statuses = repo.statuses(Some(&mut options))
        .context("Failed to get git status")?;
    
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut files = HashMap::new();
    
    for entry in statuses.iter() {
        let path = entry.path().unwrap_or("unknown").to_string();
        let status = entry.status();
        let mut file_change = FileChange::default();
        
        if config.include_staged && (status.is_index_new() || status.is_index_modified() || status.is_index_deleted()) {
            staged.push(format!("{} ({})", path, status_to_string(status)));
            file_change.status = status_to_string(status).to_string();
            
            if let Ok(diff) = get_file_diff(&repo, &path, true) {
                let line_count = diff.lines().count();
                file_change.line_count = line_count;
                file_change.diff = diff;
            }
        }
        
        if config.include_unstaged && (status.is_wt_modified() || status.is_wt_deleted() || status.is_wt_new()) {
            unstaged.push(format!("{} ({})", path, status_to_string(status)));
            if file_change.status.is_empty() {
                file_change.status = status_to_string(status).to_string();
                
                if let Ok(diff) = get_file_diff(&repo, &path, false) {
                    let line_count = diff.lines().count();
                    file_change.line_count = line_count;
                    file_change.diff = diff;
                }
            }
        }
        
        if !file_change.status.is_empty() {
            files.insert(path, file_change);
        }
    }
    
    let mut summary = String::new();
    if !staged.is_empty() {
        summary.push_str("Staged changes:\n");
        for change in &staged {
            summary.push_str(&format!("  {}\n", change));
        }
    }
    if !unstaged.is_empty() {
        if !summary.is_empty() {
            summary.push('\n');
        }
        summary.push_str("Unstaged changes:\n");
        for change in &unstaged {
            summary.push_str(&format!("  {}\n", change));
        }
    }
    
    Ok(GitChanges { staged, unstaged, files, summary })
}

pub fn create_commit(
    message: &str,
    date: Option<&str>,
    author_date: Option<&str>,
    committer_date: Option<&str>,
    amend: bool,
) -> Result<()> {
    let repo = Repository::open_from_env()
        .context("Failed to open git repository")?;
    
    let mut index = repo.index()
        .context("Failed to get index")?;
    
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .context("Failed to add files to index")?;
    
    index.write()
        .context("Failed to write index")?;
    
    let tree_id = index.write_tree()
        .context("Failed to write tree")?;
    
    let tree = repo.find_tree(tree_id)
        .context("Failed to find tree")?;

    let (author_time, author_offset) = parse_git_date(&author_date.or(date).map(String::from))?;
    let (committer_time, committer_offset) = parse_git_date(&committer_date.or(date).map(String::from))?;
    
    let default_sig = repo.signature()
        .context("Failed to get signature")?;
    
    let author = if let Some(time) = author_time {
        git2::Signature::new(
            default_sig.name().unwrap_or(""),
            default_sig.email().unwrap_or(""),
            &Time::new(time, author_offset)
        ).context("Failed to create author signature")?
    } else {
        default_sig.clone()
    };

    let committer = if let Some(time) = committer_time {
        git2::Signature::new(
            default_sig.name().unwrap_or(""),
            default_sig.email().unwrap_or(""),
            &Time::new(time, committer_offset)
        ).context("Failed to create committer signature")?
    } else {
        default_sig
    };

    if amend {
        let head = repo.head()
            .context("Failed to get HEAD reference")?;
        let head_commit = head.peel_to_commit()
            .context("Failed to get HEAD commit")?;
        
        head_commit.amend(
            Some("HEAD"),
            Some(&author),
            Some(&committer),
            None,
            Some(message),
            Some(&tree)
        ).context("Failed to amend commit")?;
    } else {
        let parent = repo.head()
            .ok()
            .and_then(|head| head.target())
            .and_then(|oid| repo.find_commit(oid).ok());
        
        let parents: Vec<&git2::Commit> = match parent {
            Some(ref p) => vec![p],
            None => vec![],
        };

        repo.commit(
            Some("HEAD"),
            &author,
            &committer,
            message,
            &tree,
            &parents,
        ).context("Failed to create commit")?;
    }
    
    Ok(())
}

fn parse_git_date(date_str: &Option<String>) -> Result<(Option<i64>, i32)> {
    if let Some(date) = date_str {
        
        if let Ok(dt) = NaiveDateTime::parse_from_str(date, "%Y-%m-%d %H:%M:%S") {
            let local_dt = Local.from_local_datetime(&dt).single().unwrap();
            let offset = local_dt.offset().local_minus_utc() / 60;
            return Ok((Some(local_dt.timestamp()), offset as i32));
        }

        let re = Regex::new(r"^(\d+)\s+(minute|hour|day|week|month|year)s?\s+ago$").unwrap();
        if let Some(caps) = re.captures(date) {
            let amount: i64 = caps[1].parse().unwrap_or(0);
            let unit = &caps[2];
            
            let now = Local::now();
            
            let duration = match unit {
                "minute" => Duration::minutes(amount),
                "hour" => Duration::hours(amount),
                "day" => Duration::days(amount),
                "week" => Duration::weeks(amount),
                "month" => Duration::days(amount * 30),
                "year" => Duration::days(amount * 365),
                _ => Duration::zero(),
            };
            
            let target_time = now - duration;
            let offset = target_time.offset().local_minus_utc() / 60;
            return Ok((Some(target_time.timestamp()), offset as i32));
        }

        return Err(anyhow::anyhow!("Invalid date format. Use 'YYYY-MM-DD HH:MM:SS' or relative format like '2 days ago'"));
    }

    Ok((None, 0))
}

fn status_to_string(status: Status) -> &'static str {
    if status.is_index_new() || status.is_wt_new() { "added" }
    else if status.is_index_modified() || status.is_wt_modified() { "modified" }
    else if status.is_index_deleted() || status.is_wt_deleted() { "deleted" }
    else { "unknown" }
}

fn get_file_diff(repo: &Repository, path: &str, staged: bool) -> Result<String> {
    let mut diff_opts = DiffOptions::new();
    diff_opts.pathspec(path);
    diff_opts.context_lines(3);
    diff_opts.id_abbrev(7);
    
    let diff = if staged {
        let head = repo.head()?.peel_to_tree()?;
        repo.diff_tree_to_index(Some(&head), None, Some(&mut diff_opts))?
    } else {
        repo.diff_index_to_workdir(None, Some(&mut diff_opts))?
    };

    let mut diff_text = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        match line.origin() {
            '+' | '-' | ' ' => {
                if let Ok(str) = std::str::from_utf8(line.content()) {
                    diff_text.push(line.origin());
                    diff_text.push_str(str);
                }
            }
            _ => {}
        }
        true
    })?;
    
    Ok(diff_text)
} 