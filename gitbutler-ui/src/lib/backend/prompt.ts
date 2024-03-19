import { listen } from '$lib/backend/ipc';
import { invoke } from '$lib/backend/ipc';

type PromptPayload = {
	prompt: string;
};
export class PromptService {
	subscribe(callback: (params: PromptPayload) => Promise<void>) {
		return listen<PromptPayload>(`prompt://`, (e) => callback(e.payload));
	}

	async respond(nonce: string, value: string) {
		return await invoke<boolean>('respond_to_prompt', { nonce, value });
	}
}
