<script lang="ts">
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import { getGitHostPrService } from '$lib/gitHost/interface/gitHostPrService';
	import PullRequestButton from '$lib/pr/PullRequestButton.svelte';

	interface Props {
		upstreamName: string | undefined;
	}

	const { upstreamName }: Props = $props();

	const hostedListingServiceStore = getGitHostListingService();
	const prStore = $derived($hostedListingServiceStore?.prs);
	const prs = $derived(prStore ? $prStore : undefined);

	const listedPr = $derived(prs?.find((pr) => pr.sourceBranch === upstreamName));
	const prNumber = $derived(listedPr?.number);

	const prService = getGitHostPrService();
	const prMonitor = $derived(prNumber ? $prService?.prMonitor(prNumber) : undefined);

	const pr = $derived(prMonitor?.pr);

	let loading = $state(false);
</script>

<div class="branch-info">
	<div class="branch-name text-14 text-bold">
		<span class="remote-name">origin/</span>{upstreamName}
	</div>
	{#if !pr}
		<PullRequestButton {loading} click={() => {}} />
	{/if}
</div>

<style lang="postcss">
	.branch-info {
		padding: 14px 16px;
		display: flex;
		border-bottom: 1px solid var(--clr-border-2);
		user-select: text;
		cursor: text;
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	.branch-name {
	}
	.remote-name {
		color: var(--clr-scale-ntrl-60);
	}
</style>
