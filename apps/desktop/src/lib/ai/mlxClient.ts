import {
	LONG_DEFAULT_BRANCH_TEMPLATE,
	LONG_DEFAULT_COMMIT_TEMPLATE,
	SHORT_DEFAULT_PR_TEMPLATE
} from '$lib/ai/prompts';
import { MessageRole, type PromptMessage, type AIClient, type Prompt } from '$lib/ai/types';
import { andThen, buildFailureFromAny, ok, wrap, wrapAsync, type Result } from '$lib/result';
import { isNonEmptyObject } from '@gitbutler/ui/utils/typeguards';
import { fetch } from '@tauri-apps/plugin-http';

export const DEFAULT_MLX_ENDPOINT = 'http://localhost:8080';
export const DEFAULT_MLX_MODEL_NAME = 'mlx-community/Llama-3.2-3B-Instruct-4bit';

enum MLXApiEndpoint {
    Chat = 'v1/chat/completions',
}

interface MLXRequestOptions {
    /**
     * The temperature of the model.
     * Increasing the temperature will make the model answer more creatively. (Default: 0.8)
     */
    temperature: number;
    repetition_penalty: number;
    top_p: number;
    max_tokens: number;
}

interface MLXChatRequest {
    model: string;
    stream: boolean;
    messages: Prompt;
	options?: MLXRequestOptions;
}

interface MLXChatResponse {
    choices: [MLXChatResponseChoice];    
}

interface MLXChatResponseChoice {
    message: PromptMessage;
}

interface MLXChatMessageFormat {
    result: string;
}

const MLX_CHAT_MESSAGE_FORMAT_SCHEMA = {
    type: 'object',
    properties: {
        result: { type: 'string' }
    },
    required: ['result'],
    additionalProperties: false
};

function isMLXChatMessageFormat(message: unknown): message is MLXChatMessageFormat {
    return isNonEmptyObject(message) && message.result !== undefined;
}

function isMLXChatResponse(response: MLXChatResponse): response is MLXChatResponse {
    if (!isNonEmptyObject(response)) {
		return false;
	}

    return response.choices.length > 0 && response.choices[0].message !== undefined;
}

export class MLXClient implements AIClient {
    defaultCommitTemplate = LONG_DEFAULT_COMMIT_TEMPLATE;
	defaultBranchTemplate = LONG_DEFAULT_BRANCH_TEMPLATE;
	defaultPRTemplate = SHORT_DEFAULT_PR_TEMPLATE;

    constructor(
		private endpoint: string,
		private modelName: string
	) {}

    async evaluate(prompt: Prompt): Promise<Result<string, Error>> {
        const messages = this.formatPrompt(prompt);
        
        const options = {
            temperature: 1.0,
            repetition_penalty: 1.5,
            top_p: 1.0,
            max_tokens: 512
        }
        const responseResult = await this.chat(messages, options);

        return andThen(responseResult, (response) => {
            const choice = response.choices[0];
            const rawResponseResult = wrap<unknown, Error>(() => JSON.parse(choice.message.content));

            return andThen(rawResponseResult, (rawResponse) => {
                if (!isMLXChatMessageFormat(rawResponse)) {
                    return buildFailureFromAny('Invalid response: ' + choice.message.content);
                }

                return ok(rawResponse.result);
            });
        });
    }

    private formatPrompt(prompt: Prompt): Prompt {
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
<json schema>
${JSON.stringify(MLX_CHAT_MESSAGE_FORMAT_SCHEMA.properties, null, 2)}
</json schema>
EXAMPLE:
<json>
{"result": "Your content here"}
</json>
Ensure that your response is valid JSON and adheres to the provided JSON schema.
`

            },
            ...withFormattedResponses
        ];
    }

    private async fetchChat(request: MLXChatRequest): Promise<Result<any, Error>> {
        const url = new URL(MLXApiEndpoint.Chat, this.endpoint);
        const body = JSON.stringify(request);
        return await wrapAsync(
            async () =>
                await fetch(url.toString(), {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json'
                    },
                    body
                }).then(async (response) => await response.json())
        );
    }

    private async chat(
            messages: Prompt,
            options?: MLXRequestOptions
    ): Promise<Result<MLXChatResponse, Error>> {
        const result = await this.fetchChat({
            model: this.modelName,
            stream: false,
            messages,
            options,
        });

        return andThen(result, (result) => {
            if (!isMLXChatResponse(result)) {
                return buildFailureFromAny('Invalid response\n' + JSON.stringify(result.data));
            }

            return ok(result);
        });
    }
}
