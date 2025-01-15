import { invoke } from '$lib/backend/ipc';
import { buildContext } from '@gitbutler/shared/context';
import type { GitConfigService } from '$lib/backend/gitConfigService';

export type SecretsService = {
	get(handle: string): Promise<string | undefined>;
	set(handle: string, secret: string): Promise<void>;
};

export const [getSecretsService, setSecretsService] =
	buildContext<SecretsService>('secretsService');

export class RustSecretService implements SecretsService {
	constructor(private gitConfigService: GitConfigService) {}

	async get(handle: string) {
		const secret = await invoke<string>('secret_get_global', { handle });
		if (secret) return secret;
	}

	async set(handle: string, secret: string) {
		await invoke('secret_set_global', {
			handle,
			secret
		});
	}
}
