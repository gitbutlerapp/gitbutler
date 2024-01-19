<script lang="ts">
	import IconButton from '$lib/components/IconButton.svelte';
	import Icon from '$lib/icons/Icon.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, Branch } from '$lib/vbranches/types';
	import BranchLabel from './BranchLabel.svelte';
	import BranchLanePopupMenu from './BranchLanePopupMenu.svelte';
	import { clickOutside } from '$lib/clickOutside';
	import Tag from './Tag.svelte';
	import { branchUrl } from './commitList';
	import type { GitHubService } from '$lib/github/service';
	import { open } from '@tauri-apps/api/shell';
	import Button from '$lib/components/Button.svelte';
	import { onMount } from 'svelte';

	export let readonly = false;
	export let branch: Branch;
	export let base: BaseBranch | undefined | null;
	export let branchController: BranchController;
	export let projectId: string;
	export let height: number | undefined;

	export let githubService: GitHubService;
	$: pr$ = githubService.get(branch.upstreamName);
	// $: prStatus$ = githubService.getStatus($pr$?.targetBranch);

	let meatballButton: HTMLDivElement;
	let visible = false;
	let observer: ResizeObserver;
	let container: HTMLDivElement;

	onMount(() => {
		observer = new ResizeObserver(() => {
			if (container) {
				height = container.offsetHeight;
			}
		});
		return observer.observe(container);
	});

	function handleBranchNameChange() {
		branchController.updateBranchName(branch.id, branch.name);
	}

	function normalizeBranchName(value: string) {
		return value.toLowerCase().replace(/[^0-9a-z/_]/g, '-');
	}

	$: hasIntegratedCommits = branch.commits?.some((b) => b.isIntegrated);
</script>

<div class="wrapper" bind:this={container}>
	<div class="concealer">
		<div class="header card">
			<div class="header__info">
				<div class="header__label">
					<BranchLabel bind:name={branch.name} on:change={handleBranchNameChange} />
				</div>
				<div class="header__remote-branch text-base-body-11">
					{#if !branch.upstream}
						{#if hasIntegratedCommits}
							<div class="status-tag deleted"><Icon name="remote-branch-small" /> deleted</div>
						{:else}
							<div class="status-tag pending"><Icon name="remote-branch-small" /> new</div>
						{/if}
						<div class="text-semibold pending-name text-base-11">
							origin/{branch.upstreamName ? branch.upstreamName : normalizeBranchName(branch.name)}
						</div>
					{:else}
						<div class="status-tag remote"><Icon name="remote-branch-small" /> remote</div>
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
							origin/{branch.upstreamName}
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
					{/if}
				</div>
				{#if !readonly}
					<div class="draggable" data-drag-handle>
						<Icon name="draggable-narrow" />
					</div>
				{/if}
			</div>
			<div class="header__actions">
				<div class="header__buttons">
					{#if branch.selectedForChanges}
						<Button icon="target">Target branch</Button>
					{:else}
						<Button
							icon="target"
							kind="outlined"
							on:click={async () => {
								await branchController.setSelectedForChanges(branch.id);
							}}
						>
							Make target
						</Button>
					{/if}
				</div>
				{#if !readonly}
					<div class="relative" bind:this={meatballButton}>
						<IconButton border icon="kebab" size="m" on:click={() => (visible = !visible)} />
						<div
							class="branch-popup-menu"
							use:clickOutside={{
								trigger: meatballButton,
								handler: () => (visible = false)
							}}
						>
							<BranchLanePopupMenu {branchController} {branch} {projectId} bind:visible on:action />
						</div>
					</div>
				{/if}
			</div>
		</div>
	</div>
</div>

<style lang="postcss">
	.wrapper {
		padding: 0 var(--space-8) var(--space-16) var(--space-8);
		position: absolute;
		z-index: 10;
		width: 100%;
	}
	.concealer {
		background: var(--clr-theme-container-pale);
		border-radius: 0 0 var(--radius-m) var(--radius-m);
		padding-top: var(--space-16);
	}
	.header {
		user-select: none;
		position: relative;
		flex-direction: column;
		gap: var(--space-2);
		top: 0;

		&:hover {
			& .draggable {
				opacity: 1;
			}
		}
	}

	.header__info {
		display: flex;
		flex-direction: column;
		transition: margin var(--transition-slow);
		padding: var(--space-12);
		gap: var(--space-10);
	}
	.header__actions {
		display: flex;
		gap: var(--space-4);
		background: var(--clr-theme-container-pale);
		padding: var(--space-12);
		justify-content: space-between;
		border-radius: 0 0 var(--radius-m) var(--radius-m);
	}
	.header__buttons {
		display: flex;
		position: relative;
		gap: var(--space-4);
	}
	.header__label {
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
		right: var(--space-4);
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
		top: calc(100% + var(--space-4));
		right: 0;
		z-index: 10;
	}

	.header__remote-branch {
		color: var(--clr-theme-scale-ntrl-50);
		padding-left: var(--space-4);
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-4);
		text-overflow: ellipsis;
		overflow-x: hidden;
		white-space: nowrap;
		align-items: center;
	}

	.status-tag {
		display: flex;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-6) var(--space-2) var(--space-4);
		border-radius: var(--radius-s);
		margin-right: var(--space-2);
	}

	.pending {
		color: var(--clr-theme-scale-pop-30);
		background: var(--clr-theme-scale-pop-80);
	}

	.pending-name {
		background: color-mix(in srgb, var(--clr-theme-scale-ntrl-50) 10%, transparent);
		border-radius: var(--radius-m);
		line-height: 120%;
		height: var(--space-20);
		display: flex;
		align-items: center;
		padding: 0 var(--space-6);
	}

	.deleted {
		color: var(--clr-theme-scale-warn-30);
		background: var(--clr-theme-warn-container-dim);
	}

	.remote {
		color: var(--clr-theme-scale-ntrl-100);
		background: var(--clr-theme-scale-ntrl-40);
	}
</style>
