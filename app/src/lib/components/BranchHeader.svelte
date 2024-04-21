<script lang="ts">
	import ActiveBranchStatus from './ActiveBranchStatus.svelte';
	import BranchLabel from './BranchLabel.svelte';
	import BranchLanePopupMenu from './BranchLanePopupMenu.svelte';
	import Tag from './Tag.svelte';
	import { Project } from '$lib/backend/projects';
	import { clickOutside } from '$lib/clickOutside';
	import Button from '$lib/components/Button.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { normalizeBranchName } from '$lib/utils/branch';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { Branch } from '$lib/vbranches/types';
	import type { Persisted } from '$lib/persisted/persisted';
	import { goto } from '$app/navigation';

	export let uncommittedChanges = 0;
	export let isUnapplied = false;
	export let isLaneCollapsed: Persisted<boolean>;

	const branchController = getContext(BranchController);
	const branchStore = getContextStore(Branch);
	const project = getContext(Project);

	$: branch = $branchStore;

	let meatballButton: HTMLDivElement;
	let visible = false;
	let isApplying = false;
	let isDeleting = false;
	let branchName = branch?.upstreamName || normalizeBranchName($branchStore.name);

	function handleBranchNameChange(title: string) {
		if (title == '') return;

		branchName = normalizeBranchName(title);
		branchController.updateBranchName(branch.id, title);
	}

	function collapseLane() {
		$isLaneCollapsed = true;
	}

	function expandLane() {
		$isLaneCollapsed = false;
	}

	$: hasIntegratedCommits = branch.commits?.some((b) => b.isIntegrated);
</script>

{#if $isLaneCollapsed}
	<div
		class="card collapsed-lane"
		on:keydown={(e) => e.key === 'Enter' && expandLane()}
		tabindex="0"
		role="button"
	>
		<div class="collapsed-lane__actions">
			<div class="collapsed-lane__draggable" data-drag-handle>
				<Icon name="draggable" />
			</div>
			<Button
				style="ghost"
				kind="solid"
				icon="unfold-lane"
				help="Expand lane"
				on:mousedown={expandLane}
			/>
		</div>

		<div class="collapsed-lane__info">
			<div class="collapsed-lane__label-wrap">
				{#if uncommittedChanges > 0}
					<Tag style="warning" kind="soft" verticalOrientation help="Uncommitted changes">
						{uncommittedChanges}
						{uncommittedChanges == 1 ? 'change' : 'changes'}
					</Tag>
				{/if}

				<h3 class="collapsed-lane__label text-base-13 text-bold">
					{branch.name}
				</h3>
			</div>

			<div class="collapsed-lane__info__details">
				<ActiveBranchStatus
					branchName={branch.upstreamName ?? branchName}
					{isUnapplied}
					{hasIntegratedCommits}
					remoteExists={!!branch.upstreamName}
					isLaneCollapsed={$isLaneCollapsed}
				/>
				{#if branch.selectedForChanges}
					<Tag style="pop" kind="solid" icon="target" verticalOrientation>Default branch</Tag>
				{/if}
			</div>
		</div>
	</div>
{:else}
	<div class="header__wrapper">
		<div class="header card">
			<div class="header__info-wrapper">
				{#if !isUnapplied}
					<div class="draggable" data-drag-handle>
						<Icon name="draggable" />
					</div>
				{/if}

				<div class="header__info">
					<BranchLabel
						name={branch.name}
						on:change={(e) => handleBranchNameChange(e.detail.name)}
						disabled={isUnapplied}
					/>
					<div class="header__remote-branch">
						<ActiveBranchStatus
							branchName={branch.upstreamName ?? branchName}
							{isUnapplied}
							{hasIntegratedCommits}
							remoteExists={!!branch.upstreamName}
							isLaneCollapsed={$isLaneCollapsed}
						/>

						{#await branch.isMergeable then isMergeable}
							{#if !isMergeable}
								<Tag
									icon="locked-small"
									style="warning"
									help="Applying this branch will add merge conflict markers that you will have to resolve"
								>
									Conflict
								</Tag>
							{/if}
						{/await}
					</div>
				</div>
			</div>

			<div class="header__actions">
				<div class="header__buttons">
					{#if branch.active}
						{#if branch.selectedForChanges}
							<Button
								style="pop"
								kind="solid"
								help="New changes will land here"
								icon="target"
								clickable={false}
								disabled={isUnapplied}>Default branch</Button
							>
						{:else}
							<Button
								style="ghost"
								kind="solid"
								help="When selected, new changes will land here"
								icon="target"
								disabled={isUnapplied}
								on:mousedown={async () => {
									await branchController.setSelectedForChanges(branch.id);
								}}
							>
								Set as default
							</Button>
						{/if}
					{/if}
				</div>

				<div class="relative" bind:this={meatballButton}>
					{#if isUnapplied}
						<Button
							style="ghost"
							kind="solid"
							help="Deletes the local virtual branch (only)"
							icon="bin-small"
							loading={isDeleting}
							on:click={async () => {
								isDeleting = true;
								try {
									await branchController.deleteBranch(branch.id);
									goto(`/${project.id}/board`);
								} catch (err) {
									showError('Failed to delete branch', err);
									console.error(err);
								} finally {
									isDeleting = false;
								}
							}}
						>
							Delete
						</Button>
						<Button
							style="ghost"
							kind="solid"
							help="Restores these changes into your working directory"
							icon="plus-small"
							loading={isApplying}
							on:click={async () => {
								isApplying = true;
								try {
									await branchController.applyBranch(branch.id);
									goto(`/${project.id}/board`);
								} catch (err) {
									showError('Failed to apply branch', err);
									console.error(err);
								} finally {
									isApplying = false;
								}
							}}
						>
							Apply
						</Button>
					{:else}
						<div class="header__buttons">
							<Button
								style="ghost"
								kind="solid"
								icon="fold-lane"
								help="Collapse lane"
								on:mousedown={collapseLane}
							/>
							<Button
								style="ghost"
								kind="solid"
								icon="kebab"
								on:mousedown={() => {
									visible = !visible;
								}}
							/>
							<div
								class="branch-popup-menu"
								use:clickOutside={{
									trigger: meatballButton,
									handler: () => (visible = false)
								}}
							>
								<BranchLanePopupMenu {isUnapplied} bind:visible on:action />
							</div>
						</div>
					{/if}
				</div>
			</div>
		</div>
		<div class="header__top-overlay" data-remove-from-draggable data-tauri-drag-region />
	</div>
{/if}

<style>
	.header__wrapper {
		z-index: var(--z-lifted);
		position: sticky;
		top: var(--size-12);
	}
	.header {
		z-index: var(--z-lifted);
		position: relative;
		flex-direction: column;
		gap: var(--size-2);
	}
	.header__top-overlay {
		z-index: var(--z-ground);
		position: absolute;
		top: calc(var(--size-16) * -1);
		left: 0;
		width: 100%;
		height: var(--size-20);
		background: var(--target-branch-background);
	}
	.header__info-wrapper {
		display: flex;
		gap: var(--size-2);
		padding: var(--size-10);
	}
	.header__info {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		gap: var(--size-10);
	}
	.header__actions {
		display: flex;
		gap: var(--size-4);
		background: var(--clr-bg-alt);
		padding: var(--size-14);
		justify-content: space-between;
		border-radius: 0 0 var(--radius-m) var(--radius-m);
		user-select: none;
	}

	.header__buttons {
		display: flex;
		position: relative;
		gap: var(--size-4);
	}
	.draggable {
		display: flex;
		height: fit-content;
		cursor: grab;
		padding: var(--size-2) var(--size-2) 0 0;
		color: var(--clr-scale-ntrl-50);
		transition: color var(--transition-slow);

		&:hover {
			color: var(--clr-scale-ntrl-40);
		}
	}

	.branch-popup-menu {
		position: absolute;
		top: calc(100% + var(--size-4));
		right: 0;
		z-index: var(--z-floating);
	}

	.header__remote-branch {
		color: var(--clr-scale-ntrl-50);
		padding-left: var(--size-2);
		padding-right: var(--size-2);
		display: flex;
		gap: var(--size-4);
		text-overflow: ellipsis;
		overflow-x: hidden;
		white-space: nowrap;
		align-items: center;
	}

	/*  COLLAPSIBLE LANE */

	.collapsed-lane {
		cursor: default;
		user-select: none;
		align-items: center;
		height: 100%;
		gap: var(--size-8);
		padding: var(--size-8) var(--size-8) var(--size-20);

		&:focus-within {
			outline: none;
		}
	}

	.collapsed-lane__actions {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--size-2);
	}

	.collapsed-lane__draggable {
		cursor: grab;
		transform: rotate(90deg);
		margin-bottom: var(--size-4);
		opacity: 0.4;
		transition: opacity var(--transition-fast);
		color: var(--clr-scale-ntrl-0);

		&:hover {
			opacity: 1;
		}
	}

	.collapsed-lane__info {
		flex: 1;
		display: flex;
		flex-direction: row-reverse;
		align-items: center;
		justify-content: space-between;
		height: 100%;

		writing-mode: vertical-rl;
		gap: var(--size-8);
	}

	.collapsed-lane__info__details {
		display: flex;
		flex-direction: row-reverse;
		align-items: center;
		gap: var(--size-4);
	}

	.collapsed-lane__label-wrap {
		overflow: hidden;
		display: flex;
		align-items: center;
		gap: var(--size-12);
	}

	.collapsed-lane__label {
		color: var(--clr-scale-ntrl-0);
		transform: rotate(180deg);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
