<script lang="ts">
	import BranchBadge from '$components/BranchBadge.svelte';
	import { Icon, AvatarGroup } from '@gitbutler/ui';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import type { BranchDetails } from '$lib/stacks/stack';
	import type { Snippet } from 'svelte';

	type Props = {
		branch: BranchDetails;
		children?: Snippet;
	};

	const { branch, children }: Props = $props();
</script>

<div class="branch-view">
	<div class="text-12 branch-view__header-container">
		<div class="factoid-wrap">
			<BranchBadge pushStatus={branch.pushStatus} />
			<span class="branch-view__details-divider">•</span>
		</div>

		{#if branch.isConflicted}
			<div class="factoid-wrap">
				<div class="branch-view__header-details-row-conflict">
					<Icon name="warning-small" /> <span>Conflicts</span>
				</div>
				<span class="branch-view__details-divider">•</span>
			</div>
		{/if}

		<div class="factoid-wrap">
			<span class="factoid-label">Contribs:</span>
			<AvatarGroup
				maxAvatars={2}
				avatars={branch.authors.map((a) => ({
					name: a.name,
					srcUrl: a.gravatarUrl
				}))}
			/>
			<span class="branch-view__details-divider">•</span>
		</div>

		{#if branch.lastUpdatedAt}
			<div class="factoid-wrap">
				<span class="truncate">{getTimeAgo(branch.lastUpdatedAt)}</span>
			</div>
		{/if}
	</div>

	{@render children?.()}
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

	.branch-view__header-details-row-conflict {
		display: flex;
		align-items: center;
		gap: 4px;
		color: var(--clr-theme-err-element);
	}

	.branch-view__details-divider {
		margin: 0 6px;
		color: var(--clr-text-3);
	}
</style>
