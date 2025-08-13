<script lang="ts">
	import { goto } from '$app/navigation';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/shared/context';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Button, Icon } from '@gitbutler/ui';

	const routes = inject(WEB_ROUTES_SERVICE);
	// get user's project page params
	const userService = inject(USER_SERVICE);
	const user = $derived(userService.user);

	function getRootLabel() {
		if ($user?.login === routes.isProjectReviewBranchPageSubset?.ownerSlug) {
			return 'My Projects';
		} else {
			return 'My Reviews';
		}
	}
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
			<span class="text-15 text-bold">Dashboard </span>
		{:else}
			<Button kind="ghost" onclick={() => goto(routes.projectsPath())} tooltip="Go to Dashboard">
				<span class="text-15 text-bold truncate breadcrumbs__path-label">
					{getRootLabel()} <span>/</span>
					{routes.isProjectReviewPageSubset?.ownerSlug}</span
				>
			</Button>
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

<style lang="postcss">
	.breadcrumbs {
		display: flex;
		flex-wrap: nowrap;
		align-items: center;
		overflow: hidden;
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
		overflow: hidden;
		gap: 4px;
	}

	.breadcrumbs__path-label > span {
		margin: 0 2px;
		opacity: 0.2;
	}

	.breadcrumbs__back-btn {
		display: flex;
		position: relative;
		align-items: center;
		height: var(--size-button);
		padding: 0 8px;
		gap: 4px;

		&:before {
			width: 1px;
			height: 18px;
			margin: 0 8px 0 0;
			background-color: var(--clr-border-2);
			content: '';
			transition: opacity 0.2s;
		}

		&:hover {
			.breadcrumbs__back-btn__icon {
				transform: translateX(-2px);
				opacity: 1;
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
