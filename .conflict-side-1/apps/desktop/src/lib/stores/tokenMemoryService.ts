import { persisted } from '@gitbutler/shared/persisted';
import { get, writable, type Readable } from 'svelte/store';
import type { SecretsService } from '$lib/secrets/secretsService';

// Exported for testing :D
export const tokenKey = 'TokenMemoryService-authToken';

const oldToken = persisted<string | undefined>(undefined, tokenKey);

/** Holds the logged in user's token in memory */
export class TokenMemoryService {
	constructor(private readonly secretsService: SecretsService) {}

	#token = writable<string | undefined>(undefined, (set) => {
		const old = get(oldToken);
		if (old) {
			set(old);

			this.secretsService.set(tokenKey, old).then(() => {
				oldToken.set(undefined);
			});
		} else {
			this.secretsService.get(tokenKey).then(set);
		}
	});

	get token(): Readable<string | undefined> {
		return this.#token;
	}

	async setToken(data: string | undefined) {
		this.#token.set(data);
		if (data !== undefined) {
			await this.secretsService.set(tokenKey, data);
		} else {
			await this.secretsService.delete(tokenKey);
		}
	}
}
