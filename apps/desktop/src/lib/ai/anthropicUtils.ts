import { isMessageRole, type Prompt } from './types';
import { isStr } from '$lib/utils/string';
import type { MessageParam } from '@anthropic-ai/sdk/resources/messages.mjs';

export function splitPromptMessages(prompt: Prompt): [MessageParam[], string | undefined] {
	const messages: MessageParam[] = [];
	let system: string | undefined = undefined;
	for (const message of prompt) {
		if (message.role === 'system') {
			system = message.content;
			continue;
		}

		messages.push({
			role: message.role,
			content: message.content
		});
	}

	return [messages, system];
}

export function messageParamToPrompt(messages: MessageParam[]): Prompt {
	const result: Prompt = [];
	for (const message of messages) {
		if (!isStr(message.content)) continue;
		if (!isMessageRole(message.role)) continue;

		result.push({
			role: message.role,
			content: message.content
		});
	}
	return result;
}
