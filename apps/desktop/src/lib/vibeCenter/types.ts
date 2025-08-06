import type { Message, MessageParam } from '@anthropic-ai/sdk/resources/index.mjs';

export type ClaudeCodeEvent =
	// An assistant message
	| {
			type: 'assistant';
			message: Message; // from Anthropic SDK
			session_id: string;
	  }

	// A user message
	| {
			type: 'user';
			message: MessageParam; // from Anthropic SDK
			session_id: string;
	  }

	// Emitted as the last message
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

	// Emitted as the last message, when we've reached the maximum number of turns
	| {
			type: 'result';
			subtype: 'error_max_turns' | 'error_during_execution';
			duration_ms: number;
			duration_api_ms: number;
			is_error: boolean;
			num_turns: number;
			session_id: string;
			total_cost_usd: number;
	  }

	// Emitted as the first message at the start of a conversation
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

export type ContentBlock = {
	text: string;
	type: 'text';
};

export type ToolUseBlock = {
	id: string;
	input: Record<string, unknown>;
	name: string;
	type: 'tool_use';
};

export type ToolResultBlock = {
	content: string;
	tool_use_id: string;
	type: 'tool_result';
};

export type Usage = {
	cache_creation_input_tokens?: number;
	cache_read_input_tokens?: number;
	input_tokens: number;
	output_tokens: number;
	service_tier?: string;
};

export type UserMessage = {
	role: string;
	content: (ContentBlock | ToolResultBlock)[];
};

export type AssistantMessage = {
	id?: string;
	type?: string;
	role: string;
	model?: string;
	content: (ContentBlock | ToolUseBlock)[];
	stop_reason?: string | null;
	stop_sequence?: string | null;
	usage?: Usage;
};

export type ToolUseResult = {
	durationMs?: number;
	filenames?: string[];
	numFiles?: number;
	truncated?: boolean;
	interrupted?: boolean;
	isImage?: boolean;
	stderr?: string;
	stdout?: string;
	mode?: string;
};

export type TranscriptEntry =
	| {
			type: 'summary';
			summary: string;
			leafUuid: string;
	  }
	| {
			type: 'user';
			parentUuid?: string;
			isSidechain?: boolean;
			userType?: string;
			cwd?: string;
			sessionId?: string;
			version?: string;
			message?: UserMessage;
			uuid?: string;
			timestamp?: string;
			gitBranch?: string;
			toolUseResult?: ToolUseResult;
	  }
	| {
			type: 'assistant';
			parentUuid?: string;
			isSidechain?: boolean;
			userType?: string;
			cwd?: string;
			sessionId?: string;
			version?: string;
			message?: AssistantMessage;
			requestId?: string;
			uuid?: string;
			timestamp?: string;
			toolUseResult?: ToolUseResult;
			gitBranch?: string;
			parent_tool_use_id?: string;
			session_id?: string;
	  }
	| {
			type: 'result';
			duration_api_ms: number;
			duration_ms: number;
			is_error: boolean;
			num_turns: number;
			result: string;
			session_id: string;
			subtype: 'success';
			total_cost_usd: number;
			usage: Usage & {
				server_tool_use?: {
					web_search_requests: number;
				};
			};
	  };
