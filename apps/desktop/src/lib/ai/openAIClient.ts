import {
	SHORT_DEFAULT_BRANCH_TEMPLATE,
	SHORT_DEFAULT_COMMIT_TEMPLATE,
	SHORT_DEFAULT_PR_TEMPLATE
} from '$lib/ai/prompts';
import { andThenAsync, ok, wrapAsync, type Result } from '$lib/ai/result';
import OpenAI from 'openai';
import type { OpenAIModelName, Prompt, AIClient, AIEvalOptions } from '$lib/ai/types';
import type { ChatCompletionChunk } from 'openai/resources/index.mjs';
import type { Stream } from 'openai/streaming.mjs';

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

	async evaluate(prompt: Prompt, options?: AIEvalOptions): Promise<Result<string, Error>> {
		const responseResult = await wrapAsync<Stream<ChatCompletionChunk>, Error>(async () => {
			return await this.client.chat.completions.create({
				max_tokens: options?.maxTokens ?? DEFAULT_MAX_TOKENS,
				messages: prompt,
				model: this.modelName,
				stream: true
			});
		});

		return await andThenAsync(responseResult, async (response) => {
			const buffer: string[] = [];
			for await (const chunk of response) {
				const token = chunk.choices[0]?.delta.content ?? '';
				options?.onToken?.(token);
				buffer.push(token);
			}
			return ok(buffer.join(''));
		});
	}
}
