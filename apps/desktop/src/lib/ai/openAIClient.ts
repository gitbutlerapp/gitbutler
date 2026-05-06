import {
	SHORT_DEFAULT_BRANCH_TEMPLATE,
	SHORT_DEFAULT_COMMIT_TEMPLATE,
	SHORT_DEFAULT_PR_TEMPLATE,
} from "$lib/ai/prompts";
import OpenAI from "openai";
import type { ChatCompletionCreateParamsStreaming } from "openai/resources/chat/completions";
import type {
	OpenAIModelName,
	OpenRouterModelName,
	Prompt,
	AIClient,
	AIEvalOptions,
} from "$lib/ai/types";

const DEFAULT_MAX_TOKENS = 1024;
type MoonshotChatCompletionCreateParamsStreaming = ChatCompletionCreateParamsStreaming & {
	thinking?: { type: "disabled" };
};

type MoonshotChatCompletionResponse = {
	choices?: Array<{
		message?: {
			content?: string | null;
		};
	}>;
};

export class OpenAIClient implements AIClient {
	defaultCommitTemplate = SHORT_DEFAULT_COMMIT_TEMPLATE;
	defaultBranchTemplate = SHORT_DEFAULT_BRANCH_TEMPLATE;
	defaultPRTemplate = SHORT_DEFAULT_PR_TEMPLATE;

	private client: OpenAI;
	private openAIKey: string;
	private modelName: OpenAIModelName | OpenRouterModelName;
	private baseURL: string | undefined;

	constructor(
		openAIKey: string,
		modelName: OpenAIModelName | OpenRouterModelName,
		baseURL: string | undefined,
	) {
		this.openAIKey = openAIKey;
		this.modelName = modelName;
		this.baseURL = baseURL;
		this.client = new OpenAI({ apiKey: openAIKey, dangerouslyAllowBrowser: true, baseURL });
	}

	async evaluate(prompt: Prompt, options?: AIEvalOptions): Promise<string> {
		const usesMoonshot = this.usesMoonshot();
		if (usesMoonshot) {
			return await this.evaluateMoonshot(prompt, options);
		}

		const tokenLimit = options?.maxTokens ?? DEFAULT_MAX_TOKENS;
		const request: MoonshotChatCompletionCreateParamsStreaming = {
			max_completion_tokens: tokenLimit,
			messages: prompt,
			model: this.modelName,
			stream: true,
		};

		const response = await this.client.chat.completions.create(request);

		const buffer: string[] = [];
		for await (const chunk of response) {
			const token = chunk.choices[0]?.delta.content ?? "";
			options?.onToken?.(token);
			buffer.push(token);
		}
		return buffer.join("");
	}

	private usesMoonshot() {
		return this.baseURL?.includes("moonshot.ai") || this.modelName.startsWith("kimi-");
	}

	private async evaluateMoonshot(prompt: Prompt, options?: AIEvalOptions): Promise<string> {
		const apiBase = (this.baseURL || "https://api.moonshot.ai/v1").replace(/\/+$/, "");
		const response = await fetch(`${apiBase}/chat/completions`, {
			method: "POST",
			headers: {
				Authorization: `Bearer ${this.openAIKey}`,
				"Content-Type": "application/json",
			},
			body: JSON.stringify({
				max_tokens: options?.maxTokens ?? DEFAULT_MAX_TOKENS,
				messages: prompt,
				model: this.modelName,
				stream: false,
				// Kimi defaults to thinking mode. For short commit and branch summaries,
				// disable it so the response arrives as normal content.
				thinking: { type: "disabled" },
			}),
		});

		if (!response.ok) {
			const errorText = await response.text();
			throw new Error(
				`Moonshot API request failed with HTTP ${response.status}: ${errorText.slice(0, 500)}`,
			);
		}

		const data = (await response.json()) as MoonshotChatCompletionResponse;
		const message = data.choices?.[0]?.message?.content ?? "";
		options?.onToken?.(message);
		return message;
	}
}
