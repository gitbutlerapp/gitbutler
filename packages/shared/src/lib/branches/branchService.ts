import { addBranch, upsertBranch, upsertBranches } from '$lib/branches/branchesSlice';
import { upsertPatches } from '$lib/branches/patchesSlice';
import {
	apiToBranch,
	apiToPatch,
	BranchStatus,
	type ApiBranch,
	type Branch,
	type LoadableBranch,
	type LoadablePatch
} from '$lib/branches/types';
import { InterestStore, type Interest } from '$lib/interest/intrestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { POLLING_GLACIALLY, POLLING_REGULAR } from '$lib/polling';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

type BranchUpdateParams = {
	status: BranchStatus.Active | BranchStatus.Closed;
	title: string;
	description: string;
};

export class BranchService {
	private readonly branchesInterests = new InterestStore<{
		repositoryId: string;
		branchStatus: BranchStatus;
	}>(POLLING_GLACIALLY);
	private readonly branchInterests = new InterestStore<{ branchId: string }>(POLLING_REGULAR);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getBranchesInterest(
		repositoryId: string,
		branchStatus: BranchStatus = BranchStatus.All
	): Interest {
		return this.branchesInterests
			.findOrCreateSubscribable({ repositoryId, branchStatus }, async () => {
				try {
					const apiBranches = await this.httpClient.get<ApiBranch[]>(
						`patch_stack/${repositoryId}?status=${branchStatus}`
					);
					const branches = apiBranches.map(
						(api): LoadableBranch => ({
							status: 'found',
							id: api.branch_id,
							value: apiToBranch(api)
						})
					);

					const patches = apiBranches
						.flatMap((branch) => branch.patches)
						.map(
							(api): LoadablePatch => ({
								status: 'found',
								id: api.change_id,
								value: apiToPatch(api)
							})
						);

					this.appDispatch.dispatch(upsertBranches(branches));
					this.appDispatch.dispatch(upsertPatches(patches));
				} catch (_error: unknown) {
					/* empty */
				}
			})
			.createInterest();
	}

	getBranchInterest(repositoryId: string, branchId: string): Interest {
		return this.branchInterests
			.findOrCreateSubscribable({ branchId }, async () => {
				this.appDispatch.dispatch(addBranch({ status: 'loading', id: branchId }));
				try {
					const apiBranch = await this.httpClient.get<ApiBranch>(
						`patch_stack/${repositoryId}/${branchId}`
					);
					const branch: LoadableBranch = {
						status: 'found',
						id: apiBranch.branch_id,
						value: apiToBranch(apiBranch)
					};

					const patches = apiBranch.patches.map(
						(api): LoadablePatch => ({
							status: 'found',
							id: api.change_id,
							value: apiToPatch(api)
						})
					);

					this.appDispatch.dispatch(upsertBranch(branch));
					this.appDispatch.dispatch(upsertPatches(patches));
				} catch (error: unknown) {
					this.appDispatch.dispatch(upsertBranch(errorToLoadable(error, branchId)));
				}
			})
			.createInterest();
	}

	async createBranch(repositoryId: string, branchId: string, oplogSha: string): Promise<Branch> {
		const apiBranch = await this.httpClient.post<ApiBranch>(`patch_stack`, {
			body: { branch_id: branchId, oplog_sha: oplogSha, project_id: repositoryId }
		});
		const branch = apiToBranch(apiBranch);

		const patches = apiBranch.patches.map(
			(api): LoadablePatch => ({ status: 'found', id: api.change_id, value: apiToPatch(api) })
		);

		this.appDispatch.dispatch(
			upsertBranch({
				status: 'found',
				id: branch.branchId,
				value: branch
			})
		);
		this.appDispatch.dispatch(upsertPatches(patches));

		return branch;
	}

	async updateBranch(branchId: string, params: BranchUpdateParams): Promise<Branch> {
		const apiBranch = await this.httpClient.put<ApiBranch>(`patch_stack/${branchId}`, {
			body: params
		});
		const branch = apiToBranch(apiBranch);

		const patches = apiBranch.patches.map(
			(api): LoadablePatch => ({ status: 'found', id: api.change_id, value: apiToPatch(api) })
		);

		this.appDispatch.dispatch(
			upsertBranch({
				status: 'found',
				id: branch.branchId,
				value: branch
			})
		);
		this.appDispatch.dispatch(upsertPatches(patches));

		return branch;
	}
}
