/**
 * IRC Protocol for Claude Session Communication
 *
 * This module defines the message types and serialization logic for
 * communicating Claude session state over IRC.
 *
 * Channel naming convention:
 *   #<prefix>-<stack-id>  - Session-specific channel (prefix defaults to 'claude')
 *   #gb-<repo-name>       - Project-level announcements
 *
 * Message format:
 *   IRC message body contains a human-readable summary
 *   +data tag contains JSON payload with full structured data
 *
 * Size limits:
 *   IRC message tags are limited to ~4KB total. Messages exceeding this
 *   limit are truncated with a continuation marker for future multi-part
 *   message support.
 */

import type {
	ClaudeMessage,
	ClaudeTodo,
	ClaudeStatus,
	PromptAttachment,
	PermissionDecision,
	AskUserQuestion,
} from "$lib/codegen/types";
import type { TreeChange } from "$lib/hunks/change";
import type { DiffHunk } from "$lib/hunks/hunk";
import type { SharedCommitPayload, SharedStackPayload } from "$lib/irc/sharedStack";

// ============================================================================
// Constants
// ============================================================================

/** Maximum size for the +data tag value in bytes */
const MAX_DATA_SIZE = 1_000_000;

/** Marker indicating the payload was truncated */
const TRUNCATION_MARKER = "__truncated__";

// ============================================================================
// Message Types
// ============================================================================

/**
 * Base envelope for all IRC protocol messages.
 */
type MessageEnvelope<T extends string, P> = {
	/** Message type discriminator */
	type: T;
	/** The actual payload */
	payload: P;
	/** True if payload was truncated due to size limits */
	truncated?: boolean;
};

// ----------------------------------------------------------------------------
// Session Lifecycle
// ----------------------------------------------------------------------------

export type SessionStartMessage = MessageEnvelope<
	"session-start",
	{
		projectId: string;
		stackId: string;
		branchName?: string;
	}
>;

export type SessionEndMessage = MessageEnvelope<
	"session-end",
	{
		code: number;
		reason?: string;
	}
>;

export type SessionStatusMessage = MessageEnvelope<
	"session-status",
	{
		status: ClaudeStatus;
		todos: ClaudeTodo[];
	}
>;

// ----------------------------------------------------------------------------
// Session Discovery (Request/Response)
// ----------------------------------------------------------------------------

export type SessionListRequestMessage = MessageEnvelope<
	"session-list-request",
	{
		/** Optional filter by project */
		projectId?: string;
	}
>;

export type SessionListResponseMessage = MessageEnvelope<
	"session-list-response",
	{
		/** Responding to this request ID (for correlation) */
		requestId?: string;
		sessions: Array<{
			projectId: string;
			stackId: string;
			branchName?: string;
			status: ClaudeStatus;
			channel: string;
		}>;
	}
>;

// ----------------------------------------------------------------------------
// Conversation Messages
// ----------------------------------------------------------------------------

export type PromptMessage = MessageEnvelope<
	"prompt",
	{
		message: string;
		attachments?: PromptAttachment[];
		/** Who sent this prompt (for authorization tracking) */
		sender?: string;
	}
>;

export type ResponseMessage = MessageEnvelope<
	"response",
	{
		/** The text response from Claude */
		text: string;
		/** Summary of tool calls (full details may be truncated) */
		toolCalls?: Array<{
			id: string;
			name: string;
			/** Truncated/summarized input */
			inputSummary?: string;
			status: "pending" | "approved" | "denied" | "completed";
		}>;
	}
>;

// ----------------------------------------------------------------------------
// Tool Permissions
// ----------------------------------------------------------------------------

export type PermissionRequestMessage = MessageEnvelope<
	"permission-request",
	{
		requestId: string;
		toolName: string;
		/** Summarized input (may be truncated) */
		inputSummary: string;
		/** Full input if it fits */
		input?: unknown;
	}
>;

export type PermissionDecisionMessage = MessageEnvelope<
	"permission-decision",
	{
		requestId: string;
		decision: PermissionDecision;
		/** Who made this decision (for authorization/audit) */
		decidedBy?: string;
	}
>;

// ----------------------------------------------------------------------------
// User Questions (AskUserQuestion tool)
// ----------------------------------------------------------------------------

export type QuestionMessage = MessageEnvelope<
	"question",
	{
		toolUseId: string;
		questions: AskUserQuestion[];
	}
>;

export type AnswerMessage = MessageEnvelope<
	"answer",
	{
		toolUseId: string;
		answers: Record<string, string>;
		/** Who answered (for authorization/audit) */
		answeredBy?: string;
	}
>;

// ----------------------------------------------------------------------------
// Control Messages
// ----------------------------------------------------------------------------

export type AbortMessage = MessageEnvelope<
	"abort",
	{
		/** Who requested abort (for authorization/audit) */
		requestedBy?: string;
	}
>;

// ----------------------------------------------------------------------------
// Raw Claude Message (for full sync)
// ----------------------------------------------------------------------------

export type ClaudeMessageWrapper = MessageEnvelope<
	"claude-message",
	{
		/** The raw ClaudeMessage, potentially truncated */
		message: ClaudeMessage;
	}
>;

export type SharedStackMessage = MessageEnvelope<
	"shared-stack",
	{
		sender: string;
		stack: SharedStackPayload;
	}
>;

export type SharedCommitMessage = MessageEnvelope<
	"shared-commit",
	{
		sender: string;
		commit: SharedCommitPayload;
	}
>;

// ============================================================================
// Union Type
// ============================================================================

export type IrcProtocolMessage =
	| SessionStartMessage
	| SessionEndMessage
	| SessionStatusMessage
	| SessionListRequestMessage
	| SessionListResponseMessage
	| PromptMessage
	| ResponseMessage
	| PermissionRequestMessage
	| PermissionDecisionMessage
	| QuestionMessage
	| AnswerMessage
	| AbortMessage
	| ClaudeMessageWrapper
	| SharedStackMessage
	| SharedCommitMessage;

// ============================================================================
// Serialization
// ============================================================================

export type SerializeResult = {
	/** Human-readable message body */
	text: string;
	/** JSON data for +data tag */
	data: string;
	/** Whether the payload was truncated */
	truncated: boolean;
};

/**
 * Serialize a protocol message for IRC transmission.
 *
 * If the payload exceeds MAX_DATA_SIZE, it will be truncated and marked.
 */
export function serialize(message: IrcProtocolMessage): SerializeResult {
	const text = generateHumanReadable(message);
	let data = JSON.stringify(message);
	let truncated = false;

	if (data.length > MAX_DATA_SIZE) {
		// Mark as truncated and try to fit
		const truncatedMessage = {
			...message,
			truncated: true,
			payload: truncatePayload(message.type, message.payload),
		};
		data = JSON.stringify(truncatedMessage);
		truncated = true;

		// If still too large, use minimal payload
		if (data.length > MAX_DATA_SIZE) {
			const minimalMessage = {
				type: message.type,
				truncated: true,
				payload: { [TRUNCATION_MARKER]: true },
			};
			data = JSON.stringify(minimalMessage);
		}
	}

	return { text, data, truncated };
}

/**
 * Parse an IRC +data tag value into a protocol message.
 */
export function parse(data: string): IrcProtocolMessage | null {
	try {
		const parsed = JSON.parse(data);
		if (typeof parsed === "object" && parsed !== null && "type" in parsed) {
			return parsed as IrcProtocolMessage;
		}
		return null;
	} catch {
		return null;
	}
}

// ============================================================================
// Message Data Parsing
// ============================================================================

export type ParsedMessageData =
	| { type: "shared-commit"; commit: SharedCommitPayload; truncated: boolean }
	| { type: "shared-hunk"; change: TreeChange; diff: DiffHunk }
	| { type: "working-files-sync"; files: string[] }
	| { type: "working-files-delta"; added: string[]; removed: string[] };

/**
 * Parse the JSON data attached to an IRC message into a typed result.
 * Handles both protocol-wrapped messages (shared-commit) and raw payloads (shared-hunk).
 */
export function parseMessageData(data: string): ParsedMessageData | undefined {
	let parsed: any;
	try {
		parsed = JSON.parse(data);
	} catch {
		return undefined;
	}

	if (parsed?.type === "shared-commit") {
		const commit = parsed.payload?.commit as SharedCommitPayload | undefined;
		if (commit) {
			return { type: "shared-commit", commit, truncated: !!parsed.truncated };
		}
	}

	if (parsed?.change && parsed?.diff) {
		return {
			type: "shared-hunk",
			change: parsed.change as TreeChange,
			diff: parsed.diff as DiffHunk,
		};
	}

	if (parsed?.type === "working-files-sync" && Array.isArray(parsed.files)) {
		return { type: "working-files-sync", files: parsed.files as string[] };
	}

	if (parsed?.type === "working-files-delta") {
		return {
			type: "working-files-delta",
			added: (parsed.added as string[]) ?? [],
			removed: (parsed.removed as string[]) ?? [],
		};
	}

	return undefined;
}

// ============================================================================
// Plain Text Command Parsing (for bare IRC clients)
// ============================================================================

/**
 * Commands that can be sent from a bare IRC client.
 *
 * Format examples:
 *   !prompt Help me refactor this function
 *   !approve abc123
 *   !deny abc123
 *   !abort
 *   !status
 *   !answer 1=optionA 2=optionB
 *   !sessions
 *
 * Or without prefix (detected by content):
 *   Just plain text is treated as a prompt
 */
export type PlainTextCommand =
	| { type: "prompt"; message: string }
	| { type: "approve"; requestId: string }
	| { type: "deny"; requestId: string; reason?: string }
	| { type: "abort" }
	| { type: "status" }
	| { type: "answer"; answers: Record<string, string> }
	| { type: "sessions"; projectId?: string }
	| { type: "unknown"; raw: string };

/**
 * Parse a plain text message from an IRC client into a command.
 *
 * Supports both bang commands (!prompt, !approve, etc.) and
 * bare text (treated as a prompt).
 */
export function parseTextCommand(text: string): PlainTextCommand {
	const trimmed = text.trim();

	// Bang commands (using ! instead of / to avoid IRC client conflicts)
	if (trimmed.startsWith("!")) {
		const spaceIdx = trimmed.indexOf(" ");
		const command =
			spaceIdx > 0 ? trimmed.slice(1, spaceIdx).toLowerCase() : trimmed.slice(1).toLowerCase();
		const args = spaceIdx > 0 ? trimmed.slice(spaceIdx + 1).trim() : "";

		switch (command) {
			case "prompt":
			case "p":
				return { type: "prompt", message: args };

			case "approve":
			case "ok":
			case "y":
				return { type: "approve", requestId: args || "latest" };

			case "deny":
			case "reject":
			case "n": {
				// Support: /deny abc123 --reason "too risky"
				const reasonMatch = args.match(/^(\S+)(?:\s+(?:--reason\s+)?["']?(.+?)["']?)?$/);
				return {
					type: "deny",
					requestId: reasonMatch?.[1] || "latest",
					reason: reasonMatch?.[2],
				};
			}

			case "abort":
			case "stop":
			case "cancel":
				return { type: "abort" };

			case "status":
			case "s":
				return { type: "status" };

			case "answer":
			case "a": {
				// Parse: /answer 1=optionA 2=optionB
				const answers: Record<string, string> = {};
				const pairs = args.match(/(\S+)=(\S+)/g) || [];
				for (const pair of pairs) {
					const [key, value] = pair.split("=");
					if (key && value) answers[key] = value;
				}
				return { type: "answer", answers };
			}

			case "sessions":
			case "list":
				return { type: "sessions", projectId: args || undefined };

			default:
				return { type: "unknown", raw: trimmed };
		}
	}

	// Treat any non-command message as a prompt
	// This makes it easier to interact - just type your message without needing !prompt
	return { type: "prompt", message: trimmed };
}

// ============================================================================
// Human-Readable Generation
// ============================================================================

/**
 * Generate human-readable text for bare IRC clients.
 *
 * The output is designed to be:
 * 1. Readable without rich rendering
 * 2. Actionable - includes command hints where relevant
 * 3. Informative - shows key details inline
 */
function generateHumanReadable(message: IrcProtocolMessage): string {
	switch (message.type) {
		case "session-start":
			return `[Session] Started: ${message.payload.branchName || message.payload.stackId}`;

		case "session-end":
			return message.payload.code === 0
				? `[Session] Completed successfully`
				: `[Session] Ended with error (code ${message.payload.code})`;

		case "session-status": {
			const { status, todos } = message.payload;
			const completed = todos.filter((t) => t.status === "completed").length;
			const inProgress = todos.filter((t) => t.status === "in_progress").length;
			const todoSummary = todos.length > 0 ? ` | Todos: ${completed}/${todos.length} done` : "";
			const currentTask = todos.find((t) => t.status === "in_progress");
			const taskInfo = currentTask ? ` | Current: ${truncateText(currentTask.content, 50)}` : "";
			return `[Status] ${status}${todoSummary}${taskInfo}`;
		}

		case "session-list-request":
			return `[Request] Listing active sessions...`;

		case "session-list-response": {
			const { sessions } = message.payload;
			if (sessions.length === 0) {
				return `[Sessions] No active sessions`;
			}
			const list = sessions
				.slice(0, 5)
				.map((s) => `${s.branchName || s.stackId} (${s.status})`)
				.join(", ");
			const more = sessions.length > 5 ? ` +${sessions.length - 5} more` : "";
			return `[Sessions] ${list}${more}`;
		}

		case "prompt": {
			const sender = message.payload.sender ? `${message.payload.sender}: ` : "";
			return `[Prompt] ${sender}${truncateText(message.payload.message, 200)}`;
		}

		case "response": {
			const { text, toolCalls } = message.payload;
			const toolInfo =
				toolCalls && toolCalls.length > 0
					? ` | Tools: ${toolCalls.map((t) => t.name).join(", ")}`
					: "";
			return `[Claude] ${truncateText(text, 150)}${toolInfo}`;
		}

		case "permission-request": {
			const { requestId, toolName, inputSummary } = message.payload;
			const shortId = requestId.slice(0, 8);
			return `[Permission] ${toolName} needs approval (${shortId}) | !approve ${shortId} or !deny ${shortId} | ${truncateText(inputSummary, 100)}`;
		}

		case "permission-decision": {
			const { requestId, decision, decidedBy } = message.payload;
			const shortId = requestId.slice(0, 8);
			const by = decidedBy ? ` by ${decidedBy}` : "";
			return `[Permission] ${shortId} ${decision}${by}`;
		}

		case "question": {
			const { toolUseId, questions } = message.payload;
			const shortId = toolUseId.slice(0, 8);
			const q = questions[0];
			if (!q) return `[Question] (${shortId}) No question content`;

			const options = q.options.map((o, i) => `${i + 1}=${o.label}`).join(" ");
			return `[Question] (${shortId}) ${q.question} | Options: ${options} | !answer ${options.replace(/=/g, "=")}`;
		}

		case "answer": {
			const { toolUseId, answers, answeredBy } = message.payload;
			const shortId = toolUseId.slice(0, 8);
			const by = answeredBy ? ` by ${answeredBy}` : "";
			const answerText = Object.entries(answers)
				.map(([k, v]) => `${k}=${v}`)
				.join(", ");
			return `[Answer] (${shortId}) ${answerText}${by}`;
		}

		case "abort": {
			const by = message.payload.requestedBy ? ` by ${message.payload.requestedBy}` : "";
			return `[Abort] Session abort requested${by}`;
		}

		case "claude-message":
			return `[Message] Claude message received`;

		case "shared-stack": {
			const { stack, sender } = message.payload;
			const branchNames = stack.branches.map((b) => b.name).join(", ");
			return `[Stack] ${sender} shared ${stack.projectName}: ${branchNames}`;
		}

		case "shared-commit": {
			const { commit, sender } = message.payload;
			const title = commit.commit.message.split("\n")[0] || "untitled";
			return `[Commit] ${sender} shared ${commit.projectName}: ${title}`;
		}

		default:
			return `[Message] Unknown message type`;
	}
}

// ============================================================================
// Truncation Helpers
// ============================================================================

function truncateText(text: string, maxLength: number): string {
	if (text.length <= maxLength) return text;
	return text.slice(0, maxLength - 3) + "...";
}

function truncatePayload(type: string, payload: unknown): unknown {
	if (typeof payload !== "object" || payload === null) {
		return payload;
	}

	const p = payload as Record<string, unknown>;

	switch (type) {
		case "prompt":
			return {
				...p,
				message: truncateText(String(p.message || ""), 500),
				attachments: p.attachments
					? `[${(p.attachments as unknown[]).length} attachments]`
					: undefined,
			};

		case "response":
			return {
				...p,
				text: truncateText(String(p.text || ""), 500),
				toolCalls: (p.toolCalls as unknown[] | undefined)?.slice(0, 5),
			};

		case "permission-request":
			return {
				...p,
				input: undefined, // Remove full input, keep summary
				inputSummary: truncateText(String(p.inputSummary || JSON.stringify(p.input)), 200),
			};

		case "claude-message":
			// For raw messages, just mark as truncated - receiver should request full data
			return {
				[TRUNCATION_MARKER]: true,
			};

		case "shared-stack": {
			// Strip all diff hunks from files, keeping branch/commit/file metadata
			const stack = p.stack as Record<string, unknown> | undefined;
			if (!stack) return p;
			const branches = stack.branches as Array<Record<string, unknown>> | undefined;
			return {
				...p,
				stack: {
					...stack,
					branches: (branches ?? []).map((branch) => ({
						...branch,
						commits: ((branch.commits as Array<Record<string, unknown>> | undefined) ?? []).map(
							(commit) => ({
								...commit,
								files: ((commit.files as Array<Record<string, unknown>> | undefined) ?? []).map(
									(file) => ({ ...file, hunks: [] }),
								),
							}),
						),
					})),
				},
			};
		}

		case "shared-commit": {
			const commit = p.commit as Record<string, unknown> | undefined;
			if (!commit) return p;
			const inner = commit.commit as Record<string, unknown> | undefined;
			if (!inner) return p;
			return {
				...p,
				commit: {
					...commit,
					commit: {
						...inner,
						files: ((inner.files as Array<Record<string, unknown>>) ?? []).map((file) => ({
							...file,
							hunks: [],
						})),
					},
				},
			};
		}

		case "session-list-response":
			return {
				...p,
				sessions: (p.sessions as unknown[] | undefined)?.slice(0, 10), // Limit to 10 sessions
			};

		default:
			return p;
	}
}

// ============================================================================
// Channel Helpers
// ============================================================================

/**
 * Sanitize an ID for use in IRC channel names.
 * Only allows alphanumeric characters and hyphens.
 */
function sanitizeForChannel(id: string): string {
	return id.replace(/[^a-zA-Z0-9/_-]/g, "");
}

/**
 * Generate the IRC channel name for a Claude session.
 * Channel format: #<username>/<branchName>
 */
export function sessionChannel(username: string, branchName: string): string {
	const safeUsername = sanitizeForChannel(username);
	const safeBranchName = sanitizeForChannel(branchName);
	return `#${safeUsername}/${safeBranchName}`;
}

/**
 * Generate the IRC channel name for project-level announcements.
 * Uses the repository name instead of project ID for more readable channels.
 */
export function projectChannel(repoName: string): string {
	const safeRepoName = sanitizeForChannel(repoName);
	return `#${safeRepoName}`;
}

/**
 * Parse a session channel name to extract username and branch name.
 */
export function parseSessionChannel(
	channel: string,
): { username: string; branchName: string } | null {
	const match = channel.match(/^#([a-zA-Z0-9_-]+)\/([a-zA-Z0-9/_-]+)$/);
	if (!match) return null;
	return { username: match[1]!, branchName: match[2]! };
}

// ============================================================================
// Message Builders
// ============================================================================

export const Messages = {
	sessionStart(params: SessionStartMessage["payload"]): SessionStartMessage {
		return { type: "session-start", payload: params };
	},

	sessionEnd(params: SessionEndMessage["payload"]): SessionEndMessage {
		return { type: "session-end", payload: params };
	},

	sessionStatus(params: SessionStatusMessage["payload"]): SessionStatusMessage {
		return { type: "session-status", payload: params };
	},

	sessionListRequest(params: SessionListRequestMessage["payload"] = {}): SessionListRequestMessage {
		return { type: "session-list-request", payload: params };
	},

	sessionListResponse(params: SessionListResponseMessage["payload"]): SessionListResponseMessage {
		return { type: "session-list-response", payload: params };
	},

	prompt(params: PromptMessage["payload"]): PromptMessage {
		return { type: "prompt", payload: params };
	},

	response(params: ResponseMessage["payload"]): ResponseMessage {
		return { type: "response", payload: params };
	},

	permissionRequest(params: PermissionRequestMessage["payload"]): PermissionRequestMessage {
		return { type: "permission-request", payload: params };
	},

	permissionDecision(params: PermissionDecisionMessage["payload"]): PermissionDecisionMessage {
		return { type: "permission-decision", payload: params };
	},

	question(params: QuestionMessage["payload"]): QuestionMessage {
		return { type: "question", payload: params };
	},

	answer(params: AnswerMessage["payload"]): AnswerMessage {
		return { type: "answer", payload: params };
	},

	abort(params: AbortMessage["payload"]): AbortMessage {
		return { type: "abort", payload: params };
	},

	claudeMessage(params: ClaudeMessageWrapper["payload"]): ClaudeMessageWrapper {
		return { type: "claude-message", payload: params };
	},

	sharedStack(params: SharedStackMessage["payload"]): SharedStackMessage {
		return { type: "shared-stack", payload: params };
	},

	sharedCommit(params: SharedCommitMessage["payload"]): SharedCommitMessage {
		return { type: "shared-commit", payload: params };
	},
};
