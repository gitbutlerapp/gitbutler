import { listen } from '$lib/backend/ipc';
import { invoke } from '$lib/backend/ipc';
import { Subject } from 'rxjs';

export type SystemPrompt = {
	id: string;
	prompt: string;
	context?: {
		// TODO: camelCase this field
		branch_id?: string;
		action?: string;
	};
	canceled?: boolean;
};

type PromptResponse = {
	id: string;
	response: string | null;
};

export class PromptService {
	prompt$ = new Subject<SystemPrompt>();

	private unlisten = listen<SystemPrompt>('git_prompt', async (e) => {
		// You can send an action token to e.g. `fetch_target_data` and it will be echoed in
		// these events. The action `auto` is used by the `BaseBranchService` so we can not
		// respond to them.
		if (e.payload.context?.action != 'auto') {
			this.prompt$.next(e.payload);
		} else {
			// Always cancel actions that are marked "auto", e.g. periodic sync
			await this.cancel(e.payload.id);
		}
	});

	constructor() {}

	async respond(payload: PromptResponse) {
		return await invoke('submit_prompt_response', payload);
	}

	async cancel(id: string) {
		return await invoke('submit_prompt_response', { id: id, response: null });
	}

	destroy() {
		this.unlisten();
	}
}
