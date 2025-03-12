import { persisted } from '@gitbutler/shared/persisted';

/** Holds the logged in user's token in memory */
export class TokenMemoryService {
	#token = persisted<string | undefined>(undefined, 'TokenMemoryService-authToken');

	get token() {
		return this.#token;
	}

	setToken(data: string | undefined) {
		this.#token.set(data);
	}
}
