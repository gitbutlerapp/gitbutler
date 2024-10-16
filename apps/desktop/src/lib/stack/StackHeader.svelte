<script lang="ts">
	import ActiveBranchStatus from '$lib/branch/ActiveBranchStatus.svelte';
	import BranchLabel from '$lib/branch/BranchLabel.svelte';
	import BranchLaneContextMenu from '$lib/branch/BranchLaneContextMenu.svelte';
	import DefaultTargetButton from '$lib/branch/DefaultTargetButton.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranch } from '$lib/vbranches/types';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { Persisted } from '@gitbutler/shared/persisted';

	interface Props {
		uncommittedChanges?: number;
		isLaneCollapsed: Persisted<boolean>;
		stackPrs?: number;
	}

	const { uncommittedChanges = 0, isLaneCollapsed, stackPrs = 0 }: Props = $props();

	const branchController = getContext(BranchController);
	const branchStore = getContextStore(VirtualBranch);
	const branch = $derived($branchStore);

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let meatballButtonEl = $state<HTMLDivElement>();
	let isTargetBranchAnimated = $state(false);

	function handleBranchNameChange(title: string) {
		if (title === '') return;

		branchController.updateBranchName(branch.id, title);
	}

	function expandLane() {
		$isLaneCollapsed = false;
	}

	function collapseLane() {
		$isLaneCollapsed = true;
	}

	const hasIntegratedCommits = $derived(branch.commits?.some((b) => b.isIntegrated));

	let headerInfoHeight = $state(0);
</script>

{#if $isLaneCollapsed}
	<div
		class="card collapsed-lane"
		class:collapsed-lane_target-branch={branch.selectedForChanges}
		onkeydown={(e) => e.key === 'Enter' && expandLane()}
		tabindex="0"
		role="button"
	>
		<div class="collapsed-lane__actions">
			<div class="draggable" data-drag-handle>
				<Icon name="draggable" />
			</div>
			<Button style="ghost" outline icon="unfold-lane" tooltip="Expand lane" onclick={expandLane} />
		</div>

		<div class="collapsed-lane__info-wrap" bind:clientHeight={headerInfoHeight}>
			<div class="collapsed-lane__info" style="width: {headerInfoHeight}px">
				<div class="collapsed-lane__label-wrap">
					<h3 class="collapsed-lane__label text-13 text-bold">
						{branch.name}
					</h3>
					{#if uncommittedChanges > 0}
						<Button
							size="tag"
							clickable={false}
							style="warning"
							kind="soft"
							tooltip="Uncommitted changes"
						>
							{uncommittedChanges}
							{uncommittedChanges === 1 ? 'change' : 'changes'}
						</Button>
					{/if}
				</div>

				<div class="collapsed-lane__info__details">
					<ActiveBranchStatus
						{hasIntegratedCommits}
						remoteExists={!!branch.upstream}
						isLaneCollapsed={$isLaneCollapsed}
					/>
					{#if branch.selectedForChanges}
						<Button style="pop" kind="soft" size="tag" clickable={false} icon="target">
							Default branch
						</Button>
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
				<div data-drag-handle>
					<Icon name="draggable" />
				</div>

				<div class="header__info">
					<div class="header__info-row spread">
						<BranchLabel name={branch.name} onChange={(name) => handleBranchNameChange(name)} />
						<Button
							bind:el={meatballButtonEl}
							style="ghost"
							size="tag"
							icon="kebab"
							onclick={() => {
								contextMenu?.toggle();
							}}
						/>
						<BranchLaneContextMenu
							bind:contextMenuEl={contextMenu}
							target={meatballButtonEl}
							onCollapse={collapseLane}
							hasPr={false}
						/>
					</div>
					<div class="header__info-row">
						<span class="button-group">
							<DefaultTargetButton
								size="tag"
								selectedForChanges={branch.selectedForChanges}
								onclick={async () => {
									isTargetBranchAnimated = true;
									await branchController.setSelectedForChanges(branch.id);
								}}
							/>
							<Button
								style="neutral"
								icon="remote-branch-small"
								size="tag"
								clickable={false}
								tooltip="Series"
							>
								{branch.series.length ?? 0}
							</Button>
							<Button
								style="neutral"
								icon="pr-small"
								size="tag"
								clickable={false}
								tooltip="Pull Requests"
							>
								{stackPrs}
							</Button>
						</span>
					</div>
				</div>
			</div>
		</div>
		<div class="header__top-overlay" data-remove-from-draggable data-tauri-drag-region></div>
	</div>
{/if}

<style>
	.header__wrapper {
		z-index: var(--z-lifted);
		top: 12px;
		padding-bottom: unset !important;
		position: sticky;
	}
	.header.card {
		border-bottom-right-radius: 0px;
		border-bottom-left-radius: 0px;
		border-bottom-width: 1px;
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
		padding: 12px;
	}
	.header__info {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		justify-content: center;
		align-items: start;
		gap: 10px;
	}
	.header__info-row {
		width: 100%;
		display: flex;
		justify-content: start;
		align-items: center;

		&.spread {
			justify-content: space-between;
		}
	}
	.button-group {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.draggable {
		display: flex;
		height: fit-content;
		align-items: center;
		cursor: grab;
		padding: 2px 2px 0 0;
		color: var(--clr-scale-ntrl-50);
		transition: color var(--transition-slow);

		&:hover {
			color: var(--clr-scale-ntrl-40);
		}
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
		gap: 10px;
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
