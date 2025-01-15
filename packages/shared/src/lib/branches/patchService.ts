import { upsertPatchSections } from '$lib/branches/patchSectionsSlice';
import { addPatch, upsertPatch } from '$lib/branches/patchesSlice';
import { apiToPatch, apiToSection, type ApiPatch, type Patch } from '$lib/branches/types';
import { InterestStore, type Interest } from '$lib/interest/interestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { POLLING_REGULAR } from '$lib/polling';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

type PatchUpdateParams = {
	signOff?: boolean;
	sectionOrder?: string[];
};

export class PatchService {
	private readonly patchInterests = new InterestStore<{ changeId: string }>(POLLING_REGULAR);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getPatchWithSectionsInterest(branchUuid: string, changeId: string): Interest {
		return this.patchInterests
			.findOrCreateSubscribable({ changeId }, async () => {
				this.appDispatch.dispatch(addPatch({ status: 'loading', id: changeId }));
				try {
					console.log(branchUuid, changeId);
					const apiPatch = await this.httpClient.get<ApiPatch>(
						`patch_stack/${branchUuid}/patch/${changeId}`
					);

					const patch = apiToPatch(apiPatch);

					// This will always be here, but this makes the typescript
					// compiler happy
					if (apiPatch.sections) {
						const sections = apiPatch.sections.map(apiToSection);
						this.appDispatch.dispatch(upsertPatchSections(sections));
					}

					this.appDispatch.dispatch(upsertPatch({ status: 'found', id: changeId, value: patch }));
				} catch (error: unknown) {
					this.appDispatch.dispatch(upsertPatch(errorToLoadable(error, changeId)));
				}
			})
			.createInterest();
	}

	async updatePatch(
		branchUuid: string,
		changeId: string,
		params: PatchUpdateParams
	): Promise<Patch> {
		const apiPatch = await this.httpClient.patch<ApiPatch>(
			`patch_stack/${branchUuid}/patch/${changeId}`,
			{
				body: {
					sign_off: params.signOff,
					section_order: params.sectionOrder
				}
			}
		);

		const patch = apiToPatch(apiPatch);
		this.appDispatch.dispatch(upsertPatch({ status: 'found', id: changeId, value: patch }));

		// This will always be here, but this makes the typescript
		// compiler happy
		if (apiPatch.sections) {
			const sections = apiPatch.sections.map(apiToSection);
			this.appDispatch.dispatch(upsertPatchSections(sections));
		}

		return patch;
	}
}
