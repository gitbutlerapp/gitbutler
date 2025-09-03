//! Tests for push options functionality

use gitbutler_git::{RefSpec, Error};
use std::path::Path;
use std::future::Future;
use gitbutler_git::executor::GitExecutor;

/// Mock executor that captures the arguments passed to git command
#[derive(Debug)]
struct MockExecutor {
    captured_args: std::sync::Mutex<Vec<Vec<String>>>,
}

impl MockExecutor {
    fn new() -> Self {
        Self {
            captured_args: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn get_captured_args(&self) -> Vec<Vec<String>> {
        self.captured_args.lock().unwrap().clone()
    }
}

impl GitExecutor for MockExecutor {
    type Error = std::io::Error;

    async fn execute<P: AsRef<Path>>(
        &self,
        _path: P,
        args: &[&str],
        _envs: Option<std::collections::HashMap<String, String>>,
    ) -> Result<(usize, String, String), Self::Error> {
        // Capture the args for verification
        let args_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        self.captured_args.lock().unwrap().push(args_vec);
        
        // Return success for the test
        Ok((0, String::new(), String::new()))
    }
}

async fn dummy_prompt(_prompt: String, _extra: ()) -> Option<String> {
    None
}

#[tokio::test]
async fn test_push_without_options() -> Result<(), Error<std::io::Error>> {
    let executor = MockExecutor::new();
    let temp_dir = tempfile::tempdir().unwrap();
    
    gitbutler_git::push(
        temp_dir.path(),
        &executor,
        "origin",
        RefSpec::parse("refs/heads/main:refs/heads/main").unwrap(),
        false,
        false,
        None, // no push options
        dummy_prompt,
        (),
    ).await?;

    let captured_args = executor.get_captured_args();
    assert_eq!(captured_args.len(), 1);
    
    let args = &captured_args[0];
    assert_eq!(args[0], "push");
    assert_eq!(args[1], "--quiet");
    assert_eq!(args[2], "--no-verify");
    assert_eq!(args[3], "origin");
    assert_eq!(args[4], "refs/heads/main:refs/heads/main");
    
    // Should not contain any -o flags
    assert!(!args.contains(&"-o".to_string()));
    
    Ok(())
}

#[tokio::test]
async fn test_push_with_single_option() -> Result<(), Error<std::io::Error>> {
    let executor = MockExecutor::new();
    let temp_dir = tempfile::tempdir().unwrap();
    let push_options = vec!["ci.skip"];
    
    gitbutler_git::push(
        temp_dir.path(),
        &executor,
        "origin",
        RefSpec::parse("refs/heads/main:refs/heads/main").unwrap(),
        false,
        false,
        Some(&push_options),
        dummy_prompt,
        (),
    ).await?;

    let captured_args = executor.get_captured_args();
    assert_eq!(captured_args.len(), 1);
    
    let args = &captured_args[0];
    assert_eq!(args[0], "push");
    assert_eq!(args[1], "--quiet");
    assert_eq!(args[2], "--no-verify");
    assert_eq!(args[3], "origin");
    assert_eq!(args[4], "refs/heads/main:refs/heads/main");
    assert_eq!(args[5], "-o");
    assert_eq!(args[6], "ci.skip");
    
    Ok(())
}

#[tokio::test]
async fn test_push_with_multiple_options() -> Result<(), Error<std::io::Error>> {
    let executor = MockExecutor::new();
    let temp_dir = tempfile::tempdir().unwrap();
    let push_options = vec!["ci.skip", "mr.create"];
    
    gitbutler_git::push(
        temp_dir.path(),
        &executor,
        "origin",
        RefSpec::parse("refs/heads/feature:refs/heads/feature").unwrap(),
        false,
        false,
        Some(&push_options),
        dummy_prompt,
        (),
    ).await?;

    let captured_args = executor.get_captured_args();
    assert_eq!(captured_args.len(), 1);
    
    let args = &captured_args[0];
    assert_eq!(args[0], "push");
    assert_eq!(args[1], "--quiet");
    assert_eq!(args[2], "--no-verify");
    assert_eq!(args[3], "origin");
    assert_eq!(args[4], "refs/heads/feature:refs/heads/feature");
    assert_eq!(args[5], "-o");
    assert_eq!(args[6], "ci.skip");
    assert_eq!(args[7], "-o");
    assert_eq!(args[8], "mr.create");
    
    Ok(())
}

#[tokio::test]
async fn test_push_with_force_and_options() -> Result<(), Error<std::io::Error>> {
    let executor = MockExecutor::new();
    let temp_dir = tempfile::tempdir().unwrap();
    let push_options = vec!["ci.skip"];
    
    gitbutler_git::push(
        temp_dir.path(),
        &executor,
        "origin",
        RefSpec::parse("refs/heads/main:refs/heads/main").unwrap(),
        true, // force
        false,
        Some(&push_options),
        dummy_prompt,
        (),
    ).await?;

    let captured_args = executor.get_captured_args();
    assert_eq!(captured_args.len(), 1);
    
    let args = &captured_args[0];
    assert!(args.contains(&"--force".to_string()));
    assert!(args.contains(&"-o".to_string()));
    assert!(args.contains(&"ci.skip".to_string()));
    
    Ok(())
}