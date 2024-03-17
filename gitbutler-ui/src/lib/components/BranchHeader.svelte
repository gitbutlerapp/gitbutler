<script lang="ts">
	import ActiveBranchStatus from './ActiveBranchStatus.svelte';
	import BranchLabel from './BranchLabel.svelte';
	import BranchLanePopupMenu from './BranchLanePopupMenu.svelte';
	import Tag from './Tag.svelte';
	import { clickOutside } from '$lib/clickOutside';
	import Button from '$lib/components/Button.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import { getContextByClass } from '$lib/utils/context';
	import * as toasts from '$lib/utils/toasts';
	import { BranchController } from '$lib/vbranches/branchController';
	import toast from 'svelte-french-toast';
	import type { Persisted } from '$lib/persisted/persisted';
	import type { Branch } from '$lib/vbranches/types';
	import { goto } from '$app/navigation';

	export let isUnapplied = false;
	export let branch: Branch;
	export let projectId: string;
	export let isLaneCollapsed: Persisted<boolean>;

	const branchController = getContextByClass(BranchController);

	let meatballButton: HTMLDivElement;
	let visible = false;
	let isApplying = false;
	let isDeleting = false;

	function handleBranchNameChange() {
		branchController.updateBranchName(branch.id, branch.name);
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
		on:mousedown={expandLane}
		on:keydown={(e) => e.key === 'Enter' && expandLane()}
		tabindex="0"
		role="button"
	>
		<div class="collapsed-lane__actions">
			<div class="collapsed-lane__draggable" data-drag-handle>
				<Icon name="draggable-narrow" />
			</div>
			<Button
				icon="unfold-lane"
				kind="outlined"
				color="neutral"
				help="Collapse lane"
				on:mousedown={expandLane}
			/>
		</div>

		<div class="collapsed-lane__info">
			<h3 class="collapsed-lane__label text-base-13 text-bold">
				{branch.name}
			</h3>

			<div class="collapsed-lane__info__details">
				<ActiveBranchStatus
					{branch}
					{isUnapplied}
					{hasIntegratedCommits}
					isLaneCollapsed={$isLaneCollapsed}
				/>
				{#if branch.selectedForChanges}
					<Tag color="pop" filled icon="target" verticalOrientation>Default branch</Tag>
				{/if}
			</div>
		</div>
	</div>
{:else}
	<div class="header__wrapper">
		<div class="header card" class:isUnapplied>
			<div class="header__info">
				<div class="header__label">
					<BranchLabel
						bind:name={branch.name}
						on:change={handleBranchNameChange}
						disabled={isUnapplied}
					/>
				</div>
				<div class="header__remote-branch">
					<ActiveBranchStatus
						{branch}
						{isUnapplied}
						{hasIntegratedCommits}
						isLaneCollapsed={$isLaneCollapsed}
					/>

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
					<Icon name="draggable" />
				</div>
			</div>
			<div class="header__actions">
				<div class="header__buttons">
					{#if branch.active}
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
						<div class="header__buttons">
							<Button
								icon="fold-lane"
								kind="outlined"
								color="neutral"
								help="Collapse lane"
								on:mousedown={collapseLane}
							/>
							<Button
								icon="kebab"
								kind="outlined"
								color="neutral"
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
								<BranchLanePopupMenu {branch} {projectId} {isUnapplied} bind:visible on:action />
							</div>
						</div>
					{/if}
				</div>
			</div>
		</div>
		<div class="header__top-overlay" data-remove-from-draggable data-tauri-drag-region />
	</div>
{/if}

<style lang="postcss">
	.header__wrapper {
		z-index: 10;
		position: sticky;
		top: var(--size-12);
	}
	.header {
		z-index: 2;
		position: relative;
		flex-direction: column;
		gap: var(--size-2);

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
		top: calc(var(--size-16) * -1);
		left: 0;
		width: 100%;
		height: var(--size-20);
		background: var(--target-branch-background);
		/* background-color: red; */
	}
	.header__info {
		display: flex;
		flex-direction: column;
		transition: margin var(--transition-slow);
		padding: var(--size-12);
		gap: var(--size-10);
	}
	.header__actions {
		display: flex;
		gap: var(--size-4);
		background: var(--clr-theme-container-pale);
		padding: var(--size-14);
		justify-content: space-between;
		border-radius: 0 0 var(--radius-m) var(--radius-m);
		user-select: none;
	}
	.isUnapplied .header__actions {
		background: var(--clr-theme-container-sub);
	}
	.header__buttons {
		display: flex;
		position: relative;
		gap: var(--size-4);
	}
	.header__label {
		display: flex;
		flex-grow: 1;
		align-items: center;
		gap: var(--size-4);
	}
	.draggable {
		display: flex;
		cursor: grab;
		position: absolute;
		right: var(--size-4);
		top: var(--size-6);
		opacity: 0;
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
		top: calc(100% + var(--size-4));
		right: 0;
		z-index: 10;
	}

	.header__remote-branch {
		color: var(--clr-theme-scale-ntrl-50);
		padding-left: var(--size-2);
		padding-right: var(--size-2);
		display: flex;
		gap: var(--size-4);
		text-overflow: ellipsis;
		overflow-x: hidden;
		white-space: nowrap;
		align-items: center;
	}

	/*  COLLAPSABLE LANE */

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
		color: var(--clr-theme-scale-ntrl-0);

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

	.collapsed-lane__label {
		color: var(--clr-theme-scale-ntrl-0);
		transform: rotate(180deg);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
