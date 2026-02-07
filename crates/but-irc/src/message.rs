//! IRC message event types.

use irc::client::prelude::*;
use irc::proto::Message;
use irc::proto::command::BatchSubCommand;
use serde::{Deserialize, Serialize};

/// High-level IRC event types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum IrcEvent {
    /// Server PING message
    Ping {
        /// Ping ID to respond with
        id: String,
    },

    /// RPL_WELCOME (001) - connection registered
    Welcome {
        /// Our confirmed nickname
        nick: String,
        /// Welcome message
        message: String,
    },

    /// User joined a channel
    UserJoined {
        /// Channel name
        channel: String,
        /// User who joined
        nick: String,
        /// IRCv3 `batch` reference tag
        batch: Option<String>,
        /// IRCv3 `server-time` tag (ISO 8601)
        server_time: Option<String>,
        /// IRCv3 `msgid` tag
        msgid: Option<String>,
    },

    /// User left a channel
    UserParted {
        /// Channel name
        channel: String,
        /// User who left
        nick: String,
        /// Part message
        message: Option<String>,
        /// IRCv3 `batch` reference tag
        batch: Option<String>,
        /// IRCv3 `server-time` tag (ISO 8601)
        server_time: Option<String>,
        /// IRCv3 `msgid` tag
        msgid: Option<String>,
    },

    /// User quit IRC
    UserQuit {
        /// User who quit
        nick: String,
        /// Quit message
        message: Option<String>,
        /// IRCv3 `batch` reference tag
        batch: Option<String>,
        /// IRCv3 `server-time` tag (ISO 8601)
        server_time: Option<String>,
        /// IRCv3 `msgid` tag
        msgid: Option<String>,
    },

    /// RPL_NAMREPLY (353) - channel user list
    NamesList {
        /// Channel name
        channel: String,
        /// List of nicks (may have @ or + prefix)
        names: Vec<String>,
    },

    /// Channel message (PRIVMSG to #channel)
    ChannelMessage {
        /// Sender nick
        from: String,
        /// Channel name
        channel: String,
        /// Message text
        text: String,
        /// Extracted data payload from +data tag (if present)
        data: Option<String>,
        /// IRCv3 `server-time` tag (ISO 8601), present on history replay messages
        server_time: Option<String>,
        /// IRCv3 `batch` reference tag, set when this message belongs to a BATCH
        batch: Option<String>,
        /// IRCv3 `msgid` tag — unique message identifier for pagination
        msgid: Option<String>,
        /// IRCv3 `+draft/reply` tag — msgid of the message being replied to
        reply_to: Option<String>,
    },

    /// Private message (PRIVMSG to nick)
    PrivateMessage {
        /// Sender nick
        from: String,
        /// PRIVMSG target (the recipient nick)
        target: String,
        /// Message text
        text: String,
        /// Extracted data payload from +data tag (if present)
        data: Option<String>,
        /// IRCv3 `server-time` tag (ISO 8601)
        server_time: Option<String>,
        /// IRCv3 `batch` reference tag
        batch: Option<String>,
        /// IRCv3 `msgid` tag — unique message identifier for pagination
        msgid: Option<String>,
        /// IRCv3 `+draft/reply` tag — msgid of the message being replied to
        reply_to: Option<String>,
    },

    /// NOTICE message
    Notice {
        /// Sender (nick or server)
        from: Option<String>,
        /// Target (channel or nick)
        target: String,
        /// Notice text
        message: String,
    },

    /// TOPIC message
    ChannelTopic {
        /// Channel name
        channel: String,
        /// Topic text
        topic: String,
    },

    /// Nick change
    NickChanged {
        /// Old nickname
        old_nick: String,
        /// New nickname
        new_nick: String,
        /// IRCv3 `batch` reference tag
        batch: Option<String>,
        /// IRCv3 `server-time` tag (ISO 8601)
        server_time: Option<String>,
        /// IRCv3 `msgid` tag
        msgid: Option<String>,
    },

    /// RPL_WHOREPLY (352) — one entry from a WHO response.
    WhoReply {
        /// Channel name
        channel: String,
        /// Nickname
        nick: String,
        /// Whether the user is away (`G` = gone) or here (`H`)
        away: bool,
    },

    /// User away status change (from `away-notify` CAP).
    /// `message` is `None` when the user comes back, `Some` when they go away.
    Away {
        /// Nick of the user whose away status changed
        nick: String,
        /// Away message, or `None` if the user is back
        message: Option<String>,
    },

    /// IRC error (numeric or ERROR command)
    Error {
        /// Error code or type
        code: String,
        /// Error message
        message: String,
    },

    /// MOTD line
    Motd {
        /// MOTD line
        message: String,
    },

    /// Start of an IRCv3 BATCH (e.g. chathistory replay)
    BatchStart {
        /// Batch reference ID
        id: String,
        /// Batch type (e.g. "chathistory")
        batch_type: String,
        /// Additional parameters (e.g. channel name)
        params: Vec<String>,
    },

    /// End of an IRCv3 BATCH
    BatchEnd {
        /// Batch reference ID
        id: String,
    },

    /// Someone invited us (or another user) to a channel
    Invited {
        /// Who sent the invite
        from: String,
        /// Who is being invited
        nick: String,
        /// Channel to join
        channel: String,
    },

    /// Confirmation that our INVITE command was accepted (RPL_INVITING 341)
    InviteSent {
        /// Nick we invited
        nick: String,
        /// Channel we invited them to
        channel: String,
    },

    /// Raw/unhandled message
    Raw {
        /// Command
        command: String,
        /// Parameters
        params: Vec<String>,
    },

    /// Connection state changed
    StateChanged {
        /// New connection state
        state: String,
    },
}

impl IrcEvent {
    /// Return a short label identifying the event variant (for logging).
    pub fn event_type(&self) -> &'static str {
        match self {
            IrcEvent::Ping { .. } => "ping",
            IrcEvent::Welcome { .. } => "welcome",
            IrcEvent::UserJoined { .. } => "user-joined",
            IrcEvent::UserParted { .. } => "user-parted",
            IrcEvent::UserQuit { .. } => "user-quit",
            IrcEvent::NamesList { .. } => "names-list",
            IrcEvent::ChannelMessage { .. } => "channel-message",
            IrcEvent::PrivateMessage { .. } => "private-message",
            IrcEvent::Notice { .. } => "notice",
            IrcEvent::ChannelTopic { .. } => "channel-topic",
            IrcEvent::NickChanged { .. } => "nick-changed",
            IrcEvent::WhoReply { .. } => "who-reply",
            IrcEvent::Away { .. } => "away",
            IrcEvent::Error { .. } => "error",
            IrcEvent::Motd { .. } => "motd",
            IrcEvent::Invited { .. } => "invited",
            IrcEvent::InviteSent { .. } => "invite-sent",
            IrcEvent::BatchStart { .. } => "batch-start",
            IrcEvent::BatchEnd { .. } => "batch-end",
            IrcEvent::Raw { .. } => "raw",
            IrcEvent::StateChanged { .. } => "state-changed",
        }
    }

    /// Convert an IRC message from the `irc` crate to our event type.
    ///
    /// Returns `None` for internal protocol messages that don't need to be
    /// forwarded (e.g. PONG, CAP, RPL_ENDOFNAMES).
    pub fn from_irc_message(msg: &Message) -> Option<Self> {
        let nick = msg
            .source_nickname()
            .map(|s| s.to_string())
            .unwrap_or_default();

        let event = match &msg.command {
            // Internal protocol messages — handled by the irc crate, not forwarded
            Command::PONG(..) | Command::CAP(..) => return None,
            // RPL_ENDOFNAMES (366) / RPL_ENDOFWHO (315) — just signal the end of a list
            Command::Response(Response::RPL_ENDOFNAMES | Response::RPL_ENDOFWHO, _) => return None,

            Command::PING(id, _) => IrcEvent::Ping { id: id.clone() },

            Command::Response(Response::RPL_WELCOME, params) => IrcEvent::Welcome {
                nick: params.first().cloned().unwrap_or_default(),
                message: params.last().cloned().unwrap_or_default(),
            },

            Command::JOIN(channel, _, _) => IrcEvent::UserJoined {
                channel: channel.clone(),
                nick,
                batch: extract_tag(msg, "batch"),
                server_time: extract_tag(msg, "time"),
                msgid: extract_tag(msg, "msgid"),
            },

            Command::PART(channel, message) => IrcEvent::UserParted {
                channel: channel.clone(),
                nick,
                message: message.clone(),
                batch: extract_tag(msg, "batch"),
                server_time: extract_tag(msg, "time"),
                msgid: extract_tag(msg, "msgid"),
            },

            Command::QUIT(message) => IrcEvent::UserQuit {
                nick,
                message: message.clone(),
                batch: extract_tag(msg, "batch"),
                server_time: extract_tag(msg, "time"),
                msgid: extract_tag(msg, "msgid"),
            },

            Command::Response(Response::RPL_NAMREPLY, params) => {
                // Format: <client> <symbol> <channel> :<names>
                let channel = params.get(2).cloned().unwrap_or_default();
                let names = params
                    .last()
                    .map(|s| s.split_whitespace().map(|n| n.to_string()).collect())
                    .unwrap_or_default();
                IrcEvent::NamesList { channel, names }
            }

            Command::Response(Response::RPL_WHOREPLY, params) => {
                // Format: <client> <channel> <user> <host> <server> <nick> <flags> :<hopcount> <realname>
                // flags starts with H (here) or G (gone/away), optionally followed by * (ircop), @ (+o), + (+v)
                let channel = params.get(1).cloned().unwrap_or_default();
                let who_nick = params.get(5).cloned().unwrap_or_default();
                let flags = params.get(6).map(|s| s.as_str()).unwrap_or("H");
                let away = flags.starts_with('G');
                IrcEvent::WhoReply {
                    channel,
                    nick: who_nick,
                    away,
                }
            }

            Command::PRIVMSG(target, text) => {
                // Extract data and visible text: check for CTCP GBDATA envelope first,
                // then fall back to +data tag for backwards compatibility.
                let (data, visible_text) = extract_gbdata(text)
                    .map(|(d, t)| (Some(d), t))
                    .unwrap_or_else(|| (extract_data_tag(msg), text.clone()));

                let server_time = extract_tag(msg, "time");
                let batch = extract_tag(msg, "batch");
                let msgid = extract_tag(msg, "msgid");
                let reply_to = extract_tag(msg, "+draft/reply");

                if target.starts_with('#') {
                    IrcEvent::ChannelMessage {
                        from: nick,
                        channel: target.clone(),
                        text: visible_text,
                        data,
                        server_time,
                        batch,
                        msgid,
                        reply_to,
                    }
                } else {
                    IrcEvent::PrivateMessage {
                        from: nick,
                        target: target.clone(),
                        text: visible_text,
                        data,
                        server_time,
                        batch,
                        msgid,
                        reply_to,
                    }
                }
            }

            Command::NOTICE(target, text) => IrcEvent::Notice {
                from: if nick.is_empty() { None } else { Some(nick) },
                target: target.clone(),
                message: text.clone(),
            },

            Command::TOPIC(channel, topic) => IrcEvent::ChannelTopic {
                channel: channel.clone(),
                topic: topic.clone().unwrap_or_default(),
            },

            // RPL_TOPIC (332) — sent by server on JOIN if a topic is set
            // Format: <client> <channel> :<topic>
            Command::Response(Response::RPL_TOPIC, params) => IrcEvent::ChannelTopic {
                channel: params.get(1).cloned().unwrap_or_default(),
                topic: params.last().cloned().unwrap_or_default(),
            },

            // RPL_NOTOPIC (331) — sent by server on JOIN if no topic is set
            Command::Response(Response::RPL_NOTOPIC, params) => IrcEvent::ChannelTopic {
                channel: params.get(1).cloned().unwrap_or_default(),
                topic: String::new(),
            },

            // INVITE <nick> <channel> — someone invites us (or we see an invite)
            Command::INVITE(target_nick, channel) => IrcEvent::Invited {
                from: nick,
                nick: target_nick.clone(),
                channel: channel.clone(),
            },

            // RPL_INVITING (341) — confirmation that our INVITE was sent
            // Format: <client> <nick> <channel>
            Command::Response(Response::RPL_INVITING, params) => IrcEvent::InviteSent {
                nick: params.get(1).cloned().unwrap_or_default(),
                channel: params.get(2).cloned().unwrap_or_default(),
            },

            Command::NICK(new_nick) => IrcEvent::NickChanged {
                old_nick: nick,
                new_nick: new_nick.clone(),
                batch: extract_tag(msg, "batch"),
                server_time: extract_tag(msg, "time"),
                msgid: extract_tag(msg, "msgid"),
            },

            Command::AWAY(message) => IrcEvent::Away {
                nick,
                message: message.clone(),
            },

            Command::Response(Response::RPL_MOTD, params)
            | Command::Response(Response::RPL_MOTDSTART, params)
            | Command::Response(Response::RPL_ENDOFMOTD, params) => IrcEvent::Motd {
                message: params.last().cloned().unwrap_or_default(),
            },

            Command::ERROR(message) => IrcEvent::Error {
                code: "ERROR".to_string(),
                message: message.clone(),
            },

            Command::BATCH(ref_tag, sub_cmd, params) => {
                if let Some(stripped) = ref_tag.strip_prefix('+') {
                    let batch_type = match sub_cmd {
                        Some(BatchSubCommand::NETSPLIT) => "netsplit".to_string(),
                        Some(BatchSubCommand::NETJOIN) => "netjoin".to_string(),
                        Some(BatchSubCommand::CUSTOM(t)) => t.clone(),
                        None => "unknown".to_string(),
                    };
                    IrcEvent::BatchStart {
                        id: stripped.to_string(),
                        batch_type,
                        params: params.clone().unwrap_or_default(),
                    }
                } else if let Some(stripped) = ref_tag.strip_prefix('-') {
                    IrcEvent::BatchEnd {
                        id: stripped.to_string(),
                    }
                } else {
                    IrcEvent::Raw {
                        command: format!("BATCH {}", ref_tag),
                        params: params.clone().unwrap_or_default(),
                    }
                }
            }

            Command::Response(code, params) => {
                let code_num = *code as u16;
                // 4xx and 5xx are errors — include all params (skip first which is our nick)
                // so we retain context like the unknown command name in 421 responses.
                if (400..600).contains(&code_num) {
                    let context_params = if params.len() > 1 {
                        &params[1..]
                    } else {
                        params.as_slice()
                    };
                    IrcEvent::Error {
                        code: code_num.to_string(),
                        message: context_params.join(" "),
                    }
                } else {
                    IrcEvent::Raw {
                        command: code_num.to_string(),
                        params: params.clone(),
                    }
                }
            }

            _ => IrcEvent::Raw {
                command: format!("{:?}", msg.command),
                params: vec![],
            },
        };
        Some(event)
    }
}

/// Extract data and human-readable text from a CTCP GBDATA envelope.
///
/// Accepts both formats:
/// - `\x01ACTION GBDATA <base64> :<human text>\x01` (current — survives Ergo chathistory)
/// - `\x01GBDATA <base64> :<human text>\x01` (legacy)
///
/// Returns `Some((decoded_data, human_text))` if the body is a GBDATA envelope,
/// `None` otherwise.
fn extract_gbdata(text: &str) -> Option<(String, String)> {
    use base64::{Engine as _, engine::general_purpose};

    let inner = text.strip_prefix('\x01')?.strip_suffix('\x01')?;
    let rest = inner
        .strip_prefix("ACTION GBDATA ")
        .or_else(|| inner.strip_prefix("GBDATA "))?;

    // Split on " :" to separate base64 from human text
    let (encoded, human) = if let Some(idx) = rest.find(" :") {
        (&rest[..idx], rest[idx + 2..].to_string())
    } else {
        (rest, String::new())
    };

    let decoded = general_purpose::STANDARD.decode(encoded).ok()?;
    let data = String::from_utf8(decoded).ok()?;
    Some((data, human))
}

/// Extract a plain tag value from an IRC message (e.g. "time", "batch").
fn extract_tag(msg: &Message, name: &str) -> Option<String> {
    let tags = msg.tags.as_ref()?;
    for tag in tags {
        if tag.0 == name {
            return tag.1.clone();
        }
    }
    None
}

/// Extract and decode the +data tag from an IRC message.
///
/// The +data tag contains base64-encoded JSON data.
fn extract_data_tag(msg: &Message) -> Option<String> {
    use base64::{Engine as _, engine::general_purpose};

    let tags = msg.tags.as_ref()?;

    for tag in tags {
        if tag.0 == "+data"
            && let Some(encoded) = &tag.1
            && let Ok(decoded) = general_purpose::STANDARD.decode(encoded)
            && let Ok(data) = String::from_utf8(decoded)
        {
            return Some(data);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine as _, engine::general_purpose};
    use irc::proto::message::Tag;

    #[test]
    fn test_extract_data_tag_present() {
        let data = r#"{"type":"test"}"#;
        let encoded = general_purpose::STANDARD.encode(data);

        let msg = Message::with_tags(
            Some(vec![Tag("+data".to_string(), Some(encoded))]),
            None,
            "PRIVMSG",
            vec!["#test", "Hello"],
        )
        .unwrap();

        let result = extract_data_tag(&msg);
        assert_eq!(result, Some(data.to_string()));
    }

    #[test]
    fn test_extract_data_tag_missing() {
        let msg = Message::with_tags(
            Some(vec![Tag("msgid".to_string(), Some("123".to_string()))]),
            None,
            "PRIVMSG",
            vec!["#test", "Hello"],
        )
        .unwrap();

        let result = extract_data_tag(&msg);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_data_tag_no_tags() {
        let msg = Message::with_tags(None, None, "PRIVMSG", vec!["#test", "Hello"]).unwrap();

        let result = extract_data_tag(&msg);
        assert_eq!(result, None);
    }

    #[test]
    fn test_batch_start_chathistory() {
        let msg = Message::with_tags(
            None,
            Some("server"),
            "BATCH",
            vec!["+abc123", "chathistory", "#channel"],
        )
        .unwrap();

        let event = IrcEvent::from_irc_message(&msg).expect("should produce event");
        assert_eq!(
            event,
            IrcEvent::BatchStart {
                id: "abc123".to_string(),
                batch_type: "CHATHISTORY".to_string(),
                params: vec!["#channel".to_string()],
            }
        );
    }

    #[test]
    fn test_batch_end() {
        let msg = Message::with_tags(None, Some("server"), "BATCH", vec!["-abc123"]).unwrap();

        let event = IrcEvent::from_irc_message(&msg).expect("should produce event");
        assert_eq!(
            event,
            IrcEvent::BatchEnd {
                id: "abc123".to_string(),
            }
        );
    }

    #[test]
    fn test_batch_start_no_subcommand() {
        // BATCH with + prefix but no sub-command
        let msg = Message::with_tags(None, Some("server"), "BATCH", vec!["+xyz"]).unwrap();

        let event = IrcEvent::from_irc_message(&msg).expect("should produce event");
        assert_eq!(
            event,
            IrcEvent::BatchStart {
                id: "xyz".to_string(),
                batch_type: "unknown".to_string(),
                params: vec![],
            }
        );
    }

    #[test]
    fn test_channel_message_with_data() {
        let data = r#"{"action":"ping"}"#;
        let encoded = general_purpose::STANDARD.encode(data);

        let msg = Message::with_tags(
            Some(vec![Tag("+data".to_string(), Some(encoded))]),
            Some("nick!user@host"),
            "PRIVMSG",
            vec!["#channel", "visible text"],
        )
        .unwrap();

        let event = IrcEvent::from_irc_message(&msg).expect("should produce event");

        match event {
            IrcEvent::ChannelMessage {
                from,
                channel,
                text,
                data: payload,
                ..
            } => {
                assert_eq!(from, "nick");
                assert_eq!(channel, "#channel");
                assert_eq!(text, "visible text");
                assert_eq!(payload, Some(data.to_string()));
            }
            _ => panic!("Expected ChannelMessage"),
        }
    }

    #[test]
    fn test_pong_filtered() {
        let msg =
            Message::with_tags(None, Some("server"), "PONG", vec!["server", "12345"]).unwrap();
        assert!(
            IrcEvent::from_irc_message(&msg).is_none(),
            "PONG should be filtered out"
        );
    }
}
