import type { Message, MessageParam, Usage } from '@anthropic-ai/sdk/resources/index.mjs';

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
export interface ClaudeSession {
	/** The unique and stable identifier for the session. This is the first session_id that was used. */
	id: string;
	/** The most recent session ID. If a session is stopped and resumed, Claude will copy over the past context into a new session. This value is unique. */
	currentId: string;
	/** The timestamp when the first session was created. */
	createdAt: string;
	/** The timestamp when the session was last updated. */
	updatedAt: string;
}

/**
 * Represents a message in a Claude session, referencing the stable session ID.
 */
export interface ClaudeMessage {
	/** Message identifier */
	id: string;
	/** The stable session ID that this message belongs to. */
	sessionId: string;
	/** The timestamp when the message was created. */
	createdAt: string;
	/** The content of the message, which can be either output from Claude or user input. */
	content: ClaudeMessageContent;
}

/**
 * Represents the kind of content in a Claude message.
 */
export type ClaudeMessageContent =
	/** Came from Claude standard out stream */
	| {
			type: 'claudeOutput';
			subject: ClaudeCodeMessage;
	  }
	/** Inserted via GitButler (what the user typed) */
	| {
			type: 'userInput';
			subject: { message: string };
	  };

/**
 * Details about a Claude session, extracted from the Claude transcript.
 * This data is derived just in time, i.e. not persisted by GitButler.
 */
export type ClaudeSessionDetails = {
	summary: string | null;
	lastPrompt: string | null;
};

export function sessionMessage(sessionDetails: ClaudeSessionDetails): string | undefined {
	return sessionDetails.summary ?? sessionDetails.lastPrompt ?? undefined;
}
