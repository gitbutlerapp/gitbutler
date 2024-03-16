import { AnthropicAIClient } from './aiClients/anthropic';
import { ButlerAIClient } from './aiClients/butler';
import { OpenAIClient } from './aiClients/openAI';
import { AIService, AnthropicModelName, KeyOption, ModelKind, OpenAIModelName } from './aiService';
import { getCloudApiClient } from './cloud';
import { expect, test, describe, vi } from 'vitest';
import type { AIClient } from './aiClient';
import type { GitConfigService } from './gitConfigService';

const defaultGitConfig = Object.freeze({
	'gitbutler.aiModelProvider': ModelKind.OpenAI,
	'gitbutler.aiOpenAIKeyOption': KeyOption.ButlerAPI,
	'gitbutler.aiAnthropicKeyOption': KeyOption.ButlerAPI,
	'gitbutler.aiOpenAIKey': undefined,
	'gitbutler.aiOpenAIModelName': OpenAIModelName.GPT35Turbo,
	'gitbutler.aiAnthropicKey': undefined,
	'gitbutler.aiAnthropicModelName': AnthropicModelName.Haiku
});

class DummyGitConfigService implements GitConfigService {
	constructor(private config: { [index: string]: string | undefined }) {}

	async get<T extends string>(key: string): Promise<T | null> {
		return (this.config[key] || null) as T | null;
	}

	async set<T extends string>(key: string, value: T): Promise<T | null> {
		return (this.config[key] = value);
	}
}

const fetchMock = vi.fn();
const cloud = getCloudApiClient({ fetch: fetchMock });

class DummyAIClient implements AIClient {
	constructor(private response = 'lorem ipsum') {}

	async evaluate(_prompt: string) {
		return this.response;
	}
}

const examplePatch = `
@@ -52,7 +52,8 @@
 
 export enum AnthropicModelName {
 	Opus = 'claude-3-opus-20240229',
-	Sonnet = 'claude-3-sonnet-20240229'
+	Sonnet = 'claude-3-sonnet-20240229',
+	Haiku = 'claude-3-haiku-20240307'
 }
 
 export const AI_SERVICE_CONTEXT = Symbol();
`;

function buildDefaultAIService() {
	const gitConfig = new DummyGitConfigService(structuredClone(defaultGitConfig));
	return new AIService(gitConfig, cloud);
}

describe.concurrent('AIService', () => {
	describe.concurrent('#buildModel', () => {
		test('With default configuration, When a user token is provided. It returns ButlerAIClient', async () => {
			const aiService = buildDefaultAIService();

			expect(await aiService.buildClient('token')).toBeInstanceOf(ButlerAIClient);
		});

		test('With default configuration, When a user is undefined. It returns undefined', async () => {
			const aiService = buildDefaultAIService();

			expect(await aiService.buildClient()).toBe(undefined);
		});

		test('When token is bring your own, When a openAI token is present. It returns OpenAIClient', async () => {
			const gitConfig = new DummyGitConfigService({
				...defaultGitConfig,
				'gitbutler.aiOpenAIKeyOption': KeyOption.BringYourOwn,
				'gitbutler.aiOpenAIKey': 'sk-asdfasdf'
			});
			const aiService = new AIService(gitConfig, cloud);

			expect(await aiService.buildClient()).toBeInstanceOf(OpenAIClient);
		});

		test('When token is bring your own, When a openAI token is blank. It returns undefined', async () => {
			const gitConfig = new DummyGitConfigService({
				...defaultGitConfig,
				'gitbutler.aiOpenAIKeyOption': KeyOption.BringYourOwn,
				'gitbutler.aiOpenAIKey': undefined
			});
			const aiService = new AIService(gitConfig, cloud);

			expect(await aiService.buildClient()).toBe(undefined);
		});

		test('When ai provider is Anthropic, When token is bring your own, When an anthropic token is present. It returns AnthropicAIClient', async () => {
			const gitConfig = new DummyGitConfigService({
				...defaultGitConfig,
				'gitbutler.aiModelProvider': ModelKind.Anthropic,
				'gitbutler.aiAnthropicKeyOption': KeyOption.BringYourOwn,
				'gitbutler.aiAnthropicKey': 'sk-ant-api03-asdfasdf'
			});
			const aiService = new AIService(gitConfig, cloud);

			expect(await aiService.buildClient()).toBeInstanceOf(AnthropicAIClient);
		});

		test('When ai provider is Anthropic, When token is bring your own, When an anthropic token is blank. It returns undefined', async () => {
			const gitConfig = new DummyGitConfigService({
				...defaultGitConfig,
				'gitbutler.aiModelProvider': ModelKind.Anthropic,
				'gitbutler.aiAnthropicKeyOption': KeyOption.BringYourOwn,
				'gitbutler.aiAnthropicKey': undefined
			});
			const aiService = new AIService(gitConfig, cloud);

			expect(await aiService.buildClient()).toBe(undefined);
		});
	});

	describe.concurrent('#summarizeCommit', async () => {
		test('When buildModel returns undefined, it returns undefined', async () => {
			const aiService = buildDefaultAIService();

			vi.spyOn(aiService, 'buildClient').mockReturnValue((async () => undefined)());

			expect(await aiService.summarizeCommit({ diff: examplePatch })).toBe(undefined);
		});

		test('When the AI returns a single line commit message, it returns it unchanged', async () => {
			const aiService = buildDefaultAIService();

			const clientResponse = 'single line commit';

			vi.spyOn(aiService, 'buildClient').mockReturnValue(
				(async () => new DummyAIClient(clientResponse))()
			);

			expect(await aiService.summarizeCommit({ diff: examplePatch })).toBe('single line commit');
		});

		test('When the AI returns a title and body that is split by a single new line, it replaces it with two', async () => {
			const aiService = buildDefaultAIService();

			const clientResponse = 'one\nnew line';

			vi.spyOn(aiService, 'buildClient').mockReturnValue(
				(async () => new DummyAIClient(clientResponse))()
			);

			expect(await aiService.summarizeCommit({ diff: examplePatch })).toBe('one\n\nnew line');
		});

		test('When the commit is in brief mode, When the AI returns a title and body, it takes just the title', async () => {
			const aiService = buildDefaultAIService();

			const clientResponse = 'one\nnew line';

			vi.spyOn(aiService, 'buildClient').mockReturnValue(
				(async () => new DummyAIClient(clientResponse))()
			);

			expect(await aiService.summarizeCommit({ diff: examplePatch, useBriefStyle: true })).toBe(
				'one'
			);
		});
	});

	describe.concurrent('#summarizeBranch', async () => {
		test('When buildModel returns undefined, it returns undefined', async () => {
			const aiService = buildDefaultAIService();

			vi.spyOn(aiService, 'buildClient').mockReturnValue((async () => undefined)());

			expect(await aiService.summarizeBranch({ diff: examplePatch })).toBe(undefined);
		});

		test('When the AI client returns a string with spaces, it replaces them with hypens', async () => {
			const aiService = buildDefaultAIService();

			const clientResponse = 'with spaces included';

			vi.spyOn(aiService, 'buildClient').mockReturnValue(
				(async () => new DummyAIClient(clientResponse))()
			);

			expect(await aiService.summarizeBranch({ diff: examplePatch })).toBe('with-spaces-included');
		});

		test('When the AI client returns multiple lines, it replaces them with hypens', async () => {
			const aiService = buildDefaultAIService();

			const clientResponse = 'with\nnew\nlines\nincluded';

			vi.spyOn(aiService, 'buildClient').mockReturnValue(
				(async () => new DummyAIClient(clientResponse))()
			);

			expect(await aiService.summarizeBranch({ diff: examplePatch })).toBe(
				'with-new-lines-included'
			);
		});

		test('When the AI client returns multiple lines and spaces, it replaces them with hypens', async () => {
			const aiService = buildDefaultAIService();

			const clientResponse = 'with\nnew lines\nincluded';

			vi.spyOn(aiService, 'buildClient').mockReturnValue(
				(async () => new DummyAIClient(clientResponse))()
			);

			expect(await aiService.summarizeBranch({ diff: examplePatch })).toBe(
				'with-new-lines-included'
			);
		});
	});
});
