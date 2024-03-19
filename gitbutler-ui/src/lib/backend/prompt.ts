import { listen } from '$lib/backend/ipc';
import { invoke } from '$lib/backend/ipc';
import { Subject } from 'rxjs';

export type SystemPrompt = {
	id: string;
	prompt: string;
	context?: {
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

	constructor() {
		this.subscribe(async (payload) => {
			if (payload.context?.action == 'auto') {
				this.cancel(payload.id).then(() => console.log('cancelled auto askpass', payload));
			} else {
				this.prompt$.next(payload);
			}
		});
	}

	private subscribe(callback: (params: SystemPrompt) => Promise<void> | void) {
		return listen<SystemPrompt>('git_prompt', (e) => callback(e.payload));
	}

	async respond(payload: PromptResponse) {
		return await invoke('submit_prompt_response', payload);
	}

	async cancel(id: string) {
		return await invoke('submit_prompt_response', { id: id, response: null });
	}
}
