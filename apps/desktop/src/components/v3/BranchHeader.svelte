<script lang="ts">
	import BranchLabel from '$components/BranchLabel.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import SeriesDescription from '$components/SeriesDescription.svelte';
	import BranchHeaderIcon from '$components/v3/BranchHeaderIcon.svelte';
	import { getColorFromBranchType } from '$components/v3/lib';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { CommitStateType, StackBranch } from '$lib/branches/v3';
	import type { Snippet } from 'svelte';

	interface Props {
		projectId: string;
		stackId: string;
		branch: StackBranch;
		selected: boolean;
		isTopBranch: boolean;
		readonly: boolean;
		lineColor?: string;
		menuBtnEl?: HTMLButtonElement;
		isMenuOpen?: boolean;
		details?: Snippet;
		onclick: () => void;
		onLabelDblClick?: () => void;
		onMenuClick: () => void;
	}

	let {
		projectId,
		stackId,
		branch,
		isTopBranch,
		readonly,
		selected,
		menuBtnEl = $bindable(),
		isMenuOpen,
		details,
		onclick,
		onLabelDblClick,
		onMenuClick
	}: Props = $props();

	const [stackService] = inject(StackService);

	const topCommitResult = $derived(stackService.commitAt(projectId, stackId, branch.name, 0));

	function editTitle(title: string) {
		console.error('FIXME', title);
	}
</script>

<div class="branch-header" class:selected {onclick} role="button" onkeypress={onclick} tabindex="0">
	<ReduxResult result={topCommitResult.current}>
		{#snippet children(commit)}
			{@const branchType: CommitStateType = commit?.state.type ?? 'LocalOnly'}

			<BranchHeaderIcon {branchType} lineTop={isTopBranch ? false : true} />
			<div class="branch-header__content">
				<div class="name-line text-14 text-bold">
					<BranchLabel
						name={branch.name}
						fontSize="15"
						readonly={readonly || !!branch.remoteTrackingBranch}
						onChange={(name) => editTitle(name)}
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
						class:activated={isMenuOpen}
						onmousedown={onMenuClick}
						onclick={(e) => {
							e.stopPropagation();
							e.preventDefault();
						}}
					>
						<Icon name="kebab" />
					</button>
				</div>

				{@render details?.()}
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

		&:hover {
			background-color: var(--clr-bg-1-muted);

			& .branch-menu-btn {
				display: flex;
			}
		}

		&.selected {
			background-color: var(--clr-selected-not-in-focus-bg);
		}

		&:focus-within.selected {
			background-color: var(--clr-selected-in-focus-bg);
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
</style>
