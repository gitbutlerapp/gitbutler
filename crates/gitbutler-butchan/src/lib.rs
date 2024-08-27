use anyhow::Result;
use gix::progress;
use std::{path::PathBuf, str::FromStr};
use uuid::Uuid;

use tokio::fs;

#[derive(Debug, Clone)]
struct Message {
    author: String,
    message: String,
}

#[derive(Debug, Clone)]
struct Channel {
    identifier: Option<Uuid>,
    file_position: usize,
}

pub async fn handle_project_change(repository_path: PathBuf) {
    let repository = gix::open(repository_path.clone()).unwrap();

    let statuses = repository.status(progress::Discard).unwrap();

    let files_with_changes = statuses
        .into_index_worktree_iter(None)
        .unwrap()
        .map(|status_entry| {
            let item = status_entry.unwrap();

            repository_path.join(PathBuf::from_str(&item.rela_path().to_string()).unwrap())
        })
        .collect::<Vec<_>>();

    for file in files_with_changes {
        parse_file(file).await.unwrap();
    }

    println!("\n\n\n\n\n\n\nDetected a change!!!!\n\n\n\n\n\n\n");
}

const GB_CHAT_HEADER: &str = "// GB Chat";

async fn parse_file(file_path: PathBuf) -> Result<()> {
    let file = fs::read_to_string(file_path.clone()).await?;

    let mut channels = find_channels(&file)?;

    for (index, channel) in channels.clone().iter().enumerate() {
        if channel.identifier.is_none() {
            channels[index] = add_uuid(file_path.clone(), &file, channel).await?;
        }
    }

    Ok(())
}

async fn find_messages_and_update_prompt(
    file_path: PathBuf,
    file_contents: &str,
    channel: &Channel,
) -> Result<Vec<Message>> {
    Ok(vec![])
}

async fn add_uuid(file_path: PathBuf, file_contents: &str, channel: &Channel) -> Result<Channel> {
    let uuid = Uuid::new_v4();

    let mut lines = file_contents.lines().collect::<Vec<&str>>();

    let new_line = format!("{} <{}>", GB_CHAT_HEADER, uuid.as_hyphenated());
    lines[channel.file_position] = &new_line;

    // hehehe fuck windows
    fs::write(file_path, lines.join("\n")).await?;

    Ok(Channel {
        identifier: Some(uuid),
        file_position: channel.file_position,
    })
}

fn find_channels(file: &str) -> Result<Vec<Channel>> {
    let mut channels = Vec::new();

    for (file_position, line) in file.lines().enumerate() {
        if line.starts_with(GB_CHAT_HEADER) {
            if let Some(rest) = line.strip_prefix(GB_CHAT_HEADER) {
                let mut uuid = String::new();

                let mut started_uuid = false;

                'uuid: for char in rest.chars() {
                    if started_uuid {
                        if char == '>' {
                            break 'uuid;
                        } else {
                            uuid.push(char)
                        }
                    }

                    if !started_uuid && char == '<' {
                        started_uuid = true
                    }
                }

                if started_uuid {
                    if let Ok(uuid) = Uuid::parse_str(&uuid) {
                        channels.push(Channel {
                            identifier: Some(uuid),
                            file_position,
                        })
                    }
                } else {
                    channels.push(Channel {
                        identifier: None,
                        file_position,
                    })
                }
            } else {
                // unsure this ever happens
                channels.push(Channel {
                    identifier: None,
                    file_position,
                })
            }
        }
    }

    Ok(channels)
}
