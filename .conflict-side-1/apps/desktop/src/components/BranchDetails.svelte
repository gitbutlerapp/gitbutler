<script lang="ts">
	import BranchBadge from '$components/BranchBadge.svelte';
	import { AvatarGroup, TimeAgo, Button } from '@gitbutler/ui';
	import type { BranchDetails } from '$lib/stacks/stack';
	import type { Snippet } from 'svelte';

	type Props = {
		branch: BranchDetails;
		children?: Snippet;
		conflictedCommits?: Snippet;
		onResolveConflicts?: () => void;
	};

	const { branch, children, conflictedCommits, onResolveConflicts }: Props = $props();
</script>

<div class="branch-view">
	<div class="text-12 branch-view__header-container">
		<div class="factoid-wrap">
			<BranchBadge pushStatus={branch.pushStatus} unstyled />
			<span class="branch-view__details-divider">•</span>
		</div>

		<div class="factoid-wrap">
			<span class="factoid-label">Contribs:</span>
			<AvatarGroup
				maxAvatars={2}
				avatars={branch.authors.map((a) => ({
					username: a.name,
					srcUrl: a.gravatarUrl
				}))}
			/>
			<span class="branch-view__details-divider">•</span>
		</div>

		{#if branch.lastUpdatedAt}
			<div class="factoid-wrap">
				<TimeAgo date={new Date(branch.lastUpdatedAt)} />
			</div>
		{/if}
	</div>

	{@render children?.()}

	{#if branch.isConflicted && conflictedCommits}
		<div class="header-details__conflicts">
			{@render conflictedCommits?.()}

			<div class="header-details__conflicts-action">
				<div class="stack-v gap-8">
					<h3 class="text-13 text-semibold">Conflicted commits</h3>
					<p class="text-12 text-body clr-text-2">
						GitButler opens the earliest commit first, since later commits depend on it.
					</p>
				</div>
				<Button onclick={onResolveConflicts} style="error">Start resolving</Button>
			</div>
		</div>
	{/if}
</div>

<style lang="postcss">
	.branch-view {
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		padding: 14px;
		gap: 16px;
	}

	.branch-view__header-container {
		display: flex;
		row-gap: 8px;
		flex-wrap: wrap;
		align-items: center;
		width: 100%;
		color: var(--clr-text-2);
	}

	.factoid-wrap {
		display: flex;
		align-items: center;
	}

	.factoid-label {
		margin-right: 4px;
	}

	.branch-view__details-divider {
		margin: 0 6px;
		color: var(--clr-text-3);
	}

	.header-details__conflicts {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.header-details__conflicts-action {
		display: flex;
		flex-direction: column;
		padding: 12px;
		gap: 12px;
	}
</style>
