import { listen } from '$lib/backend/ipc';
import { invoke } from '$lib/backend/ipc';
import { observableToStore } from '$lib/rxjs/store';
import {
	Observable,
	Subject,
	catchError,
	filter,
	merge,
	mergeWith,
	of,
	switchMap,
	tap,
	throwError,
	timeout
} from 'rxjs';
import { writable, type Readable } from 'svelte/store';

export type SystemPrompt = {
	id: string;
	prompt: string;
	context?: {
		// TODO: camelCase this field
		branch_id?: string;
		action?: string;
	};
	handled?: boolean;
};

type PromptResponse = {
	id: string;
	response: string | null;
};

type FilterParams = {
	// This can be e.g. push, fetch, auto etc
	action?: string | undefined;
	// Filter to a specific `BranchLane`
	branchId?: string | undefined;
	// Time until auto closing prompt
	timeoutMs?: number | undefined;
};

/**
 * This service is used for handling CLI prompts from the back end.
 */
export class PromptService {
	// Used to reset the stream after submit/cancel
	private reset = new Subject<undefined | SystemPrompt>();

	// This subject is used to reset timeouts
	private submission = new Subject<SystemPrompt | undefined>();

	// Base observable that opens Tauri subscription on first subscriber, and tears
	// down automatically when the last subscriber disconnects.
	private promptStream = new Observable<SystemPrompt | undefined>((subscriber) => {
		this.unlistenTauri = this.listenTauri((prompt) => subscriber.next(prompt));
		return () => {
			if (this.unlistenTauri) this.unlistenTauri();
		};
	}).pipe(
		tap(() => {
			this.updatedAt.set(new Date());
		}),
		mergeWith(this.reset)
	);

	// Feeds user supplied string as input to askpass
	async respond(payload: PromptResponse) {
		this.reset.next(undefined);
		return await invoke('submit_prompt_response', payload);
	}

	// Cancels the executable input prompt
	async cancel(id: string) {
		this.reset.next(undefined);
		return await invoke('submit_prompt_response', { id: id, response: null });
	}

	/**
	 * When you first call this function it creates two stores out of one observable,
	 * and when these stores are _first_ subscribed to we start listening to the backend
	 * for prompt events. When the last of all stores created with this function unsubscribes
	 * we also stop listening for events from tauri.
	 */
	filter({
		action = undefined,
		branchId = undefined,
		timeoutMs = 120 * 1000
	}: FilterParams): [Readable<SystemPrompt | undefined>, Readable<any>] {
		return observableToStore<SystemPrompt | undefined>(
			this.promptStream.pipe(
				filter((prompt) => {
					if (!prompt) return true;
					const promptBranchId = prompt?.context?.branch_id;
					const promptAction = prompt?.context?.action;
					return !!(
						(promptAction && promptAction == action) ||
						(promptBranchId && promptBranchId == branchId)
					);
				}),
				switchMap((prompt) => {
					if (!prompt) return of(undefined);
					return merge(
						of(prompt),
						this.submission.pipe(
							timeout(timeoutMs),
							catchError((err) => {
								if (prompt) this.cancel(prompt.id);
								return throwError(() => err);
							})
						)
					);
				})
			)
		);
	}

	// This can e.g. be used to disable interval polling
	readonly updatedAt = writable<Date | undefined>();

	// Stop tauri subscription when last store subscriber unsubscribes
	unlistenTauri: (() => Promise<void>) | undefined = undefined;

	// This is how we are notified of input prompts from the backend
	private listenTauri(next: (event: SystemPrompt) => void): () => Promise<void> {
		const unsubscribe = listen<SystemPrompt>('git_prompt', async (e) => {
			this.updatedAt.set(new Date());
			// You can send an action token to e.g. `fetch_from_target` and it will be echoed in
			// these events. The action `auto` is used by the `BaseBranchService` so we can not
			// respond to them.
			if (e.payload.context?.action == 'auto') {
				// Always cancel actions that are marked "auto", e.g. periodic sync
				await this.cancel(e.payload.id);
				// Note: ingore store update to avoid other side-effects
			} else {
				next(e.payload);
			}
		});
		return async () => {
			// Called after service stops listening for events
			await unsubscribe();
		};
	}
}
