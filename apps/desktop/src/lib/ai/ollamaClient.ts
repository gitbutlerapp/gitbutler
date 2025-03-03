import {
	LONG_DEFAULT_BRANCH_TEMPLATE,
	LONG_DEFAULT_COMMIT_TEMPLATE,
	SHORT_DEFAULT_PR_TEMPLATE
} from '$lib/ai/prompts';
import { MessageRole, type PromptMessage, type AIClient, type Prompt } from '$lib/ai/types';
import { isNonEmptyObject } from '@gitbutler/ui/utils/typeguards';
import { fetch } from '@tauri-apps/plugin-http';

export const DEFAULT_OLLAMA_ENDPOINT = 'http://127.0.0.1:11434';
export const DEFAULT_OLLAMA_MODEL_NAME = 'llama3';

enum OllamaAPEndpoint {
	Generate = 'api/generate',
	Chat = 'api/chat',
	Embed = 'api/embeddings'
}

interface OllamaRequestOptions {
	/**
	 * The temperature of the model.
	 * Increasing the temperature will make the model answer more creatively. (Default: 0.8)
	 */
	temperature: number;
}

interface OllamaChatRequest {
	model: string;
	messages: Prompt;
	stream: boolean;
	format?: 'json';
	options?: OllamaRequestOptions;
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

interface OllamaChatMessageFormat {
	result: string;
}

const OLLAMA_CHAT_MESSAGE_FORMAT_SCHEMA = {
	type: 'object',
	properties: {
		result: { type: 'string' }
	},
	required: ['result'],
	additionalProperties: false
};

function isOllamaChatMessageFormat(message: unknown): message is OllamaChatMessageFormat {
	if (!isNonEmptyObject(message)) {
		return false;
	}

	return typeof message.result === 'string';
}

function isOllamaChatResponse(response: unknown): response is OllamaChatResponse {
	if (!isNonEmptyObject(response)) {
		return false;
	}

	return (
		isNonEmptyObject(response.message) &&
		typeof response.message.role === 'string' &&
		typeof response.message.content === 'string'
	);
}

export class OllamaClient implements AIClient {
	defaultCommitTemplate = LONG_DEFAULT_COMMIT_TEMPLATE;
	defaultBranchTemplate = LONG_DEFAULT_BRANCH_TEMPLATE;
	defaultPRTemplate = SHORT_DEFAULT_PR_TEMPLATE;

	constructor(
		private endpoint: string,
		private modelName: string
	) {}

	async evaluate(prompt: Prompt): Promise<string> {
		const messages = this.formatPrompt(prompt);

		const response = await this.chat(messages);

		const rawResponse = JSON.parse(response.message.content);

		if (!isOllamaChatMessageFormat(rawResponse)) {
			throw new Error('Invalid response: ' + response.message.content);
		}

		return rawResponse.result;
	}

	/**
	 * Appends a system message which instructs the model to respond using a particular JSON schema
	 * Modifies the prompt's Assistant messages to make use of the correct schema
	 */
	private formatPrompt(prompt: Prompt) {
		const withFormattedResponses = prompt.map((promptMessage) => {
			if (promptMessage.role === MessageRole.Assistant) {
				return {
					role: MessageRole.Assistant,
					content: JSON.stringify({ result: promptMessage.content })
				};
			} else {
				return promptMessage;
			}
		});

		return [
			{
				role: MessageRole.System,
				content: `You are an expert in software development. Answer the given user prompts following the specified instructions.
Return your response in JSON and only use the following JSON schema:
${JSON.stringify(OLLAMA_CHAT_MESSAGE_FORMAT_SCHEMA, null, 2)}`
			},
			...withFormattedResponses
		];
	}

	/**
	 * Fetches the chat using the specified request.
	 * @param request - The OllamaChatRequest object containing the request details.
	 * @returns A Promise that resolves to the Response object.
	 */
	private async fetchChat(request: OllamaChatRequest): Promise<unknown> {
		console.log(request);
		const url = new URL(OllamaAPEndpoint.Chat, this.endpoint);
		console.log(url);
		const body = JSON.stringify(request);
		console.log(body);

		const response = await fetch(url.toString(), {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json'
			},
			body
		});
		console.log(response);

		return await response.json();
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
		options?: OllamaRequestOptions
	): Promise<OllamaChatResponse> {
		const result = await this.fetchChat({
			model: this.modelName,
			stream: false,
			messages,
			options,
			format: 'json'
		});
		console.log(result);

		if (!isOllamaChatResponse(result)) {
			throw new Error('Invalid response\n' + JSON.stringify(result));
		}

		return result;
	}
}
