<script lang="ts">
	import BranchLabel from '$components/BranchLabel.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchHeaderIcon from '$components/v3/BranchHeaderIcon.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { CommitStateType, StackBranch } from '$lib/branches/v3';
	import type { Snippet } from 'svelte';

	interface Props {
		el?: HTMLElement;
		projectId: string;
		stackId: string;
		branch: StackBranch;
		selected: boolean;
		isTopBranch: boolean;
		readonly: boolean;
		lineColor?: string;
		menuBtnEl?: HTMLButtonElement;
		isMenuOpenByBtn?: boolean;
		isMenuOpenByMouse?: boolean;
		isNewBranch?: boolean;
		details?: Snippet;
		onclick: () => void;
		onLabelDblClick?: () => void;
		onMenuBtnClick: () => void;
		onContextMenu: (e: MouseEvent) => void;
	}

	let {
		el = $bindable(),
		projectId,
		stackId,
		branch,
		isTopBranch,
		readonly,
		selected,
		menuBtnEl = $bindable(),
		isMenuOpenByBtn,
		isMenuOpenByMouse,
		isNewBranch,
		details,
		onclick,
		onLabelDblClick,
		onMenuBtnClick,
		onContextMenu
	}: Props = $props();

	const [stackService] = inject(StackService);

	const topCommitResult = $derived(stackService.commitAt(projectId, stackId, branch.name, 0));
	const [updateName, nameUpdate] = stackService.updateBranchName;

	function updateBranchName(title: string) {
		updateName({
			projectId,
			stackId,
			branchName: branch.name,
			newName: title
		});
	}
</script>

<div
	bind:this={el}
	role="button"
	class="branch-header"
	class:new-branch={isNewBranch}
	class:selected
	{onclick}
	oncontextmenu={(e) => {
		e.stopPropagation();
		e.preventDefault();
		onContextMenu(e);
	}}
	onkeypress={onclick}
	tabindex="0"
	class:activated={isMenuOpenByMouse || isMenuOpenByBtn}
>
	<ReduxResult result={topCommitResult.current}>
		{#snippet children(commit)}
			{@const branchType: CommitStateType = commit?.state.type ?? 'LocalOnly'}

			<BranchHeaderIcon {commit} lineTop={isTopBranch ? false : true} />
			<div class="branch-header__content">
				<div class="name-line text-14 text-bold">
					<BranchLabel
						name={branch.name}
						fontSize="15"
						disabled={nameUpdate.current.isLoading}
						readonly={readonly || !!branch.remoteTrackingBranch}
						onChange={(name) => updateBranchName(name)}
						onDblClick={() => {
							if (branchType !== 'Integrated') {
								onLabelDblClick?.();
							}
						}}
					/>

					<button
						bind:this={menuBtnEl}
						type="button"
						class="branch-menu-btn"
						class:activated={isMenuOpenByBtn}
						onmousedown={(e) => {
							e.stopPropagation();
							e.preventDefault();
							onMenuBtnClick();
						}}
						onclick={(e) => {
							e.stopPropagation();
							e.preventDefault();
						}}
					>
						<Icon name="kebab" />
					</button>
				</div>

				{#if isNewBranch}
					<p class="text-12 text-body branch-header__empty-state">
						<span>This is an empty branch.</span> <span>Click for details.</span>
						<br />
						Create or drag & drop commits here.
					</p>
				{:else}
					{@render details?.()}
				{/if}
			</div>
		{/snippet}
	</ReduxResult>
</div>

<style lang="postcss">
	.branch-header {
		position: relative;
		display: flex;
		justify-content: flex-start;
		align-items: center;
		padding-right: 10px;
		border-top-right-radius: var(--radius-ml);
		border-top-left-radius: var(--radius-ml);
		border-bottom: 1px solid var(--clr-border-2);
		overflow: hidden;

		&:before {
			content: '';
			position: absolute;
			top: 14px;
			left: 0;
			width: 4px;
			height: 20px;
			transform: translateX(-100%);
			border-radius: 0 var(--radius-ml) var(--radius-ml) 0;
			background-color: var(--clr-selected-in-focus-element);
			transition: transform var(--transition-fast);
		}

		&:hover,
		&.activated {
			background-color: var(--clr-bg-1-muted);

			& .branch-menu-btn {
				display: flex;
			}
		}

		&:focus-within,
		&.selected {
			background-color: var(--clr-selected-not-in-focus-bg);

			& .branch-menu-btn {
				display: flex;
			}

			&:before {
				transform: translateX(0%);
			}
		}

		&:focus-within.selected {
			background-color: var(--clr-selected-in-focus-bg);
		}

		&.new-branch {
			border-bottom: none;
			border-radius: var(--radius-ml);
		}
	}

	.name-line {
		display: flex;
		align-items: center;
		justify-content: space-between;
		min-width: 0;
		flex-grow: 1;
	}

	.branch-header__content {
		overflow: hidden;
		flex: 1;
		width: 100%;
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 14px 0;
		margin-left: -2px;
		text-overflow: ellipsis;
	}

	.branch-menu-btn {
		display: none;
		padding: 0 4px;
		color: var(--clr-text-1);
		opacity: 0.5;

		&:hover {
			opacity: 1;
		}

		&.activated {
			display: flex;
			opacity: 1;
		}
	}

	.branch-header__empty-state {
		padding: 4px;
		opacity: 0.8;
		color: var(--clr-text-2);
		margin-top: -8px;

		& span {
			text-wrap: nowrap;
		}
	}
</style>
