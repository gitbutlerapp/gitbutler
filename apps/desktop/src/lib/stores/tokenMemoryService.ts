import { writable, type Readable } from "svelte/store";
import type { UserService as _UserService } from "$lib/user/userService";

/**
 * Holds the logged in user's token in memory
 *
 * Persistence is handled by the login process.
 * @see _UserService#setUser for more details.
 */
export class TokenMemoryService {
	constructor() {}

	#token = writable<string | undefined>(undefined, () => {});

	get token(): Readable<string | undefined> {
		return this.#token;
	}

	async setToken(data: string | undefined) {
		this.#token.set(data);
	}
}
