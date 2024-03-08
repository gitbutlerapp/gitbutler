import { invoke } from './ipc';

export type GitCredentialCheck = {
	error?: string;
	ok: boolean;
};

export class AuthService {
	async getPublicKey() {
		return await invoke<string>('get_public_key');
	}
}
