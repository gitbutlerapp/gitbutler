<script lang="ts">
	import BranchLabel from './BranchLabel.svelte';
	import BranchLanePopupMenu from './BranchLanePopupMenu.svelte';
	import Tag from './Tag.svelte';
	import { clickOutside } from '$lib/clickOutside';
	import Button from '$lib/components/Button.svelte';
	import Icon, { type IconColor } from '$lib/components/Icon.svelte';
	import { normalizeBranchName } from '$lib/utils/branch';
	import * as toasts from '$lib/utils/toasts';
	import { tooltip } from '$lib/utils/tooltip';
	import { open } from '@tauri-apps/api/shell';
	import toast from 'svelte-french-toast';
	import type { BranchService } from '$lib/branches/service';
	import type { GitHubService } from '$lib/github/service';
	import type { PrStatus } from '$lib/github/types';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, Branch } from '$lib/vbranches/types';
	import type iconsJson from '../icons/icons.json';
	import { goto } from '$app/navigation';

	export let isUnapplied = false;
	export let branch: Branch;
	export let base: BaseBranch | undefined | null;
	export let branchController: BranchController;
	export let branchService: BranchService;
	export let projectId: string;

	export let githubService: GitHubService;
	$: pr$ = githubService.get(branch.upstreamName);
	$: prStatus$ = githubService.getStatus($pr$?.targetBranch);

	let meatballButton: HTMLDivElement;
	let visible = false;
	let container: HTMLDivElement;
	let isApplying = false;
	let isDeleting = false;
	let isMerging = false;

	function handleBranchNameChange() {
		branchController.updateBranchName(branch.id, branch.name);
	}

	$: prColor = statusToColor($prStatus$);
	$: prIcon = statusToIcon($prStatus$);

	function statusToColor(status: PrStatus | undefined): IconColor {
		if (!status) return 'neutral';
		if (status && !status.hasChecks) return 'neutral';
		if (status.completed) {
			return status.success ? 'success' : 'error';
		}
		return 'warn';
	}

	function statusToIcon(status: PrStatus | undefined): keyof typeof iconsJson | undefined {
		if (!status) return;
		if (status && !status.hasChecks) return;
		if (status.completed) {
			return status.success ? 'success' : 'error';
		}
		return 'warning';
	}

	$: hasIntegratedCommits = branch.commits?.some((b) => b.isIntegrated);
</script>

<div class="header__wrapper">
	<div class="header card" bind:this={container} class:isUnapplied>
		<div class="header__info">
			<div class="header__label">
				<BranchLabel
					bind:name={branch.name}
					on:change={handleBranchNameChange}
					disabled={isUnapplied}
				/>
			</div>
			<div class="header__remote-branch">
				{#if !branch.upstream}
					{#if !branch.active}
						<div
							class="status-tag text-base-11 text-semibold unapplied"
							use:tooltip={'These changes are stashed away from your working directory.'}
						>
							<Icon name="removed-branch-small" /> unapplied
						</div>
					{:else if hasIntegratedCommits}
						<div
							class="status-tag text-base-11 text-semibold integrated"
							use:tooltip={'These changes have been integrated upstream, update your workspace to make this lane disappear.'}
						>
							<Icon name="removed-branch-small" /> integrated
						</div>
					{:else}
						<div
							class="status-tag text-base-11 text-semibold pending"
							use:tooltip={'These changes are in your working directory.'}
						>
							<Icon name="virtual-branch-small" /> virtual
						</div>
					{/if}
					{#if !isUnapplied}
						<div
							class="pending-name"
							use:tooltip={'Branch name that will be used when pushing. You can change it from the lane menu.'}
						>
							<span class="text-base-11 text-semibold">
								origin/{branch.upstreamName
									? branch.upstreamName
									: normalizeBranchName(branch.name)}
							</span>
						</div>
					{/if}
				{:else}
					<div
						class="status-tag text-base-11 text-semibold remote"
						use:tooltip={'At least some of your changes have been pushed'}
					>
						<Icon name="remote-branch-small" /> remote
					</div>
					<Tag
						icon="open-link"
						color="ghost"
						border
						clickable
						shrinkable
						on:click={(e) => {
							const url = base?.branchUrl(branch.upstream?.name);
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
					{#if prIcon}
						<Icon name={prIcon} color={prColor} />
					{/if}
				{/if}
				{#await branch.isMergeable then isMergeable}
					{#if !isMergeable}
						<Tag
							icon="locked-small"
							color="warning"
							help="Applying this branch will add merge conflict markers that you will have to resolve"
						>
							Conflict
						</Tag>
					{/if}
				{/await}
			</div>
			<div class="draggable" data-drag-handle>
				<Icon name="draggable-narrow" />
			</div>
		</div>
		<div class="header__actions">
			<div class="header__buttons">
				{#if branch.selectedForChanges}
					<Button
						help="New changes will land here"
						icon="target"
						notClickable
						disabled={isUnapplied}>Default branch</Button
					>
				{:else}
					<Button
						help="When selected, new changes will land here"
						icon="target"
						kind="outlined"
						color="neutral"
						disabled={isUnapplied}
						on:click={async () => {
							await branchController.setSelectedForChanges(branch.id);
						}}
					>
						Set as default
					</Button>
				{/if}
				{#if $prStatus$?.success}
					<Button
						help="Merge pull request and refresh"
						disabled={isUnapplied}
						loading={isMerging}
						on:click={async () => {
							isMerging = true;
							try {
								if ($pr$) await githubService.merge($pr$.number);
								branchService.reloadVirtualBranches();
							} catch {
								toasts.error('Failed to merge pull request');
							} finally {
								isMerging = false;
							}
						}}
					>
						Merge
					</Button>
				{/if}
			</div>
			<div class="relative" bind:this={meatballButton}>
				{#if isUnapplied}
					<Button
						help="Deletes the local virtual branch (only)"
						icon="bin-small"
						color="neutral"
						kind="outlined"
						loading={isDeleting}
						on:click={async () => {
							isDeleting = true;
							try {
								await branchController.deleteBranch(branch.id);
								goto(`/${projectId}/board`);
							} catch (e) {
								const err = 'Failed to delete branch';
								toasts.error(err);
								console.error(err, e);
							} finally {
								isDeleting = false;
							}
						}}
					>
						Delete
					</Button>
					<Button
						help="Restores these changes into your working directory"
						icon="plus-small"
						color="primary"
						kind="outlined"
						loading={isApplying}
						on:click={async () => {
							isApplying = true;
							try {
								await branchController.applyBranch(branch.id);
								goto(`/${projectId}/board`);
							} catch (e) {
								const err = 'Failed to apply branch';
								toast.error(err);
								console.error(err, e);
							} finally {
								isApplying = false;
							}
						}}
					>
						Apply
					</Button>
				{:else}
					<Button
						icon="kebab"
						kind="outlined"
						color="neutral"
						on:click={() => (visible = !visible)}
					/>
					<div
						class="branch-popup-menu"
						use:clickOutside={{
							trigger: meatballButton,
							handler: () => (visible = false)
						}}
					>
						<BranchLanePopupMenu
							{branchController}
							{branch}
							{projectId}
							{isUnapplied}
							bind:visible
							on:action
						/>
					</div>
				{/if}
			</div>
		</div>
	</div>
	<div class="header__top-overlay" data-remove-from-draggable data-tauri-drag-region />
</div>

<style lang="postcss">
	.header__wrapper {
		z-index: 10;
		position: sticky;
		top: var(--space-12);
	}
	.header {
		z-index: 2;
		position: relative;
		flex-direction: column;
		gap: var(--space-2);

		&:hover {
			& .draggable {
				opacity: 1;
			}
		}
		&.isUnapplied {
			background: var(--clr-theme-container-pale);
		}
	}
	.header__top-overlay {
		z-index: 1;
		position: absolute;
		top: calc(var(--space-16) * -1);
		left: 0;
		width: 100%;
		height: var(--space-20);
		background: var(--target-branch-background);
		/* background-color: red; */
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
		user-select: none;
	}
	.isUnapplied .header__actions {
		background: var(--clr-theme-container-dim);
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
		gap: var(--space-4);
		text-overflow: ellipsis;
		overflow-x: hidden;
		white-space: nowrap;
		align-items: center;
	}

	.status-tag {
		cursor: default;
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-6) var(--space-2) var(--space-4);
		border-radius: var(--radius-m);
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
		overflow: hidden;

		& span {
			overflow: hidden;
			text-overflow: ellipsis;
		}
	}

	.pending {
		color: var(--clr-theme-scale-ntrl-30);
		background: color-mix(in srgb, var(--clr-theme-scale-ntrl-50) 20%, transparent);
	}

	.integrated {
		color: var(--clr-theme-succ-on-element);
		background: var(--clr-theme-succ-element);
	}

	.remote {
		color: var(--clr-theme-scale-ntrl-100);
		background: var(--clr-theme-scale-ntrl-40);
	}

	.unapplied {
		color: var(--clr-theme-scale-ntrl-30);
		background: var(--clr-theme-scale-ntrl-80);
	}
</style>
