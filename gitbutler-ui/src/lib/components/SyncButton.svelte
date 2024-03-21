<script lang="ts">
	import { syncToCloud } from '$lib/backend/cloud';
	import { Project } from '$lib/backend/projects';
	import Icon from '$lib/components/Icon.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import { GitHubService } from '$lib/github/service';
	import { getContextByClass } from '$lib/utils/context';
	import { tooltip } from '$lib/utils/tooltip';
	import { BaseBranchService } from '$lib/vbranches/branchStoresCache';

	const project = getContextByClass(Project);

	let cloudEnabled: boolean;

	const githubService = getContextByClass(GitHubService);
	const baseBranchService = getContextByClass(BaseBranchService);
	const baseBranch = baseBranchService.base;

	$: baseServiceBusy$ = baseBranchService.busy$;
	$: cloudEnabled = project?.api?.sync || false;
</script>

<button
	class="sync-btn"
	class:sync-btn-busy={$baseServiceBusy$}
	on:mousedown={async (e) => {
		e.preventDefault();
		e.stopPropagation();
		if (cloudEnabled) syncToCloud(project.id); // don't wait for this
		await baseBranchService.fetchFromTarget('modal');
		if (githubService.isEnabled) {
			await githubService.reload();
		}
	}}
>
	{#if !$baseServiceBusy$}
		<div class="sync-btn__icon">
			<Icon name="update-small" />
		</div>
	{/if}

	<span class="text-base-11 text-semibold sync-btn__label" use:tooltip={'Last fetch from upstream'}>
		{#if $baseServiceBusy$}
			<div class="sync-btn__busy-label">busy…</div>
		{:else if $baseBranch?.lastFetched}
			<TimeAgo date={$baseBranch?.lastFetched} />
		{/if}
	</span>
</button>

<style lang="postcss">
	.sync-btn {
		display: flex;
		align-items: center;
		gap: var(--size-2);
		height: var(--size-20);
		padding-left: var(--size-2);
		padding-right: var(--size-4);
		background: var(--clr-theme-container-light);
		border: 1px solid var(--clr-theme-container-outline-light);
		border-radius: var(--radius-m);
		cursor: pointer;

		&.sync-btn-busy {
			cursor: default;
		}

		transition:
			background var(--transition-fast),
			border var(--transition-fast);

		&:hover {
			background: var(--clr-theme-container-light);
			border: 1px solid var(--clr-theme-container-outline-pale);
		}

		&:hover .sync-btn__icon {
			fill: var(--clr-theme-scale-ntrl-40);
		}

		&:hover .sync-btn__label {
			color: var(--clr-theme-scale-ntrl-40);
		}
	}

	.sync-btn__icon {
		display: flex;
		color: var(--clr-theme-scale-ntrl-40);
		transform-origin: center;
		transform: rotate(0deg);
		transition:
			fill var(--transition-fast),
			transform var(--transition-slow);
	}

	.sync-btn__label {
		display: block;
		line-height: 1;
		white-space: nowrap;
		color: var(--clr-theme-scale-ntrl-40);
		transition: color var(--transition-fast);
	}

	.sync-btn__busy-label {
		padding-left: var(--size-4);
	}
</style>
