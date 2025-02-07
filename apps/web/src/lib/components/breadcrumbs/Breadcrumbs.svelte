<script lang="ts">
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { lookupLatestBranchUuid } from '@gitbutler/shared/branches/latestBranchLookup.svelte';
	import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
	import { getContext } from '@gitbutler/shared/context';
	import { map } from '@gitbutler/shared/network/loadable';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';

	const appState = getContext(AppState);
	const branchService = getContext(BranchService);
	const routes = getContext(WebRoutesService);
	const latestBranchLookupService = getContext(LatestBranchLookupService);

	const branchUuid = $derived.by(() => {
		if (!routes.isProjectReviewBranchPageSubset) return;
		const ownerSlug = routes.isProjectReviewBranchPageSubset.ownerSlug;
		const projectSlug = routes.isProjectReviewBranchPageSubset.projectSlug;
		const branchId = routes.isProjectReviewBranchPageSubset.branchId;
		return lookupLatestBranchUuid(
			appState,
			latestBranchLookupService,
			ownerSlug,
			projectSlug,
			branchId
		);
	});

	const branch = $derived(
		map(branchUuid?.current, (branchUuid) => getBranchReview(appState, branchService, branchUuid))
	);
</script>

<ol class="breadcrumbs">
	<li class="text-12 text-semibold breadcrumb-item">
		<a
			class:breadcrumb-item_disabled={!routes.isProjectReviewPageSubset}
			href={routes.projectsPath()}
		>
			All projects
		</a>
	</li>

	<span class="text-12 text-semibold nav-slash">/</span>
	{#if routes.isProjectReviewPageSubset}
		<li class="text-12 text-semibold breadcrumb-item truncate">
			<a
				class:breadcrumb-item_disabled={!routes.isProjectReviewBranchPageSubset}
				href={routes.isProjectPageSubset
					? `${routes.projectPath(routes.isProjectReviewPageSubset)}/reviews`
					: ''}
				>{routes.isProjectReviewPageSubset.ownerSlug}/{routes.isProjectReviewPageSubset
					.projectSlug}</a
			>
		</li>

		<span class="text-12 text-semibold nav-slash">/</span>
	{/if}
	{#if routes.isProjectReviewBranchCommitPageSubset}
		<li class="text-12 text-semibold breadcrumb-item truncate">
			{#if branch?.current && branch.current.status === 'found'}
				<a href={routes.projectReviewBranchPath(routes.isProjectReviewBranchCommitPageSubset)}>
					{branch.current.value.title}
				</a>
			{:else}
				<span class="breadcrumb-item_disabled">...</span>
			{/if}
		</li>
	{/if}
</ol>

<style lang="postcss">
	.breadcrumbs {
		display: flex;
		flex-wrap: nowrap;
		align-items: center;
		overflow: hidden;
		gap: 8px;
	}

	.breadcrumb-item {
		color: var(--clr-text-1);
		white-space: nowrap;

		a:hover {
			text-decoration: underline;
		}
	}

	.breadcrumb-item_disabled {
		color: var(--clr-text-3);
		pointer-events: none;
	}

	.nav-slash {
		color: var(--clr-text-3);
	}
</style>
