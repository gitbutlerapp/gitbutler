<script lang="ts">
	import BranchBadge from '$components/v3/BranchBadge.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
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
		gap: 16px;
		height: 100%;
		width: 100%;
	}

	.branch-view__header-container {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		row-gap: 8px;
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
		color: var(--clr-text-3);
		margin: 0 6px;
	}
</style>
