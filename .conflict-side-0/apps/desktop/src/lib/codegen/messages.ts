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

	for (const event of events) {
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
				subject.type === 'unhandledException'
			) {
				wrapUpAgentSide();
			}

			if (subject.type === 'claudeExit' && subject.subject.code !== 0) {
				if (previousEventLoginFailureResult(events, event)) {
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

function previousEventLoginFailureResult(events: ClaudeMessage[], event: ClaudeMessage): boolean {
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
		name: 'claude-opus',
		input: 15,
		output: 75,
		writeCache: 18.75,
		readCache: 1.5
	},
	{
		name: 'claude-sonnet',
		input: 3,
		output: 6,
		writeCache: 3.75,
		readCache: 0.3
	},
	{
		name: 'claude-haiku',
		input: 0.8,
		output: 4,
		writeCache: 1,
		readCache: 0.08
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
} {
	let tokens = 0;
	let cost = 0;
	for (const event of events) {
		if (event.content.type !== 'claudeOutput') continue;
		const message = event.content.subject;
		if (message.type !== 'assistant') continue;
		const usage = message.message.usage;
		if (message.message.stop_reason === 'tool_use') continue;
		tokens += usage.input_tokens;
		tokens += usage.output_tokens;

		const modelPricing = findModelPricing(message.message.model);
		if (!modelPricing) continue;

		cost += (usage.input_tokens * modelPricing.input) / 1_000_000;
		cost += (usage.output_tokens * modelPricing.output) / 1_000_000;
		cost += ((usage.cache_creation_input_tokens || 0) * modelPricing.writeCache) / 1_000_000;
		cost += ((usage.cache_read_input_tokens || 0) * modelPricing.readCache) / 1_000_000;
		cost += ((usage.server_tool_use?.web_search_requests || 0) * webRequestCost) / 1_000;
	}

	return { tokens, cost };
}

function findModelPricing(name: string) {
	for (const p of pricing) {
		// We do a starts with so we don't have to deal with all the versioning
		if (name.startsWith(p.name)) {
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
		(lastEvent.content.subject.type === 'userAbort' ||
			lastEvent.content.subject.type === 'claudeExit' ||
			lastEvent.content.subject.type === 'unhandledException')
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

function normalizeDate(date: Date): Date {
	return new Date(date.getTime() - date.getTimezoneOffset() * 60 * 1000);
}

/**
 * Thinking duration in ms
 */
export function lastUserMessageSentAt(events: ClaudeMessage[]): Date | undefined {
	let event: ClaudeMessage | undefined;
	for (let i = events.length - 1; i >= 0; --i) {
		if (events[i]!.content.type === 'userInput') {
			event = events[i]!;
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
