import { MessageRole, type PromptMessage, type AIClient } from '$lib/ai/types';
import { isNonEmptyObject } from '$lib/utils/typeguards';

export const DEFAULT_OLLAMA_ENDPOINT = 'http://127.0.0.1:11434';
export const DEFAULT_OLLAMA_MODEL_NAME = 'llama3';

enum OllamaAPEndpoint {
	Generate = 'api/generate',
	Chat = 'api/chat',
	Embed = 'api/embeddings'
}
interface OllamaChatResponse {
	message: PromptMessage;
	created_at: string;
	done: boolean;
	model: string;
}

function isOllamaChatResponse(response: unknown): response is OllamaChatResponse {
	if (!isNonEmptyObject(response)) {
		return false;
	}

	return (
		isNonEmptyObject(response.message) &&
		typeof response.message.role == 'string' &&
		typeof response.message.content == 'string'
	);
}

export class OllamaClient implements AIClient {
	constructor(
		private endpoint: string,
		private modelName: string
	) {}
	async evaluate(prompt: string) {
		const messages: PromptMessage[] = [
			{
				role: MessageRole.User,
				content: prompt
			}
		];

		const url = new URL(OllamaAPEndpoint.Chat, this.endpoint);
		const body = {
			model: this.modelName,
			stream: false,
			messages
		};
		const result = await fetch(url, {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json'
			},
			body: JSON.stringify(body)
		});

		const json = await result.json();

		if (!isOllamaChatResponse(json)) {
			throw new Error(`Invalid response ${JSON.stringify(json)}`);
		}

		return json.message.content;
	}
}
