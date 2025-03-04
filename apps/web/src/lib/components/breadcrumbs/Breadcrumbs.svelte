<script lang="ts">
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { lookupLatestBranchUuid } from '@gitbutler/shared/branches/latestBranchLookup.svelte';
	import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
	import { getContext } from '@gitbutler/shared/context';
	import { map } from '@gitbutler/shared/network/loadable';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';

	interface Props {
		isUserLoggedIn: boolean;
	}

	let { isUserLoggedIn }: Props = $props();

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

{#snippet backButton({ href, label = 'Back' }: { href: string; label: string })}
	<a {href} class="breadcrumbs__back-btn">
		<div class="breadcrumbs__back-btn__icon">
			<Icon name="chevron-left" />
		</div>
		<span class="text-12 text-semibold">
			{label}
		</span>
	</a>
{/snippet}

<div class="breadcrumbs">
	<div class="breadcrumbs__path">
		{#if !routes.isProjectReviewBranchPageSubset}
			<span class="text-15 text-bold"> Dashboard </span>
		{:else}
			<span class="text-15 text-bold truncate"> My projects </span>
			<span class="text-14 text-bold breadcrumbs_slash">/</span>
			<span class="text-15 text-bold truncate">{routes.isProjectReviewPageSubset?.ownerSlug}</span>
		{/if}
	</div>

	{#if routes.isProjectReviewBranchCommitPageSubset}
		{@render backButton({
			label: 'Back',
			href: routes.projectReviewBranchPath(routes.isProjectReviewBranchCommitPageSubset)
		})}
	{:else if routes.isProjectReviewBranchPageSubset}
		{@render backButton({
			label: 'Back',
			href: `${routes.projectPath(routes.isProjectReviewBranchPageSubset)}/reviews`
		})}
	{/if}
</div>

<!-- function getBackButtonHref() {
	if (routes.isProjectReviewBranchCommitPageSubset) {
		return routes.projectReviewBranchPath(routes.isProjectReviewBranchCommitPageSubset);
	}

	if (routes.isProjectReviewPageSubset) {
		console.log('routes.isProjectReviewBranchPageSubset', routes.isProjectReviewBranchPageSubset);
		return `${routes.projectPath(routes.isProjectReviewPageSubset)}/reviews`;
	}
} -->

<!-- <ol class="breadcrumbs">
	<li class="text-12 text-semibold breadcrumb-item">
		<a
			class:breadcrumb-item_disabled={!routes.isProjectReviewPageSubset || !isUserLoggedIn}
			href={routes.projectsPath()}
		>
			All projects
		</a>
	</li>

	<span class="text-12 text-semibold nav-slash">/</span>
	{#if routes.isProjectReviewPageSubset}
		<li class="text-12 text-semibold breadcrumb-item truncate">
			<a
				class:breadcrumb-item_disabled={!routes.isProjectReviewBranchPageSubset || !isUserLoggedIn}
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
</ol> -->

<style lang="postcss">
	.breadcrumbs {
		display: flex;
		flex-wrap: nowrap;
		align-items: center;
		overflow: hidden;
		gap: 8px;
		text-wrap: nowrap;

		@container (max-width: 500px) {
			& .breadcrumbs__path {
				display: none;
			}
			& .breadcrumbs__back-btn {
				padding-left: 0;
			}
		}
	}

	.breadcrumbs__path {
		display: flex;
		align-items: center;
		gap: 4px;
		overflow: hidden;
	}

	.breadcrumbs_slash {
		color: var(--clr-text-3);
	}

	.breadcrumbs__back-btn {
		position: relative;
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 0 8px;
		height: var(--size-button);

		&:before {
			content: '';
			width: 1px;
			height: 18px;
			background-color: var(--clr-border-2);
			transition: opacity 0.2s;
			margin: 0 8px 0 0;
		}

		&:hover {
			.breadcrumbs__back-btn__icon {
				opacity: 1;
				transform: translateX(-2px);
			}
		}
	}

	.breadcrumbs__back-btn__icon {
		display: flex;
		opacity: 0.5;
		transition:
			opacity var(--transition-fast),
			transform var(--transition-fast);
	}
</style>
