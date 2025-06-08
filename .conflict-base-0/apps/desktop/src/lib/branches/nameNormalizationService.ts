import { invoke as ipcInvoke } from '$lib/backend/ipc';
import { buildContext } from '@gitbutler/shared/context';

export interface NameNormalizationService {
	normalize(branchName: string): Promise<string>;
}

export class IpcNameNormalizationService implements NameNormalizationService {
	constructor(private invoke: typeof ipcInvoke) {}

	async normalize(branchName: string): Promise<string> {
		return await this.invoke('normalize_branch_name', { name: branchName });
	}
}

export const [getNameNormalizationServiceContext, setNameNormalizationServiceContext] =
	buildContext<NameNormalizationService>('Name normalization service');
