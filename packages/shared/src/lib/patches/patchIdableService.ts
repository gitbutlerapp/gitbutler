import { InterestStore } from '$lib/interest/interestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { patchIdableTable } from '$lib/patches/patchIdablesSlice';
import { upsertPatchSections } from '$lib/patches/patchSectionsSlice';
import { apiToPatch, apiToSection, patchIdableId, type ApiPatchIdable } from '$lib/patches/types';
import { InjectionToken } from '../context';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

type PatchIdableParams = {
	branchUuid: string;
	changeId: string;
	oldVersion?: number;
	newVersion: number;
};

export const PATCH_IDABLE_SERVICE_TOKEN = new InjectionToken<PatchIdableService>('PatchIdableService');

export class PatchIdableService {
	// We don't want to specify a polling frequency, because diffs are constat data.
	private readonly patchIntrests = new InterestStore<{
		branchUuid: string;
		changeId: string;
		oldVersion?: number;
		newVersion: number;
	}>();

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getPatchIdableInterest({ branchUuid, changeId, oldVersion, newVersion }: PatchIdableParams) {
		return this.patchIntrests
			.findOrCreateSubscribable({ branchUuid, changeId, oldVersion, newVersion }, async () => {
				const key = patchIdableId({ branchUuid, changeId, oldVersion, newVersion });
				this.appDispatch.dispatch(patchIdableTable.addOne({ status: 'loading', id: key }));

				try {
					let queryString = `?version_new=${newVersion}`;
					if (oldVersion) {
						queryString += `&version_old=${oldVersion}`;
					}

					const apiPatch = await this.httpClient.get<ApiPatchIdable>(
						`diff/${branchUuid}/change/${changeId}${queryString}`
					);

					const patch = apiToPatch(apiPatch);

					// This will always be here, but this makes the typescript
					// compiler happy
					if (apiPatch.sections) {
						const sections = apiPatch.sections.map(apiToSection);
						this.appDispatch.dispatch(upsertPatchSections(sections));
					}

					this.appDispatch.dispatch(
						patchIdableTable.upsertOne({ status: 'found', id: key, value: patch })
					);
				} catch (error: unknown) {
					this.appDispatch.dispatch(patchIdableTable.addOne(errorToLoadable(error, key)));
				}
			})
			.createInterest();
	}
}
