<script lang="ts">
	import Icon from '$lib/Icon.svelte';
	import TimeAgo from '$lib/TimeAgo.svelte';
	import { tooltip } from '$lib/utils/tooltip';
	import { onMount, type Snippet } from 'svelte';

	interface Props {
		onMouseDown?: () => void;
		onFirstSeen?: () => void;
		selected?: boolean;
		title: string;
		applied?: boolean;
		pullRequestDetails?: { title: string; draft: boolean };
		lastCommitDetails?: { authorName: string; lastCommitAt?: Date };
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
	<h4 class="text-13 text-semibold branch-name">
		{title}
	</h4>

	<div class="row">
		<div class="row-group authors-and-tags">
			{@render authorAvatars()}
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
				<div
					use:tooltip={{ text: pullRequestDetails.title, delay: tooltipDelay }}
					class="branch-tag tag-pr"
					class:tag-pr={!pullRequestDetails.draft}
					class:tag-draft-pr={pullRequestDetails.draft}
				>
					<span class="text-10 text-semibold">
						{#if !pullRequestDetails.draft}PR{:else}Draft{/if}
					</span>
					<Icon name="pr-small" />
				</div>
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
			<span
				class="branch-time text-11 details truncate"
				use:tooltip={lastCommitDetails.lastCommitAt.toLocaleString('en-GB')}
			>
				{#if lastCommitDetails}
					<TimeAgo date={lastCommitDetails.lastCommitAt} addSuffix />
					by {lastCommitDetails.authorName}
				{/if}
			</span>
		{:else}
			<span class="branch-time text-11 details truncate">
				{#if lastCommitDetails}
					by {lastCommitDetails.authorName}
				{/if}
			</span>
		{/if}

		<div class="stats">
			{#if branchDetails}
				<div
					use:tooltip={{
						text: 'Code changes',
						delay: tooltipDelay
					}}
					class="code-changes"
				>
					<span class="text-10 text-semibold">+{branchDetails.linesAdded}</span>
					<span class="text-10 text-semibold">-{branchDetails.linesRemoved}</span>
				</div>

				<div
					use:tooltip={{
						text: 'Number of commits',
						delay: tooltipDelay
					}}
					class="branch-tag tag-commits"
				>
					<svg
						width="12"
						height="8"
						viewBox="0 0 12 8"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
					>
						<circle cx="6.16675" cy="4" r="2.5" stroke="currentColor" stroke-width="1.5" />
						<path d="M8.66675 4H12.0001" stroke="currentColor" stroke-width="1.5" />
						<path d="M0.333374 4H3.66671" stroke="currentColor" stroke-width="1.5" />
					</svg>

					<span class="text-10 text-semibold">{branchDetails.commitCount}</span>
				</div>
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
			background-color: var(--clr-theme-pop-element);
			transform: translateX(-100%);

			transition: transform var(--transition-medium);
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
		gap: 4px;
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

	.tag-local,
	.tag-remote {
		background-color: var(--clr-theme-ntrl-soft-hover);
		color: var(--clr-text-1);
	}

	.tag-pr {
		background-color: var(--clr-theme-succ-element);
		color: var(--clr-theme-succ-on-element);
	}

	.tag-draft-pr {
		background-color: var(--clr-theme-ntrl-soft);
		color: var(--clr-theme-ntrl-on-soft);
	}

	.tag-applied {
		background-color: var(--clr-scale-ntrl-40);
		color: var(--clr-theme-ntrl-on-element);

		&:first-child {
			margin-left: 0;
		}
	}

	.tag-commits {
		border: 1px solid var(--clr-border-2);
		color: var(--clr-text-2);
		gap: 4px;
	}

	/*  */

	.code-changes {
		display: flex;
		align-items: center;
		height: 16px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-s);
		overflow: hidden;

		& span {
			padding: 0 4px;
			color: var(--clr-text-2);
		}

		& span:first-child {
			border-right: 1px solid var(--clr-border-2);
		}
	}

	.stats {
		display: flex;
		align-items: center;
		gap: 4px;
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

	/* MODIFIERS */

	.selected {
		background-color: var(--clr-bg-2);

		&::after {
			transform: translateX(0);
		}
	}
</style>
