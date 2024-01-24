<script lang="ts">
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
	import toast from 'svelte-french-toast';
	import Tooltip from '$lib/components/Tooltip.svelte';

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
	let container: HTMLDivElement;
	let isApplying = false;

	function handleBranchNameChange() {
		branchController.updateBranchName(branch.id, branch.name);
	}

	function normalizeBranchName(value: string) {
		return value.toLowerCase().replace(/[^0-9a-z/_]+/g, '-');
	}

	$: hasIntegratedCommits = branch.commits?.some((b) => b.isIntegrated);
</script>

<div class="header__wrapper">
	<div class="header card" bind:this={container} class:readonly>
		<div class="header__info">
			<div class="header__label">
				<BranchLabel
					bind:name={branch.name}
					on:change={handleBranchNameChange}
					disabled={readonly}
				/>
			</div>
			<div class="header__remote-branch">
				{#if !branch.upstream}
					{#if !branch.active}
						<Tooltip label="These changes are stashed away. Apply the lane to bring them back.">
							<div class="status-tag text-base-11 text-semibold unapplied">
								<Icon name="removed-branch-small" /> unapplied
							</div>
						</Tooltip>
					{:else if hasIntegratedCommits}
						<Tooltip
							label="These changes have been integrated upstream, update your workspace to make this lane disappear."
						>
							<div class="status-tag text-base-11 text-semibold integrated">
								<Icon name="removed-branch-small" /> integrated
							</div>
						</Tooltip>
					{:else}
						<Tooltip label="These changes are in a virtual branch.">
							<div class="status-tag text-base-11 text-semibold pending">
								<Icon name="virtual-branch-small" /> virtual
							</div>
						</Tooltip>
					{/if}
					{#if !readonly}
						<div class="pending-name">
							<Tooltip
								label="Branch name that will be used when pushing. You can change it from the lane menu."
							>
								<span class="text-base-11 text-semibold">
									origin/{branch.upstreamName
										? branch.upstreamName
										: normalizeBranchName(branch.name)}
								</span>
							</Tooltip>
						</div>
					{/if}
				{:else}
					<Tooltip label="At least some of your changes have been pushed">
						<div class="status-tag text-base-11 text-semibold remote">
							<Icon name="remote-branch-small" /> remote
						</div>
					</Tooltip>
					<Tag
						icon="open-link"
						color="ghost"
						border
						clickable
						shrinkable
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
				{#await branch.isMergeable then isMergeable}
					{#if !isMergeable}
						<Tooltip
							timeoutMilliseconds={100}
							label="Applying this branch will add merge conflict markers that you will have to resolve"
						>
							<Tag icon="locked-small" color="warning">Conflict</Tag>
						</Tooltip>
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
					<Tooltip timeoutMilliseconds={1000} label="New changes will land here">
						<Button class="w-32" icon="target" notClickable disabled={readonly}
							>Default branch</Button
						>
					</Tooltip>
				{:else}
					<Tooltip timeoutMilliseconds={1000} label="When selected, new changes will land here">
						<Button
							class="w-32"
							icon="target"
							kind="outlined"
							color="neutral"
							disabled={readonly}
							on:click={async () => {
								await branchController.setSelectedForChanges(branch.id);
							}}
						>
							Set as default
						</Button>
					</Tooltip>
				{/if}
				{#if !readonly}
					<Button
						icon="cross-small"
						color="primary"
						kind="outlined"
						loading={isApplying}
						on:click={async () => {
							isApplying = true;
							try {
								await branchController.unapplyBranch(branch.id);
							} catch (e) {
								const err = 'Failed to apply branch';
								toast.error(err);
								console.error(err, e);
							} finally {
								isApplying = false;
							}
						}}
					>
						Unapply
					</Button>
				{:else}
					<Button
						icon="plus-small"
						color="primary"
						kind="outlined"
						loading={isApplying}
						on:click={async () => {
							isApplying = true;
							try {
								await branchController.applyBranch(branch.id);
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
				{/if}
			</div>
			<div class="relative" bind:this={meatballButton}>
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
						{readonly}
						bind:visible
						on:action
					/>
				</div>
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
		&.readonly {
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
	.readonly .header__actions {
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
