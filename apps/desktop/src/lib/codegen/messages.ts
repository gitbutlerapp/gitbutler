/**
 * This module is responsible for taking the messages from a near-incomprehesable mess into something _vaugly_ useful.
 */

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
						return [];
					} else if (!result.content) {
						foundToolCall.result = undefined;
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
