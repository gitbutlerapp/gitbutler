<script lang="ts">
	import ChatComponent from '$lib/components/ChatComponent.svelte';
	import ChangeStatus from '$lib/components/changes/ChangeStatus.svelte';
	import ChangeActionButton from '$lib/components/review/ChangeActionButton.svelte';
	import ChangeNavigator from '$lib/components/review/ChangeNavigator.svelte';
	import Section from '$lib/components/review/Section.svelte';
	import { projectReviewBranchCommitPath, type ProjectReviewCommitParameters } from '$lib/routing';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { PatchService } from '@gitbutler/shared/branches/patchService';
	import { getPatch, getPatchSections } from '@gitbutler/shared/branches/patchesPreview.svelte';
	import {
		getPatchContributorsWithAvatars,
		getPatchReviewersWithAvatars
	} from '@gitbutler/shared/branches/types';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { compose, dig, isFound } from '@gitbutler/shared/network/loadable';
	import { lookupProject } from '@gitbutler/shared/organizations/repositoryIdLookupPreview.svelte';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';

	const BRANCH_TITLE_PLACE_HOLDER = 'No branch title provided';
	const DESCRIPTION_PLACE_HOLDER = 'No description provided';
	const NO_REVIEWERS = 'Not reviewed yet';
	const NO_CONTRIBUTORS = 'No contributors';
	const NO_COMMENTS = 'No comments yet';

	interface Props {
		data: ProjectReviewCommitParameters;
	}

	let { data }: Props = $props();

	const repositoryIdLookupService = getContext(RepositoryIdLookupService);
	const branchService = getContext(BranchService);
	const patchService = getContext(PatchService);
	const appState = getContext(AppState);

	const repositoryId = $derived(
		lookupProject(appState, repositoryIdLookupService, data.ownerSlug, data.projectSlug)
	);

	const branch = $derived(
		isFound(repositoryId.current)
			? getBranchReview(appState, branchService, repositoryId.current.value, data.branchId)
			: undefined
	);

	const patchIds = $derived(dig(branch?.current, (b) => b.patchIds));
	const branchUuid = $derived(dig(branch?.current, (b) => b.uuid));
	const branchName = $derived(dig(branch?.current, (b) => b.title) ?? BRANCH_TITLE_PLACE_HOLDER);

	const patch = $derived(
		branchUuid !== undefined
			? getPatch(appState, patchService, branchUuid, data.changeId)
			: undefined
	);

	const patchSections = $derived(
		branchUuid !== undefined
			? getPatchSections(appState, patchService, branchUuid, data.changeId)
			: undefined
	);

	const contributors = $derived(
		isFound(patch?.current)
			? getPatchContributorsWithAvatars(patch.current.value)
			: Promise.resolve([])
	);

	const reviewers = $derived(
		isFound(patch?.current)
			? getPatchReviewersWithAvatars(patch.current.value)
			: Promise.resolve([])
	);

	function goToPatch(changeId: string) {
		const url = projectReviewBranchCommitPath({
			ownerSlug: data.ownerSlug,
			projectSlug: data.projectSlug,
			branchId: data.branchId,
			changeId
		});

		window.location.href = url;
	}
</script>

<div class="review-page">
	<Loading loadable={compose(patch?.current, repositoryId.current)}>
		{#snippet children([patch, repositoryId])}
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

				<div class="review-main-content-info">
					<div class="review-main-content-info__entry">
						<p class="review-main-content-info__header">Status:</p>
						<ChangeStatus {patch} />
					</div>

					<div class="review-main-content-info__entry">
						<p class="review-main-content-info__header">Reviewed by:</p>
						<div>
							{#await reviewers then reviewers}
								{#if reviewers.length === 0}
									<p class="review-main-content-info__value">{NO_REVIEWERS}</p>
								{:else}
									<AvatarGroup avatars={reviewers}></AvatarGroup>
								{/if}
							{/await}
						</div>
					</div>

					<div class="review-main-content-info__entry">
						<p class="review-main-content-info__header">Commented by:</p>
						<p class="review-main-content-info__value">{NO_COMMENTS}</p>
					</div>

					<div class="review-main-content-info__entry">
						<p class="review-main-content-info__header">Authors:</p>
						<div>
							{#await contributors then contributors}
								{#if contributors.length === 0}
									<p class="review-main-content-info__value">{NO_CONTRIBUTORS}</p>
								{:else}
									<AvatarGroup avatars={contributors}></AvatarGroup>
								{/if}
							{/await}
						</div>
					</div>
				</div>

				<pre>{JSON.stringify(patch.statistics)}</pre>

				{#if patchSections?.current !== undefined}
					{#each patchSections.current as section}
						<Section {section} />
					{/each}
				{/if}
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

	.review-main-content-info {
		display: flex;
		gap: 30px;
	}

	.review-main-content-info__entry {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.review-main-content-info__header {
		overflow: hidden;
		color: var(--text-2, #867e79);
		text-overflow: ellipsis;

		/* base/12 */
		font-family: var(--font-family-default, Inter);
		font-size: 12px;
		font-style: normal;
		font-weight: var(--weight-regular, 400);
		line-height: 120%; /* 14.4px */
	}

	.review-main-content-info__value {
		overflow: hidden;
		color: var(--text-3, #b4afac);
		text-overflow: ellipsis;

		/* base/12 */
		font-family: var(--font-family-default, Inter);
		font-size: 12px;
		font-style: normal;
		font-weight: var(--weight-regular, 400);
		line-height: 120%; /* 14.4px */
	}

	.review-chat {
		width: 100%;
	}
</style>
