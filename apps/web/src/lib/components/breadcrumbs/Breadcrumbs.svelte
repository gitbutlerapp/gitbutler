<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';

	const routes = getContext(WebRoutesService);
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
