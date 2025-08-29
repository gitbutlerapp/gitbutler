import { sleep } from '$lib/utils/sleep';
import { InjectionToken } from '@gitbutler/shared/context';
import { writable, type Writable } from 'svelte/store';
import type { IBackend } from '$lib/backend';

export const PROMPT_SERVICE = new InjectionToken<PromptService>('PromptService');

type SystemPrompt = {
	id: string;
	prompt: string;
	context?: {
		// TODO: camelCase this field
		branch_id?: string;
		action?: string;
	};
	handled?: boolean;
};

type FilterParams = {
	// This can be e.g. push, fetch, auto etc
	action?: string | undefined;
	// Filter to a specific `BranchLane`
	branchId?: string | undefined;
	// Time until auto closing prompt
	timeoutMs?: number | undefined;
};

export type SystemPromptHandle = {
	prompt: string;
	respond: (response: string | null) => Promise<void>;
};

type PromptHandler = (prompt: SystemPrompt, signal: AbortSignal) => Promise<string | null>;
type FilterEventEntry = [filter: FilterParams, handler: PromptHandler];

/**
 * Handle the abort signal for a prompt handler.
 *
 * This is required to be able to await the handling of a prompt.
 */
async function handleAbortSignal(
	signal: AbortSignal,
	promptStore: Writable<SystemPromptHandle | undefined>,
	errorStore: Writable<any>
): Promise<void> {
	const signalHandler = new Promise<void>((resolve) => {
		signal.addEventListener('abort', () => {
			promptStore.set(undefined);
			switch (signal.reason) {
				case 'timeout':
					errorStore.set('Timed out waiting for response');
					break;
				default:
					errorStore.set(undefined);
					break;
			}
			resolve();
		});
	});

	await signalHandler;
}

/**
 * This service is used for handling CLI prompts from the back end.
 */
export class PromptService {
	// Holds "matched" handlers - handlers that filter on action or branch_id.
	// Handlers return whether or not they handled the prompt.
	private matchHandlers: Set<FilterEventEntry> = new Set();

	// Holds default handlers - handlers executed when no match is found
	private defaultHandlers: Set<FilterEventEntry> = new Set();

	// If subscribed to the global event, holds the number of subscribers
	// and an unsubscribe function handle.
	private subscriberCount = 0;
	private unsubscriber: (() => Promise<void>) | null = null;

	constructor(private backend: IBackend) {}

	private async handleEvent(e: SystemPrompt): Promise<string | null> {
		// TODO(qix-): By default we ignore `auto`. Ideally we do this externally (as in, something
		// TODO(qix-): subscribes to a specific filter) but for now this is sufficient.
		if (e.context?.action === 'auto') return null;

		const abortHandle = new AbortController();

		let matchers = Array.from(this.matchHandlers).filter(([filter]) => {
			if (filter.action && filter.action !== e.context?.action) return false;
			if (filter.branchId && filter.branchId !== e.context?.branch_id) return false;
			return true;
		});

		if (matchers.length === 0) {
			matchers = Array.from(this.defaultHandlers);
		}

		return await Promise.race(
			matchers.map(
				async ([filter, handler]) =>
					await Promise.race([
						handler(e, abortHandle.signal).then((response) => {
							abortHandle.abort('handled');
							return response;
						}),
						filter.timeoutMs
							? sleep(filter.timeoutMs).then(() => {
									abortHandle.abort('timeout');
									return null;
								})
							: <Promise<string | null>>new Promise(() => null)
					])
			)
		);
	}

	private subscribe() {
		this.unsubscriber ??= this.backend.listen<SystemPrompt>('git_prompt', async (e) => {
			const response = await this.handleEvent(e.payload);
			return await this.backend.invoke('submit_prompt_response', { id: e.payload.id, response });
		});

		++this.subscriberCount;
	}

	private unsubscribe() {
		if (this.subscriberCount > 0 && !--this.subscriberCount) {
			this.unsubscriber?.();
		}
	}

	onPrompt(filter: FilterParams, handler: PromptHandler): () => void {
		const entry: FilterEventEntry = [filter, handler];

		const handlerSet =
			!filter.action && !filter.branchId ? this.defaultHandlers : this.matchHandlers;

		handlerSet.add(entry);

		this.subscribe();

		return () => {
			if (handlerSet.delete(entry)) {
				this.unsubscribe();
			}
		};
	}

	reactToPrompt(filter: FilterParams): [Writable<SystemPromptHandle | undefined>, Writable<any>] {
		const promptStore = writable<SystemPromptHandle | undefined>();
		const errorStore = writable<any>();

		this.onPrompt(filter, async (prompt, signal) => {
			let resolver: (response: string | null) => void;
			const promise: Promise<string | null> = new Promise((r) => {
				resolver = r;
			});

			promptStore.set({
				prompt: prompt.prompt,
				respond: async (response: string | null) => {
					resolver(response);
					await handleAbortSignal(signal, promptStore, errorStore);
				}
			});

			return await promise;
		});

		return [promptStore, errorStore];
	}
}
