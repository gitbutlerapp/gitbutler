<script lang="ts">
	import CommitHeader from '$components/v3/CommitHeader.svelte';
	import CommitLine from '$components/v3/CommitLine.svelte';
	import ContextMenu from '$components/v3/ContextMenu.svelte';
	import { type CommitStatusType } from '$lib/commits/commit';
	import { TestId } from '$lib/testing/testIds';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { slide } from 'svelte/transition';
	import type { Snippet } from 'svelte';

	type BaseProps = {
		type: CommitStatusType;
		projectId: string;
		branchName: string;
		commitId: string;
		commitMessage: string;
		createdAt: number;
		tooltip?: string;
		first?: boolean;
		lastCommit?: boolean;
		lastBranch?: boolean;
		selected?: boolean;
		opacity?: number;
		borderTop?: boolean;
		draggable?: boolean;
		disableCommitActions?: boolean;
		menu?: Snippet<[{ close: () => void }]>;
		onclick?: () => void;
	};

	type RemoteStatusProps = {
		type: 'LocalOnly' | 'Integrated' | 'Remote';
	};

	type LocalAndRemoteWithActions = {
		type: 'LocalAndRemote';
		disableCommitActions: false;
		diverged: boolean;
		hasConflicts: boolean;
	};

	type LocalAndRemoteDisabled = {
		type: 'LocalAndRemote';
		disableCommitActions: true;
	};

	type WithStackId = {
		disableCommitActions: false;
		stackId: string;
	};

	type WithoutStackId = {
		disableCommitActions: true;
	};

	type Props = BaseProps &
		(RemoteStatusProps | LocalAndRemoteWithActions | LocalAndRemoteDisabled) &
		(WithStackId | WithoutStackId);

	const {
		commitMessage,
		tooltip,
		first,
		lastCommit,
		lastBranch,
		selected,
		opacity,
		borderTop,
		onclick,
		menu: menu2,
		...args
	}: Props = $props();

	let kebabMenuTrigger = $state<HTMLButtonElement>();
	let container = $state<HTMLDivElement>();
	let contextMenu = $state<ReturnType<typeof ContextMenu>>();

	let isOpenedByKebabButton = $state(false);
	let isOpenedByMouse = $state(false);

	let isConflicted = $derived(
		args.type === 'LocalAndRemote' && !args.disableCommitActions && args.hasConflicts
	);
</script>

<div
	bind:this={container}
	role="button"
	tabindex="0"
	aria-label="Commit row"
	class="commit-row"
	class:menu-shown={isOpenedByKebabButton || isOpenedByMouse}
	class:first
	class:selected
	style:opacity
	class:border-top={borderTop || first}
	class:last={lastCommit}
	onclick={(e) => {
		e.stopPropagation();
		onclick?.();
	}}
	onkeydown={(e) => {
		if (e.key === 'Enter' || e.key === ' ') {
			e.stopPropagation();
			onclick?.();
		}
	}}
	oncontextmenu={(e) => {
		if (args.disableCommitActions) return;
		e.preventDefault();
		contextMenu?.open(e);
	}}
>
	{#if selected}
		<div class="commit-row__select-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
	{/if}

	<CommitLine
		commitStatus={args.type}
		diverged={args.type === 'LocalAndRemote' && !args.disableCommitActions ? args.diverged : false}
		{tooltip}
		{lastCommit}
		{lastBranch}
	/>

	<div data-testid={TestId.CommitRow} class="commit-content" class:shift-to-left={isConflicted}>
		{#if isConflicted}
			<div class="commit-conflict-indicator">
				<Icon name="warning" size={12} />
			</div>
		{/if}

		<div class="commit-name truncate">
			<CommitHeader {commitMessage} row className="text-13 text-semibold" />
		</div>

		{#if !args.disableCommitActions}
			<button
				type="button"
				bind:this={kebabMenuTrigger}
				class="commit-menu-btn"
				data-testid={TestId.CommitMenuButton}
				class:activated={isOpenedByKebabButton}
				onmousedown={(e) => {
					e.stopPropagation();
					contextMenu?.toggle();
				}}
			>
				<Icon name="kebab" /></button
			>
		{/if}
	</div>
</div>

<ContextMenu
	bind:this={contextMenu}
	leftClickTrigger={kebabMenuTrigger}
	rightClickTrigger={container}
	bind:isOpenedByKebabButton
	bind:isOpenedByMouse
>
	{#snippet menu(args)}
		{@render menu2?.(args)}
	{/snippet}
</ContextMenu>

<style lang="postcss">
	.commit-row {
		position: relative;
		display: flex;
		width: 100%;
		overflow: hidden;
		transition: background-color var(--transition-fast);

		&:hover,
		&.menu-shown {
			background-color: var(--clr-bg-1-muted);

			& .commit-menu-btn {
				display: flex;
			}
		}

		&:not(.last) {
			border-bottom: 1px solid var(--clr-border-2);
		}

		&.last {
			border-radius: 0 0 var(--radius-ml) var(--radius-ml);
		}

		&:focus-within,
		&.selected {
			background-color: var(--clr-selected-not-in-focus-bg);

			& .commit-menu-btn {
				display: flex;
			}
		}

		&:focus-within.selected {
			background-color: var(--clr-selected-in-focus-bg);
		}
	}

	.commit-row__select-indicator {
		position: absolute;
		top: 50%;
		left: 0;
		width: 4px;
		height: 45%;
		transform: translateY(-50%);
		border-radius: 0 var(--radius-ml) var(--radius-ml) 0;
		background-color: var(--clr-selected-in-focus-element);
		transition: transform var(--transition-fast);
	}

	.commit-content {
		display: flex;
		align-items: center;
		position: relative;
		gap: 4px;
		width: 100%;
		overflow: hidden;
		padding-right: 10px;
	}

	.commit-name {
		flex: 1;
		padding: 14px 0 14px 0;
		display: flex;
	}

	.commit-menu-btn {
		display: none;
		padding: 3px;
		color: var(--clr-text-1);
		opacity: 0.5;
		transition: opacity var(--transition-fast);

		&:hover,
		&.activated {
			opacity: 1;
		}
	}

	.commit-conflict-indicator {
		display: flex;
		color: var(--clr-theme-err-element);
		margin-right: 4px;
	}

	/* MODIFIERS */
	.shift-to-left {
		margin-left: -3px;
	}
</style>
