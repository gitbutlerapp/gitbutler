import { branchReviewListingTable } from '$lib/branches/branchReviewListingsSlice';
import { branchTable } from '$lib/branches/branchesSlice';
import {
	apiToBranch,
	branchReviewListingKey,
	BranchStatus,
	type ApiBranch,
	type Branch,
	type LoadableBranch
} from '$lib/branches/types';
import { InterestStore, type Interest } from '$lib/interest/interestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { patchCommitTable } from '$lib/patches/patchCommitsSlice';
import { apiToPatch, type LoadablePatchCommit } from '$lib/patches/types';
import { POLLING_GLACIALLY, POLLING_REGULAR } from '$lib/polling';
import { InjectionToken } from '@gitbutler/core/context';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

type BranchUpdateParams = {
	status?: BranchStatus.Active | BranchStatus.Closed;
	title?: string;
	description?: string;
	forgeUrl?: string;
	forgeDescription?: string;
};

export const BRANCH_SERVICE: InjectionToken<BranchService> = new InjectionToken('BranchService');

export class BranchService {
	private readonly branchesInterests = new InterestStore<{
		ownerSlug: string;
		projectSlug: string;
		branchStatus: BranchStatus;
	}>(POLLING_GLACIALLY);
	private readonly branchInterests = new InterestStore<{ uuid: string }>(POLLING_REGULAR);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getBranchesInterest(
		ownerSlug: string,
		projectSlug: string,
		branchStatus: BranchStatus = BranchStatus.All
	): Interest {
		return this.branchesInterests
			.findOrCreateSubscribable({ ownerSlug, projectSlug, branchStatus }, async () => {
				this.appDispatch.dispatch(
					branchReviewListingTable.addOne({
						id: branchReviewListingKey(ownerSlug, projectSlug, branchStatus),
						status: 'loading'
					})
				);
				try {
					const apiBranches = await this.httpClient.get<ApiBranch[]>(
						`patch_stack/${ownerSlug}/${projectSlug}?status=${branchStatus}`
					);

					const branches = apiBranches.map(
						(api): LoadableBranch => ({
							status: 'found',
							id: api.uuid,
							value: apiToBranch(api)
						})
					);

					const patches = apiBranches
						.flatMap((branch) => branch.patches)
						.map((api): LoadablePatchCommit => {
							if (!api) return { status: 'not-found', id: '' };
							return {
								status: 'found',
								id: api.change_id,
								value: apiToPatch(api)
							};
						});

					this.appDispatch.dispatch(patchCommitTable.upsertMany(patches));
					this.appDispatch.dispatch(branchTable.upsertMany(branches));
					this.appDispatch.dispatch(
						branchReviewListingTable.upsertOne({
							id: branchReviewListingKey(ownerSlug, projectSlug, branchStatus),
							status: 'found',
							value: apiBranches.map((branch) => branch.uuid)
						})
					);
				} catch (error: unknown) {
					this.appDispatch.dispatch(
						branchReviewListingTable.addOne(
							errorToLoadable(error, branchReviewListingKey(ownerSlug, projectSlug, branchStatus))
						)
					);
				}
			})
			.createInterest();
	}

	async getBranch(uuid: string): Promise<Branch | undefined> {
		try {
			const apiBranch = await this.httpClient.get<ApiBranch>(`patch_stack/${uuid}`);
			const loadableBranch: LoadableBranch = {
				status: 'found',
				id: apiBranch.uuid,
				value: apiToBranch(apiBranch)
			};

			const patches = apiBranch.patches?.map(
				(api): LoadablePatchCommit => ({
					status: 'found',
					id: api.change_id,
					value: apiToPatch(api)
				})
			);
			this.appDispatch.dispatch(branchTable.upsertOne(loadableBranch));
			if (patches) {
				this.appDispatch.dispatch(patchCommitTable.upsertMany(patches));
			}

			return apiToBranch(apiBranch);
		} catch (_: unknown) {
			/* empty */
		}
	}

	getBranchInterest(uuid: string): Interest {
		return this.branchInterests
			.findOrCreateSubscribable({ uuid }, async () => {
				this.appDispatch.dispatch(branchTable.addOne({ status: 'loading', id: uuid }));
				try {
					const apiBranch = await this.httpClient.get<ApiBranch>(`patch_stack/${uuid}`);
					const branch: LoadableBranch = {
						status: 'found',
						id: apiBranch.uuid,
						value: apiToBranch(apiBranch)
					};

					const patches = apiBranch.patches?.map(
						(api): LoadablePatchCommit => ({
							status: 'found',
							id: api.change_id,
							value: apiToPatch(api)
						})
					);
					this.appDispatch.dispatch(branchTable.upsertOne(branch));
					if (patches) {
						this.appDispatch.dispatch(patchCommitTable.upsertMany(patches));
					}
				} catch (error: unknown) {
					this.appDispatch.dispatch(branchTable.addOne(errorToLoadable(error, uuid)));
				}
			})
			.createInterest();
	}

	async refreshBranch(uuid: string) {
		await this.branchInterests.invalidate({ uuid });
	}

	async updateBranch(uuid: string, params: BranchUpdateParams): Promise<Branch> {
		const apiBranch = await this.httpClient.patch<ApiBranch>(`patch_stack/${uuid}`, {
			body: {
				status: params.status,
				title: params.title,
				description: params.description,
				forge_url: params.forgeUrl,
				forge_description: params.forgeDescription
			}
		});
		const branch = apiToBranch(apiBranch);

		const patches = apiBranch.patches?.map(
			(api): LoadablePatchCommit => ({ status: 'found', id: api.change_id, value: apiToPatch(api) })
		);

		this.appDispatch.dispatch(
			branchTable.upsertOne({
				status: 'found',
				id: branch.uuid,
				value: branch
			})
		);
		this.appDispatch.dispatch(patchCommitTable.upsertMany(patches ?? []));

		return branch;
	}
}
