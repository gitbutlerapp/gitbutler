<script lang="ts">
	import BranchesCardTemplate from '$components/branchesPage/BranchesCardTemplate.svelte';
	import { Avatar, SeriesLabelsRow, TestId } from '@gitbutler/ui';
	import type { Author } from '$lib/commits/commit';
	interface Props {
		originName: string;
		commitsAmount?: number;
		lastCommit?: { author: Author; ago: string; branch: string; sha: string };
		selected?: boolean;
		onclick: () => void;
	}

	const { originName, commitsAmount: commitCount, lastCommit, selected, onclick }: Props = $props();

	const authorName = $derived(lastCommit?.author.name ?? lastCommit?.author.email ?? 'Unknown');
	const authorAvatar = $derived(lastCommit?.author.gravatarUrl ?? '');

	const fromOtherBranch = $derived(
		lastCommit && originName.endsWith(lastCommit.branch) ? '' : `from ${lastCommit?.branch}`
	);
</script>

<BranchesCardTemplate testId={TestId.CurrentOriginListCard} {onclick} {selected}>
	{#snippet content()}
		<SeriesLabelsRow fontSize="13" origin series={[originName]} />

		<button type="button" class="workspace-target-card__about">
			<Avatar size="medium" tooltip={authorName} srcUrl={authorAvatar} />
			{#if lastCommit}
				<p class="text-12 truncate workspace-target-card__text">
					{authorName}
					{lastCommit.ago}
					{fromOtherBranch}
				</p>
			{/if}
		</button>
	{/snippet}

	{#snippet details()}
		<div class="workspace-target-card__details">
			{#if commitCount}
				<div class="workspace-target-card__details-item">
					<svg
						width="14"
						height="12"
						viewBox="0 0 14 12"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
					>
						<path
							d="M10 6C10 7.65685 8.65685 9 7 9C5.34315 9 4 7.65685 4 6M10 6C10 4.34315 8.65685 3 7 3C5.34315 3 4 4.34315 4 6M10 6H14M4 6H0"
							stroke="currentColor"
						/>
					</svg>

					<span>{commitCount}</span>
				</div>

				<span class="workspace-target-card__divider">•</span>
			{/if}

			<div class="workspace-target-card__details-item">
				{#if lastCommit}
					<span>
						head {lastCommit.sha.slice(0, 7)}
					</span>
				{/if}
			</div>
		</div>
	{/snippet}
</BranchesCardTemplate>

<style lang="postcss">
	.workspace-target-card__about {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.workspace-target-card__text {
		color: var(--clr-text-2);
	}

	.workspace-target-card__details {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.workspace-target-card__details-item {
		display: flex;
		align-items: center;
		gap: 4px;
		color: var(--clr-text-2);
	}

	.workspace-target-card__divider {
		color: var(--clr-text-3);
	}
</style>
