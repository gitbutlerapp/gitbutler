<script lang="ts">
	import Tag from '$lib/components/Tag.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import { GitHubService } from '$lib/github/service';
	import { getContext } from '$lib/utils/context';
	import { BaseBranchService } from '$lib/vbranches/baseBranch';

	const githubService = getContext(GitHubService);
	const baseBranchService = getContext(BaseBranchService);
	const baseBranch = baseBranchService.base;

	$: baseServiceBusy$ = baseBranchService.busy$;
</script>

<Tag
	clickable
	reversedDirection
	style="ghost"
	kind="solid"
	icon="update-small"
	help="Last fetch from upstream"
	loading={$baseServiceBusy$}
	on:mousedown={async (e) => {
		e.preventDefault();
		e.stopPropagation();
		await baseBranchService.fetchFromTarget('modal');
		if (githubService.isEnabled) {
			await githubService.reload();
		}
	}}
>
	{#if $baseServiceBusy$}
		<div class="sync-btn__busy-label">busy…</div>
	{:else if $baseBranch?.lastFetchedAt}
		<TimeAgo date={$baseBranch?.lastFetchedAt} />
	{/if}
</Tag>

<style lang="postcss">
	.sync-btn__busy-label {
		padding-left: var(--size-4);
	}
</style>
