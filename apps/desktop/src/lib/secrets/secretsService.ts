import { invoke } from '$lib/backend/ipc';
import { InjectionToken } from '@gitbutler/shared/context';
import type { GitConfigService } from '$lib/config/gitConfigService';

export type SecretsService = {
	get(handle: string): Promise<string | undefined>;
	set(handle: string, secret: string): Promise<void>;
};

export const SECRET_SERVICE = new InjectionToken<SecretsService>('SecretService');

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
