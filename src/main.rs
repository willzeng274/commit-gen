mod config;
mod git;
mod ollama;
mod utils;

use anyhow::Result;
use clap::Parser;
use colored::*;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    config: Option<PathBuf>,

    #[arg(short, long)]
    yes: bool,

    #[arg(short, long)]
    diff: bool,

    #[arg(short, long)]
    verbose: bool,

    #[arg(short = 'x', long)]
    xml: bool,

    #[arg(short = 'i', long)]
    issue: Option<u32>,

    #[arg(short = 'p', long)]
    pr: Option<u32>,

    #[arg(long)]
    date: Option<String>,

    #[arg(long)]
    author_date: Option<String>,

    #[arg(long)]
    committer_date: Option<String>,

    #[arg(long)]
    amend: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    let config = utils::load_config(cli.config)?;
    
    let git_changes = git::get_changes(&config.git)?;
    
    if git_changes.is_empty() {
        println!("{}", "No changes to commit!".yellow());
        return Ok(());
    }
    
    let (commit_message, raw_xml) = ollama::generate_commit_message(&config, &git_changes, cli.verbose).await?;
    
    if cli.diff {
        println!("\n{}", "Changes:".green().bold());
        println!("{}", git_changes);
    }

    if cli.xml {
        println!("\n{}", "Raw XML Response:".blue().bold());
        println!("{}", raw_xml);
    }
    
    println!("\n{}", "Generated Commit Message:".green().bold());
    let mut final_message = commit_message;

    let mut references = Vec::new();
    if let Some(issue) = cli.issue {
        references.push(format!("Fixes issue #{}", issue));
    }
    if let Some(pr) = cli.pr {
        references.push(format!("Related to PR #{}", pr));
    }
    if !references.is_empty() {
        final_message = format!("{}\n\n{}", final_message, references.join("\n"));
    }

    println!("{}", final_message);
    
    if !cli.yes {
        print!("\n{}", "Do you want to commit with this message? [Y/n] ".cyan());
        std::io::stdout().flush()?;
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        
        if input.trim().to_lowercase() != "y" {
            println!("{}", "Commit aborted.".yellow());
            return Ok(());
        }
    }
    
    git::create_commit(
        &final_message, 
        cli.date.as_deref(),
        cli.author_date.as_deref(),
        cli.committer_date.as_deref(),
        cli.amend,
    )?;
    
    Ok(())
}
