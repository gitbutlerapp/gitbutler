<script lang="ts">
	import Icon from '$lib/icon/Icon.svelte';
	import TimeAgo from '$lib/timeAgo/TimeAgo.svelte';
	import { tooltip } from '$lib/utils/tooltip';
	import { onMount, type Snippet } from 'svelte';

	interface Props {
		onMouseDown?: () => void;
		onFirstSeen?: () => void;
		selected?: boolean;
		title: string;
		applied?: boolean;
		pullRequestDetails?: { title: string };
		lastCommitDetails?: { authorName: string; lastCommitAt: Date };
		branchDetails?: { commitCount: number; linesAdded: number; linesRemoved: number };
		remotes?: string[];
		local?: boolean;

		authorAvatars: Snippet;
	}

	const {
		onMouseDown = () => {},
		onFirstSeen = () => {},
		selected = false,
		applied = false,
		title,
		pullRequestDetails,
		lastCommitDetails,
		branchDetails,
		remotes = [],
		local = false,

		authorAvatars
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

	const tooltipDelay = 500;
</script>

<button class="branch" class:selected onmousedown={onMouseDown} bind:this={intersectionTarget}>
	<h4 class="text-base-13 text-semibold branch-name">
		{title}
	</h4>

	<div class="row">
		<div class="row-group authors-and-tags">
			{@render authorAvatars()}
			<div class="branch-remotes">
				<!-- NEED API -->
				{#each remotes as remote}
					<div class="branch-tag tag-remote">
						<span class="text-base-10 text-semibold">{remote}</span>
					</div>
				{/each}
				{#if local}
					<div class="branch-tag tag-local">
						<span class="text-base-10 text-semibold">local</span>
					</div>
				{/if}
			</div>
		</div>

		<div class="row-group">
			{#if pullRequestDetails}
				<div
					use:tooltip={{ text: pullRequestDetails.title, delay: tooltipDelay }}
					class="branch-tag tag-pr"
				>
					<span class="text-base-10 text-semibold">PR</span>
					<Icon name="pr-small" />
				</div>
			{/if}
			{#if applied}
				<div class="branch-tag tag-applied">
					<span class="text-base-10 text-semibold">applied</span>
				</div>
			{/if}
		</div>
	</div>

	<div class="row">
		{#if lastCommitDetails}
			<span
				class="branch-time text-base-11 details truncate"
				use:tooltip={lastCommitDetails.lastCommitAt.toLocaleString('en-GB')}
			>
				<TimeAgo date={lastCommitDetails.lastCommitAt} addSuffix />
				by {lastCommitDetails.authorName}
			</span>
		{/if}

		<div class="stats">
			{#if branchDetails}
				<div
					use:tooltip={{
						text: 'Number of commits',
						delay: tooltipDelay
					}}
					class="branch-tag tag-commits"
				>
					<span class="text-base-10 text-semibold">{branchDetails.commitCount}</span>
					<Icon name="commit" />
				</div>

				<div
					use:tooltip={{
						text: 'Code changes',
						delay: tooltipDelay
					}}
					class="code-changes"
				>
					<span class="text-base-10 text-semibold">+{branchDetails.linesAdded}</span>
					<span class="text-base-10 text-semibold">-{branchDetails.linesRemoved}</span>
				</div>
			{/if}
		</div>
	</div>
</button>

<style lang="postcss">
	.branch {
		display: flex;
		flex-direction: column;
		padding: 10px 14px 12px 14px;
		gap: 8px;
		width: 100%;
		text-align: left;
		border-bottom: 1px solid var(--clr-border-3);
		transition: background-color var(--transition-fast);
		/* Using a fixed height to prevent content-shift when loading in */
		height: 86px;

		&:last-child {
			border-bottom: none;
		}
	}

	.authors-and-tags {
		:global(& > *:first-child:empty) {
			display: none;
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
		gap: 6px;
	}

	/* TAG */

	.branch-tag {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 2px;
		padding: 0 4px;
		height: 16px;
		border-radius: var(--radius-s);
	}

	.tag-remote {
		background-color: var(--clr-theme-ntrl-soft-hover);
		color: var(--clr-text-1);
	}

	.tag-local {
		background-color: var(--clr-theme-ntrl-soft-hover);
		color: var(--clr-text-2);
	}

	.tag-pr {
		background-color: var(--clr-theme-succ-element);
		color: var(--clr-theme-succ-on-element);
	}

	.tag-applied {
		background-color: var(--clr-scale-ntrl-40);
		color: var(--clr-theme-ntrl-on-element);

		&:first-child {
			margin-left: 0;
		}
	}

	.tag-commits {
		background-color: var(--clr-bg-3);
		color: var(--clr-text-2);
	}

	/*  */

	.code-changes {
		display: flex;
		height: 16px;

		& span {
			padding: 2px 4px;
			height: 100%;
		}

		& span:first-child {
			background-color: var(--clr-theme-succ-soft);
			color: var(--clr-theme-succ-on-soft);
			border-radius: var(--radius-s) 0 0 var(--radius-s);
		}

		& span:last-child {
			background-color: var(--clr-theme-err-soft);
			color: var(--clr-theme-err-on-soft);
			border-radius: 0 var(--radius-s) var(--radius-s) 0;
		}
	}

	.stats {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.branch-remotes {
		display: flex;
		gap: 6px;
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

	.branch:not(.selected):hover,
	.branch:not(.selected):focus {
		background-color: var(--clr-bg-1-muted);
		transition: none;
	}

	.selected {
		background-color: var(--clr-bg-2);
	}
</style>
