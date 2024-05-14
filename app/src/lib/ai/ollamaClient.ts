import { MessageRole, type PromptMessage, type AIClient } from '$lib/ai/types';
import { isNonEmptyObject } from '$lib/utils/typeguards';

export const DEFAULT_OLLAMA_ENDPOINT = 'http://127.0.0.1:11434';
export const DEFAULT_OLLAMA_MODEL_NAME = 'llama3';

const PROMT_EXAMPLE_COMMIT_MESSAGE_GENERATION = `Please could you write a commit message for my changes.
Explain what were the changes and why the changes were done.
Focus the most important changes.
Use the present tense.
Use a semantic commit prefix.
Hard wrap lines at 72 characters.
Ensure the title is only 50 characters.
Do not start any lines with the hash symbol.
Only respond with the commit message.

Here is my git diff:
diff --git a/src/utils/typing.ts b/src/utils/typing.ts
index 1cbfaa2..7aeebcf 100644
--- a/src/utils/typing.ts
+++ b/src/utils/typing.ts
@@ -35,3 +35,10 @@ export function isNonEmptyObject(something: unknown): something is UnknownObject
     (Object.keys(something).length > 0 || Object.getOwnPropertySymbols(something).length > 0)
   );
 }
+
+export function isArrayOf<T>(
+  something: unknown,
+  check: (value: unknown) => value is T
+): something is T[] {
+  return Array.isArray(something) && something.every(check);
+}`;

const PROMPT_EXAMPLE_BRANCH_NAME_GENERATION = `Please could you write a branch name for my changes.
A branch name represent a brief description of the changes in the diff (branch).
Branch names should contain no whitespace and instead use dashes to separate words.
Branch names should contain a maximum of 5 words.
Only respond with the branch name.

Here is my git diff:
diff --git a/src/utils/typing.ts b/src/utils/typing.ts
index 1cbfaa2..7aeebcf 100644
--- a/src/utils/typing.ts
+++ b/src/utils/typing.ts
@@ -35,3 +35,10 @@ export function isNonEmptyObject(something: unknown): something is UnknownObject
     (Object.keys(something).length > 0 || Object.getOwnPropertySymbols(something).length > 0)
   );
 }
+
+export function isArrayOf<T>(
+  something: unknown,
+  check: (value: unknown) => value is T
+): something is T[] {
+  return Array.isArray(something) && something.every(check);
+}`;

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
	messages: PromptMessage[];
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

const OllamaChatMessageFormatSchema = {
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
		typeof response.message.role == 'string' &&
		typeof response.message.content == 'string'
	);
}

export class OllamaClient implements AIClient {
	constructor(
		private endpoint: string,
		private modelName: string
	) {}

	async branchName(prompt: string): Promise<string> {
		const messages: PromptMessage[] = [
			{
				role: MessageRole.System,
				content: `You are an expert in software development. Answer the given user prompts following the specified instructions.
Return your response in JSON and only use the following JSON schema:
${JSON.stringify(OllamaChatMessageFormatSchema, null, 2)}`
			},
			{
				role: MessageRole.User,
				content: PROMPT_EXAMPLE_BRANCH_NAME_GENERATION
			},
			{
				role: MessageRole.Assistant,
				content: JSON.stringify({
					result: `utils-typing-is-array-of-type`
				})
			},
			{ role: MessageRole.User, content: prompt }
		];

		const response = await this.chat(messages);
		const rawResponse = JSON.parse(response.message.content);
		if (!isOllamaChatMessageFormat(rawResponse)) {
			throw new Error('Invalid response: ' + response.message.content);
		}

		return rawResponse.result;
	}

	async evaluate(prompt: string) {
		const messages: PromptMessage[] = [
			{
				role: MessageRole.System,
				content: `You are an expert in software development. Answer the given user prompts following the specified instructions.
Return your response in JSON and only use the following JSON schema:
${JSON.stringify(OllamaChatMessageFormatSchema, null, 2)}`
			},
			{
				role: MessageRole.User,
				content: PROMT_EXAMPLE_COMMIT_MESSAGE_GENERATION
			},
			{
				role: MessageRole.Assistant,
				content: JSON.stringify({
					result: `Typing utilities: Check for array of type

Added an utility function to check whether a given value is an array of a specific type.`
				})
			},
			{ role: MessageRole.User, content: prompt }
		];

		const response = await this.chat(messages);
		const rawResponse = JSON.parse(response.message.content);
		if (!isOllamaChatMessageFormat(rawResponse)) {
			throw new Error('Invalid response: ' + response.message.content);
		}

		return rawResponse.result;
	}

	/**
	 * Fetches the chat using the specified request.
	 * @param request - The OllamaChatRequest object containing the request details.
	 * @returns A Promise that resolves to the Response object.
	 */
	private async fetchChat(request: OllamaChatRequest): Promise<Response> {
		const url = new URL(OllamaAPEndpoint.Chat, this.endpoint);
		const result = await fetch(url, {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json'
			},
			body: JSON.stringify(request)
		});
		return result;
	}

	/**
	 * Sends a chat message to the LLM model and returns the response.
	 *
	 * @param messages - An array of LLMChatMessage objects representing the chat messages.
	 * @param options - Optional LLMRequestOptions object for specifying additional options.
	 * @throws Error if the response is invalid.
	 * @returns A Promise that resolves to an LLMResponse object representing the response from the LLM model.
	 */
	private async chat(
		messages: PromptMessage[],
		options?: OllamaRequestOptions
	): Promise<OllamaChatResponse> {
		const result = await this.fetchChat({
			model: this.modelName,
			stream: false,
			messages,
			options,
			format: 'json'
		});

		const json = await result.json();
		if (!isOllamaChatResponse(json)) {
			throw new Error('Invalid response\n' + JSON.stringify(json));
		}

		return json;
	}
}
