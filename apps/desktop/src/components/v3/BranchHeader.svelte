<script lang="ts">
	import BranchLabel from '$components/BranchLabel.svelte';
	import BranchRenameModal from '$components/BranchRenameModal.svelte';
	import BranchHeaderIcon from '$components/v3/BranchHeaderIcon.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { slide } from 'svelte/transition';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { Snippet } from 'svelte';

	export type Props = {
		type: 'draft-branch' | 'normal-branch' | 'stack-branch';
		el?: HTMLElement;
		projectId: string;
		branchName: string;
		readonly: boolean;
		iconName: keyof typeof iconsJson;
		lineColor: string;
		menuBtnEl?: HTMLButtonElement;
		isCommitting?: boolean;
	} & (
		| { type: 'draft-branch' }
		| {
				type: 'normal-branch';
				selected: boolean;
				trackingBranch?: string;
				details: Snippet;
				onclick: () => void;
		  }
		| {
				type: 'stack-branch';
				selected: boolean;
				stackId: string;
				trackingBranch?: string;
				isTopBranch: boolean;
				isMenuOpenByBtn?: boolean;
				isMenuOpenByMouse?: boolean;
				isNewBranch?: boolean;
				details: Snippet;
				onclick: () => void;
				onMenuBtnClick: () => void;
				onContextMenu: (e: MouseEvent) => void;
		  }
	);

	let {
		el = $bindable(),
		projectId,
		branchName,
		readonly,
		iconName,
		lineColor,
		isCommitting,
		menuBtnEl = $bindable(),
		...args
	}: Props = $props();

	const [stackService, uiState] = inject(StackService, UiState);

	const [updateName, nameUpdate] = stackService.updateBranchName;

	const isPushed = $derived(!!(args.type === 'draft-branch' ? undefined : args.trackingBranch));
	let renameBranchModal: BranchRenameModal | undefined = $state();

	async function updateBranchName(title: string) {
		if (args.type === 'draft-branch') {
			uiState.global.draftBranchName.set(title);
			const normalized = await stackService.normalizeBranchName(title);
			if (normalized.data) {
				uiState.global.draftBranchName.set(normalized.data);
			}
		} else if (args.type === 'stack-branch') {
			updateName({
				projectId,
				stackId: args.stackId,
				branchName,
				newName: title
			});
		}
	}
</script>

{#if args.type === 'stack-branch'}
	<div
		data-testid={TestId.BranchHeader}
		bind:this={el}
		role="button"
		class="branch-header"
		class:new-branch={args.isNewBranch}
		class:is-committing={isCommitting}
		class:selected={args.selected}
		onclick={args.onclick}
		oncontextmenu={(e) => {
			e.stopPropagation();
			e.preventDefault();
			args.onContextMenu(e);
		}}
		onkeypress={args.onclick}
		tabindex="0"
		class:activated={args.isMenuOpenByMouse || args.isMenuOpenByBtn}
	>
		{#if args.selected}
			<div class="branch-header__select-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
		{/if}

		<BranchHeaderIcon
			{lineColor}
			{iconName}
			lineTop={args.isTopBranch ? false : true}
			isDashed={args.isNewBranch}
		/>
		<div class="branch-header__content">
			<div class="name-line text-14 text-bold">
				<BranchLabel
					name={branchName}
					fontSize="15"
					disabled={nameUpdate.current.isLoading}
					readonly={readonly || isPushed}
					onChange={(name) => updateBranchName(name)}
					onDblClick={() => {
						if (isPushed) {
							renameBranchModal?.show();
						}
					}}
				/>

				<button
					bind:this={menuBtnEl}
					type="button"
					class="branch-menu-btn"
					class:activated={args.isMenuOpenByBtn}
					onmousedown={(e) => {
						e.stopPropagation();
						e.preventDefault();
						args.onMenuBtnClick();
					}}
					onclick={(e) => {
						e.stopPropagation();
						e.preventDefault();
					}}
				>
					<Icon name="kebab" />
				</button>
			</div>

			{#if args.isNewBranch}
				<p class="text-12 text-body branch-header__empty-state">
					<span>This is an empty branch.</span> <span>Click for details.</span>
					<br />
					Create or drag & drop commits here.
				</p>
			{/if}
		</div>
	</div>
	<BranchRenameModal
		{projectId}
		stackId={args.stackId}
		{branchName}
		bind:this={renameBranchModal}
		isPushed={!!args.trackingBranch}
	/>
{:else if args.type === 'normal-branch'}
	<div
		data-testid={TestId.BranchHeader}
		bind:this={el}
		role="button"
		class="branch-header"
		class:selected={args.selected}
		onclick={args.onclick}
		onkeypress={args.onclick}
		tabindex="0"
	>
		{#if args.selected}
			<div class="branch-header__select-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
		{/if}

		<BranchHeaderIcon {lineColor} {iconName} lineTop={false} />
		<div class="branch-header__content">
			<div class="name-line text-14 text-bold">
				<BranchLabel name={branchName} fontSize="15" readonly={true} />
			</div>
		</div>
	</div>
{:else}
	<div
		data-testid={TestId.BranchHeader}
		bind:this={el}
		role="button"
		class="branch-header new-branch draft selected"
		tabindex="0"
	>
		<BranchHeaderIcon {lineColor} {iconName} isDashed lineTop={false} />
		<div class="branch-header__content">
			<div class="name-line text-14 text-bold">
				<BranchLabel
					allowClear
					name={branchName}
					fontSize="15"
					onChange={(name) => updateBranchName(name)}
				/>
			</div>
			<p class="text-12 text-body branch-header__empty-state">
				A new branch will be created for your commit. You can click the branch name to change it now
				or later.
			</p>
		</div>
	</div>
{/if}

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
		}

		&:focus-within.selected {
			background-color: var(--clr-selected-in-focus-bg);
		}

		&.new-branch {
			border-bottom: none;
			border-radius: var(--radius-ml);
		}
		&.is-committing {
			border-radius: var(--radius-ml) var(--radius-ml) 0 0;
		}
	}

	.branch-header__select-indicator {
		position: absolute;
		top: 14px;
		left: 0;
		width: 4px;
		height: 20px;
		border-radius: 0 var(--radius-ml) var(--radius-ml) 0;
		background-color: var(--clr-selected-in-focus-element);
		transition: transform var(--transition-fast);
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
		gap: 8px;
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
