<script lang="ts">
	import BranchesCardTemplate from '$components/v3/branchesPage/BranchesCardTemplate.svelte';
	import SeriesLabelsRow from '@gitbutler/ui/SeriesLabelsRow.svelte';
	import Avatar from '@gitbutler/ui/avatar/Avatar.svelte';

	interface Props {
		originName: string;
		commitsAmount: number;
		lastCommit?: { author?: string; ago: string; branch: string; sha: string };
		onclick: () => void;
	}

	const { originName, commitsAmount, lastCommit, onclick }: Props = $props();
</script>

<BranchesCardTemplate {onclick}>
	{#snippet content()}
		<SeriesLabelsRow origin series={[originName]} />

		<button type="button" class="workspace-target-card__about">
			<Avatar
				size="medium"
				tooltip="origin"
				srcUrl="https://avatars.githubusercontent.com/u/1?v=4"
			/>
			{#if lastCommit}
				<p class="text-12 truncate workspace-target-card__text">
					{lastCommit.author}
					{lastCommit.ago} ago from {lastCommit.branch}
				</p>
			{/if}
		</button>
	{/snippet}

	{#snippet details()}
		<div class="workspace-target-card__details">
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

				<span>{commitsAmount}</span>
			</div>

			<span>•</span>

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
		gap: 6px;
		align-items: center;
	}

	.workspace-target-card__details-item {
		display: flex;
		gap: 4px;
		align-items: center;
		color: var(--clr-text-2);
	}
</style>
