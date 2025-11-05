<script lang="ts">
	import CommitLine from '$components/CommitLine.svelte';
	import CommitTitle from '$components/CommitTitle.svelte';
	import { type CommitStatusType } from '$lib/commits/commit';
	import { URL_SERVICE } from '$lib/utils/url';
	import { inject } from '@gitbutler/core/context';
	import { Avatar, Icon, TestId } from '@gitbutler/ui';
	import { focusable } from '@gitbutler/ui/focus/focusable';

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
		editable?: boolean;
		gerritReviewUrl?: string;
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
		active,
		editable,
		gerritReviewUrl,
		onclick,
		menu,
		...args
	}: Props = $props();

	let container = $state<HTMLDivElement>();

	const urlService = inject(URL_SERVICE);

	function extractReviewId(url: string | undefined): string | null {
		if (!url) return null;
		// Extract review ID from URLs like: http://15a45d4cba1a/c/gerrit-test/+/41
		const match = url.match(/\+\/(\d+)/);
		return match?.[1] ?? null;
	}

	async function openGerritReview() {
		if (gerritReviewUrl) {
			await urlService.openExternalUrl(gerritReviewUrl);
		}
	}
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
	class:has-conflicts={hasConflicts}
	class:border-top={borderTop || first}
	class:last={lastCommit}
	class:disabled
	{onclick}
	use:focusable={{
		onAction: () => onclick?.(),
		focusable: true
	}}
>
	{#if selected}
		<div
			class="commit-row__select-indicator"
			class:active
			in:slide={{ axis: 'x', duration: 150 }}
		></div>
	{/if}

	{#if !selected && !args.disableCommitActions}
		<div class="commit-row__drag-handle">
			<Icon name="draggable-narrow" />
		</div>
	{/if}

	<CommitLine
		commitStatus={args.type}
		diverged={args.type === 'LocalAndRemote' ? (args.diverged ?? false) : false}
		{tooltip}
		{lastCommit}
		{lastBranch}
		{hasConflicts}
	/>

	<div class="commit-content" class:has-conflicts={hasConflicts}>
		{#if hasConflicts}
			<div class="commit-conflict-indicator">
				<Icon name="warning-small" />
			</div>
		{/if}

		{#if author}
			<div class="commit-author-avatar">
				<Avatar
					srcUrl={author.gravatarUrl}
					username={author.name}
					tooltip={`${author.name} (${author.email})`}
					size="medium"
				/>
			</div>
		{/if}

		<div class="commit-name truncate">
			<CommitTitle {commitMessage} truncate className="text-13 text-semibold" {editable} />
			{#if gerritReviewUrl}
				{@const reviewId = extractReviewId(gerritReviewUrl)}
				{#if reviewId}
					<!-- svelte-ignore a11y_click_events_have_key_events -->
					<div
						class="gerrit-review-pill"
						role="button"
						tabindex="0"
						onclick={openGerritReview}
						title="Open Gerrit review #{reviewId}"
					>
						<span class="text-10 text-semibold">{reviewId}</span>
					</div>
				{/if}
			{/if}
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
		background-color: var(--clr-bg-1);
		transition: background-color var(--transition-fast);

		&:hover,
		&.menu-shown {
			background-color: var(--clr-bg-1-muted);
		}

		&:hover .commit-row__drag-handle {
			opacity: 0.4;
			pointer-events: auto;
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

		&.has-conflicts {
			background-color: var(--clr-theme-err-bg);

			&:not(.selected):hover,
			&.menu-shown {
				background-color: var(--clr-theme-err-bg-muted);
			}

			.commit-name {
				color: var(--clr-theme-err-element);
			}
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

		&.has-conflicts {
			margin-left: -4px;
		}
	}

	.commit-name {
		display: flex;
		flex: 1;
		align-items: center;
		min-width: 0; /* Allow truncation to work properly */
		padding: 14px 0 14px 0;
		gap: 6px;
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

	.commit-row__drag-handle {
		display: flex;
		position: absolute;
		top: 50%;
		left: 0;
		transform: translateY(-50%);
		color: var(--clr-text-1);
		opacity: 0;
		pointer-events: none;
	}

	.gerrit-review-pill {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: fit-content;
		height: var(--size-icon);
		margin-left: 6px;
		padding: 0 4px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1-muted);
		color: var(--clr-text-2);
		line-height: 1;
		cursor: pointer;
		transition:
			opacity var(--transition-fast),
			background-color var(--transition-fast),
			border-color var(--transition-fast);

		&:hover {
			border-color: var(--clr-border-1);
			background-color: var(--clr-bg-1);
			color: var(--clr-text-1);
		}

		&:focus {
			outline: 2px solid var(--clr-focus-border);
			outline-offset: 1px;
		}
	}
</style>
