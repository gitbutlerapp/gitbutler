<script lang="ts">
	import CommitHeader from '$components/v3/CommitHeader.svelte';
	import CommitLine from '$components/v3/CommitLine.svelte';
	import ContextMenu from '$components/v3/ContextMenu.svelte';
	import { type CommitStatusType } from '$lib/commits/commit';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { slide } from 'svelte/transition';
	import type { Snippet } from 'svelte';

	type Props = {
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
	} & (
		| { type: 'LocalOnly'; stackId: string }
		| {
				type: 'LocalAndRemote';
				diverged: boolean;
				hasConflicts: boolean;
				stackId: string;
		  }
		| { type: 'Integrated'; stackId: string }
		| { type: 'Remote'; stackId: string }
		| { type: 'Base' }
	);

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
		disableCommitActions = false,
		...args
	}: Props = $props();

	let kebabMenuTrigger = $state<HTMLButtonElement>();
	let contextMenu = $state<ReturnType<typeof ContextMenu>>();

	let isOpenedByKebabButton = $state(false);
	let isOpenedByMouse = $state(false);
</script>

<div
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
		e.preventDefault();
		e.stopPropagation();
		if (disableCommitActions) return;
		onclick?.();
	}}
	onkeydown={(e) => {
		if (disableCommitActions) return;
		if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			onclick?.();
		}
	}}
	oncontextmenu={(e) => {
		if (disableCommitActions) return;
		e.preventDefault();
		isOpenedByKebabButton = false;
		contextMenu?.open(e);
	}}
>
	{#if selected}
		<div class="commit-row__select-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
	{/if}

	<CommitLine
		commitStatus={args.type}
		diverged={args.type === 'LocalAndRemote' ? args.diverged : false}
		{tooltip}
		{lastCommit}
		{lastBranch}
	/>

	<div class="commit-content">
		<!-- <button type="button" {onclick} tabindex="0"> -->
		<div class="commit-name truncate">
			<CommitHeader {commitMessage} row className="text-13 text-semibold" />
		</div>

		{#if args.type === 'LocalAndRemote' && args.hasConflicts}
			<div class="commit-conflict-indicator">
				<Icon name="warning" size={12} />
			</div>
		{/if}

		<button
			type="button"
			bind:this={kebabMenuTrigger}
			class="commit-menu-btn"
			class:activated={isOpenedByKebabButton}
			onmousedown={(e) => {
				e.preventDefault();
				e.stopPropagation();
				isOpenedByKebabButton = true;
				contextMenu?.toggle();
			}}
			onclick={(e) => {
				e.preventDefault();
				e.stopPropagation();
			}}
		>
			<Icon name="kebab" /></button
		>
	</div>
</div>

<ContextMenu bind:this={contextMenu} leftClickTrigger={kebabMenuTrigger}>
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
		padding: 14px 0 14px 4px;
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
		position: absolute;
		/* Account for the kebab menu that appears on hover */
		right: 42px;
		color: var(--clr-theme-err-element);
	}
</style>
