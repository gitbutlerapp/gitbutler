/**
 * This module is responsible for taking the messages from a near-incomprehesable mess into something _vaugly_ useful.
 */

import { isDefined } from '@gitbutler/ui/utils/typeguards';
import type {
	ClaudeMessage,
	ClaudePermissionRequest,
	ClaudeStatus,
	ClaudeTodo
} from '$lib/codegen/types';

export type Message =
	/* This is strictly only things that the real fleshy human has said */
	| {
			type: 'user';
			message: string;
	  }
	/* Output from claude. This is grouped as: A claude message with a bunch of tool calls. */
	| {
			type: 'claude';
			message: string;
			toolCalls: ToolCall[];
			toolCallsPendingApproval: ToolCall[];
	  }
	| {
			type: 'claude';
			subtype: 'compaction';
			message: string;
			toolCalls: ToolCall[];
			toolCallsPendingApproval: ToolCall[];
	  };

export type ToolCall = {
	name: string;
	id: string;
	input: object;
	result: string | undefined;
	requestAt: Date;
	approvedAt?: Date;
};

export function toolCallLoading(toolCall: ToolCall): boolean {
	return toolCall.result === undefined;
}

const loginRequiredMessage = 'Invalid API key · Please run /login';

export function reverseMessages(messages: Message[]): Message[] {
	return [...messages].reverse();
}

export function formatMessages(
	events: ClaudeMessage[],
	permissionRequests: ClaudePermissionRequest[],
	isActive: boolean
): Message[] {
	const permReqsById: Record<string, ClaudePermissionRequest> = {};
	for (const request of permissionRequests) {
		permReqsById[request.id] = request;
	}

	const out: Message[] = [];
	// A mapping to better handle tool call responses when they come in.
	let toolCalls: Record<string, ToolCall> = {};
	let lastAssistantMessage: Message | undefined = undefined;

	for (const [_idx, event] of events.entries()) {
		if (event.content.type === 'userInput') {
			wrapUpAgentSide();
			out.push({
				type: 'user',
				message: event.content.subject.message
			});
			lastAssistantMessage = undefined;
		} else if (event.content.type === 'claudeOutput') {
			const subject = event.content.subject;
			// We've either triggered a tool call, or sent a message
			if (subject.type === 'assistant') {
				const message = subject.message;
				if (message.content[0]!.type === 'text') {
					if (message.content[0]!.text === loginRequiredMessage) {
						continue;
					}
					lastAssistantMessage = {
						type: 'claude',
						message: message.content[0]!.text,
						toolCalls: [],
						toolCallsPendingApproval: []
					};
					out.push(lastAssistantMessage);
				} else if (message.content[0]!.type === 'tool_use') {
					const content = message.content[0]!;
					const toolCall: ToolCall = {
						id: content.id,
						name: content.name,
						input: content.input as object,
						result: undefined,
						requestAt: normalizeDate(new Date(event.createdAt))
					};
					if (!lastAssistantMessage) {
						lastAssistantMessage = {
							type: 'claude',
							message: '',
							toolCalls: [],
							toolCallsPendingApproval: []
						};
						out.push(lastAssistantMessage);
					}

					const permReq = permReqsById[toolCall.id];
					if (permReq && !isDefined(permReq.approved)) {
						lastAssistantMessage.toolCallsPendingApproval.push(toolCall);
					} else {
						if (permReq) {
							toolCall.approvedAt = new Date(permReq.updatedAt);
						}
						lastAssistantMessage.toolCalls.push(toolCall);
					}
					toolCalls[toolCall.id] = toolCall;
				}
			} else if (subject.type === 'user') {
				const content = subject.message.content;
				if (Array.isArray(content) && content[0]!.type === 'tool_result') {
					const result = content[0]!;
					const foundToolCall = toolCalls[result.tool_use_id];
					if (!foundToolCall) {
						// This should never happen
						continue;
					} else if (!isDefined(result.content)) {
						foundToolCall.result = 'Tool completed with no output';
					} else if (typeof result.content === 'string') {
						foundToolCall.result = result.content;
					} else if (result.content[0]!.type === 'text') {
						foundToolCall.result = result.content[0]!.text;
					}
				}
			}
		} else if (event.content.type === 'gitButlerMessage') {
			const subject = event.content.subject;
			if (
				subject.type === 'claudeExit' ||
				subject.type === 'userAbort' ||
				subject.type === 'unhandledException' ||
				subject.type === 'compactStart' ||
				subject.type === 'compactFinished'
			) {
				wrapUpAgentSide();
			}

			if (subject.type === 'claudeExit' && subject.subject.code !== 0) {
				if (previousEventLoginFailureQuery(events, event)) {
					const message: Message = {
						type: 'claude',
						message: `Claude Code is currently not logged in.\n\n Please run \`claude\` in your terminal and complete the login flow in order to use the GitButler Claude Code integration.`,
						toolCalls: [],
						toolCallsPendingApproval: []
					};
					out.push(message);
				} else {
					const message: Message = {
						type: 'claude',
						message: `Claude exited with non 0 error code \n\n\`\`\`\n${subject.subject.message}\n\`\`\``,
						toolCalls: [],
						toolCallsPendingApproval: []
					};
					out.push(message);
				}
			}
			if (subject.type === 'unhandledException') {
				const message: Message = {
					type: 'claude',
					message: `Encountered an unhandled exception when executing Claude.\nPlease verify your Claude Code installation location and try clearing the context. \n\n\`\`\`\n${subject.subject.message}\n\`\`\``,
					toolCalls: [],
					toolCallsPendingApproval: []
				};
				out.push(message);
			}
			if (subject.type === 'userAbort') {
				const message: Message = {
					type: 'claude',
					message: `I've stopped! What can I help you with next?`,
					toolCalls: [],
					toolCallsPendingApproval: []
				};
				out.push(message);
			}
			if (subject.type === 'compactFinished') {
				const message: Message = {
					type: 'claude',
					subtype: 'compaction',
					message: `Context compaction completed: ${subject.subject.summary}`,
					toolCalls: [],
					toolCallsPendingApproval: []
				};
				out.push(message);
			}
		}
	}

	// If the stack is not active, treat it as if the conversation has stopped
	if (!isActive) {
		wrapUpAgentSide();
	}

	function wrapUpAgentSide() {
		// Mark all pending tool calls as aborted (similar to claudeExit/userAbort)
		for (const toolCall of Object.values(toolCalls)) {
			if (toolCall.result) continue;
			toolCall.result = 'Tool call aborted - session is no longer active';
		}
		toolCalls = {};
		// Move pending approval tool calls to completed tool calls
		if (lastAssistantMessage?.type === 'claude') {
			lastAssistantMessage.toolCalls = [
				...lastAssistantMessage.toolCalls,
				...lastAssistantMessage.toolCallsPendingApproval
			];
			lastAssistantMessage.toolCallsPendingApproval = [];
		}
	}

	return out;
}

function previousEventLoginFailureQuery(events: ClaudeMessage[], event: ClaudeMessage): boolean {
	const idx = events.findIndex((e) => e === event);
	if (idx <= 0) return false;
	const previous = events[idx - 1]!;
	if (previous.content.type !== 'claudeOutput') return false;
	if (previous.content.subject.type !== 'result') return false;
	if (previous.content.subject.subtype !== 'success') return false;
	if (previous.content.subject.result !== loginRequiredMessage) return false;
	return true;
}

type UserFeedbackStatus =
	| {
			waitingForFeedback: true;
			// The first tool call requiring feedback.
			toolCall: ToolCall;
	  }
	| {
			waitingForFeedback: false;
			msSpentWaiting: number;
	  };

export function userFeedbackStatus(messages: Message[]): UserFeedbackStatus {
	const lastMessage = messages.at(-1);
	if (!lastMessage || lastMessage.type === 'user') {
		return { waitingForFeedback: false, msSpentWaiting: 0 };
	}
	if (lastMessage.toolCallsPendingApproval.length > 0) {
		return { waitingForFeedback: true, toolCall: lastMessage.toolCallsPendingApproval[0]! };
	}
	let msSpentWaiting = 0;
	for (const tc of lastMessage.toolCalls) {
		if (tc.approvedAt) {
			msSpentWaiting += tc.approvedAt.getTime() - tc.requestAt.getTime();
		}
	}
	return { waitingForFeedback: false, msSpentWaiting };
}

/** Anthropic prices, per 1M tokens */
const pricing = [
	{
		name: 'opus',
		input: 15,
		output: 75,
		writeCache: 18.75,
		readCache: 1.5,
		context: 200_000
	},
	// Ordering the 1m model before the 200k model so it matches first.
	{
		name: 'sonnet',
		subtype: '[1m]',
		input: 6,
		output: 22.5,
		writeCache: 7.5,
		readCache: 0.6,
		context: 1_000_000
	},
	{
		name: 'sonnet',
		input: 3,
		output: 15,
		writeCache: 3.75,
		readCache: 0.3,
		context: 200_000
	},
	{
		name: 'haiku',
		input: 1,
		output: 5,
		writeCache: 1.25,
		readCache: 0.1,
		context: 200_000
	}
] as const;

/** Cost of anthropic making web request calls per 1K calls */
const webRequestCost = 10;

/**
 * Calculates the usage stats from the message log.
 *
 * This makes use of the "assistant" messages rather than the "result" ones
 * because the "assistant" ones come in more frequently.
 *
 * For some reason the final quantity of tokens ends up slightly greater than if
 * you were using the result. I'm not entirly sure where the discrepency is.
 */
export function usageStats(events: ClaudeMessage[]): {
	tokens: number;
	cost: number;
	/** Percentage (0 to 1) of how full the context is */
	contextUtilization: number;
} {
	let tokens = 0;
	let contextUtilization = 0;
	let lastAssistantMessage;

	for (let i = events.length - 1; i >= 0; i--) {
		const event = events[i]!;
		if (event.content.type !== 'claudeOutput') continue;
		const message = event.content.subject;
		if (message.type !== 'assistant') continue;
		lastAssistantMessage = message;
		break;
	}

	if (lastAssistantMessage) {
		const usage = lastAssistantMessage.message.usage;
		tokens += usage.cache_read_input_tokens ?? 0;
		tokens += usage.cache_creation_input_tokens ?? 0;
		tokens += usage.input_tokens;
		tokens += usage.output_tokens;
		const modelPricing = findModelPricing(lastAssistantMessage.message.model);
		if (modelPricing) {
			contextUtilization = tokens / modelPricing.context;
		}
	}

	let cost = 0;
	const usedIds = new Set();

	for (let i = events.length - 1; i >= 0; i--) {
		const event = events[i]!;
		if (event.content.type !== 'claudeOutput') continue;
		const message = event.content.subject;
		if (message.type !== 'assistant') continue;
		if (usedIds.has(message.message.id)) continue;
		usedIds.add(message.message.id);
		const modelPricing = findModelPricing(message.message.model);
		if (!modelPricing) continue;
		const usage = message.message.usage;

		cost += (usage.input_tokens * modelPricing.input) / 1_000_000;
		cost += (usage.output_tokens * modelPricing.output) / 1_000_000;
		cost += ((usage.cache_creation_input_tokens || 0) * modelPricing.writeCache) / 1_000_000;
		cost += ((usage.cache_read_input_tokens || 0) * modelPricing.readCache) / 1_000_000;
		cost += ((usage.server_tool_use?.web_search_requests || 0) * webRequestCost) / 1_000;
	}

	return { tokens, cost, contextUtilization };
}

function findModelPricing(name: string) {
	for (const p of pricing) {
		// We do a starts with so we don't have to deal with all the versioning
		if (name.includes(p.name) && ('subtype' in p ? name.includes(p.subtype) : true)) {
			return p;
		}
	}
}

/**
 * Based on the current event log, determine the current status
 */
export function currentStatus(events: ClaudeMessage[], isActive: boolean): ClaudeStatus {
	if (events.length === 0) return 'disabled';
	const lastEvent = events.at(-1)!;
	if (lastEvent.content.type === 'claudeOutput' && lastEvent.content.subject.type === 'result') {
		// Once we have the TODOs, if all the TODOs are completed, we can change
		// this to conditionally return 'enabled' or 'completed'
		return 'enabled';
	}

	if (
		lastEvent.content.type === 'gitButlerMessage' &&
		lastEvent.content.subject.type === 'compactStart'
	) {
		return 'compacting';
	}

	if (
		lastEvent.content.type === 'gitButlerMessage' &&
		(lastEvent.content.subject.type === 'userAbort' ||
			lastEvent.content.subject.type === 'claudeExit' ||
			lastEvent.content.subject.type === 'unhandledException' ||
			lastEvent.content.subject.type === 'compactFinished')
	) {
		// Once we have the TODOs, if all the TODOs are completed, we can change
		// this to conditionally return 'enabled' or 'completed'
		return 'enabled';
	}
	// If the stack is not active, consider it no longer running
	if (!isActive) {
		return 'enabled';
	}
	return 'running';
}

export type CompletedStatus =
	| { type: 'notCompleted' }
	| { type: 'noMessagesSent' }
	| { type: 'completed'; code: number };

export function isCompletedWithStatus(events: ClaudeMessage[], isActive: boolean): CompletedStatus {
	if (events.length === 0) return { type: 'noMessagesSent' };
	const status = currentStatus(events, isActive);
	if (status === 'enabled') {
		// The last event after 'completed' is _usually_ "claudeExit", but not
		// always. If it is, we use it, or just assume success.
		const lastEvent = events.at(-1)!;
		if (
			lastEvent.content.type === 'gitButlerMessage' &&
			lastEvent.content.subject.type === 'claudeExit'
		) {
			return { type: 'completed', code: lastEvent.content.subject.subject.code };
		} else {
			return { type: 'completed', code: 0 };
		}
	} else {
		return { type: 'notCompleted' };
	}
}

function normalizeDate(date: Date): Date {
	return new Date(date.getTime() - date.getTimezoneOffset() * 60 * 1000);
}

/**
 * Thinking duration in ms
 */
export function thinkingOrCompactingStartedAt(events: ClaudeMessage[]): Date | undefined {
	let event: ClaudeMessage | undefined;
	for (let i = events.length - 1; i >= 0; --i) {
		const e = events[i]!;
		if (
			e.content.type === 'userInput' ||
			(e.content.type === 'gitButlerMessage' && e.content.subject.type === 'compactStart')
		) {
			event = e;
			break;
		}
	}
	if (!event) return;
	return normalizeDate(new Date(event.createdAt));
}

/**
 * Gets the timestamp of the last interaction (any message) in the chat
 */
export function lastInteractionTime(events: ClaudeMessage[]): Date | undefined {
	if (events.length === 0) return undefined;
	const lastEvent = events[events.length - 1];
	if (!lastEvent) return;
	return normalizeDate(new Date(lastEvent.createdAt));
}

/**
 * Extracts the TODO list from the message history.
 *
 * The TODOs are written when the "TodoWrite" command is called which replaces
 * the entire TODO list each time it is called.
 */
export function getTodos(events: ClaudeMessage[]): ClaudeTodo[] {
	let todos: ClaudeTodo[] | undefined;
	for (let i = events.length - 1; i >= 0; --i) {
		const content = events[i]!.content;
		if (content.type !== 'claudeOutput') continue;
		const subject = content.subject;
		if (subject.type !== 'assistant') continue;
		const msgContent = subject.message.content[0]!;
		if (msgContent.type !== 'tool_use') continue;
		if (msgContent.name !== 'TodoWrite') continue;
		todos = (msgContent.input as { todos: ClaudeTodo[] }).todos;
		break;
	}
	return todos ?? [];
}
