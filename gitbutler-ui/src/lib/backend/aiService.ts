import { AnthropicAIClient } from '$lib/backend/aiClients/anthropic';
import { ButlerAIClient } from '$lib/backend/aiClients/butler';
import { OpenAIClient } from '$lib/backend/aiClients/openAI';
import OpenAI from 'openai';
import { get, writable, type Writable } from 'svelte/store';
import type { User, getCloudApiClient } from './cloud';
import type { GitConfig } from './gitConfig';
import type { Observable } from 'rxjs';

const diffLengthLimit = 20000;

const defaultCommitTemplate = `
Please could you write a commit message for my changes.
Explain what were the changes and why the changes were done.
Focus the most important changes.
Use the present tense.
Always use semantic commit prefixes.
Hard wrap lines at 72 characters.
Only respond with the commit message.
%{brief_style}
%{emoji_style}

Here is my git diff:
%{diff}
`;

const defaultBranchTemplate = `
Please could you write a branch name for my changes.
A branch name represent a brief description of the changes in the diff (branch).
Branch names should contain no whitespace and instead use dashes to separate words.
Branch names should contain a maximum of 5 words.

Here is my git diff:
%{diff}
`;

export enum ModelKind {
	OpenAI = 'openai',
	Anthropic = 'anthropic'
}

export enum KeyOption {
	BringYourOwn = 'bringYourOwn',
	ButlerAPI = 'butlerAPI'
}

export enum OpenAIModelName {
	GPT35Turbo = 'gpt-3.5-turbo',
	GPT4 = 'gpt-4',
	GPT4Turbo = 'gpt-4-turbo-preview'
}

export enum AnthropicModelName {
	Opus = 'claude-3-opus-20240229',
	Sonnet = 'claude-3-sonnet-20240229',
	Haiku = 'claude-3-haiku-20240307'
}

export const AI_SERVICE_CONTEXT = Symbol();

export class AIService {
	private user$: Writable<User | undefined>;

	constructor(
		private gitConfig: GitConfig,
		private cloud: ReturnType<typeof getCloudApiClient>,
		user$: Observable<User | undefined>
	) {
		this.user$ = writable<User | undefined>();

		user$.subscribe((user) => this.user$.set(user));
	}

	// This optionally returns a summarizer. There are a few conditions for how this may occur
	// Firstly, if the user has opted to use the GB API and isn't logged in, it will return undefined
	// Secondly, if the user has opted to bring their own key but hasn't provided one, it will return undefined
	async buildClient() {
		const modelKind =
			(await this.gitConfig.get<ModelKind>('gitbutler.aiModelProvider')) || ModelKind.OpenAI;
		const openAIKeyOption =
			(await this.gitConfig.get<KeyOption>('gitbutler.aiOpenAIKeyOption')) || KeyOption.ButlerAPI;
		const anthropicKeyOption =
			(await this.gitConfig.get<KeyOption>('gitbutler.aiAnthropicKeyOption')) ||
			KeyOption.ButlerAPI;

		if (
			(modelKind == ModelKind.OpenAI && openAIKeyOption == KeyOption.ButlerAPI) ||
			(modelKind == ModelKind.Anthropic && anthropicKeyOption == KeyOption.ButlerAPI)
		) {
			const user = get(this.user$);

			// TODO: Provide feedback to user
			if (!user) return;
			return new ButlerAIClient(this.cloud, user, ModelKind.OpenAI);
		}

		if (modelKind == ModelKind.OpenAI) {
			const openAIModelName =
				(await this.gitConfig.get<OpenAIModelName>('gitbutler.aiOpenAIModelName')) ||
				OpenAIModelName.GPT35Turbo;
			const openAIKey = await this.gitConfig.get('gitbutler.aiOpenAIKey');

			// TODO: Provide feedback to user
			if (!openAIKey) return;

			const openAI = new OpenAI({ apiKey: openAIKey, dangerouslyAllowBrowser: true });
			return new OpenAIClient(openAIModelName, openAI);
		}
		if (modelKind == ModelKind.Anthropic) {
			const anthropicModelName =
				(await this.gitConfig.get<AnthropicModelName>('gitbutler.aiAnthropicModelName')) ||
				AnthropicModelName.Sonnet;
			const anthropicKey = await this.gitConfig.get('gitbutler.aiAnthropicKey');

			// TODO: Provide feedback to user
			if (!anthropicKey) return;

			return new AnthropicAIClient(anthropicKey, anthropicModelName);
		}
	}

	async commit(
		diff: string,
		useEmojiStyle: boolean,
		useBriefStyle: boolean,
		commitTemplate?: string
	) {
		const aiClient = await this.buildClient();
		if (!aiClient) return;

		let prompt = (commitTemplate || defaultCommitTemplate).replaceAll(
			'%{diff}',
			diff.slice(0, diffLengthLimit)
		);

		if (useBriefStyle) {
			prompt = prompt.replaceAll(
				'%{brief_style}',
				'The commit message must be only one sentence and as short as possible.'
			);
		} else {
			prompt = prompt.replaceAll('%{brief_style}', '');
		}
		if (useEmojiStyle) {
			prompt = prompt.replaceAll('%{emoji_style}', 'Make use of GitMoji in the title prefix.');
		} else {
			prompt = prompt.replaceAll('%{emoji_style}', "Don't use any emoji.");
		}

		let message = await aiClient.evaluate(prompt);

		if (useBriefStyle) {
			message = message.split('\n')[0];
		}

		const firstNewLine = message.indexOf('\n');
		const summary = firstNewLine > -1 ? message.slice(0, firstNewLine).trim() : message;
		const description = firstNewLine > -1 ? message.slice(firstNewLine + 1).trim() : '';

		return description.length > 0 ? `${summary}\n\n${description}` : summary;
	}

	async branch(diff: string, branchTemplate?: string) {
		const aiClient = await this.buildClient();
		if (!aiClient) return;

		const prompt = (branchTemplate || defaultBranchTemplate).replaceAll(
			'%{diff}',
			diff.slice(0, diffLengthLimit)
		);

		let message = await aiClient.evaluate(prompt);

		message = message.replaceAll(' ', '-');
		message = message.replaceAll('\n', '-');
		return message;
	}
}
