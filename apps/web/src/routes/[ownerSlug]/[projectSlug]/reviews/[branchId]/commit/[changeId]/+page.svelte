<script lang="ts">
	import ChatComponent from '$lib/components/ChatComponent.svelte';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { PatchService } from '@gitbutler/shared/branches/patchService';
	import { getPatch } from '@gitbutler/shared/branches/patchesPreview.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { isFound } from '@gitbutler/shared/network/loadable';
	import { lookupProject } from '@gitbutler/shared/organizations/repositoryIdLookupPreview.svelte';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import type { ProjectReviewCommitParameters } from '$lib/project/types';

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

	const branchUuid = $derived(isFound(branch?.current) ? branch.current.value.uuid : undefined);

	const change = $derived(
		branchUuid !== undefined
			? getPatch(appState, patchService, branchUuid, data.changeId)
			: undefined
	);
</script>

<div class="review-page">
	<Loading loadable={change?.current}>
		{#snippet children(change)}
			<div class="review-main-content">
				<h3 class="review-main-content-title">{change.title}</h3>
				<p>{change.description}</p>
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
	}

	.review-main-content-title {
		color: var(--text-1, #1a1614);

		font-family: var(--font-family-default, Inter);
		font-size: 18px;
		font-style: normal;
		font-weight: var(--weight-bold, 600);
		line-height: 120%; /* 21.6px */
	}

	.review-chat {
		width: 100%;
	}
</style>
