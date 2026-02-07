//! In-memory message storage for IRC channels.
//!
//! Stores messages per (connection_id, channel) so the frontend can query
//! message history independently of component lifecycle.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

/// Check whether a stored message is a reaction payload that should be hidden
/// from the chat message list.
fn is_reaction_message(msg: &StoredMessage) -> bool {
    let Some(ref data) = msg.data else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
        return false;
    };
    matches!(
        json["type"].as_str(),
        Some("commit-reaction" | "hunk-reaction" | "message-reaction")
    )
}

/// A reaction (emoji or approve/feedback) sent via IRC.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reaction {
    pub sender: String,
    pub reaction: String,
}

/// Default maximum number of messages stored per channel.
const DEFAULT_MAX_PER_CHANNEL: usize = 1000;

/// Direction of an IRC message relative to us.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MessageDirection {
    Incoming,
    Outgoing,
}

/// A stored IRC message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredMessage {
    /// Sender nick
    pub sender: String,
    /// Message text
    pub content: String,
    /// +data tag payload (if present)
    pub data: Option<String>,
    /// Milliseconds since UNIX epoch
    pub timestamp: u64,
    /// Whether this message was incoming or outgoing
    pub direction: MessageDirection,
    /// Target channel or nick
    pub target: String,
    /// Whether this message is from channel history replay (chathistory batch)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_history: bool,
    /// IRCv3 message ID (for pagination with CHATHISTORY BEFORE)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msgid: Option<String>,
    /// IRCv3 `+draft/reply` — msgid of the message being replied to
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    /// Event tag for display (e.g. "001", "notice", "join", "part", "quit", "motd", "error").
    /// `None` for regular PRIVMSG messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

impl StoredMessage {
    /// Set the tag on this message and return it (builder pattern).
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }
}

/// A user tracked in a channel, with optional away status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserEntry {
    pub nick: String,
    /// `true` if the user is currently marked as away.
    pub away: bool,
}

impl UserEntry {
    fn new(nick: impl Into<String>) -> Self {
        Self {
            nick: nick.into(),
            away: false,
        }
    }
}

/// Channel metadata returned by queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelInfo {
    /// Channel name (e.g. "#general")
    pub name: String,
    /// Channel topic (if known)
    pub topic: Option<String>,
    /// Number of unread messages
    pub unread_count: usize,
    /// Known users in the channel
    pub users: Vec<UserEntry>,
}

/// In-memory storage for IRC messages and channel state.
///
/// Thread safety: this struct is **not** internally synchronized. The caller
/// is responsible for wrapping it in an appropriate lock (e.g. `RwLock`).
pub struct MessageStore {
    /// Messages keyed by (connection_id, channel/target).
    messages: HashMap<(String, String), VecDeque<StoredMessage>>,
    /// Known msgids per (connection_id, target) for O(1) dedup.
    seen_msgids: HashMap<(String, String), HashSet<String>>,
    /// Channel metadata keyed by connection_id, then by channel name.
    channels: HashMap<String, HashMap<String, ChannelState>>,
    /// Commit reactions indexed by (connection_id, commit_id).
    reactions: HashMap<(String, String), Vec<Reaction>>,
    /// Message reactions indexed by (connection_id, msg_id).
    message_reactions: HashMap<(String, String), Vec<Reaction>>,
    /// Reverse index: (connection_id, file_path, hunk_key) → [share_msg_id].
    /// Used to look up which share messages correspond to a given file's hunks.
    hunk_share_index: HashMap<(String, String, String), Vec<String>>,
    /// Files being worked on by users, keyed by (connection_id, channel) → { nick → files }.
    working_files: HashMap<(String, String), HashMap<String, Vec<String>>>,
    /// Maximum messages stored per channel.
    max_per_channel: usize,
    /// Counter for assigning synthetic msgids to messages that lack one.
    next_synthetic_id: u64,
}

/// Per-channel state tracked by the store.
struct ChannelState {
    topic: Option<String>,
    users: Vec<UserEntry>,
    /// Timestamp of the last message the frontend has seen.
    last_read_timestamp: u64,
}

impl Default for MessageStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageStore {
    /// Create a new empty message store with default capacity.
    pub fn new() -> Self {
        Self {
            messages: HashMap::new(),
            seen_msgids: HashMap::new(),
            channels: HashMap::new(),
            reactions: HashMap::new(),
            message_reactions: HashMap::new(),
            hunk_share_index: HashMap::new(),
            working_files: HashMap::new(),
            max_per_channel: DEFAULT_MAX_PER_CHANNEL,
            next_synthetic_id: 0,
        }
    }

    /// Assign a synthetic msgid if the message doesn't already have one.
    fn ensure_msgid(&mut self, msg: &mut StoredMessage) {
        if msg.msgid.is_none() {
            self.next_synthetic_id += 1;
            msg.msgid = Some(format!("synthetic-{}", self.next_synthetic_id));
        }
    }

    /// Store an incoming message. Returns `None` if deduplicated by `msgid`.
    #[allow(clippy::too_many_arguments)]
    pub fn store_incoming(
        &mut self,
        connection_id: &str,
        target: &str,
        sender: &str,
        content: &str,
        data: Option<&str>,
        msgid: Option<&str>,
        reply_to: Option<&str>,
        tag: Option<&str>,
    ) -> Option<StoredMessage> {
        let mut msg = StoredMessage {
            sender: sender.to_string(),
            content: content.to_string(),
            data: data.map(|s| s.to_string()),
            timestamp: now_millis(),
            direction: MessageDirection::Incoming,
            target: target.to_string(),
            is_history: false,
            msgid: msgid.map(|s| s.to_string()),
            reply_to: reply_to.map(|s| s.to_string()),
            tag: tag.map(|s| s.to_string()),
        };
        self.ensure_msgid(&mut msg);
        if self.push_message(connection_id, target, msg.clone()) {
            Some(msg)
        } else {
            None
        }
    }

    /// Store an incoming history message (from a chathistory batch replay).
    ///
    /// If `server_time` is provided (ISO 8601), the timestamp is parsed from it;
    /// otherwise falls back to `now()`.
    /// Returns `None` if the message was a duplicate (by msgid).
    #[allow(clippy::too_many_arguments)]
    pub fn store_history(
        &mut self,
        connection_id: &str,
        target: &str,
        sender: &str,
        content: &str,
        data: Option<&str>,
        server_time: Option<&str>,
        msgid: Option<&str>,
    ) -> Option<StoredMessage> {
        let parsed = server_time.and_then(parse_iso8601_millis);
        if parsed.is_none() && server_time.is_some() {
            tracing::debug!(
                server_time = ?server_time,
                "Failed to parse server_time from history message",
            );
        }
        let timestamp = parsed.unwrap_or_else(now_millis);
        let mut msg = StoredMessage {
            sender: sender.to_string(),
            content: content.to_string(),
            data: data.map(|s| s.to_string()),
            timestamp,
            direction: MessageDirection::Incoming,
            target: target.to_string(),
            is_history: true,
            msgid: msgid.map(|s| s.to_string()),
            reply_to: None,
            tag: None,
        };
        self.ensure_msgid(&mut msg);
        if self.insert_by_timestamp(connection_id, target, msg.clone()) {
            Some(msg)
        } else {
            None
        }
    }

    /// Store an entire batch of history messages at once.
    ///
    /// The batch is assumed to be pre-sorted by timestamp.  Each message is
    /// inserted at the correct position and deduplicated by `msgid`.
    /// Store a batch of history messages. Returns only the messages that were
    /// actually inserted (i.e. not already present by msgid).
    pub fn store_history_batch(
        &mut self,
        connection_id: &str,
        target: &str,
        batch: Vec<StoredMessage>,
    ) -> Vec<StoredMessage> {
        let mut inserted = Vec::with_capacity(batch.len());
        for mut msg in batch {
            self.ensure_msgid(&mut msg);
            let clone = msg.clone();
            if self.insert_by_timestamp(connection_id, target, msg) {
                inserted.push(clone);
            }
        }
        inserted
    }

    /// Store an outgoing message (local fallback when echo-message is not negotiated).
    pub fn store_outgoing(
        &mut self,
        connection_id: &str,
        target: &str,
        sender: &str,
        content: &str,
        data: Option<&str>,
        reply_to: Option<&str>,
    ) -> StoredMessage {
        let mut msg = StoredMessage {
            sender: sender.to_string(),
            content: content.to_string(),
            data: data.map(|s| s.to_string()),
            timestamp: now_millis(),
            direction: MessageDirection::Outgoing,
            target: target.to_string(),
            is_history: false,
            msgid: None,
            reply_to: reply_to.map(|s| s.to_string()),
            tag: None,
        };
        self.ensure_msgid(&mut msg);
        self.push_message(connection_id, target, msg.clone());
        msg
    }

    /// Store an outgoing message that was echoed back by the server.
    ///
    /// This is used with the `echo-message` capability: the server sends our own
    /// messages back to us with a real server-assigned `msgid`.
    #[allow(clippy::too_many_arguments)]
    pub fn store_outgoing_echo(
        &mut self,
        connection_id: &str,
        target: &str,
        sender: &str,
        content: &str,
        data: Option<&str>,
        msgid: Option<&str>,
        reply_to: Option<&str>,
    ) -> StoredMessage {
        let mut msg = StoredMessage {
            sender: sender.to_string(),
            content: content.to_string(),
            data: data.map(|s| s.to_string()),
            timestamp: now_millis(),
            direction: MessageDirection::Outgoing,
            target: target.to_string(),
            is_history: false,
            msgid: msgid.map(|s| s.to_string()),
            reply_to: reply_to.map(|s| s.to_string()),
            tag: None,
        };
        self.ensure_msgid(&mut msg);
        self.push_message(connection_id, target, msg.clone());
        msg
    }

    /// Get stored messages for a channel, sorted by timestamp.
    /// Reaction-only messages are excluded.
    ///
    /// Messages are maintained in sorted order by `push_message` and
    /// `insert_by_timestamp`, so no additional sort is needed here.
    pub fn get_messages(&self, connection_id: &str, target: &str) -> Vec<StoredMessage> {
        let key = (connection_id.to_string(), target.to_string());
        self.messages
            .get(&key)
            .map(|q| {
                q.iter()
                    .filter(|m| !is_reaction_message(m))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Track a user joining a channel.
    pub fn user_joined(&mut self, connection_id: &str, channel: &str, nick: &str) {
        let channel_state = self.ensure_channel(connection_id, channel);
        if !channel_state.users.iter().any(|u| u.nick == nick) {
            channel_state.users.push(UserEntry::new(nick));
        }
    }

    /// Track a user leaving a channel.
    pub fn user_parted(&mut self, connection_id: &str, channel: &str, nick: &str) {
        let channels = self.channels.entry(connection_id.to_string()).or_default();
        if let Some(ch) = channels.get_mut(channel) {
            ch.users.retain(|u| u.nick != nick);
        }
    }

    /// Remove a user from all channels (QUIT). Returns the channel names they were in.
    pub fn user_quit(&mut self, connection_id: &str, nick: &str) -> Vec<String> {
        let channels = self.channels.entry(connection_id.to_string()).or_default();
        let mut affected = Vec::new();
        for (name, ch) in channels.iter_mut() {
            if ch.users.iter().any(|u| u.nick == nick) {
                ch.users.retain(|u| u.nick != nick);
                affected.push(name.clone());
            }
        }
        affected
    }

    /// Rename a user in all channels (NICK). Returns the channel names they were in.
    pub fn nick_changed(
        &mut self,
        connection_id: &str,
        old_nick: &str,
        new_nick: &str,
    ) -> Vec<String> {
        let channels = self.channels.entry(connection_id.to_string()).or_default();
        let mut affected = Vec::new();
        for (name, ch) in channels.iter_mut() {
            if let Some(entry) = ch.users.iter_mut().find(|u| u.nick == old_nick) {
                entry.nick = new_nick.to_string();
                affected.push(name.clone());
            }
        }
        affected
    }

    /// Set the channel's user list (from NAMES reply).
    pub fn set_users(&mut self, connection_id: &str, channel: &str, nicks: Vec<String>) {
        let channel_state = self.ensure_channel(connection_id, channel);
        channel_state.users = nicks.into_iter().map(UserEntry::new).collect();
    }

    /// Update away status for a user across all channels they're in.
    /// Returns the channel names they were found in.
    pub fn set_user_away(&mut self, connection_id: &str, nick: &str, away: bool) -> Vec<String> {
        let channels = self.channels.entry(connection_id.to_string()).or_default();
        let mut affected = Vec::new();
        for (name, ch) in channels.iter_mut() {
            if let Some(entry) = ch.users.iter_mut().find(|u| u.nick == nick) {
                entry.away = away;
                affected.push(name.clone());
            }
        }
        affected
    }

    /// Set the channel topic.
    pub fn set_topic(&mut self, connection_id: &str, channel: &str, topic: &str) {
        let channel_state = self.ensure_channel(connection_id, channel);
        channel_state.topic = Some(topic.to_string());
    }

    /// Add a channel (e.g. on JOIN).
    pub fn add_channel(&mut self, connection_id: &str, channel: &str) {
        self.ensure_channel(connection_id, channel);
    }

    /// Remove a channel (e.g. on PART).
    pub fn remove_channel(&mut self, connection_id: &str, channel: &str) {
        let channels = self.channels.entry(connection_id.to_string()).or_default();
        channels.remove(channel);
        // Also remove stored messages and seen msgids for this channel
        let key = (connection_id.to_string(), channel.to_string());
        self.messages.remove(&key);
        self.seen_msgids.remove(&key);
    }

    /// Get channel info for all channels on a connection.
    pub fn get_channels(&self, connection_id: &str) -> Vec<ChannelInfo> {
        self.channels
            .get(connection_id)
            .map(|channels| {
                channels
                    .iter()
                    .map(|(name, ch)| {
                        let key = (connection_id.to_string(), name.clone());
                        let unread_count = self
                            .messages
                            .get(&key)
                            .map(|msgs| {
                                msgs.iter()
                                    .filter(|m| m.timestamp > ch.last_read_timestamp)
                                    .count()
                            })
                            .unwrap_or(0);
                        ChannelInfo {
                            name: name.clone(),
                            topic: ch.topic.clone(),
                            unread_count,
                            users: ch.users.clone(),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get users for a specific channel.
    pub fn get_users(&self, connection_id: &str, channel: &str) -> Vec<UserEntry> {
        self.channels
            .get(connection_id)
            .and_then(|channels| channels.get(channel))
            .map(|ch| ch.users.clone())
            .unwrap_or_default()
    }

    /// Mark a channel as read up to the current time.
    pub fn mark_read(&mut self, connection_id: &str, channel: &str) {
        let channel_state = self.ensure_channel(connection_id, channel);
        channel_state.last_read_timestamp = now_millis();
    }

    /// Mark a channel as read up to a specific timestamp (from server MARKREAD).
    ///
    /// Only updates if the provided timestamp is newer than the current one,
    /// to avoid going backwards.
    pub fn mark_read_at(&mut self, connection_id: &str, channel: &str, timestamp_ms: u64) {
        let channel_state = self.ensure_channel(connection_id, channel);
        if timestamp_ms > channel_state.last_read_timestamp {
            channel_state.last_read_timestamp = timestamp_ms;
        }
    }

    /// Remove a single message by its msgid. Returns true if found and removed.
    pub fn remove_message_by_msgid(
        &mut self,
        connection_id: &str,
        target: &str,
        msgid: &str,
    ) -> bool {
        let key = (connection_id.to_string(), target.to_string());
        if let Some(messages) = self.messages.get_mut(&key) {
            let before = messages.len();
            messages.retain(|m| m.msgid.as_deref() != Some(msgid));
            if messages.len() < before {
                if let Some(seen) = self.seen_msgids.get_mut(&key) {
                    seen.remove(msgid);
                }
                // Also remove any reactions targeting this message
                let reaction_key = (connection_id.to_string(), msgid.to_string());
                self.message_reactions.remove(&reaction_key);
                return true;
            }
        }
        false
    }

    /// Clear stored messages for a specific channel (keeps channel metadata).
    pub fn clear_messages(&mut self, connection_id: &str, channel: &str) {
        let key = (connection_id.to_string(), channel.to_string());
        self.messages.remove(&key);
        self.seen_msgids.remove(&key);
    }

    /// Record a commit reaction (approve/etc). Multiple reactions per sender are
    /// allowed; the same (sender, reaction) pair is deduplicated.
    pub fn store_reaction(
        &mut self,
        connection_id: &str,
        commit_id: &str,
        sender: &str,
        reaction: &str,
    ) {
        let key = (connection_id.to_string(), commit_id.to_string());
        let entry = self.reactions.entry(key).or_default();
        if !entry
            .iter()
            .any(|r| r.sender == sender && r.reaction == reaction)
        {
            entry.push(Reaction {
                sender: sender.to_string(),
                reaction: reaction.to_string(),
            });
        }
    }

    /// Remove a commit reaction for the given sender and emoji.
    pub fn remove_reaction(
        &mut self,
        connection_id: &str,
        commit_id: &str,
        sender: &str,
        reaction: &str,
    ) {
        let key = (connection_id.to_string(), commit_id.to_string());
        if let Some(entry) = self.reactions.get_mut(&key) {
            entry.retain(|r| !(r.sender == sender && r.reaction == reaction));
        }
    }

    /// Get all commit reactions for a connection, keyed by commit ID.
    pub fn get_all_reactions(&self, connection_id: &str) -> HashMap<String, Vec<Reaction>> {
        self.reactions
            .iter()
            .filter(|((conn_id, _), _)| conn_id == connection_id)
            .map(|((_, commit_id), reactions)| (commit_id.clone(), reactions.clone()))
            .collect()
    }

    /// Record a message reaction keyed by the msgid of the reacted-to message.
    /// Multiple reactions per sender are allowed; the same (sender, reaction)
    /// pair is deduplicated.
    pub fn store_message_reaction(
        &mut self,
        connection_id: &str,
        msg_id: &str,
        sender: &str,
        reaction: &str,
    ) {
        let key = (connection_id.to_string(), msg_id.to_string());
        let entry = self.message_reactions.entry(key).or_default();
        if !entry
            .iter()
            .any(|r| r.sender == sender && r.reaction == reaction)
        {
            entry.push(Reaction {
                sender: sender.to_string(),
                reaction: reaction.to_string(),
            });
        }
    }

    /// Find a message by its IRC msgid across all channels for a connection.
    /// Returns the message's data payload (if any).
    pub fn find_message_data_by_msgid(&self, connection_id: &str, msgid: &str) -> Option<String> {
        for ((conn_id, _), msgs) in &self.messages {
            if conn_id != connection_id {
                continue;
            }
            for msg in msgs {
                if msg.msgid.as_deref() == Some(msgid) {
                    return msg.data.clone();
                }
            }
        }
        None
    }

    /// Remove a message reaction for the given sender and emoji.
    pub fn remove_message_reaction(
        &mut self,
        connection_id: &str,
        msg_id: &str,
        sender: &str,
        reaction: &str,
    ) {
        let key = (connection_id.to_string(), msg_id.to_string());
        if let Some(entry) = self.message_reactions.get_mut(&key) {
            entry.retain(|r| !(r.sender == sender && r.reaction == reaction));
        }
    }

    /// Get all message reactions for a connection, keyed by message ID.
    pub fn get_all_message_reactions(&self, connection_id: &str) -> HashMap<String, Vec<Reaction>> {
        self.message_reactions
            .iter()
            .filter(|((conn_id, _), _)| conn_id == connection_id)
            .map(|((_, share_msg_id), reactions)| (share_msg_id.clone(), reactions.clone()))
            .collect()
    }

    /// Record that a hunk share message exists for a given file path and hunk key.
    ///
    /// `hunk_key` should be formatted as `"oldStart:oldLines:newStart:newLines"`.
    pub fn index_hunk_share(
        &mut self,
        connection_id: &str,
        file_path: &str,
        hunk_key: &str,
        share_msg_id: &str,
    ) {
        let key = (
            connection_id.to_string(),
            file_path.to_string(),
            hunk_key.to_string(),
        );
        let entry = self.hunk_share_index.entry(key).or_default();
        if !entry.contains(&share_msg_id.to_string()) {
            entry.push(share_msg_id.to_string());
        }
    }

    /// Get all message reactions for a specific file, keyed by hunk key
    /// (formatted as `"oldStart:oldLines:newStart:newLines"`).
    ///
    /// This uses the hunk share index to find which share messages correspond
    /// to hunks in the given file, then collects their reactions.
    pub fn get_file_message_reactions(
        &self,
        connection_id: &str,
        file_path: &str,
    ) -> HashMap<String, Vec<Reaction>> {
        let mut result: HashMap<String, Vec<Reaction>> = HashMap::new();

        for ((conn_id, fp, hunk_key), msg_ids) in &self.hunk_share_index {
            if conn_id != connection_id || fp != file_path {
                continue;
            }
            let mut reactions = Vec::new();
            for msg_id in msg_ids {
                let rkey = (connection_id.to_string(), msg_id.clone());
                if let Some(rs) = self.message_reactions.get(&rkey) {
                    reactions.extend(rs.iter().cloned());
                }
            }
            if !reactions.is_empty() {
                result.insert(hunk_key.clone(), reactions);
            }
        }

        result
    }

    /// Apply a full working-files sync for a user on a channel.
    pub fn apply_working_files_sync(
        &mut self,
        connection_id: &str,
        channel: &str,
        nick: &str,
        files: Vec<String>,
    ) {
        self.working_files
            .entry((connection_id.to_string(), channel.to_string()))
            .or_default()
            .insert(nick.to_string(), files);
    }

    /// Apply a working-files delta (added/removed) for a user on a channel.
    pub fn apply_working_files_delta(
        &mut self,
        connection_id: &str,
        channel: &str,
        nick: &str,
        added: Vec<String>,
        removed: Vec<String>,
    ) {
        let entry = self
            .working_files
            .entry((connection_id.to_string(), channel.to_string()))
            .or_default()
            .entry(nick.to_string())
            .or_default();
        for f in added {
            if !entry.contains(&f) {
                entry.push(f);
            }
        }
        entry.retain(|f| !removed.contains(f));
    }

    /// Get the current working files for all users on a channel.
    pub fn get_working_files(
        &self,
        connection_id: &str,
        channel: &str,
    ) -> HashMap<String, Vec<String>> {
        self.working_files
            .get(&(connection_id.to_string(), channel.to_string()))
            .cloned()
            .unwrap_or_default()
    }

    /// Remove a user's working files from all channels (on PART or QUIT).
    pub fn remove_working_files_user(&mut self, connection_id: &str, nick: &str) {
        for ((conn_id, _), users) in self.working_files.iter_mut() {
            if conn_id == connection_id {
                users.remove(nick);
            }
        }
    }

    /// Remove all data for a connection (on disconnect/remove).
    pub fn remove_connection(&mut self, connection_id: &str) {
        self.channels.remove(connection_id);
        self.messages.retain(|key, _| key.0 != connection_id);
        self.seen_msgids.retain(|key, _| key.0 != connection_id);
        self.reactions
            .retain(|(conn_id, _), _| conn_id != connection_id);
        self.message_reactions
            .retain(|(conn_id, _), _| conn_id != connection_id);
        self.hunk_share_index
            .retain(|(conn_id, _, _), _| conn_id != connection_id);
        self.working_files
            .retain(|(conn_id, _), _| conn_id != connection_id);
    }

    fn push_message(&mut self, connection_id: &str, target: &str, msg: StoredMessage) -> bool {
        let key = (connection_id.to_string(), target.to_string());

        // Deduplicate by msgid when present.
        if let Some(ref id) = msg.msgid {
            let seen = self.seen_msgids.entry(key.clone()).or_default();
            if !seen.insert(id.clone()) {
                return false;
            }
        }

        let queue = self.messages.entry(key.clone()).or_default();
        queue.push_back(msg);
        while queue.len() > self.max_per_channel {
            if let Some(evicted) = queue.pop_front()
                && let Some(ref id) = evicted.msgid
                && let Some(seen) = self.seen_msgids.get_mut(&key)
            {
                seen.remove(id);
            }
        }
        true
    }

    /// Insert a message at the correct position by timestamp (binary search).
    ///
    /// Used for history messages that may arrive with timestamps older than
    /// what is already stored. Deduplicates by `msgid` when present.
    /// Returns `true` if the message was actually inserted (not a duplicate).
    fn insert_by_timestamp(
        &mut self,
        connection_id: &str,
        target: &str,
        msg: StoredMessage,
    ) -> bool {
        let key = (connection_id.to_string(), target.to_string());

        // Deduplicate by msgid if present.
        if let Some(ref id) = msg.msgid {
            let seen = self.seen_msgids.entry(key.clone()).or_default();
            if !seen.insert(id.clone()) {
                return false;
            }
        }

        // Binary search on timestamp.  The VecDeque is kept sorted.
        let ts = msg.timestamp;
        let queue = self.messages.entry(key.clone()).or_default();
        let pos = queue.partition_point(|m| m.timestamp <= ts);
        queue.insert(pos, msg);

        while queue.len() > self.max_per_channel {
            if let Some(evicted) = queue.pop_front()
                && let Some(ref id) = evicted.msgid
                && let Some(seen) = self.seen_msgids.get_mut(&key)
            {
                seen.remove(id);
            }
        }
        true
    }

    fn ensure_channel(&mut self, connection_id: &str, channel: &str) -> &mut ChannelState {
        self.channels
            .entry(connection_id.to_string())
            .or_default()
            .entry(channel.to_string())
            .or_insert_with(|| ChannelState {
                topic: None,
                users: Vec::new(),
                last_read_timestamp: 0,
            })
    }
}

pub(crate) fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Parse an IRCv3 `server-time` tag (ISO 8601) to milliseconds since UNIX epoch.
///
/// Format: `2025-01-15T21:55:19.123Z` (the fractional seconds are optional).
pub(crate) fn parse_iso8601_millis(s: &str) -> Option<u64> {
    use chrono::{DateTime, NaiveDateTime, Utc};

    // Try full RFC 3339 / ISO 8601 with timezone first
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis() as u64);
    }
    // Fall back to common IRC server-time format: "2024-01-15T12:34:56.789Z"
    // with optional fractional seconds and trailing Z
    let s = s
        .strip_suffix('Z')
        .or_else(|| s.strip_suffix('z'))
        .unwrap_or(s);
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(naive.and_utc().timestamp_millis() as u64);
    }
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(naive.and_utc().timestamp_millis() as u64);
    }
    // Try parsing as a DateTime<Utc> directly
    if let Ok(dt) = s.parse::<DateTime<Utc>>() {
        return Some(dt.timestamp_millis() as u64);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_retrieve_messages() {
        let mut store = MessageStore::new();

        store.store_incoming("conn1", "#test", "alice", "hello", None, None, None, None);
        store.store_outgoing("conn1", "#test", "bob", "hi there", None, None);

        let msgs = store.get_messages("conn1", "#test");
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].sender, "alice");
        assert_eq!(msgs[0].direction, MessageDirection::Incoming);
        assert_eq!(msgs[1].sender, "bob");
        assert_eq!(msgs[1].direction, MessageDirection::Outgoing);
    }

    #[test]
    fn test_ring_buffer_eviction() {
        let mut store = MessageStore {
            max_per_channel: 3,
            ..Default::default()
        };

        store.store_incoming("conn1", "#test", "alice", "msg1", None, None, None, None);
        store.store_incoming("conn1", "#test", "alice", "msg2", None, None, None, None);
        store.store_incoming("conn1", "#test", "alice", "msg3", None, None, None, None);
        store.store_incoming("conn1", "#test", "alice", "msg4", None, None, None, None);

        let msgs = store.get_messages("conn1", "#test");
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].content, "msg2");
        assert_eq!(msgs[2].content, "msg4");
    }

    #[test]
    fn test_separate_channels() {
        let mut store = MessageStore::new();

        store.store_incoming(
            "conn1", "#chan1", "alice", "in chan1", None, None, None, None,
        );
        store.store_incoming("conn1", "#chan2", "bob", "in chan2", None, None, None, None);

        assert_eq!(store.get_messages("conn1", "#chan1").len(), 1);
        assert_eq!(store.get_messages("conn1", "#chan2").len(), 1);
        assert_eq!(store.get_messages("conn1", "#chan3").len(), 0);
    }

    #[test]
    fn test_separate_connections() {
        let mut store = MessageStore::new();

        store.store_incoming(
            "conn1",
            "#test",
            "alice",
            "from conn1",
            None,
            None,
            None,
            None,
        );
        store.store_incoming(
            "conn2",
            "#test",
            "bob",
            "from conn2",
            None,
            None,
            None,
            None,
        );

        assert_eq!(store.get_messages("conn1", "#test").len(), 1);
        assert_eq!(store.get_messages("conn2", "#test").len(), 1);
    }

    #[test]
    fn test_channel_tracking() {
        let mut store = MessageStore::new();

        store.add_channel("conn1", "#chan1");
        store.add_channel("conn1", "#chan2");

        let channels = store.get_channels("conn1");
        assert_eq!(channels.len(), 2);

        store.remove_channel("conn1", "#chan1");
        let channels = store.get_channels("conn1");
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "#chan2");
    }

    #[test]
    fn test_user_tracking() {
        let mut store = MessageStore::new();

        store.add_channel("conn1", "#test");
        store.user_joined("conn1", "#test", "alice");
        store.user_joined("conn1", "#test", "bob");

        let users = store.get_users("conn1", "#test");
        assert_eq!(users.len(), 2);
        assert!(users.iter().any(|u| u.nick == "alice"));
        assert!(users.iter().any(|u| u.nick == "bob"));

        store.user_parted("conn1", "#test", "alice");
        let users = store.get_users("conn1", "#test");
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].nick, "bob");
    }

    #[test]
    fn test_user_quit_removes_from_all_channels() {
        let mut store = MessageStore::new();

        store.add_channel("conn1", "#chan1");
        store.add_channel("conn1", "#chan2");
        store.user_joined("conn1", "#chan1", "alice");
        store.user_joined("conn1", "#chan2", "alice");

        store.user_quit("conn1", "alice");

        assert_eq!(store.get_users("conn1", "#chan1").len(), 0);
        assert_eq!(store.get_users("conn1", "#chan2").len(), 0);
    }

    #[test]
    fn test_set_users_replaces_list() {
        let mut store = MessageStore::new();

        store.add_channel("conn1", "#test");
        store.user_joined("conn1", "#test", "old_user");

        store.set_users(
            "conn1",
            "#test",
            vec!["new1".to_string(), "new2".to_string()],
        );

        let users = store.get_users("conn1", "#test");
        assert_eq!(users.len(), 2);
        assert!(!users.iter().any(|u| u.nick == "old_user"));
    }

    #[test]
    fn test_remove_connection_clears_everything() {
        let mut store = MessageStore::new();

        store.add_channel("conn1", "#test");
        store.store_incoming("conn1", "#test", "alice", "hello", None, None, None, None);
        store.user_joined("conn1", "#test", "alice");

        store.remove_connection("conn1");

        assert_eq!(store.get_channels("conn1").len(), 0);
        assert_eq!(store.get_messages("conn1", "#test").len(), 0);
        assert_eq!(store.get_users("conn1", "#test").len(), 0);
    }

    #[test]
    fn test_history_inserts_by_timestamp() {
        let mut store = MessageStore::new();

        // Simulate live messages arriving in real time.
        store.store_incoming("conn1", "#test", "alice", "live1", None, None, None, None);
        store.store_incoming("conn1", "#test", "bob", "live2", None, None, None, None);

        let live_ts = store.get_messages("conn1", "#test")[0].timestamp;

        // Now load history with older timestamps (like CHATHISTORY BEFORE).
        store.store_history(
            "conn1",
            "#test",
            "carol",
            "old1",
            None,
            Some("2020-01-01T00:00:01.000Z"),
            Some("msg-old1"),
        );
        store.store_history(
            "conn1",
            "#test",
            "dave",
            "old2",
            None,
            Some("2020-01-01T00:00:02.000Z"),
            Some("msg-old2"),
        );

        let msgs = store.get_messages("conn1", "#test");
        assert_eq!(msgs.len(), 4);
        // History messages should come first (older timestamps).
        assert_eq!(msgs[0].content, "old1");
        assert_eq!(msgs[1].content, "old2");
        // Live messages should come after.
        assert!(msgs[2].timestamp >= live_ts);
        assert!(msgs[3].timestamp >= live_ts);
    }

    #[test]
    fn test_history_deduplicates_by_msgid() {
        let mut store = MessageStore::new();

        store.store_history(
            "conn1",
            "#test",
            "alice",
            "hello",
            None,
            Some("2020-06-15T12:00:00.000Z"),
            Some("msg-abc"),
        );
        // Same msgid again — should be ignored.
        store.store_history(
            "conn1",
            "#test",
            "alice",
            "hello",
            None,
            Some("2020-06-15T12:00:00.000Z"),
            Some("msg-abc"),
        );

        assert_eq!(store.get_messages("conn1", "#test").len(), 1);
    }

    #[test]
    fn test_data_payload_stored() {
        let mut store = MessageStore::new();

        store.store_incoming(
            "conn1",
            "#test",
            "alice",
            "hello",
            Some(r#"{"type":"test"}"#),
            None,
            None,
            None,
        );

        let msgs = store.get_messages("conn1", "#test");
        assert_eq!(msgs[0].data, Some(r#"{"type":"test"}"#.to_string()));
    }

    #[test]
    fn test_synthetic_msgid_assigned_when_missing() {
        let mut store = MessageStore::new();

        // Messages without a msgid should get a synthetic one.
        store.store_incoming("conn1", "#test", "alice", "no id", None, None, None, None);
        store.store_outgoing("conn1", "#test", "bob", "also no id", None, None);

        let msgs = store.get_messages("conn1", "#test");
        assert_eq!(msgs[0].msgid, Some("synthetic-1".to_string()));
        assert_eq!(msgs[1].msgid, Some("synthetic-2".to_string()));
    }

    #[test]
    fn test_real_msgid_preserved() {
        let mut store = MessageStore::new();

        // Messages WITH a real msgid should keep it.
        store.store_incoming(
            "conn1",
            "#test",
            "alice",
            "has id",
            None,
            Some("real-abc"),
            None,
            None,
        );

        let msgs = store.get_messages("conn1", "#test");
        assert_eq!(msgs[0].msgid, Some("real-abc".to_string()));
    }

    #[test]
    fn test_synthetic_msgid_counter_increments_across_methods() {
        let mut store = MessageStore::new();

        store.store_incoming("conn1", "#test", "a", "msg1", None, None, None, None);
        // This one has a real msgid — counter should NOT increment.
        store.store_incoming(
            "conn1",
            "#test",
            "b",
            "msg2",
            None,
            Some("real-1"),
            None,
            None,
        );
        store.store_outgoing("conn1", "#test", "c", "msg3", None, None);
        store.store_history(
            "conn1",
            "#test",
            "d",
            "msg4",
            None,
            Some("2020-01-01T00:00:00.000Z"),
            None,
        );

        let msgs = store.get_messages("conn1", "#test");
        // History message (oldest) comes first, then the rest by timestamp.
        let history = msgs.iter().find(|m| m.content == "msg4").unwrap();
        assert_eq!(history.msgid, Some("synthetic-3".to_string()));

        let msg1 = msgs.iter().find(|m| m.content == "msg1").unwrap();
        assert_eq!(msg1.msgid, Some("synthetic-1".to_string()));

        let msg2 = msgs.iter().find(|m| m.content == "msg2").unwrap();
        assert_eq!(msg2.msgid, Some("real-1".to_string()));

        let msg3 = msgs.iter().find(|m| m.content == "msg3").unwrap();
        assert_eq!(msg3.msgid, Some("synthetic-2".to_string()));
    }
}
