<script lang="ts">
	import ChatComponent from '$lib/components/ChatComponent.svelte';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { PatchService } from '@gitbutler/shared/branches/patchService';
	import { getPatch, getPatchSections } from '@gitbutler/shared/branches/patchesPreview.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { dig, isFound } from '@gitbutler/shared/network/loadable';
	import { lookupProject } from '@gitbutler/shared/organizations/repositoryIdLookupPreview.svelte';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import type { ProjectReviewCommitParameters } from '$lib/project/types';

	const DESCRIPTION_PLACE_HOLDER = 'No description provided';

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
</script>

<div class="review-page">
	<Loading loadable={patch?.current}>
		{#snippet children(patch)}
			<div class="review-main-content">
				<h3 class="review-main-content-title">{patch.title}</h3>
				<div>
					<p>{patchIds?.length}</p>
				</div>

				<p class="review-main-content-description">
					{patch.description?.trim() || DESCRIPTION_PLACE_HOLDER}
				</p>

				<div class="review-main-content-info">
					<p>Contributors: {patch.contributors.join(', ')}</p>
					<p>Created: {patch.createdAt}</p>
					<pre>{JSON.stringify(patch.review)}</pre>
				</div>

				<div>
					<pre>{JSON.stringify(patchSections?.current, null, 2)}</pre>
				</div>
			</div>
		{/snippet}
	</Loading>

	<Loading loadable={repositoryId.current}>
		{#snippet children(repositoryId)}
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
	}

	.review-main-content {
		width: 100%;
		max-width: 50%;
	}

	.review-main-content-title {
		color: var(--text-1, #1a1614);

		font-family: var(--font-family-default, Inter);
		font-size: 18px;
		font-style: normal;
		font-weight: var(--weight-bold, 600);
		line-height: 120%; /* 21.6px */
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
	}
</style>
