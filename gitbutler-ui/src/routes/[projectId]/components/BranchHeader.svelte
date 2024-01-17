<script lang="ts">
	import IconButton from '$lib/components/IconButton.svelte';
	import Icon from '$lib/icons/Icon.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, Branch } from '$lib/vbranches/types';
	import { fade } from 'svelte/transition';
	import BranchLabel from './BranchLabel.svelte';
	import BranchLanePopupMenu from './BranchLanePopupMenu.svelte';
	import { clickOutside } from '$lib/clickOutside';
	import Tag from './Tag.svelte';
	import { branchUrl } from './commitList';
	import type { GitHubService } from '$lib/github/service';
	import BranchIcon from '../navigation/BranchIcon.svelte';
	import { open } from '@tauri-apps/api/shell';

	export let readonly = false;
	export let branch: Branch;
	export let base: BaseBranch | undefined | null;
	export let branchController: BranchController;
	export let projectId: string;

	export let githubService: GitHubService;
	$: pr$ = githubService.get(branch.upstreamName);
	// $: prStatus$ = githubService.getStatus($pr$?.targetBranch);

	let meatballButton: HTMLDivElement;
	let visible = false;

	function handleBranchNameChange() {
		branchController.updateBranchName(branch.id, branch.name);
	}

	$: hasIntegratedCommits = branch.commits?.some((b) => b.isIntegrated);
</script>

<div class="card__header">
	{#if !readonly}
		<div class="draggable" data-drag-handle>
			<Icon name="draggable-narrow" />
		</div>
	{/if}
	<div class="header__content">
		<div class="header__row">
			<div class="header__label">
				<BranchLabel bind:name={branch.name} on:change={handleBranchNameChange} />
			</div>
			<div class="flex items-center gap-x-1" transition:fade={{ duration: 150 }}>
				{#if !readonly}
					<div bind:this={meatballButton}>
						<IconButton icon="kebab" size="m" on:click={() => (visible = !visible)} />
					</div>
					<div
						class="branch-popup-menu"
						use:clickOutside={{
							trigger: meatballButton,
							handler: () => (visible = false)
						}}
					>
						<BranchLanePopupMenu {branchController} {branch} {projectId} bind:visible on:action />
					</div>
				{/if}
			</div>
		</div>
		{#if branch.upstreamName}
			<div class="header__remote-branch text-base-body-11">
				{#if !branch.upstream}
					{#if hasIntegratedCommits}
						<div class="status-tag deleted">deleted</div>
					{:else}
						<div class="status-tag pending">pending</div>
					{/if}
				{/if}
				<div>origin/{branch.upstreamName}</div>
			</div>
		{/if}
		{#if branch.upstream}
			<div class="header__links">
				<BranchIcon name="remote-branch" color="neutral" />

				<Tag
					icon="open-link"
					color="ghost"
					border
					clickable
					on:click={(e) => {
						const url = branchUrl(base, branch.upstream?.name);
						if (url) open(url);
						e.preventDefault();
						e.stopPropagation();
					}}
				>
					View remote branch
				</Tag>
				{#if $pr$?.htmlUrl}
					<Tag
						icon="pr-small"
						color="ghost"
						border
						clickable
						on:click={(e) => {
							const url = $pr$?.htmlUrl;
							if (url) open(url);
							e.preventDefault();
							e.stopPropagation();
						}}
					>
						View PR
					</Tag>
				{/if}
			</div>
		{/if}
	</div>
</div>

<style lang="postcss">
	.card__header {
		user-select: none;
		position: relative;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-12);

		&:hover {
			& .header__content {
				margin-left: var(--space-6);
			}

			& .draggable {
				opacity: 1;
			}
		}
	}
	.header__content {
		display: flex;
		flex-direction: column;
		transition: margin var(--transition-slow);
	}
	.header__row {
		width: 100%;
		display: flex;
		justify-content: space-between;
		gap: var(--space-8);
		overflow-x: hidden;
	}
	.header__label {
		overflow-x: hidden;
		display: flex;
		flex-grow: 1;
		align-items: center;
		gap: var(--space-4);
	}

	.header__links {
		display: flex;
		gap: var(--space-4);
		align-items: center;
		padding-left: var(--space-4);
		margin-top: var(--space-8);
		margin-bottom: var(--space-4);
		fill: var(--clr-core-ntrl-50);
	}

	.draggable {
		position: absolute;
		left: var(--space-4);
		top: var(--space-6);
		opacity: 0;
		display: flex;
		cursor: grab;
		color: var(--clr-theme-scale-ntrl-50);
		transition:
			opacity var(--transition-slow),
			color var(--transition-slow);

		&:hover {
			color: var(--clr-theme-scale-ntrl-40);
		}

		& svg {
			pointer-events: none;
		}
	}

	.branch-popup-menu {
		position: absolute;
		top: calc(var(--space-2) + var(--space-40));
		right: var(--space-12);
		z-index: 10;
	}

	.header__remote-branch {
		color: var(--clr-theme-scale-ntrl-50);
		padding-left: var(--space-4);
		display: flex;
		gap: var(--space-4);
		text-overflow: ellipsis;
		overflow-x: hidden;
		white-space: nowrap;
		align-items: center;
	}

	.status-tag {
		padding: var(--space-2) var(--space-4);
		border-radius: var(--radius-s);
		margin-right: var(--space-2);
	}

	.pending {
		color: var(--clr-theme-scale-ntrl-40);
		background: var(--clr-theme-container-sub);
	}

	.deleted {
		color: var(--clr-theme-scale-warn-30);
		background: var(--clr-theme-warn-container-dim);
	}
</style>
