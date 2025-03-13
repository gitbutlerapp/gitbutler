import {
	SHORT_DEFAULT_BRANCH_TEMPLATE,
	SHORT_DEFAULT_COMMIT_TEMPLATE,
	SHORT_DEFAULT_PR_TEMPLATE
} from '$lib/ai/prompts';
import OpenAI from 'openai';
import type { OpenAIModelName, Prompt, AIClient, AIEvalOptions } from '$lib/ai/types';

const DEFAULT_MAX_TOKENS = 1024;

export class OpenAIClient implements AIClient {
	defaultCommitTemplate = SHORT_DEFAULT_COMMIT_TEMPLATE;
	defaultBranchTemplate = SHORT_DEFAULT_BRANCH_TEMPLATE;
	defaultPRTemplate = SHORT_DEFAULT_PR_TEMPLATE;

	private client: OpenAI;
	private openAIKey: string;
	private modelName: OpenAIModelName;

	constructor(openAIKey: string, modelName: OpenAIModelName) {
		this.openAIKey = openAIKey;
		this.modelName = modelName;
		this.client = new OpenAI({ apiKey: openAIKey, dangerouslyAllowBrowser: true });
	}

	async evaluate(prompt: Prompt, options?: AIEvalOptions): Promise<string> {
		// The 'max_tokens' parameter has been renamed to 'max_completion_tokens' in the OpenAI API.
		// This change aligns with the updated API specification where 'max_completion_tokens'
		// specifically controls the maximum number of tokens for the completion portion of the response.
		// https://platform.openai.com/docs/api-reference/completions/create
		const response = await this.client.chat.completions.create({
			max_completion_tokens: options?.maxTokens ?? DEFAULT_MAX_TOKENS,
			messages: prompt,
			model: this.modelName,
			stream: true
		});

		const buffer: string[] = [];
		for await (const chunk of response) {
			const token = chunk.choices[0]?.delta.content ?? '';
			options?.onToken?.(token);
			buffer.push(token);
		}
		return buffer.join('');
	}
}
