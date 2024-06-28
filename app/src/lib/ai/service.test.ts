import { AnthropicAIClient } from '$lib/ai/anthropicClient';
import { ButlerAIClient } from '$lib/ai/butlerClient';
import { OpenAIClient } from '$lib/ai/openAIClient';
import { SHORT_DEFAULT_BRANCH_TEMPLATE, SHORT_DEFAULT_COMMIT_TEMPLATE } from '$lib/ai/prompts';
import { AIService, GitAIConfigKey, KeyOption, buildDiff } from '$lib/ai/service';
import {
	AnthropicModelName,
	ModelKind,
	OpenAIModelName,
	type AIClient,
	type Prompt
} from '$lib/ai/types';
import { HttpClient } from '$lib/backend/httpClient';
import { buildFailureFromAny, ok, unwrap, type Result } from '$lib/result';
import { Hunk } from '$lib/vbranches/types';
import { plainToInstance } from 'class-transformer';
import { expect, test, describe, vi } from 'vitest';
import type { GbConfig, GitConfigService } from '$lib/backend/gitConfigService';

const defaultGitConfig = Object.freeze({
	[GitAIConfigKey.ModelProvider]: ModelKind.OpenAI,
	[GitAIConfigKey.OpenAIKeyOption]: KeyOption.ButlerAPI,
	[GitAIConfigKey.OpenAIKey]: undefined,
	[GitAIConfigKey.OpenAIModelName]: OpenAIModelName.GPT35Turbo,
	[GitAIConfigKey.AnthropicKeyOption]: KeyOption.ButlerAPI,
	[GitAIConfigKey.AnthropicKey]: undefined,
	[GitAIConfigKey.AnthropicModelName]: AnthropicModelName.Haiku
});

class DummyGitConfigService implements GitConfigService {
	constructor(private config: { [index: string]: string | undefined }) {}
	async getGbConfig(_projectId: string): Promise<GbConfig> {
		throw new Error('Method not implemented.');
	}
	async setGbConfig(_projectId: string, _config: GbConfig): Promise<unknown> {
		throw new Error('Method not implemented.');
	}
	async get<T extends string>(key: string): Promise<T | undefined> {
		return (this.config[key] || undefined) as T | undefined;
	}

	async getWithDefault<T extends string>(key: string): Promise<T> {
		return this.config[key] as T;
	}

	async set<T extends string>(key: string, value: T): Promise<T | undefined> {
		return (this.config[key] = value);
	}
}

const fetchMock = vi.fn();
const cloud = new HttpClient(fetchMock);

class DummyAIClient implements AIClient {
	defaultCommitTemplate = SHORT_DEFAULT_COMMIT_TEMPLATE;
	defaultBranchTemplate = SHORT_DEFAULT_BRANCH_TEMPLATE;
	constructor(private response = 'lorem ipsum') {}

	async evaluate(_prompt: Prompt): Promise<Result<string, Error>> {
		return ok(this.response);
	}
}

const diff1 = `
@@ -52,7 +52,8 @@

 export enum AnthropicModelName {
 	Opus = 'claude-3-opus-20240229',
-	Sonnet = 'claude-3-sonnet-20240229'
+	Sonnet = 'claude-3-sonnet-20240229',
+	Haiku = 'claude-3-haiku-20240307'
 }

 export const AI_SERVICE_CONTEXT = Symbol();
`;

const hunk1 = plainToInstance(Hunk, {
	id: 'asdf',
	diff: diff1,
	modifiedAt: new Date().toISOString(),
	filePath: 'foo/bar/baz.ts',
	locked: false,
	lockedTo: undefined,
	changeType: 'added'
});

const diff2 = `
@@ -52,7 +52,8 @@
 }
 async function commit() {
 	console.log('quack quack goes the dog');
+	const message = concatMessage(title, description);
 	isCommitting = true;
 try {
`;

const hunk2 = plainToInstance(Hunk, {
	id: 'asdf',
	diff: diff2,
	modifiedAt: new Date().toISOString(),
	filePath: 'random.ts',
	locked: false,
	lockedTo: undefined,
	changeType: 'added'
});

const exampleHunks = [hunk1, hunk2];

function buildDefaultAIService() {
	const gitConfig = new DummyGitConfigService(structuredClone(defaultGitConfig));
	return new AIService(gitConfig, cloud);
}

describe.concurrent('AIService', () => {
	describe.concurrent('#buildModel', () => {
		test('With default configuration, When a user token is provided. It returns ButlerAIClient', async () => {
			const aiService = buildDefaultAIService();

			expect(unwrap(await aiService.buildClient('token'))).toBeInstanceOf(ButlerAIClient);
		});

		test('With default configuration, When a user is undefined. It returns undefined', async () => {
			const aiService = buildDefaultAIService();

			expect(await aiService.buildClient()).toStrictEqual(
				buildFailureFromAny("When using GitButler's API to summarize code, you must be logged in")
			);
		});

		test('When token is bring your own, When a openAI token is present. It returns OpenAIClient', async () => {
			const gitConfig = new DummyGitConfigService({
				...defaultGitConfig,
				[GitAIConfigKey.OpenAIKeyOption]: KeyOption.BringYourOwn,
				[GitAIConfigKey.OpenAIKey]: 'sk-asdfasdf'
			});
			const aiService = new AIService(gitConfig, cloud);

			expect(unwrap(await aiService.buildClient())).toBeInstanceOf(OpenAIClient);
		});

		test('When token is bring your own, When a openAI token is blank. It returns undefined', async () => {
			const gitConfig = new DummyGitConfigService({
				...defaultGitConfig,
				[GitAIConfigKey.OpenAIKeyOption]: KeyOption.BringYourOwn,
				[GitAIConfigKey.OpenAIKey]: undefined
			});
			const aiService = new AIService(gitConfig, cloud);

			expect(await aiService.buildClient()).toStrictEqual(
				buildFailureFromAny(
					'When using OpenAI in a bring your own key configuration, you must provide a valid token'
				)
			);
		});

		test('When ai provider is Anthropic, When token is bring your own, When an anthropic token is present. It returns AnthropicAIClient', async () => {
			const gitConfig = new DummyGitConfigService({
				...defaultGitConfig,
				[GitAIConfigKey.ModelProvider]: ModelKind.Anthropic,
				[GitAIConfigKey.AnthropicKeyOption]: KeyOption.BringYourOwn,
				[GitAIConfigKey.AnthropicKey]: 'sk-ant-api03-asdfasdf'
			});
			const aiService = new AIService(gitConfig, cloud);

			expect(unwrap(await aiService.buildClient())).toBeInstanceOf(AnthropicAIClient);
		});

		test('When ai provider is Anthropic, When token is bring your own, When an anthropic token is blank. It returns undefined', async () => {
			const gitConfig = new DummyGitConfigService({
				...defaultGitConfig,
				[GitAIConfigKey.ModelProvider]: ModelKind.Anthropic,
				[GitAIConfigKey.AnthropicKeyOption]: KeyOption.BringYourOwn,
				[GitAIConfigKey.AnthropicKey]: undefined
			});
			const aiService = new AIService(gitConfig, cloud);

			expect(await aiService.buildClient()).toStrictEqual(
				buildFailureFromAny(
					'When using Anthropic in a bring your own key configuration, you must provide a valid token'
				)
			);
		});
	});

	describe.concurrent('#summarizeCommit', async () => {
		test('When buildModel returns undefined, it returns undefined', async () => {
			const aiService = buildDefaultAIService();

			vi.spyOn(aiService, 'buildClient').mockReturnValue(
				(async () => buildFailureFromAny('Failed to build'))()
			);

			expect(await aiService.summarizeCommit({ hunks: exampleHunks })).toStrictEqual(
				buildFailureFromAny('Failed to build')
			);
		});

		test('When the AI returns a single line commit message, it returns it unchanged', async () => {
			const aiService = buildDefaultAIService();

			const clientResponse = 'single line commit';

			vi.spyOn(aiService, 'buildClient').mockReturnValue(
				(async () => ok<AIClient, Error>(new DummyAIClient(clientResponse)))()
			);

			expect(await aiService.summarizeCommit({ hunks: exampleHunks })).toStrictEqual(
				ok('single line commit')
			);
		});

		test('When the AI returns a title and body that is split by a single new line, it replaces it with two', async () => {
			const aiService = buildDefaultAIService();

			const clientResponse = 'one\nnew line';

			vi.spyOn(aiService, 'buildClient').mockReturnValue(
				(async () => ok<AIClient, Error>(new DummyAIClient(clientResponse)))()
			);

			expect(await aiService.summarizeCommit({ hunks: exampleHunks })).toStrictEqual(
				ok('one\n\nnew line')
			);
		});

		test('When the commit is in brief mode, When the AI returns a title and body, it takes just the title', async () => {
			const aiService = buildDefaultAIService();

			const clientResponse = 'one\nnew line';

			vi.spyOn(aiService, 'buildClient').mockReturnValue(
				(async () => ok<AIClient, Error>(new DummyAIClient(clientResponse)))()
			);

			expect(
				await aiService.summarizeCommit({ hunks: exampleHunks, useBriefStyle: true })
			).toStrictEqual(ok('one'));
		});
	});

	describe.concurrent('#summarizeBranch', async () => {
		test('When buildModel returns undefined, it returns undefined', async () => {
			const aiService = buildDefaultAIService();

			vi.spyOn(aiService, 'buildClient').mockReturnValue(
				(async () => buildFailureFromAny('Failed to build client'))()
			);

			expect(await aiService.summarizeBranch({ hunks: exampleHunks })).toStrictEqual(
				buildFailureFromAny('Failed to build client')
			);
		});

		test('When the AI client returns a string with spaces, it replaces them with hypens', async () => {
			const aiService = buildDefaultAIService();

			const clientResponse = 'with spaces included';

			vi.spyOn(aiService, 'buildClient').mockReturnValue(
				(async () => ok<AIClient, Error>(new DummyAIClient(clientResponse)))()
			);

			expect(await aiService.summarizeBranch({ hunks: exampleHunks })).toStrictEqual(
				ok('with-spaces-included')
			);
		});

		test('When the AI client returns multiple lines, it replaces them with hypens', async () => {
			const aiService = buildDefaultAIService();

			const clientResponse = 'with\nnew\nlines\nincluded';

			vi.spyOn(aiService, 'buildClient').mockReturnValue(
				(async () => ok<AIClient, Error>(new DummyAIClient(clientResponse)))()
			);

			expect(await aiService.summarizeBranch({ hunks: exampleHunks })).toStrictEqual(
				ok('with-new-lines-included')
			);
		});

		test('When the AI client returns multiple lines and spaces, it replaces them with hypens', async () => {
			const aiService = buildDefaultAIService();

			const clientResponse = 'with\nnew lines\nincluded';

			vi.spyOn(aiService, 'buildClient').mockReturnValue(
				(async () => ok<AIClient, Error>(new DummyAIClient(clientResponse)))()
			);

			expect(await aiService.summarizeBranch({ hunks: exampleHunks })).toStrictEqual(
				ok('with-new-lines-included')
			);
		});
	});
});

describe.concurrent('buildDiff', () => {
	test('When provided one hunk, it returns the formatted diff', () => {
		const expectedOutput = `${hunk1.filePath} - ${hunk1.diff}`;

		expect(buildDiff([hunk1], 10000)).to.eq(expectedOutput);
	});

	test('When provided one hunk and its longer than the limit, it returns the truncated formatted diff', () => {
		expect(buildDiff([hunk1], 100).length).to.eq(100);
	});

	test('When provided multiple hunks, it joins them together with newlines', () => {
		const expectedOutput1 = `${hunk1.filePath} - ${hunk1.diff}\n${hunk2.filePath} - ${hunk2.diff}`;
		const expectedOutput2 = `${hunk2.filePath} - ${hunk2.diff}\n${hunk1.filePath} - ${hunk1.diff}`;

		const outputMatchesExpectedValue = [expectedOutput1, expectedOutput2].includes(
			buildDiff([hunk1, hunk1], 10000)
		);

		expect(outputMatchesExpectedValue).toBeTruthy;
	});
});
