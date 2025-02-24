<script lang="ts">
	import BranchesCardTemplate from '$components/branchesPage/BranchesCardTemplate.svelte';
	import { Avatar, ReviewBadge, SeriesIcon, TestId, TimeAgo } from '@gitbutler/ui';
	import type { ReviewUnitInfo } from '$lib/forge/interface/forgePrService';
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
		reviewUnit: ReviewUnitInfo | undefined;
		onclick?: (prData: basePrData) => void;
		selected?: boolean;
		noRemote?: boolean;
	}

	const {
		reviewUnit,
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
				<span class="clr-text-2">#{number}:</span>
				{title}
			</h4>
		</div>

		<div class="text-12 sidebar-entry__about">
			<ReviewBadge
				type={reviewUnit?.abbr}
				status={isDraft ? 'draft' : 'unknown'}
				{title}
				{number}
			/>
			<span class="sidebar-entry__divider">•</span>

			{#if noRemote || !sourceBranch}
				<span>No remote</span>
			{:else}
				<div class="sidebar-entry__branch truncate">
					<SeriesIcon single />
					<span class="text-semibold truncate">{sourceBranch}</span>
				</div>
			{/if}
		</div>
	{/snippet}
	{#snippet details()}
		{#if author && modifiedAt}
			<Avatar srcUrl={author.gravatarUrl || ''} username={author.name || unknownName} />
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
		align-items: flex-start;
		gap: 10px;

		& h4 {
			line-height: 1.4;
		}
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
		color: var(--clr-text-2);
	}
</style>
