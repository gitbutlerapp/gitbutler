<script lang="ts">
	import BranchBadge from '$components/v3/BranchBadge.svelte';
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
	<div class="branch-view__header-container">
		<div class="text-12 branch-view__header-details-row">
			<BranchBadge pushStatus={branch.pushStatus} />
			<span class="branch-view__details-divider">•</span>

			{#if branch.isConflicted}
				<span class="branch-view__header-details-row-conflict">Has conflicts</span>
				<span class="branch-view__details-divider">•</span>
			{/if}

			<span>Contribs:</span>
			<AvatarGroup
				maxAvatars={2}
				avatars={branch.authors.map((a) => ({
					name: a.name,
					srcUrl: a.gravatarUrl
				}))}
			/>

			{#if branch.lastUpdatedAt}
				<span class="branch-view__details-divider">•</span>
				<span class="truncate">{getTimeAgo(branch.lastUpdatedAt)}</span>
			{/if}
		</div>
	</div>

	{@render children?.()}
</div>

<style lang="postcss">
	.branch-view {
		display: flex;
		flex-direction: column;
		gap: 16px;
		height: 100%;
	}

	.branch-view__header-container {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 16px;
		overflow: hidden;
	}

	.branch-view__header-details-row {
		width: 100%;
		color: var(--clr-text-2);
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.branch-view__header-details-row-conflict {
		color: var(--clr-theme-err-element);
	}

	.branch-view__details-divider {
		color: var(--clr-text-3);
	}
</style>
