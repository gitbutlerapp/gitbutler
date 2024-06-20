<script lang="ts">
	import ActiveBranchStatus from './ActiveBranchStatus.svelte';
	import BranchLabel from './BranchLabel.svelte';
	import BranchLanePopupMenu from './BranchLanePopupMenu.svelte';
	import PullRequestButton from '../pr/PullRequestButton.svelte';
	import { Project } from '$lib/backend/projects';
	import { BranchService } from '$lib/branches/service';
	import { clickOutside } from '$lib/clickOutside';
	import { GitHubService } from '$lib/github/service';
	import { showError } from '$lib/notifications/toasts';
	import Button from '$lib/shared/Button.svelte';
	import Icon from '$lib/shared/Icon.svelte';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { error } from '$lib/utils/toasts';
	import { BranchController } from '$lib/vbranches/branchController';
	import { BaseBranch, Branch } from '$lib/vbranches/types';
	import type { PullRequest } from '$lib/github/types';
	import type { Persisted } from '$lib/persisted/persisted';
	import { goto } from '$app/navigation';

	export let uncommittedChanges = 0;
	export let isUnapplied = false;
	export let isLaneCollapsed: Persisted<boolean>;

	const branchController = getContext(BranchController);
	const githubService = getContext(GitHubService);
	const branchStore = getContextStore(Branch);
	const project = getContext(Project);
	const branchService = getContext(BranchService);
	const baseBranch = getContextStore(BaseBranch);

	$: branch = $branchStore;
	$: pr$ = githubService.getPr$(branch.upstream?.sha || branch.head);
	$: hasPullRequest = branch.upstreamName && $pr$;

	let meatballButton: HTMLDivElement;
	let visible = false;
	let isApplying = false;
	let isDeleting = false;
	let isLoading: boolean;
	let isTargetBranchAnimated = false;

	function handleBranchNameChange(title: string) {
		if (title === '') return;

		branchController.updateBranchName(branch.id, title);
	}

	function expandLane() {
		$isLaneCollapsed = false;
	}

	$: hasIntegratedCommits = branch.commits?.some((b) => b.isIntegrated);

	let headerInfoHeight = 0;

	interface CreatePrOpts {
		draft: boolean;
	}

	const defaultPrOpts: CreatePrOpts = {
		draft: true
	};

	async function createPr(createPrOpts: CreatePrOpts): Promise<PullRequest | undefined> {
		const opts = { ...defaultPrOpts, ...createPrOpts };
		if (!githubService.isEnabled) {
			error('Cannot create PR without GitHub credentials');
			return;
		}

		if (!$baseBranch?.shortName) {
			error('Cannot create PR without base branch');
			return;
		}

		isLoading = true;
		try {
			await branchService.createPr(branch, $baseBranch.shortName, opts.draft);
			await githubService.reload();
		} finally {
			isLoading = false;
		}
	}
</script>

{#if $isLaneCollapsed}
	<div
		class="card collapsed-lane"
		class:collapsed-lane_target-branch={branch.selectedForChanges}
		on:keydown={(e) => e.key === 'Enter' && expandLane()}
		tabindex="0"
		role="button"
	>
		<div class="collapsed-lane__actions">
			<div class="collapsed-lane__draggable" data-drag-handle>
				<Icon name="draggable" />
			</div>
			<Button style="ghost" outline icon="unfold-lane" help="Expand lane" on:click={expandLane} />
		</div>

		<div class="collapsed-lane__info-wrap" bind:clientHeight={headerInfoHeight}>
			<div class="collapsed-lane__info" style="width: {headerInfoHeight}px">
				<div class="collapsed-lane__label-wrap">
					<h3 class="collapsed-lane__label text-base-13 text-bold">
						{branch.name}
					</h3>
					{#if uncommittedChanges > 0}
						<Button
							size="tag"
							clickable={false}
							style="warning"
							kind="soft"
							help="Uncommitted changes"
						>
							{uncommittedChanges}
							{uncommittedChanges === 1 ? 'change' : 'changes'}
						</Button>
					{/if}
				</div>

				<div class="collapsed-lane__info__details">
					<ActiveBranchStatus
						{isUnapplied}
						{hasIntegratedCommits}
						remoteExists={!!branch.upstream}
						isLaneCollapsed={$isLaneCollapsed}
					/>
					{#if branch.selectedForChanges}
						<Button style="pop" kind="soft" size="tag" clickable={false} icon="target"
							>Default branch</Button
						>
					{/if}
				</div>
			</div>
		</div>
	</div>
{:else}
	<div class="header__wrapper">
		<div
			class="header card"
			class:header_target-branch={branch.selectedForChanges}
			class:header_target-branch-animation={isTargetBranchAnimated && branch.selectedForChanges}
		>
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
							{isUnapplied}
							{hasIntegratedCommits}
							remoteExists={!!branch.upstream}
							isLaneCollapsed={$isLaneCollapsed}
						/>

						{#await branch.isMergeable then isMergeable}
							{#if !isMergeable}
								<Button
									size="tag"
									clickable={false}
									icon="locked-small"
									style="warning"
									help="Applying this branch will add merge conflict markers that you will have to resolve"
								>
									Conflict
								</Button>
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
								kind="soft"
								help="New changes will land here"
								icon="target"
								clickable={false}
								disabled={isUnapplied}
							>
								Default branch
							</Button>
						{:else}
							<Button
								style="ghost"
								outline
								help="When selected, new changes will land here"
								icon="target"
								disabled={isUnapplied}
								on:click={async () => {
									isTargetBranchAnimated = true;
									await branchController.setSelectedForChanges(branch.id);
								}}
							>
								Set as default
							</Button>
						{/if}
					{/if}
				</div>

				<div class="relative">
					{#if isUnapplied}
						<Button
							style="ghost"
							outline
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
							outline
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
							{#if !hasPullRequest}
								<PullRequestButton
									on:exec={async (e) => await createPr({ draft: e.detail.action === 'draft' })}
									loading={isLoading}
								/>
							{/if}
							<Button
								element={meatballButton}
								style="ghost"
								outline
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
		<div class="header__top-overlay" data-remove-from-draggable data-tauri-drag-region></div>
	</div>
{/if}

<style>
	.header__wrapper {
		z-index: var(--z-lifted);
		position: sticky;
		top: 12px;
		padding-bottom: 8px;
	}
	.header {
		z-index: var(--z-lifted);
		position: relative;
		flex-direction: column;
		gap: 2px;
		transition:
			border-color 0.12s ease-in-out,
			box-shadow 0.12s ease-in-out;
	}
	.header_target-branch {
		border-color: var(--clr-theme-pop-element);
		box-shadow: 0 4px 0 var(--clr-theme-pop-element);
		margin-bottom: 4px;
	}
	.header_target-branch-animation {
		animation: setTargetAnimation 0.3s ease-in-out forwards;
	}
	@keyframes setTargetAnimation {
		0% {
		}
		40% {
			transform: scale(1.017) rotate(1deg);
		}
		50% {
			border-color: var(--clr-theme-pop-element);
			box-shadow: 0 4px 0 var(--clr-theme-pop-element);
			margin-bottom: 4px;
		}
		70%,
		100% {
			transform: scale(1);
			border-color: var(--clr-theme-pop-element);
			box-shadow: 0 4px 0 var(--clr-theme-pop-element);
			margin-bottom: 4px;
		}
	}

	.header__top-overlay {
		z-index: var(--z-ground);
		position: absolute;
		top: -16px;
		left: 0;
		width: 100%;
		height: 20px;
		background: var(--clr-bg-2);
	}
	.header__info-wrapper {
		display: flex;
		gap: 2px;
		padding: 10px;
	}
	.header__info {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		gap: 10px;
	}
	.header__actions {
		display: flex;
		gap: 4px;
		background: var(--clr-bg-1);
		border-top: 1px solid var(--clr-border-2);
		padding: 14px;
		justify-content: space-between;
		border-radius: 0 0 var(--radius-m) var(--radius-m);
		user-select: none;
	}

	.header__buttons {
		display: flex;
		position: relative;
		gap: 4px;
	}
	.draggable {
		display: flex;
		height: fit-content;
		cursor: grab;
		padding: 2px 2px 0 0;
		color: var(--clr-scale-ntrl-50);
		transition: color var(--transition-slow);

		&:hover {
			color: var(--clr-scale-ntrl-40);
		}
	}

	.branch-popup-menu {
		position: absolute;
		top: calc(100% + 4px);
		right: 0;
		z-index: var(--z-floating);
	}

	.header__remote-branch {
		color: var(--clr-scale-ntrl-50);
		padding-left: 2px;
		padding-right: 2px;
		display: flex;
		gap: 4px;
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
		width: 48px;
		overflow: hidden;
		gap: 8px;
		padding: 8px 8px 20px;

		&:focus-within {
			outline: none;
		}
	}

	.collapsed-lane_target-branch {
		border-color: var(--clr-theme-pop-element);
	}

	.collapsed-lane__actions {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 2px;
	}

	.collapsed-lane__draggable {
		cursor: grab;
		transform: rotate(90deg);
		margin-bottom: 4px;
		opacity: 0.4;
		transition: opacity var(--transition-fast);
		color: var(--clr-scale-ntrl-0);

		&:hover {
			opacity: 1;
		}
	}

	/*  */

	.collapsed-lane__info-wrap {
		display: flex;
		height: 100%;
	}

	.collapsed-lane__info {
		display: flex;
		justify-content: space-between;
		gap: 8px;
		transform: rotate(-90deg);
		direction: ltr;
	}

	/*  */

	.collapsed-lane__info__details {
		display: flex;
		flex-direction: row-reverse;
		align-items: center;
		gap: 4px;
	}

	.collapsed-lane__label-wrap {
		overflow: hidden;
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.collapsed-lane__label {
		color: var(--clr-scale-ntrl-0);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
