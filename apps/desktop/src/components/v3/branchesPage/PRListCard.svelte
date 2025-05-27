<script lang="ts">
	import BranchesCardTemplate from '$components/v3/branchesPage/BranchesCardTemplate.svelte';
	import { TestId } from '$lib/testing/testIds';
	import ReviewBadge from '@gitbutler/ui/ReviewBadge.svelte';
	import SeriesIcon from '@gitbutler/ui/SeriesIcon.svelte';
	import TimeAgo from '@gitbutler/ui/TimeAgo.svelte';
	import Avatar from '@gitbutler/ui/avatar/Avatar.svelte';

	type basePrData = {
		number: number;
		isDraft: boolean;
		title: string;
		sourceBranch?: string;
		author?: {
			name?: string;
			email?: string;
			gravatarUrl?: string;
		};
		modifiedAt?: string;
	};

	interface Props extends basePrData {
		onclick?: (prData: basePrData) => void;
		selected?: boolean;
		noRemote?: boolean;
	}

	const {
		selected,
		noRemote,
		isDraft,
		number,
		title,
		sourceBranch,
		author,
		modifiedAt,
		onclick
	}: Props = $props();

	const unknownName = 'Unknown Author';

	// console.log('PRListCard', {
	// 	pullRequest
	// });
</script>

<BranchesCardTemplate
	testId={TestId.PRListCard}
	{selected}
	onclick={() =>
		onclick?.({
			number,
			isDraft,
			title,
			sourceBranch,
			author,
			modifiedAt
		})}
>
	{#snippet content()}
		<div class="sidebar-entry__header">
			<h4 class="text-13 text-semibold">
				<span class="text-clr2">#{number}:</span>
				{title}
			</h4>
		</div>

		<div class="text-12 sidebar-entry__about">
			<ReviewBadge prStatus={isDraft ? 'draft' : 'unknown'} prTitle={title} prNumber={number} />
			<span class="sidebar-entry__divider">•</span>

			{#if noRemote}
				<span>No remote</span>
			{:else}
				<div class="sidebar-entry__branch">
					<SeriesIcon single />
					<span class="text-semibold">{sourceBranch}</span>
				</div>
			{/if}
		</div>
	{/snippet}
	{#snippet details()}
		{#if author && modifiedAt}
			<Avatar srcUrl={author.gravatarUrl || ''} tooltip={author.name || unknownName} />
			<TimeAgo date={new Date(modifiedAt)} addSuffix /> by
			{author.name || unknownName}
		{/if}
	{/snippet}
</BranchesCardTemplate>

<style lang="postcss">
	.sidebar-entry__about {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--clr-text-2);
	}

	.sidebar-entry__header {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.sidebar-entry__divider {
		color: var(--clr-text-3);

		&:last-child {
			display: none;
		}
	}

	.sidebar-entry__branch {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--clr-text-1);
	}
</style>
