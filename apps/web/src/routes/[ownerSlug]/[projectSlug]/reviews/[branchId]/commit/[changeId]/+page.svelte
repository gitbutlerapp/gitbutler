<script lang="ts">
	import ChatComponent from '$lib/components/ChatComponent.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { lookupProject } from '@gitbutler/shared/organizations/repositoryIdLookupPreview.svelte';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import type { ProjectReviewCommitParameters } from '$lib/project/types';

	interface Props {
		data: ProjectReviewCommitParameters;
	}

	let { data }: Props = $props();

	const repositoryIdLookupService = getContext(RepositoryIdLookupService);
	const appState = getContext(AppState);

	const repositoryId = $derived(
		lookupProject(appState, repositoryIdLookupService, data.ownerSlug, data.projectSlug)
	);
</script>

<Loading loadable={repositoryId.current}>
	{#snippet children(repositoryId)}
		<h2>Review page: {data.ownerSlug}/{data.projectSlug} {data.branchId}/{data.changeId}</h2>

		<div class="review-page">
			<div class="review-main-content">the main area with the diffs</div>
			<div class="review-chat">
				<ChatComponent projectId={repositoryId} branchId={data.branchId} changeId={data.changeId} />
			</div>
		</div>
	{/snippet}
</Loading>

<style>
	.review-page {
		display: flex;
		width: 100%;
		flex-grow: 1;
	}

	.review-main-content {
		width: 100%;
	}

	.review-chat {
		width: 100%;
	}
</style>
