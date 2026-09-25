import {
	SHORT_DEFAULT_BRANCH_TEMPLATE,
	SHORT_DEFAULT_COMMIT_TEMPLATE,
	SHORT_DEFAULT_PR_TEMPLATE,
} from "$lib/ai/prompts";
import OpenAI from "openai";
import type { Prompt, AIClient, AIEvalOptions } from "$lib/ai/types";

const DEFAULT_MAX_TOKENS = 1024;

export type OpenAIModelDiscoveryErrorKind =
	| "authentication"
	| "unsupported"
	| "invalid-response"
	| "unavailable";

export class OpenAIModelDiscoveryError extends Error {
	constructor(readonly kind: OpenAIModelDiscoveryErrorKind) {
		super(`Model discovery failed: ${kind}`);
		this.name = "OpenAIModelDiscoveryError";
	}
}

function createOpenAIClient(openAIKey: string, baseURL: string | undefined) {
	return new OpenAI({ apiKey: openAIKey, dangerouslyAllowBrowser: true, baseURL });
}

function parseOpenAIModelIds(response: unknown): string[] {
	if (!response || typeof response !== "object") {
		throw new OpenAIModelDiscoveryError("invalid-response");
	}

	const { data } = response as { data?: unknown };
	if (!Array.isArray(data)) {
		throw new OpenAIModelDiscoveryError("invalid-response");
	}

	const modelIds = data.map((model) => {
		if (!model || typeof model !== "object") {
			throw new OpenAIModelDiscoveryError("invalid-response");
		}

		const { id } = model as { id?: unknown };
		if (typeof id !== "string" || !id.trim()) {
			throw new OpenAIModelDiscoveryError("invalid-response");
		}

		return id.trim();
	});

	return [...new Set(modelIds)].sort((a, b) => a.localeCompare(b));
}

function classifyOpenAIModelDiscoveryError(error: unknown): OpenAIModelDiscoveryError {
	if (error instanceof OpenAIModelDiscoveryError) return error;

	const status =
		error && typeof error === "object" && "status" in error && typeof error.status === "number"
			? error.status
			: undefined;
	if (status === 401 || status === 403) {
		return new OpenAIModelDiscoveryError("authentication");
	}
	if (status === 404 || status === 405 || status === 501) {
		return new OpenAIModelDiscoveryError("unsupported");
	}
	return new OpenAIModelDiscoveryError("unavailable");
}

export async function listOpenAIModels(openAIKey: string, baseURL: string): Promise<string[]> {
	if (!openAIKey.trim() || !baseURL.trim()) {
		throw new OpenAIModelDiscoveryError("unavailable");
	}

	const client = createOpenAIClient(openAIKey.trim(), baseURL.trim());
	try {
		const response: unknown = await client.models.list();
		return parseOpenAIModelIds(response);
	} catch (error) {
		throw classifyOpenAIModelDiscoveryError(error);
	}
}

export class OpenAIClient implements AIClient {
	defaultCommitTemplate = SHORT_DEFAULT_COMMIT_TEMPLATE;
	defaultBranchTemplate = SHORT_DEFAULT_BRANCH_TEMPLATE;
	defaultPRTemplate = SHORT_DEFAULT_PR_TEMPLATE;

	private client: OpenAI;
	private openAIKey: string;
	private modelName: string;

	constructor(openAIKey: string, modelName: string, baseURL: string | undefined) {
		this.openAIKey = openAIKey;
		this.modelName = modelName;
		this.client = createOpenAIClient(openAIKey, baseURL);
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
			stream: true,
		});

		const buffer: string[] = [];
		for await (const chunk of response) {
			const token = chunk.choices[0]?.delta.content ?? "";
			options?.onToken?.(token);
			buffer.push(token);
		}
		return buffer.join("");
	}
}
