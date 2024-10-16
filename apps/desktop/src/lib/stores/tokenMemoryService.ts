import { writable } from 'svelte/store';

/** Holds the logged in user's token in memory */
export class TokenMemoryService {
	#token = writable<string | undefined>(undefined);

	get token() {
		return this.#token;
	}

	setToken(data: string | undefined) {
		this.#token.set(data);
	}
}
