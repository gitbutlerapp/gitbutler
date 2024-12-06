<script lang="ts">
	import Tooltip from './Tooltip.svelte';
	import Icon from '$lib/Icon.svelte';
	import SeriesLabelsRow from '$lib/SeriesLabelsRow.svelte';
	import TimeAgo from '$lib/TimeAgo.svelte';
	import AvatarGroup from '$lib/avatar/AvatarGroup.svelte';
	import { onMount } from 'svelte';

	interface Props {
		onMouseDown?: () => void;
		onFirstSeen?: () => void;
		title?: string;
		series?: string[];
		selected?: boolean;
		applied?: boolean;
		pullRequestDetails?: { title: string; draft: boolean };
		lastCommitDetails?: { authorName: string; lastCommitAt?: Date };
		branchDetails?: { commitCount: number; linesAdded: number; linesRemoved: number };
		remotes?: string[];
		local?: boolean;
		avatars?: { name: string; srcUrl: string }[];
	}

	const {
		onMouseDown = () => {},
		onFirstSeen = () => {},
		title,
		series,
		selected = false,
		applied = false,
		pullRequestDetails,
		lastCommitDetails,
		branchDetails,
		remotes = [],
		local = false,
		avatars
	}: Props = $props();

	let intersectionTarget = $state<HTMLButtonElement>();

	const observer = new IntersectionObserver((event) => {
		event.forEach((entry) => {
			if (entry.isIntersecting) {
				onFirstSeen();
			}
		});
	});

	$effect(() => {
		if (intersectionTarget) {
			observer.observe(intersectionTarget);
		}
	});

	onMount(() => {
		return () => {
			observer.disconnect();
		};
	});
</script>

<button
	type="button"
	class="branch"
	class:selected
	onmousedown={onMouseDown}
	bind:this={intersectionTarget}
>
	{#if series}
		<SeriesLabelsRow {series} showRestAmount {selected} />
	{/if}

	{#if title}
		<h4 class="text-13 text-semibold branch-name">
			{title}
		</h4>
	{/if}

	<div class="row">
		<div class="authors-and-tags">
			{#if avatars}
				<AvatarGroup {avatars} />
			{/if}

			<div class="branch-remotes">
				<!-- NEED API -->
				{#each remotes as remote}
					<div class="branch-tag tag-remote">
						<span class="text-10 text-semibold">{remote}</span>
					</div>
				{/each}
				{#if local}
					<div class="branch-tag tag-local">
						<span class="text-10 text-semibold">local</span>
					</div>
				{/if}
			</div>
		</div>

		<div class="row-group">
			{#if pullRequestDetails}
				<Tooltip text={pullRequestDetails.title}>
					<div
						class="branch-tag tag-pr"
						class:tag-pr={!pullRequestDetails.draft}
						class:tag-draft-pr={pullRequestDetails.draft}
					>
						<span class="text-10 text-semibold">
							{#if !pullRequestDetails.draft}PR{:else}Draft{/if}
						</span>
						<Icon name="pr-small" />
					</div>
				</Tooltip>
			{/if}
			{#if applied}
				<div class="branch-tag tag-applied">
					<span class="text-10 text-semibold">Workspace</span>
				</div>
			{/if}
		</div>
	</div>

	<div class="row">
		{#if lastCommitDetails?.lastCommitAt}
			<Tooltip text={lastCommitDetails.lastCommitAt.toLocaleString('en-GB')}>
				<span class="branch-time text-11 details truncate">
					{#if lastCommitDetails}
						<TimeAgo date={lastCommitDetails.lastCommitAt} addSuffix />
						by {lastCommitDetails.authorName}
					{/if}
				</span>
			</Tooltip>
		{:else}
			<span class="branch-time text-11 details truncate">
				{#if lastCommitDetails}
					by {lastCommitDetails.authorName}
				{/if}
			</span>
		{/if}

		<div class="stats">
			{#if branchDetails}
				<Tooltip text="Code changes">
					<div class="stats-group text-11">
						<span>+{branchDetails.linesAdded}</span>
						<span>-{branchDetails.linesRemoved}</span>
					</div>
				</Tooltip>

				<Tooltip text="Number of commits">
					<div class="stats-group">
						<svg
							width="14"
							height="13"
							viewBox="0 0 14 13"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<circle cx="7" cy="6.5" r="3" stroke="currentColor" />
							<path d="M10 6.5H14" stroke="currentColor" />
							<path d="M0 6.5H4" stroke="currentColor" />
						</svg>

						<span class="text-11">{branchDetails.commitCount}</span>
					</div>
				</Tooltip>
			{/if}
		</div>
	</div>
</button>

<style lang="postcss">
	.branch {
		position: relative;
		display: flex;
		flex-direction: column;
		padding: 10px 14px 12px 14px;
		gap: 8px;
		width: 100%;
		text-align: left;
		border-bottom: 1px solid var(--clr-border-3);
		overflow: hidden;
		transition:
			background-color var(--transition-fast),
			transform var(--transition-medium);
		/* Using a fixed height to prevent content-shift when loading in */
		min-height: 86px;

		&:last-child {
			border-bottom: none;
		}

		&::after {
			content: '';
			position: absolute;
			top: 0;
			left: 0;
			width: 4px;
			height: 100%;
			transform: translateX(-100%);
			transition: transform var(--transition-medium);
		}

		&:not(.selected):hover {
			&::after {
				background-color: var(--clr-scale-ntrl-60);
				transform: translateX(0);
			}
		}
	}

	/* ROW */

	.row {
		display: flex;
		align-items: center;
		width: 100%;
		gap: 6px;
		justify-content: space-between;
	}

	.row-group {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.authors-and-tags {
		display: flex;
		align-items: center;
		gap: 6px;
		overflow: hidden;
	}

	/* TAG */

	.branch-tag {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 2px;
		padding: 4px;
		height: 16px;
		border-radius: var(--radius-s);
	}

	.tag-local,
	.tag-remote {
		border: 1px solid var(--clr-border-2);
	}

	.tag-pr,
	.tag-draft-pr {
		padding: 0 2px 0 4px;
	}

	.tag-pr {
		background-color: var(--clr-theme-succ-element);
		color: var(--clr-theme-succ-on-element);
	}

	.tag-draft-pr {
		background-color: var(--clr-theme-ntrl-soft);
		color: var(--clr-text-2);
		border: 1px solid var(--clr-border-2);
	}

	.tag-applied {
		background-color: var(--clr-scale-ntrl-40);
		color: var(--clr-theme-ntrl-on-element);

		&:first-child {
			margin-left: 0;
		}
	}

	/*  */
	.stats {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.stats-group {
		display: flex;
		gap: 3px;
		align-items: center;
		overflow: hidden;
		color: var(--clr-text-2);
		margin-left: 2px;
	}

	.branch-remotes {
		display: flex;
		gap: 4px;
		overflow: hidden;
	}

	.branch-name {
		color: var(--clr-text-1);
		width: 100%;
		white-space: nowrap;
		overflow-x: hidden;
		text-overflow: ellipsis;
	}

	.branch-time {
		color: var(--clr-scale-ntrl-50);
	}

	/* MODIFIERS */

	.selected {
		background-color: var(--clr-bg-1-muted);

		&::after {
			background-color: var(--clr-theme-pop-element);
			transform: translateX(0);
		}
	}
</style>
