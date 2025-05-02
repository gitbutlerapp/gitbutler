import { AIService } from '$lib/ai/service';
import { MessageRole, type PromptMessage } from '$lib/ai/types';
import { invoke } from '$lib/backend/ipc';
import { get, writable, type Writable } from 'svelte/store';
import type { Action, Dependencies } from '$lib/agent/types';

type AgentMessage = {
	role: 'system' | 'user' | 'assistant' | 'tool-response';
	content: string;
	display?: string;
};

function toPromptMessage(message: AgentMessage): PromptMessage {
	if (message.role === 'tool-response') {
		return {
			role: MessageRole.Assistant,
			content: `Tool response:\n\n${message.content}`
		};
	}

	return {
		role: message.role as MessageRole,
		content: message.content
	};
}

const defaultActions: Action[] = [
	{
		name: 'FINISH',
		description:
			'Use this action when you have finished responding or if you need the user to give more information',
		requiredArgs: [],
		execute: async (_args, _dependencies) => {
			return {
				status: 'stop-execution'
			};
		}
	},
	{
		name: 'DEBUG LOG',
		description: 'Use this action to log a message to the console',
		requiredArgs: ['message'],
		execute: async (args, _dependencies) => {
			// eslint-disable-next-line no-console
			console.log(args.message);
			return {
				status: 'success',
				message: 'Echo successfull'
			};
		}
	},
	{
		name: 'READ FILE',
		description:
			'This action can be used to read the contents of a file inside the project. The provided `path` argument is relative to the project root.',
		requiredArgs: ['path'],
		execute: async (args, { projectId }) => {
			try {
				const content = await invoke<string>('agent_read_file', {
					projectId,
					path: args.path
				});
				return {
					status: 'success',
					message: `Here is the content of the file:\n\n${content}`.trim()
				};
			} catch (error: unknown) {
				return {
					status: 'failure',
					message:
						`There was an error reading the file:\n\n${error && typeof error === 'object' && 'message' in error ? error.message : error}`.trim()
				};
			}
		}
	},
	{
		name: 'READ DIRECTORY',
		description:
			'This action can be used to read the contents of a directory inside the project. The provided `path` argument is relative to the project root.',
		requiredArgs: ['path'],
		execute: async (args, { projectId }) => {
			try {
				const content = await invoke<string[]>('agent_read_directory', {
					projectId,
					path: args.path
				});
				return {
					status: 'success',
					message: `Here is the content of the file:\n\n${content.join('\n')}`.trim()
				};
			} catch (error: unknown) {
				return {
					status: 'failure',
					message:
						`There was an error reading the file:\n\n${error && typeof error === 'object' && 'message' in error ? error.message : error}`.trim()
				};
			}
		}
	}
];

class Agent {
	readonly messages: Writable<AgentMessage[]> = writable([]);
	private state: 'waiting-for-user-input' | 'reasoning' | 'performing-task' =
		'waiting-for-user-input';
	private readonly dependencies: Dependencies;

	constructor(
		private readonly aiService: AIService,
		dependencies: Omit<Dependencies, 'aiService'>,
		private readonly actions: Action[] = defaultActions
	) {
		this.dependencies = {
			aiService,
			...dependencies
		};
		this.messages.set([formatSystemMessage(actions)]);
	}

	async userInput(input: string): Promise<AgentMessage[]> {
		if (this.state !== 'waiting-for-user-input') {
			throw new Error('Agent is not waiting for user input');
		}

		const userMessage = formatUserMessage(input, this.actions);

		this.messages.update((messages) => [...messages, userMessage]);

		const newMessages: AgentMessage[] = [];
		this.state = 'reasoning';

		while (this.state !== 'waiting-for-user-input') {
			if (this.state === 'reasoning') {
				const response = await this.aiService.chat({
					messages: get(this.messages).map(toPromptMessage)
				});

				if (response) {
					const message: AgentMessage = { role: 'assistant', content: response };
					newMessages.push(message);
					this.messages.update((messages) => [...messages, message]);
				} else {
					this.state = 'waiting-for-user-input';
					continue;
				}

				const action = await this.identifyAction(response);

				if (action) {
					this.state = 'performing-task';

					const result = await action.action.execute(action.arguments, this.dependencies);

					if (result.status === 'success' || result.status === 'failure') {
						const message: AgentMessage = {
							role: 'tool-response',
							content: result.message
						};
						newMessages.push(message);
						this.messages.update((messages) => [...messages, message]);

						this.state = 'reasoning';
					} else {
						this.state = 'waiting-for-user-input';
					}
				} else {
					this.state = 'waiting-for-user-input';
				}
			}
		}

		return newMessages;
	}

	private async identifyAction(
		response: string
	): Promise<{ arguments: Record<string, string>; action: Action } | undefined> {
		if (!response.includes('EXECUTE TOOL:')) {
			return undefined;
		}

		try {
			const actionString = response.split('EXECUTE TOOL:')[1];

			if (!actionString) {
				return undefined;
			}

			const parsedAction = JSON.parse(actionString) as {
				name: string;
				arguments: Record<string, string>;
			};

			if (!parsedAction.name || !parsedAction.arguments) {
				return undefined;
			}

			const action = this.actions.find((a) => a.name === parsedAction.name.toUpperCase());

			if (!action) {
				return undefined;
			}

			if (
				action.requiredArgs.length > 0 &&
				!action.requiredArgs.every((key) => key in parsedAction.arguments)
			) {
				return undefined;
			}

			return {
				arguments: parsedAction.arguments,
				action
			};
		} catch (_) {
			return undefined;
		}
	}
}

export class AgentFactory {
	constructor(
		private readonly aiService: AIService,
		private readonly actions: Action[] = defaultActions
	) {}

	createAgent(projectId: string): Agent {
		return new Agent(this.aiService, { projectId }, this.actions);
	}
}

function formatActions(actions: Action[]) {
	let output = `You are able to use various tools to help the user with their tasks.

In order to use a tool, say "EXECUTE TOOL:" at the end of your response, followed by the name of the tool and the arguments in JSON format.

For example:

EXECUTE TOOL:
{
    "name": "<action-name>",
    "arguments": {
        "<arg1-name>": "<arg1-value>",
        "<arg2-name>": "<arg2-value>"
    }
}

Here are the tools you can use:\n\n`;

	for (const action of actions) {
		output += `## Tool: ${action.name}
${action.description}
Required arguments: ${action.requiredArgs.join(', ')}\n\n`;
	}

	return output;
}

function formatUserMessage(message: string, actions: Action[]): AgentMessage {
	return {
		role: MessageRole.User,
		content: `Here is the user's next request:

${message}

Gather relevant information about the project and use the tools provided to you to help the user.

${formatActions(actions)}`,
		display: message
	};
}

function formatSystemMessage(actions: Action[]): PromptMessage {
	return {
		role: MessageRole.System,
		content: `You are a highly skilled software developer.
You are going to be helping your friend with their tasks.

You are going to be asked about or given tasks to complete on a particular project.
Learn about the project before responding to the user.

Follow the user's instructions carefully.
Provide short and concise responses.

${formatActions(actions)}`
	};
}
