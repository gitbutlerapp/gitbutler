<script lang="ts">
	import { GitHubService } from '$lib/github/service';
	import Button from '$lib/shared/Button.svelte';
	import TimeAgo from '$lib/shared/TimeAgo.svelte';
	import { getContext } from '$lib/utils/context';
	import { BaseBranchService } from '$lib/vbranches/baseBranch';

	const githubService = getContext(GitHubService);
	const baseBranchService = getContext(BaseBranchService);
	const baseBranch = baseBranchService.base;

	$: baseServiceBusy$ = baseBranchService.busy$;
</script>

<Button
	size="tag"
	clickable
	reversedDirection
	style="ghost"
	outline
	icon="update-small"
	help="Last fetch from upstream"
	loading={$baseServiceBusy$}
	on:mousedown={async (e) => {
		e.preventDefault();
		e.stopPropagation();
		await baseBranchService.fetchFromRemotes('modal');
		if (githubService.isEnabled) {
			await githubService.reload();
		}
	}}
>
	{#if $baseServiceBusy$}
		<div class="sync-btn__busy-label">busy…</div>
	{:else if $baseBranch?.lastFetched}
		<TimeAgo date={$baseBranch?.lastFetched} />
	{/if}
</Button>

<style lang="postcss">
	.sync-btn__busy-label {
		padding-left: 4px;
	}
</style>
