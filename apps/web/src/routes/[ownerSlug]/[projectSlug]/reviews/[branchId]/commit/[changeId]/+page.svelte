<script lang="ts">
	import ChatComponent from '$lib/components/ChatComponent.svelte';
	import ChangeActionButton from '$lib/components/review/ChangeActionButton.svelte';
	import ChangeNavigator from '$lib/components/review/ChangeNavigator.svelte';
	import ReviewInfo from '$lib/components/review/ReviewInfo.svelte';
	import ReviewSections from '$lib/components/review/ReviewSections.svelte';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { lookupLatestBranchUuid } from '@gitbutler/shared/branches/latestBranchLookup.svelte';
	import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
	import { PatchService } from '@gitbutler/shared/branches/patchService';
	import { getPatch, getPatchSections } from '@gitbutler/shared/branches/patchesPreview.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { combine, map } from '@gitbutler/shared/network/loadable';
	import { lookupProject } from '@gitbutler/shared/organizations/repositoryIdLookupPreview.svelte';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import {
		WebRoutesService,
		type ProjectReviewCommitParameters
	} from '@gitbutler/shared/routing/webRoutes.svelte';

	const BRANCH_TITLE_PLACE_HOLDER = 'No branch title provided';
	const DESCRIPTION_PLACE_HOLDER = 'No description provided';

	interface Props {
		data: ProjectReviewCommitParameters;
	}

	let { data }: Props = $props();

	const repositoryIdLookupService = getContext(RepositoryIdLookupService);
	const latestBranchLookupService = getContext(LatestBranchLookupService);
	const branchService = getContext(BranchService);
	const patchService = getContext(PatchService);
	const appState = getContext(AppState);
	const routes = getContext(WebRoutesService);

	const repositoryId = $derived(
		lookupProject(appState, repositoryIdLookupService, data.ownerSlug, data.projectSlug)
	);

	const branchUuid = $derived(
		lookupLatestBranchUuid(
			appState,
			latestBranchLookupService,
			data.ownerSlug,
			data.projectSlug,
			data.branchId
		)
	);

	const branch = $derived(
		map(branchUuid?.current, (branchUuid) => {
			return getBranchReview(appState, branchService, branchUuid);
		})
	);

	const patchIds = $derived(map(branch?.current, (b) => b.patchIds));
	const branchName = $derived(map(branch?.current, (b) => b.title) ?? BRANCH_TITLE_PLACE_HOLDER);

	const patch = $derived(
		map(branchUuid?.current, (branchUuid) => {
			return getPatch(appState, patchService, branchUuid, data.changeId);
		})
	);

	const patchSections = $derived(
		map(branchUuid?.current, (branchUuid) => {
			return getPatchSections(appState, patchService, branchUuid, data.changeId);
		})
	);

	function goToPatch(changeId: string) {
		const url = routes.projectReviewBranchCommitPath({
			ownerSlug: data.ownerSlug,
			projectSlug: data.projectSlug,
			branchId: data.branchId,
			changeId
		});

		window.location.href = url;
	}
</script>

<div class="review-page">
	<Loading loadable={combine([patch?.current, repositoryId.current, branchUuid?.current])}>
		{#snippet children([patch, repositoryId, branchUuid])}
			<div class="review-main-content">
				<div class="review-main__header">
					<p class="review-main__branch-title-line">
						Branch: <span class="review-main__branch-title">{branchName}</span>
					</p>

					<h3 class="review-main-content-title">{patch.title}</h3>

					<div class="review-main-content__patch-navigator">
						{#if patchIds !== undefined}
							<ChangeNavigator {goToPatch} currentPatchId={patch.changeId} {patchIds} />
						{/if}

						{#if branchUuid !== undefined}
							<ChangeActionButton {branchUuid} {patch} />
						{/if}
					</div>
				</div>

				<p class="review-main-content-description">
					{patch.description?.trim() || DESCRIPTION_PLACE_HOLDER}
				</p>

				<ReviewInfo {patch} />
				<ReviewSections {patch} patchSections={patchSections?.current} />
			</div>

			<div class="review-chat">
				<ChatComponent projectId={repositoryId} branchId={data.branchId} changeId={data.changeId} />
			</div>
		{/snippet}
	</Loading>
</div>

<style>
	.review-page {
		display: flex;
		width: 100%;
		flex-grow: 1;
		gap: 20px;
	}

	.review-main-content {
		display: flex;
		flex-direction: column;
		gap: 24px;
		width: 100%;
		max-width: 50%;
	}

	.review-main__header {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.review-main__branch-title-line {
		color: var(--text-3, #b4afac);

		/* base/12 */
		font-family: var(--font-family-default, Inter);
		font-size: 12px;
		font-style: normal;
		font-weight: var(--weight-regular, 400);
		line-height: 120%; /* 14.4px */
	}

	.review-main__branch-title {
		color: var(--text-1, #1a1614);

		/* base/12 */
		font-family: var(--font-family-default, Inter);
		font-size: 12px;
		font-style: normal;
		font-weight: var(--weight-regular, 400);
		line-height: 120%;
	}

	.review-main-content-title {
		color: var(--text-1, #1a1614);

		font-family: var(--font-family-default, Inter);
		font-size: 18px;
		font-style: normal;
		font-weight: var(--weight-bold, 600);
		line-height: 120%; /* 21.6px */
	}

	.review-main-content__patch-navigator {
		display: flex;
		gap: 6px;
	}

	.review-main-content-description {
		color: var(--text-1, #1a1614);
		font-family: var(--font-family-mono, 'Geist Mono');
		font-size: 12px;
		font-style: normal;
		font-weight: var(--weight-regular, 400);
		line-height: 160%; /* 19.2px */
	}

	.review-chat {
		width: 100%;
		--top-nav-offset: 84px;
		--bottom-margin: 10px;
		top: var(--top-nav-offset);
		display: flex;
		height: calc(100vh - var(--top-nav-offset) - var(--bottom-margin));
		position: sticky;
	}
</style>
