import {
	CloudPatchWithFiles,
	MINUTES_15,
	type ApiPatchWithFiles,
	type LoadableOptional
} from '$lib/cloud/types';
import { writableDerived } from '$lib/storeUtils';
import { derived, get, type Readable, type Writable } from 'svelte/store';
import type { HttpClient } from '$lib/httpClient';

interface PatchUpdate {
	sign_off?: boolean;
	section_order?: string[];
}

export class ApiPatchService {
	readonly canGetPatch: Readable<boolean>;
	readonly canUpdatePatch: Readable<boolean>;

	constructor(private readonly httpClient: HttpClient) {
		this.canGetPatch = httpClient.authenticationAvailable;
		this.canUpdatePatch = httpClient.authenticationAvailable;
	}

	async getPatch(cloudBranchId: string, changeId: string): Promise<ApiPatchWithFiles | undefined> {
		try {
			return await this.httpClient.get<ApiPatchWithFiles>(
				`patch_stack/${cloudBranchId}/patch/${changeId}`
			);
		} catch (e) {
			// If the internet is down, silently fail
			if (e instanceof TypeError) {
				return undefined;
			} else {
				throw e;
			}
		}
	}

	async updatePatch(
		cloudBranchId: string,
		changeId: string,
		params: PatchUpdate
	): Promise<ApiPatchWithFiles | undefined> {
		try {
			return await this.httpClient.patch<ApiPatchWithFiles>(
				`patch_stack/${cloudBranchId}/patch/${changeId}`,
				{
					body: params
				}
			);
		} catch (e) {
			// If the internet is down, silently fail
			if (e instanceof TypeError) {
				return undefined;
			} else {
				throw e;
			}
		}
	}
}

interface CloudPatchUpdate {
	signOff?: boolean;
	sectionOrder?: string[];
}

export class CloudPatchService {
	readonly #apiPatch: Writable<LoadableOptional<ApiPatchWithFiles>>;
	readonly patch: Readable<LoadableOptional<CloudPatchWithFiles>>;
	readonly canUpdatePatch: Readable<boolean>;

	constructor(
		private readonly cloudBranchId: Readable<string | undefined>,
		private readonly changeId: Readable<string | undefined>,
		private readonly apiPatchService: ApiPatchService
	) {
		this.canUpdatePatch = apiPatchService.canUpdatePatch;

		const values = derived(
			[cloudBranchId, changeId, apiPatchService.canGetPatch],
			(values) => values
		);

		this.#apiPatch = writableDerived(
			values,
			{ state: 'uninitialized' } as LoadableOptional<ApiPatchWithFiles>,
			([cloudBranchId, changeId, canGetPatch], set) => {
				if (!cloudBranchId || !changeId || !canGetPatch) {
					set({ state: 'uninitialized' });
					return;
				}

				let canceled = false;

				const callback = () => {
					this.apiPatchService.getPatch(cloudBranchId, changeId).then((apiPatchWithFiles) => {
						if (!canceled) {
							if (apiPatchWithFiles) {
								set({ state: 'found', value: apiPatchWithFiles });
							} else {
								set({ state: 'not-found' });
							}
						}
					});
				};

				// Automatically refresh every 15 minutes
				callback();
				const interval = setInterval(callback, MINUTES_15);

				return () => {
					canceled = true;
					clearInterval(interval);
				};
			}
		);

		this.patch = derived(
			this.#apiPatch,
			(patch): LoadableOptional<CloudPatchWithFiles> => {
				if (patch.state === 'found') {
					console.log('found', patch);
					return {
						state: 'found',
						value: new CloudPatchWithFiles(patch.value)
					};
				} else {
					console.log('notfound', patch);
					return patch;
				}
			},
			{ state: 'uninitialized' }
		);
	}

	/** Refresh the patch */
	async refresh(): Promise<void> {
		const cloudBranchId = get(this.cloudBranchId);
		const changeId = get(this.changeId);
		const canGetPatch = get(this.apiPatchService.canGetPatch);

		if (cloudBranchId && changeId && canGetPatch) {
			const apiPatch = await this.apiPatchService.getPatch(cloudBranchId, changeId);
			if (apiPatch) {
				this.#apiPatch.set({ state: 'found', value: apiPatch });
			} else {
				this.#apiPatch.set({ state: 'not-found' });
			}
		} else {
			this.#apiPatch.set({ state: 'uninitialized' });
		}
	}

	async update({ signOff, sectionOrder }: CloudPatchUpdate): Promise<void> {
		const cloudBranchId = get(this.cloudBranchId);
		const changeId = get(this.changeId);
		const canUpdatePatch = get(this.apiPatchService.canUpdatePatch);

		if (cloudBranchId && changeId && canUpdatePatch) {
			const apiPatch = await this.apiPatchService.updatePatch(cloudBranchId, changeId, {
				sign_off: signOff,
				section_order: sectionOrder
			});
			if (apiPatch) {
				this.#apiPatch.set({ state: 'found', value: apiPatch });
			} else {
				this.#apiPatch.set({ state: 'not-found' });
			}
		} else {
			this.#apiPatch.set({ state: 'uninitialized' });
		}
	}
}
