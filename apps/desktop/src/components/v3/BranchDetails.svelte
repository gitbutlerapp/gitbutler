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
		<BranchBadge pushStatus={branch.pushStatus} />
		<span class="branch-view__details-divider">•</span>

		<!-- {#if branch.isConflicted} -->
		<div class="branch-view__header-details-row-conflict">
			<Icon name="warning-small" /> <span>Conflicts</span>
		</div>
		<span class="branch-view__details-divider">•</span>
		<!-- {/if} -->

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
		align-items: center;
		flex-wrap: wrap;
		gap: 6px;
		width: 100%;
		color: var(--clr-text-2);
	}

	.branch-view__header-details-row-conflict {
		display: flex;
		align-items: center;
		gap: 4px;
		color: var(--clr-theme-err-element);
	}

	.branch-view__details-divider {
		color: var(--clr-text-3);
	}
</style>
