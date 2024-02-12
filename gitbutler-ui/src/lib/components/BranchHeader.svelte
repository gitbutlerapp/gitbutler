<script lang="ts">
	import ActiveBranchStatus from './ActiveBranchStatus.svelte';
	import BranchLabel from './BranchLabel.svelte';
	import BranchLanePopupMenu from './BranchLanePopupMenu.svelte';
	import MergeButton from './MergeButton.svelte';
	import Tag from './Tag.svelte';
	import { clickOutside } from '$lib/clickOutside';
	import Button from '$lib/components/Button.svelte';
	import Icon, { type IconColor } from '$lib/components/Icon.svelte';
	import * as toasts from '$lib/utils/toasts';
	import { tooltip } from '$lib/utils/tooltip';
	import toast from 'svelte-french-toast';
	import type { BranchService } from '$lib/branches/service';
	import type { GitHubService } from '$lib/github/service';
	import type { PrStatus } from '$lib/github/types';
	import type { Persisted } from '$lib/persisted/persisted';
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

	export let isLaneCollapsed: Persisted<boolean>;

	export let githubService: GitHubService;
	$: pr$ = githubService.get(branch.upstreamName);

	let meatballButton: HTMLDivElement;
	let visible = false;
	let isApplying = false;
	let isDeleting = false;
	let isMerging = false;
	let isFetching = false;
	let prStatus: PrStatus | undefined;

	function handleBranchNameChange() {
		branchController.updateBranchName(branch.id, branch.name);
	}

	async function fetchPrStatus() {
		isFetching = true;
		try {
			prStatus = await githubService.getStatus($pr$?.targetBranch);
		} catch (e: any) {
			if (!e.message.includes('No commit found')) {
				toasts.error('Failed to update PR status');
				console.error(e);
			}
			prStatus = undefined;
		} finally {
			isFetching = false;
		}

		if (prStatus) scheduleNextPrFetch();
	}

	function scheduleNextPrFetch() {
		if (!prStatus || prStatus.completed) {
			return;
		}
		const startedAt = prStatus.startedAt;
		const secondsAgo = (new Date().getTime() - startedAt.getTime()) / 1000;

		let timeUntilUdate: number | undefined = undefined;
		if (secondsAgo < 600) {
			timeUntilUdate = 30;
		} else if (secondsAgo < 1200) {
			timeUntilUdate = 60;
		} else if (secondsAgo < 3600) {
			timeUntilUdate = 120;
		}

		if (!timeUntilUdate) {
			// Stop polling for status.
			return;
		}

		setTimeout(() => fetchPrStatus(), timeUntilUdate * 1000);
	}

	$: prColor = statusToColor(prStatus);
	$: prIcon = statusToIcon(prStatus);
	$: if ($pr$) fetchPrStatus();

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

	function statusToTooltip(status: PrStatus | undefined): string | undefined {
		if (!status) return;
		if (status && !status.hasChecks) return;
		if (status.completed) {
			return status.success ? 'All checks succeeded' : 'Some check(s) have failed';
		}
		return 'Checks are running';
	}

	const foldLine = () => {
		$isLaneCollapsed = true;
	};

	const unfoldLine = () => {
		$isLaneCollapsed = false;
	};

	$: hasIntegratedCommits = branch.commits?.some((b) => b.isIntegrated);
</script>

{#if $isLaneCollapsed}
	<div
		class="card collapsed-lane"
		on:dblclick={unfoldLine}
		on:keydown={(e) => e.key === 'Enter' && unfoldLine()}
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
				on:click={unfoldLine}
			/>
		</div>

		<div class="collapsed-lane__info">
			<h3 class="collapsed-lane__label text-base-13 text-bold">
				{branch.name}
			</h3>

			<div class="collapsed-lane__info__details">
				<ActiveBranchStatus
					{base}
					{branch}
					{isUnapplied}
					{hasIntegratedCommits}
					{isLaneCollapsed}
					prUrl={$pr$?.htmlUrl}
				/>
				{#if branch.selectedForChanges}
					<Tag color="pop" filled icon="target" verticalOrientation>Default branch</Tag>
				{/if}
			</div>
		</div>
	</div>
{:else}
	<!-- svelte-ignore a11y-no-static-element-interactions -->
	<div class="header__wrapper" on:dblclick={foldLine}>
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
						{base}
						{branch}
						{isUnapplied}
						{hasIntegratedCommits}
						{isLaneCollapsed}
						prUrl={$pr$?.htmlUrl}
					/>

					{#if branch.upstream && prIcon}
						<div
							class="pr-status"
							role="button"
							tabindex="0"
							on:click={fetchPrStatus}
							on:keypress={fetchPrStatus}
							use:tooltip={statusToTooltip(prStatus)}
						>
							{#if isFetching}
								<Icon name="spinner" />
							{:else}
								<Icon name={prIcon} color={prColor} />
							{/if}
						</div>
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
								on:click={async () => {
									await branchController.setSelectedForChanges(branch.id);
								}}
							>
								Set as default
							</Button>
						{/if}
					{/if}
					<!-- We can't show the merge button until we've waited for checks

                We use a octokit.checks.listForRef to find checks running for a PR, but right after
                creation this request succeeds but returns an empty array. So we need a better way
                determining "no checks will run for this PR" such that we can show the merge button
                immediately.  -->
					{#if $pr$ && !isFetching && (!prStatus || prStatus?.success)}
						<MergeButton
							{projectId}
							disabled={isUnapplied || !$pr$}
							loading={isMerging}
							help="Merge pull request and refresh"
							on:click={async (e) => {
								isMerging = true;
								const method = e.detail.method;
								try {
									if ($pr$) {
										await githubService.merge($pr$.number, method);
									}
								} catch {
									// TODO: Should we show the error from GitHub?
									toasts.error('Failed to merge pull request');
								} finally {
									isMerging = false;
									await fetchPrStatus();
									await branchService.reloadVirtualBranches();
								}
							}}
						/>
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
								on:click={foldLine}
							/>
							<Button
								icon="kebab"
								kind="outlined"
								color="neutral"
								on:click={() => {
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
								<BranchLanePopupMenu
									{branchController}
									{branch}
									{projectId}
									{isUnapplied}
									bind:visible
									on:action
								/>
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
		display: flex;
		cursor: grab;
		position: absolute;
		right: var(--space-4);
		top: var(--space-6);
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

	.pr-status {
		cursor: default;
	}

	/*  COLLAPSABLE LANE */

	.collapsed-lane {
		cursor: default;
		user-select: none;
		align-items: center;
		height: 100%;
		gap: var(--space-8);
		padding: var(--space-8) var(--space-8) var(--space-20);

		&:focus-within {
			outline: none;
		}
	}

	.collapsed-lane__actions {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-2);
	}

	.collapsed-lane__draggable {
		cursor: grab;
		transform: rotate(90deg);
		margin-bottom: var(--space-4);
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
		gap: var(--space-8);
	}

	.collapsed-lane__info__details {
		display: flex;
		flex-direction: row-reverse;
		align-items: center;
		gap: var(--space-4);
	}

	.collapsed-lane__label {
		color: var(--clr-theme-scale-ntrl-0);
		transform: rotate(180deg);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
