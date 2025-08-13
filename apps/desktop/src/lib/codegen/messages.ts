/**
 * This module is responsible for taking the messages from a near-incomprehesable mess into something _vaugly_ useful.
 */

import { isDefined } from '@gitbutler/ui/utils/typeguards';
import type { ClaudeMessage } from '$lib/codegen/types';

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
	  };

export type ToolCall = {
	name: string;
	id: string;
	input: object;
	result: string | undefined;
};

export function toolCallLoading(toolCall: ToolCall): boolean {
	return toolCall.result === undefined;
}

export function formatMessages(events: ClaudeMessage[]): Message[] {
	const out: Message[] = [];
	// A mapping to better handle tool call responses when they come in.
	const toolCalls: Record<string, ToolCall> = {};
	let lastAssistantMessage: Message | undefined = undefined;

	for (const event of events) {
		if (event.content.type === 'userInput') {
			out.push({
				type: 'user',
				message: event.content.subject.message
			});
		} else if (event.content.type === 'claudeOutput') {
			const subject = event.content.subject;
			// We've either triggered a tool call, or sent a message
			if (subject.type === 'assistant') {
				const message = subject.message;
				if (message.content[0]!.type === 'text') {
					lastAssistantMessage = {
						type: 'claude',
						message: message.content[0]!.text,
						toolCalls: []
					};
					out.push(lastAssistantMessage);
				} else if (message.content[0]!.type === 'tool_use') {
					const content = message.content[0]!;
					const toolCall: ToolCall = {
						id: content.id,
						name: content.name,
						input: content.input as object,
						result: undefined
					};
					if (!lastAssistantMessage) {
						lastAssistantMessage = {
							type: 'claude',
							message: '',
							toolCalls: []
						};
						out.push(lastAssistantMessage);
					}
					lastAssistantMessage.toolCalls.push(toolCall);
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
		}
	}

	return out;
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
 * you were using the result, however, the calculated cost ends up being the
 * same as the cost provided in the result based messages.
 *
 * I can only assume that there is a mistake in the token counting code on CC's
 * side.
 */
export function usageStats(events: ClaudeMessage[]): { tokens: number; cost: number } {
	let tokens = 0;
	let cost = 0;
	for (const event of events) {
		if (event.content.type !== 'claudeOutput') continue;
		const message = event.content.subject;
		if (message.type !== 'assistant') continue;

		const usage = message.message.usage;
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
