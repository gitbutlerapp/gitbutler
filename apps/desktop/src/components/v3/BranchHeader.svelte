<script lang="ts">
	import BranchLabel from '$components/BranchLabel.svelte';
	import BranchRenameModal from '$components/BranchRenameModal.svelte';
	import DeleteBranchModal from '$components/DeleteBranchModal.svelte';
	import BranchBadge from '$components/v3/BranchBadge.svelte';
	import BranchHeaderIcon from '$components/v3/BranchHeaderIcon.svelte';
	import ContextMenu from '$components/v3/ContextMenu.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import ReviewBadge from '@gitbutler/ui/ReviewBadge.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import { slide } from 'svelte/transition';
	import type { PushStatus } from '$lib/stacks/stack';
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
				lastUpdatedAt?: number;
				isTopBranch?: boolean;
				onclick: () => void;
		  }
		| {
				type: 'stack-branch';
				selected: boolean;
				stackId: string;
				trackingBranch?: string;
				isTopBranch: boolean;
				isNewBranch?: boolean;
				prNumber?: number;
				reviewId?: string;
				pushStatus: PushStatus;
				lastUpdatedAt?: number;
				isCommitting: boolean;
				isConflicted: boolean;
				onclick: () => void;
				menu?: Snippet<
					[
						{
							onToggle: (open: boolean, isLeftClick: boolean) => void;
							showBranchRenameModal: () => void;
							showDeleteBranchModal: () => void;
						}
					]
				>;
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

	let contextMenu = $state<ContextMenu>();
	let rightClickTrigger = $state<HTMLDivElement>();
	let leftClickTrigger = $state<HTMLButtonElement>();

	let isMenuOpenByBtn = $state(false);
	let isMenuOpenByMouse = $state(false);

	let renameBranchModal = $state<BranchRenameModal>();
	let deleteBranchModal = $state<DeleteBranchModal>();

	function showBranchRenameModal() {
		renameBranchModal?.show();
	}

	function showDeleteBranchModal() {
		deleteBranchModal?.show();
	}

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

	function onToggle(isOpen: boolean, isLeftClick: boolean) {
		if (isLeftClick) {
			isMenuOpenByBtn = isOpen;
		} else {
			isMenuOpenByMouse = isLeftClick;
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
			contextMenu?.toggle(e);
		}}
		onkeypress={args.onclick}
		tabindex="0"
		class:activated={isMenuOpenByMouse || isMenuOpenByBtn}
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
					class:activated={isMenuOpenByBtn}
					onmousedown={(e) => {
						e.stopPropagation();
						e.preventDefault();
						contextMenu?.toggle();
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
			{:else}
				<div class="text-12 branch-header__details">
					<span class="branch-header__item">
						<BranchBadge pushStatus={args.pushStatus} unstyled />
					</span>
					<span class="branch-header__divider">•</span>

					{#if args.isConflicted}
						<span class="branch-header__item branch-header__item--conflict"> Has conflicts </span>
						<span class="branch-header__divider">•</span>
					{/if}

					{#if args.lastUpdatedAt}
						<span class="branch-header__item">
							{getTimeAgo(new Date(args.lastUpdatedAt))}
						</span>
					{/if}

					{#if args.reviewId || args.prNumber}
						<span class="branch-header__divider">•</span>
						<div class="branch-header__review-badges">
							{#if args.reviewId}
								<ReviewBadge brId={args.reviewId} brStatus="unknown" />
							{/if}
							{#if args.prNumber}
								<ReviewBadge prNumber={args.prNumber} prStatus="unknown" />
							{/if}
						</div>
					{/if}
				</div>
			{/if}
		</div>
	</div>
	<ContextMenu
		testId={TestId.BranchHeaderContextMenu}
		bind:this={contextMenu}
		{leftClickTrigger}
		{rightClickTrigger}
		ontoggle={(isOpen, isLeftClick) => {
			onToggle?.(isOpen, isLeftClick);
		}}
	>
		{@render args.menu?.({
			onToggle,
			showBranchRenameModal,
			showDeleteBranchModal
		})}
	</ContextMenu>
	<BranchRenameModal
		{projectId}
		stackId={args.stackId}
		{branchName}
		bind:this={renameBranchModal}
		isPushed={!!args.trackingBranch}
	/>
	<DeleteBranchModal
		{projectId}
		stackId={args.stackId}
		{branchName}
		bind:this={deleteBranchModal}
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

		<BranchHeaderIcon {lineColor} {iconName} lineTop={!args.isTopBranch} />
		<div class="branch-header__content">
			<div class="name-line text-14 text-bold">
				<BranchLabel name={branchName} fontSize="15" readonly={true} />
			</div>
		</div>
		<div class="text-12 branch-header__details">
			{#if args.lastUpdatedAt}
				<span class="branch-header__item">
					{getTimeAgo(new Date(args.lastUpdatedAt))}
				</span>
			{/if}
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
		<BranchHeaderIcon {lineColor} {iconName} isDashed lineTop />
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
		--branch-selected-bg: var(--clr-bg-1);
		--branch-selected-element-bg: var(--clr-selected-not-in-focus-element);

		position: relative;
		display: flex;
		justify-content: flex-start;
		align-items: center;
		padding-right: 10px;
		border-top-right-radius: var(--radius-ml);
		border-top-left-radius: var(--radius-ml);
		border-bottom: 1px solid var(--clr-border-2);
		overflow: hidden;
		background-color: var(--branch-selected-bg);

		/* show menu button on hover or if selected */
		&:hover,
		&.selected {
			& .branch-menu-btn {
				display: flex; /* show menu button on hover */
			}
		}

		/* Selected but NOT in focus */
		&:hover,
		&.activated {
			--branch-selected-bg: var(--clr-bg-1-muted);
		}
		&:focus-within,
		&.selected {
			--branch-selected-bg: var(--clr-selected-not-in-focus-bg);
		}

		/* Selected in focus */
		&:focus-within.selected {
			--branch-selected-bg: var(--clr-selected-in-focus-bg);
			--branch-selected-element-bg: var(--clr-selected-in-focus-element);
		}

		/* MODIFIERS */
		&.new-branch {
			border-bottom: none;
			border-radius: var(--radius-ml);
		}
		&.is-committing {
			border-radius: var(--radius-ml) var(--radius-ml) 0 0;
		}
	}

	.branch-header__details {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 6px;
		color: var(--clr-text-2);
		margin-left: 4px;
	}

	.branch-header__select-indicator {
		position: absolute;
		top: 14px;
		left: 0;
		width: 4px;
		height: 20px;
		border-radius: 0 var(--radius-ml) var(--radius-ml) 0;
		background-color: var(--branch-selected-element-bg);
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
	.branch-header__review-badges {
		display: flex;
		gap: 3px;
	}

	.branch-header__item {
		white-space: nowrap;
		color: var(--clr-text-2);
	}

	.branch-header__item--conflict {
		color: var(--clr-theme-err-element);
	}

	.branch-header__divider {
		color: var(--clr-text-3);
	}
</style>
