use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    #[serde(rename = "type")]
    pub activity_type: String,
    pub timestamp_ms: u128,
    pub message: String,
}

pub fn parse_reflog_line(line: &str) -> Result<Activity> {
    match line.split('\t').collect::<Vec<&str>>()[..] {
        [meta, message] => {
            let meta_parts = meta.split_whitespace().collect::<Vec<&str>>();
            let timestamp_s = meta_parts[meta_parts.len() - 2].parse::<u64>()?;

            match message.split(": ").collect::<Vec<&str>>()[..] {
                [entry_type, msg] => Ok(Activity {
                    activity_type: entry_type.to_string(),
                    message: msg.to_string(),
                    timestamp_ms: timestamp_s as u128 * 1000,
                }),
                _ => Err(anyhow!("failed to parse reflog line: {}", line)),
            }
        }
        _ => Err(anyhow!("failed to parse reflog line: {}", line)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_reflog_line() {
        let test_cases = vec![
        (
            "9ea641990993cb60c7d89c41606f6b457adb9681 3f2657e0d1eae57f58d7734aae10310a861de8e8 Nikita Galaiko <nikita@galaiko.rocks> 1676275740 +0100	commit: try sturdy mac dev certificate",
            Activity{ activity_type: "commit".to_string(), timestamp_ms: 1676275740000, message: "try sturdy mac dev certificate".to_string() }
        ),
        (
            "999bc2f0194ea001f71ba65b5422a742b5e66d9f bb98b5411d597fdede63053c190260a38d459ecb Nikita Galaiko <nikita@galaiko.rocks> 1675428111 +0100	checkout: moving from production-build to master",
            Activity{ activity_type: "checkout".to_string(), timestamp_ms: 1675428111000, message: "moving from production-build to master".to_string() },
        ),
        (
            "0000000000000000000000000000000000000000 9aa96f488fbdb8f7b15151d9d2e47690d3b21b46 Nikita Galaiko <nikita@galaiko.rocks> 1675176957 +0100	commit (initial): simple tauri example",
            Activity{ activity_type: "commit (initial)".to_string(), timestamp_ms: 1675176957000, message: "simple tauri example".to_string() },
        ),
        (
            "d083bb9213fc5e0bb6d07c2c6c1eae5be483be25 dc870a80fddb843583baa36cb637c5c820b1e863 Nikita Galaiko <nikita@galaiko.rocks> 1675425613 +0100	commit (amend): build app with github actions",
            Activity{ activity_type: "commit (amend)".to_string(), timestamp_ms: 1675425613000, message: "build app with github actions".to_string() },
        ),
        (
            "2843be38a72ac8418c7e5c5630cba3c4803916d1 fbb7a9356484b948bde4c7ee9fdeb6439edff8c0 Nikita Galaiko <nikita@galaiko.rocks> 1676274883 +0100	pull: Fast-forward",
            Activity{ activity_type: "pull".to_string(), timestamp_ms: 1676274883000, message: "Fast-forward".to_string() },
        ),
        (
            "3f2657e0d1eae57f58d7734aae10310a861de8e8 3f2657e0d1eae57f58d7734aae10310a861de8e8 Nikita Galaiko <nikita@galaiko.rocks> 1676277401 +0100	reset: moving to HEAD",
            Activity{ activity_type: "reset".to_string() , timestamp_ms: 1676277401000, message: "moving to HEAD".to_string() },
        ),
        (
            "9a831ba2fa07aa6a399bbb498e8effd913cec2e0 add94e65594e4c240b0f6b03973a3be3ff594306 Nikita Galaiko <nikita@galaiko.rocks> 1676039997 +0100	pull --rebase (start): checkout add94e65594e4c240b0f6b03973a3be3ff594306",
            Activity{ activity_type: "pull --rebase (start)".to_string(), timestamp_ms: 1676039997000, message: "checkout add94e65594e4c240b0f6b03973a3be3ff594306".to_string() },
        ),
        (
            "add94e65594e4c240b0f6b03973a3be3ff594306 bcc93167c068649868aa3df4999ba154468a62b5 Nikita Galaiko <nikita@galaiko.rocks> 1676039997 +0100	pull --rebase (pick): make app run in background",
            Activity{ activity_type: "pull --rebase (pick)".to_string(), timestamp_ms: 1676039997000, message: "make app run in background".to_string() },
        ),
        (
            "bcc93167c068649868aa3df4999ba154468a62b5 bcc93167c068649868aa3df4999ba154468a62b5 Nikita Galaiko <nikita@galaiko.rocks> 1676039997 +0100	pull --rebase (finish): returning to refs/heads/master",
            Activity{ activity_type: "pull --rebase (finish)".to_string(), timestamp_ms: 1676039997000, message: "returning to refs/heads/master".to_string() },
        )
    ];

        for (line, expected) in test_cases {
            let actual = parse_reflog_line(line);
            assert!(actual.is_ok());
            assert_eq!(actual.unwrap(), expected);
        }
    }
}
