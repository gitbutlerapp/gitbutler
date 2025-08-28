<script lang="ts">
	import CommitLine from '$components/CommitLine.svelte';
	import CommitTitle from '$components/CommitTitle.svelte';
	import { type CommitStatusType } from '$lib/commits/commit';
	import { DefinedFocusable } from '$lib/focus/focusManager';
	import { focusable } from '$lib/focus/focusable';
	import { Avatar, Icon, TestId } from '@gitbutler/ui';

	import { slide } from 'svelte/transition';
	import type { Snippet } from 'svelte';

	type BaseProps = {
		type: CommitStatusType;
		branchName: string;
		commitId: string;
		commitMessage: string;
		createdAt: number;
		author?: { name: string; email: string; gravatarUrl: string };
		tooltip?: string;
		first?: boolean;
		lastCommit?: boolean;
		lastBranch?: boolean;
		selected?: boolean;
		opacity?: number;
		borderTop?: boolean;
		disableCommitActions?: boolean;
		isOpen?: boolean;
		active?: boolean;
		hasConflicts?: boolean;
		disabled?: boolean;
		menu?: Snippet<[{ rightClickTrigger: HTMLElement }]>;
		onclick?: () => void;
	};

	type RemoteStatusProps = {
		type: 'LocalOnly' | 'Integrated' | 'Remote';
	};

	type LocalAndRemoteWithActions = {
		type: 'LocalAndRemote';
		disableCommitActions: false;
		diverged: boolean;
	};

	type LocalAndRemoteDisabled = {
		type: 'LocalAndRemote';
		disableCommitActions: true;
		diverged: boolean;
	};

	type WithStackId = {
		disableCommitActions: false;
		stackId?: string;
	};

	type WithoutStackId = {
		disableCommitActions: true;
	};

	type Props = BaseProps &
		(RemoteStatusProps | LocalAndRemoteWithActions | LocalAndRemoteDisabled) &
		(WithStackId | WithoutStackId);

	const {
		commitMessage,
		author,
		tooltip,
		first,
		lastCommit,
		lastBranch,
		selected,
		opacity,
		borderTop,
		isOpen,
		disabled,
		hasConflicts,
		onclick,
		menu,
		...args
	}: Props = $props();

	const active = $derived(selected);
	let container = $state<HTMLDivElement>();
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
	data-testid={TestId.CommitRow}
	bind:this={container}
	role="button"
	tabindex="0"
	aria-label="Commit row"
	class="commit-row"
	class:menu-shown={isOpen}
	class:first
	class:selected
	class:active
	style:opacity
	class:border-top={borderTop || first}
	class:last={lastCommit}
	class:disabled
	{onclick}
	use:focusable={{
		id: DefinedFocusable.Commit,
		onKeydown: (e) => {
			if (disabled) return false;

			if (e.key === 'Enter' || e.key === ' ' || (!e.metaKey && e.key === 'ArrowRight')) {
				e.stopPropagation();
				onclick?.();
				return true;
			}
		}
	}}
>
	{#if selected}
		<div
			class="commit-row__select-indicator"
			class:active
			in:slide={{ axis: 'x', duration: 150 }}
		></div>
	{/if}

	<CommitLine
		commitStatus={args.type}
		diverged={args.type === 'LocalAndRemote' ? (args.diverged ?? false) : false}
		{tooltip}
		{lastCommit}
		{lastBranch}
	/>

	<div class="commit-content">
		{#if hasConflicts}
			<div class="commit-conflict-indicator">
				<Icon name="warning" size={12} />
			</div>
		{/if}

		{#if author}
			<div class="commit-author-avatar">
				<Avatar
					srcUrl={author.gravatarUrl}
					tooltip={`${author.name} (${author.email})`}
					size="medium"
				/>
			</div>
		{/if}

		<div class="commit-name truncate">
			<CommitTitle {commitMessage} truncate className="text-13 text-semibold" />
		</div>

		{#if !args.disableCommitActions}
			{@render menu?.({ rightClickTrigger: container })}
		{/if}
	</div>
</div>

<style lang="postcss">
	.commit-row {
		display: flex;
		position: relative;
		width: 100%;
		overflow: hidden;
		outline: none;
		transition: background-color var(--transition-fast);

		&:hover,
		&.menu-shown {
			background-color: var(--clr-bg-1-muted);
		}

		&:not(.last) {
			border-bottom: 1px solid var(--clr-border-2);
		}

		&.selected {
			background-color: var(--clr-selected-not-in-focus-bg);
		}

		&.active.selected {
			background-color: var(--clr-selected-in-focus-bg);
		}

		&.disabled {
			pointer-events: none;
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
		background-color: var(--clr-selected-not-in-focus-element);
		transition: transform var(--transition-fast);
		&.active {
			background-color: var(--clr-selected-in-focus-element);
		}
	}

	.commit-content {
		display: flex;
		position: relative;
		align-items: center;
		width: 100%;
		padding-right: 9px;
		overflow: hidden;
		gap: 4px;
	}

	.commit-name {
		display: flex;
		flex: 1;
		padding: 14px 0 14px 0;
	}

	.commit-author-avatar {
		display: flex;
		margin-right: 8px;
	}

	.commit-conflict-indicator {
		display: flex;
		margin-right: 4px;
		color: var(--clr-theme-err-element);
	}
</style>
