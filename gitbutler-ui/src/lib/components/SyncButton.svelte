<script lang="ts">
	import { syncToCloud } from '$lib/backend/cloud';
	import { Project } from '$lib/backend/projects';
	import Tag from '$lib/components/Tag.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import { GitHubService } from '$lib/github/service';
	import { getContext } from '$lib/utils/context';
	import { BaseBranchService } from '$lib/vbranches/branchStoresCache';

	const project = getContext(Project);

	let cloudEnabled: boolean;

	const githubService = getContext(GitHubService);
	const baseBranchService = getContext(BaseBranchService);
	const baseBranch = baseBranchService.base;

	$: baseServiceBusy$ = baseBranchService.busy$;
	$: cloudEnabled = project?.api?.sync || false;
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
		if (cloudEnabled) syncToCloud(project.id); // don't wait for this
		await baseBranchService.fetchFromTarget('modal');
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
</Tag>

<style lang="postcss">
	.sync-btn__busy-label {
		padding-left: var(--size-4);
	}
</style>
