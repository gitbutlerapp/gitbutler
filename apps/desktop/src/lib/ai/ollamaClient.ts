import {
	LONG_DEFAULT_BRANCH_TEMPLATE,
	LONG_DEFAULT_COMMIT_TEMPLATE,
	SHORT_DEFAULT_PR_TEMPLATE,
} from "$lib/ai/prompts";
import { type PromptMessage, type AIClient, type Prompt } from "$lib/ai/types";
import { isNonEmptyObject } from "@gitbutler/ui/utils/typeguards";
import { Ollama } from "ollama/browser";

export const DEFAULT_OLLAMA_ENDPOINT = "http://127.0.0.1:11434";
export const DEFAULT_OLLAMA_MODEL_NAME = "llama3";

interface OllamaRequestOptions {
	/**
	 * The temperature of the model.
	 * Increasing the temperature will make the model answer more creatively. (Default: 0.8)
	 */
	temperature: number;
}

interface BaseOllamaMResponse {
	created_at: string;
	done: boolean;
	model: string;
}

interface OllamaChatResponse extends BaseOllamaMResponse {
	message: PromptMessage;
	done: true;
}

function isOllamaChatResponse(response: unknown): response is OllamaChatResponse {
	if (!isNonEmptyObject(response)) {
		return false;
	}

	return (
		isNonEmptyObject(response.message) &&
		typeof response.message.role === "string" &&
		typeof response.message.content === "string"
	);
}

export class OllamaClient implements AIClient {
	defaultCommitTemplate = LONG_DEFAULT_COMMIT_TEMPLATE;
	defaultBranchTemplate = LONG_DEFAULT_BRANCH_TEMPLATE;
	defaultPRTemplate = SHORT_DEFAULT_PR_TEMPLATE;

	readonly ollama: Ollama;

	constructor(
		private endpoint: string,
		private modelName: string,
	) {
		this.ollama = new Ollama({
			host: this.endpoint,
		});
	}

	async evaluate(prompt: Prompt): Promise<string> {
		const response = await this.chat(prompt);
		return response.message.content;
	}

	/**
	 * Sends a chat message to the LLM model and returns the response.
	 *
	 * @param messages - An array of LLMChatMessage objects representing the chat messages.
	 * @param options - Optional LLMRequestOptions object for specifying additional options.
	 * @returns A Promise that resolves to an LLMResponse object representing the response from the LLM model.
	 */
	private async chat(
		messages: Prompt,
		options?: OllamaRequestOptions,
	): Promise<OllamaChatResponse> {
		const result = await this.ollama.chat({
			model: this.modelName,
			messages,
			stream: false,
			options,
		});

		if (!isOllamaChatResponse(result)) {
			throw new Error("Invalid response: " + JSON.stringify(result));
		}

		return result;
	}
}
