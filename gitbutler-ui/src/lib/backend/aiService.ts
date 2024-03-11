import { ButlerAIProvider, OpenAIProvider, AnthropicAIProvider } from './aiProviders';
import { Summarizer } from './summarizer';
import { KeyOption, ModelKind } from './summarizerSettings';
import OpenAI from 'openai';
import { derived, writable, type Readable } from 'svelte/store';
import type { User, getCloudApiClient } from './cloud';
import type { AllSummarizerSettings, SummarizerSettings } from './summarizerSettings';
import type { Observable } from 'rxjs';

export class AIService {
	public summarizer$: Readable<Summarizer | undefined>;

	constructor(
		summarizerSettings: SummarizerSettings,
		private cloud: ReturnType<typeof getCloudApiClient>,
		user$: Observable<User | undefined>
	) {
		this.summarizer$ = derived(
			[summarizerSettings.all$, this.observableToStore(user$)],
			([summarizerSettings, user]) => this.buildSummarizer(summarizerSettings, user)
		);
	}

	// This optionally returns a summarizer. There are a few conditions for how this may occur
	// Firstly, if the user has opted to use the GB API and isn't logged in, it will return undefined
	// Secondly, if the user has opted to bring their own key but hasn't provided one, it will return undefined
	buildSummarizer(allSummarizerSettings: AllSummarizerSettings, user: User | undefined) {
		const modelKind = allSummarizerSettings.modelKind;
		const keyOption = allSummarizerSettings.keyOption;

		if (keyOption === KeyOption.ButlerAPI) {
			if (!user) return;

			const aiProvider = new ButlerAIProvider(this.cloud, user, modelKind);
			return new Summarizer(aiProvider);
		}

		if (modelKind == ModelKind.OpenAI) {
			const openAIKey = allSummarizerSettings.openAIKey;

			if (!openAIKey) return;

			const openAIModel = allSummarizerSettings.openAIModel;
			const openAI = new OpenAI({ apiKey: openAIKey, dangerouslyAllowBrowser: true });
			const aiProvider = new OpenAIProvider(openAIModel, openAI);
			return new Summarizer(aiProvider);
		}

		if (modelKind == ModelKind.Anthropic) {
			const anthropicKey = allSummarizerSettings.anthropicKey;

			if (!anthropicKey) return;

			const anthropicModel = allSummarizerSettings.anthropicModel;
			const aiProvider = new AnthropicAIProvider(anthropicKey, anthropicModel);
			return new Summarizer(aiProvider);
		}
	}

	private observableToStore<T>(observable: Observable<T>) {
		const output = writable<T>();
		observable.subscribe((value) => output.set(value));

		return output;
	}
}
