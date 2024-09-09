import { invoke as ipcInvoke } from '$lib/backend/ipc';
import { buildContext } from '$lib/utils/context';
import { isStr } from '$lib/utils/string';

const CACHE_STALE_TIME = 1000 * 60 * 5;

export interface NameNormalizationService {
	normalize(branchName: string): Promise<string>;
}

interface TimestampedBranchName {
	name: string;
	timestamp: number;
}

export class IpcNameNormalizationService implements NameNormalizationService {
	private cache: Map<string, TimestampedBranchName>;

	constructor(private invoke: typeof ipcInvoke) {
		this.cache = new Map();
	}

	async normalize(branchName: string): Promise<string> {
		const now = Date.now();
		const cached = this.cache.get(branchName);
		if (cached && now - cached.timestamp < CACHE_STALE_TIME) {
			return cached.name;
		}
		const result = await this.invoke('normalize_branch_name', { name: branchName });

		if (!isStr(result)) {
			throw new Error('Branch name normalization failed');
		}

		this.cache.set(branchName, { name: result, timestamp: now });
		return result;
	}
}

export const [getNameNormalizationServiceContext, setNameNormalizationServiceContext] =
	buildContext<NameNormalizationService>('Name normalization service');
