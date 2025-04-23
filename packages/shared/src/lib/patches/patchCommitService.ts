import { InterestStore, type Interest } from '$lib/interest/interestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { patchCommitTable } from '$lib/patches/patchCommitsSlice';
import { upsertPatchSections } from '$lib/patches/patchSectionsSlice';
import { apiToPatch, apiToSection, type ApiPatchCommit, type Patch } from '$lib/patches/types';
import { POLLING_REGULAR } from '$lib/polling';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

type PatchUpdateParams = {
	signOff?: boolean;
	sectionOrder?: string[];
	message?: string;
};

export class PatchCommitService {
	private readonly patchInterests = new InterestStore<{ changeId: string }>(POLLING_REGULAR);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getPatchWithSectionsInterest(branchUuid: string, changeId: string): Interest {
		return this.patchInterests
			.findOrCreateSubscribable({ changeId }, async () => {
				this.appDispatch.dispatch(patchCommitTable.addOne({ status: 'loading', id: changeId }));
				try {
					const apiPatch = await this.httpClient.get<ApiPatchCommit>(
						`patch_stack/${branchUuid}/patch/${changeId}`
					);

					const patch = apiToPatch(apiPatch);

					// This will always be here, but this makes the typescript
					// compiler happy
					if (apiPatch.sections) {
						const sections = apiPatch.sections.map(apiToSection);
						this.appDispatch.dispatch(upsertPatchSections(sections));
					}

					this.appDispatch.dispatch(
						patchCommitTable.upsertOne({ status: 'found', id: changeId, value: patch })
					);
				} catch (error: unknown) {
					this.appDispatch.dispatch(patchCommitTable.addOne(errorToLoadable(error, changeId)));
				}
			})
			.createInterest();
	}

	async refreshPatchWithSections(changeId: string) {
		await this.patchInterests.invalidate({ changeId });
	}

	async updatePatch(
		branchUuid: string,
		changeId: string,
		params: PatchUpdateParams
	): Promise<Patch> {
		const apiPatch = await this.httpClient.patch<ApiPatchCommit>(
			`patch_stack/${branchUuid}/patch/${changeId}`,
			{
				body: {
					sign_off: params.signOff,
					section_order: params.sectionOrder,
					message: params.message
				}
			}
		);

		const patch = apiToPatch(apiPatch);
		this.appDispatch.dispatch(
			patchCommitTable.upsertOne({ status: 'found', id: changeId, value: patch })
		);

		// This will always be here, but this makes the typescript
		// compiler happy
		if (apiPatch.sections) {
			const sections = apiPatch.sections.map(apiToSection);
			this.appDispatch.dispatch(upsertPatchSections(sections));
		}

		return patch;
	}
}
