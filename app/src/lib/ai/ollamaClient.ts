import { LONG_DEFAULT_BRANCH_TEMPLATE, LONG_DEFAULT_COMMIT_TEMPLATE } from '$lib/ai/prompts';
import { MessageRole, type PromptMessage, type AIClient, type Prompt } from '$lib/ai/types';
import { andThen, buildFailureFromAny, ok, wrap, wrapAsync, type Result } from '$lib/result';
import { isNonEmptyObject } from '$lib/utils/typeguards';
import { fetch, Body, Response } from '@tauri-apps/api/http';

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

	constructor(
		private endpoint: string,
		private modelName: string
	) {}

	async evaluate(prompt: Prompt): Promise<Result<string, Error>> {
		const messages = this.formatPrompt(prompt);

		const responseResult = await this.chat(messages);

		return andThen(responseResult, (response) => {
			const rawResponseResult = wrap<unknown, Error>(() => JSON.parse(response.message.content));

			return andThen(rawResponseResult, (rawResponse) => {
				if (!isOllamaChatMessageFormat(rawResponse)) {
					return buildFailureFromAny('Invalid response: ' + response.message.content);
				}

				return ok(rawResponse.result);
			});
		});
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
	private async fetchChat(request: OllamaChatRequest): Promise<Result<Response<any>, Error>> {
		const url = new URL(OllamaAPEndpoint.Chat, this.endpoint);
		const body = Body.json(request);
		return await wrapAsync(
			async () =>
				await fetch(url.toString(), {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json'
					},
					body
				})
		);
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
	): Promise<Result<OllamaChatResponse, Error>> {
		const result = await this.fetchChat({
			model: this.modelName,
			stream: false,
			messages,
			options,
			format: 'json'
		});

		return andThen(result, (result) => {
			if (!isOllamaChatResponse(result.data)) {
				return buildFailureFromAny('Invalid response\n' + JSON.stringify(result.data));
			}

			return ok(result.data);
		});
	}
}
