import { splitPromptMessages } from '$lib/ai/anthropicUtils';
import {
	SHORT_DEFAULT_COMMIT_TEMPLATE,
	SHORT_DEFAULT_BRANCH_TEMPLATE,
	SHORT_DEFAULT_PR_TEMPLATE
} from '$lib/ai/prompts';
import { type AIEvalOptions } from '$lib/ai/types';
import { type AIClient, type AnthropicModelName, type Prompt } from '$lib/ai/types';
import Anthropic from '@anthropic-ai/sdk';

const DEFAULT_MAX_TOKENS = 1024;

export class AnthropicAIClient implements AIClient {
	defaultCommitTemplate = SHORT_DEFAULT_COMMIT_TEMPLATE;
	defaultBranchTemplate = SHORT_DEFAULT_BRANCH_TEMPLATE;
	defaultPRTemplate = SHORT_DEFAULT_PR_TEMPLATE;

	private client: Anthropic;
	private apiKey: string;
	private modelName: AnthropicModelName;

	constructor(apiKey: string, modelName: AnthropicModelName) {
		this.apiKey = apiKey;
		this.modelName = modelName;
		this.client = new Anthropic({
			apiKey: this.apiKey,
			dangerouslyAllowBrowser: true
		});
	}

	async evaluate(prompt: Prompt, options?: AIEvalOptions): Promise<string> {
		const [messages, system] = splitPromptMessages(prompt);
		const response = await this.client.messages.create({
			max_tokens: options?.maxTokens ?? DEFAULT_MAX_TOKENS,
			system,
			messages,
			model: this.modelName,
			stream: true
		});

		const buffer: string[] = [];
		for await (const event of response) {
			if (event.type === 'content_block_delta' && event.delta.type === 'text_delta') {
				const token = event.delta.text;
				options?.onToken?.(token);
				buffer.push(token);
			}
		}
		return buffer.join('');
	}
}
