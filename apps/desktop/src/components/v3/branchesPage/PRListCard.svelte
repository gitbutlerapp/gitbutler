<script lang="ts">
	import BranchesCardTemplate from '$components/v3/branchesPage/BranchesCardTemplate.svelte';
	import { BranchService } from '$lib/branches/branchService.svelte';
	import { GitConfigService } from '$lib/config/gitConfigService';
	import { TestId } from '$lib/testing/testIds';
	import { UserService } from '$lib/user/userService';
	import { inject } from '@gitbutler/shared/context';
	import ReviewBadge from '@gitbutler/ui/ReviewBadge.svelte';
	import SeriesIcon from '@gitbutler/ui/SeriesIcon.svelte';
	import TimeAgo from '@gitbutler/ui/TimeAgo.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import type { PullRequest } from '$lib/forge/interface/types';

	interface Props {
		projectId: string;
		pullRequest: PullRequest;
		selected: boolean;
		noSourceBranch?: boolean;
		onclick: (listing: PullRequest) => void;
	}

	const { pullRequest, selected, noSourceBranch, onclick }: Props = $props();

	const unknownName = 'unknown';

	const [userService] = inject(UserService, GitConfigService, BranchService);

	const user = userService.user;

	const authorImgUrl = $derived.by(() => {
		return pullRequest.author?.email?.toLowerCase() === $user?.email?.toLowerCase()
			? $user?.picture
			: pullRequest.author?.gravatarUrl;
	});
</script>

<BranchesCardTemplate testId={TestId.PRListCard} {selected} onclick={() => onclick?.(pullRequest)}>
	{#snippet content()}
		<div class="sidebar-entry__header">
			<h4 class="text-13 text-semibold">
				<span class="text-clr2">#{pullRequest.number}:</span>
				{pullRequest.title}
			</h4>
		</div>

		<div class="text-12 sidebar-entry__about">
			<ReviewBadge
				prStatus={pullRequest.draft ? 'draft' : 'unknown'}
				prTitle={pullRequest.title}
				prNumber={pullRequest.number}
			/>
			<span class="sidebar-entry__divider">•</span>

			{#if noSourceBranch}
				<span>No source branch</span>
			{:else}
				<div class="sidebar-entry__branch">
					<SeriesIcon single />
					<span class="text-semibold">{pullRequest.sourceBranch}</span>
				</div>
			{/if}
		</div>
	{/snippet}
	{#snippet details()}
		<AvatarGroup
			avatars={[
				{
					name: pullRequest.author?.name || 'unknown',
					srcUrl: authorImgUrl || ''
				}
			]}
		/>
		<TimeAgo date={new Date(pullRequest.modifiedAt)} addSuffix /> by
		{pullRequest.author?.name || unknownName}
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
