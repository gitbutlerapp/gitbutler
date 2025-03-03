import { InterestStore } from '$lib/interest/interestStore';
import { upsertPatchIdable } from '$lib/patches/patchIdablesSlice';
import { upsertPatchSections } from '$lib/patches/patchSectionsSlice';
import { apiToPatch, apiToSection, type ApiPatchIdable } from '$lib/patches/types';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

type PatchIdableParams = {
	projectUuid: string;
	branchId: string;
	changeId: string;
	oldVersion?: number;
	newVersion: number;
};

export class PatchIdableService {
	// We don't want to specify a polling frequency, because diffs are constat data.
	private readonly patchIntrests = new InterestStore<{
		branchId: string;
		oldVersion?: number;
		newVersion: number;
	}>();

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getPatchIdableInterest({
		projectUuid,
		branchId,
		changeId,
		oldVersion,
		newVersion
	}: PatchIdableParams) {
		return this.patchIntrests
			.findOrCreateSubscribable({ branchId, oldVersion, newVersion }, async () => {
				try {
					let queryString = `?v_new=${newVersion}`;
					if (oldVersion) {
						queryString += `&v_old=${oldVersion}`;
					}

					const apiPatch = await this.httpClient.get<ApiPatchIdable>(
						`interdiff/${projectUuid}/branch/${branchId}/change/${changeId}${queryString}`
					);

					const patch = apiToPatch(apiPatch);

					// This will always be here, but this makes the typescript
					// compiler happy
					if (apiPatch.sections) {
						const sections = apiPatch.sections.map(apiToSection);
						this.appDispatch.dispatch(upsertPatchSections(sections));
					}

					this.appDispatch.dispatch(
						upsertPatchIdable({ status: 'found', id: patch.patchId, value: patch })
					);
				} catch (_: unknown) {
					/* hahaha */
				}
			})
			.createInterest();
	}
}
