import type { Message, MessageParam, Usage } from '@anthropic-ai/sdk/resources/index.mjs';

/**
 * Represents a file attachment with full content (used in API input).
 */
export type PromptAttachment =
	| { type: 'file'; path: string; commitId?: string }
	| { type: 'lines'; path: string; start: number; end: number; commitId?: string }
	| { type: 'commit'; commitId: string };

/**
 * Result of checking Claude Code availability
 */
export type ClaudeCheckResult =
	| { status: 'available'; version: string }
	| { status: 'not_available' };

/**
 * Represents different types of events that can occur during Claude code interactions
 */
export type ClaudeCodeMessage =
	/** An assistant message */
	| {
			type: 'assistant';
			/** Message from Anthropic SDK */
			message: Message;
			session_id: string;
	  }

	/** A user message */
	| {
			type: 'user';
			/** Message from Anthropic SDK */
			message: MessageParam;
			session_id: string;
	  }

	/** Emitted as the last message */
	| {
			type: 'result';
			subtype: 'success';
			duration_ms: number;
			duration_api_ms: number;
			is_error: boolean;
			num_turns: number;
			result: string;
			session_id: string;
			total_cost_usd: number;
			usage: Usage;
	  }

	/** Emitted as the last message, when we've reached the maximum number of turns */
	| {
			type: 'result';
			subtype: 'error_max_turns' | 'error_during_execution';
			duration_ms: number;
			duration_api_ms: number;
			is_error: boolean;
			num_turns: number;
			session_id: string;
			total_cost_usd: number;
			usage: Usage;
	  }

	/** Emitted as the first message at the start of a conversation */
	| {
			type: 'system';
			subtype: 'init';
			apiKeySource: string;
			cwd: string;
			session_id: string;
			tools: string[];
			mcp_servers: {
				name: string;
				status: string;
			}[];
			model: string;
			permissionMode: 'default' | 'acceptEdits' | 'bypassPermissions' | 'plan';
	  };

/**
 * Represents a Claude Code session that GitButler is tracking.
 */
export type ClaudeSession = {
	/** The unique and stable identifier for the session. This is the first session_id that was used. */
	id: string;
	/** The most recent session ID. If a session is stopped and resumed, Claude will copy over the past context into a new session. This value is unique. */
	currentId: string;
	/** All session IDs that have been used for this session, including the current one. */
	sessionIds: string[];
	/** The timestamp when the first session was created. */
	createdAt: string;
	/** The timestamp when the session was last updated. */
	updatedAt: string;
};

/**
 * Represents a message in a Claude session, referencing the stable session ID.
 */
export type ClaudeMessage = {
	/** Message identifier */
	id: string;
	/** The stable session ID that this message belongs to. */
	sessionId: string;
	/** The timestamp when the message was created. */
	createdAt: string;
	/** The payload of the message from different sources. */
	payload: MessagePayload;
};

/**
 * The actual message payload from different sources.
 * Uses external tagging for protobuf compatibility.
 */
export type MessagePayload =
	/** Output from Claude Code CLI stdout stream */
	| ({ source: 'claude' } & ClaudeOutput)
	/** Input provided by the user */
	| ({ source: 'user' } & UserInput)
	/** System message from GitButler about the session */
	| ({ source: 'system' } & SystemMessage);

/**
 * Raw output from Claude API
 */
export type ClaudeOutput = {
	/** Raw JSON value from Claude API streaming output */
	data: ClaudeCodeMessage;
};

export type UserInput = {
	message: string;
	attachments?: PromptAttachment[];
};

/**
 * System messages from GitButler about the Claude session state.
 */
export type SystemMessage =
	| {
			type: 'claudeExit';
			code: number;
			message: string;
	  }
	| {
			type: 'userAbort';
	  }
	| {
			type: 'unhandledException';
			message: string;
	  }
	| {
			type: 'compactStart';
	  }
	| {
			type: 'compactFinished';
			summary: string;
	  }
	| {
			type: 'commitCreated';
			stackId: string;
			branchName: string;
			commitIds: string[];
	  };

/**
 * Details about a Claude session, extracted from the Claude transcript.
 * This data is derived just in time, i.e. not persisted by GitButler.
 */
export type ClaudeSessionDetails = {
	summary: string | null;
	lastPrompt: string | null;
	inGui: boolean;
};

export function sessionMessage(sessionDetails: ClaudeSessionDetails): string | undefined {
	return sessionDetails.summary ?? sessionDetails.lastPrompt ?? undefined;
}

export type ClaudeStatus = 'disabled' | 'enabled' | 'running' | 'compacting';

/**
 * Represents a permission decision with both the action (allow/deny) and scope.
 */
export type PermissionDecision =
	| 'allowOnce'
	| 'allowSession'
	| 'allowProject'
	| 'allowAlways'
	| 'denyOnce'
	| 'denySession'
	| 'denyProject'
	| 'denyAlways';

export type ClaudePermissionRequest = {
	/** Maps to the tool_use_id from the MCP request */
	id: string;
	/** When the request was made */
	createdAt: string;
	/** When the request was updated */
	updatedAt: string;
	/** The tool for which permission is requested */
	toolName: string;
	/** The input for the tool */
	input: unknown;
	/** The permission decision or null if not yet handled */
	decision?: PermissionDecision;
};

export type ClaudeTodo = {
	status: 'pending' | 'in_progress' | 'completed';
	content: string;
};

export type ThinkingLevel = 'normal' | 'think' | 'megaThink' | 'ultraThink';

export type ModelType = 'haiku' | 'sonnet' | 'sonnet[1m]' | 'opus' | 'opusplan';

export type PermissionMode = 'default' | 'plan' | 'acceptEdits';

export type PromptTemplate = {
	fileName: string;
	template: string;
};

export type PromptDir = {
	label: string;
	path: string;
	filters: string[];
};

export type McpConfig = {
	mcpServers: Record<string, McpServer>;
};

export type McpServer = {
	type: string | null;
	command: string | null;
	url: string | null;
	args: string[] | null;
	env: Record<string, string> | null;
};

export type SubAgent = {
	name: string;
	description: string;
	// If this is null, all tools are allowed
	tools: string[] | null;
	model: string | null;
};
