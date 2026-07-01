<script lang="ts">
	import { lastFetched as getLastFetched } from "$lib/baseBranch/baseBranch";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { BRANCH_SERVICE } from "$lib/branches/branchService.svelte";
	import { FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
	import { LISTING_SERVICE } from "$lib/forge/listingService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Button, TimeAgo, Icon, TestId } from "@gitbutler/ui";

	interface Props {
		projectId: string;
		disabled?: boolean;
	}

	const { projectId, disabled = false }: Props = $props();

	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const branchService = inject(BRANCH_SERVICE);
	const baseBranch = $derived(baseBranchService.baseBranch(projectId));

	const listingService = inject(LISTING_SERVICE);
	const forgeInfoService = inject(FORGE_INFO_SERVICE);

	// `list_reviews` errors for forges without listing support (Bitbucket/
	// Azure) or when no forge can be derived, so only refresh the listing
	// when the forge actually supports it.
	const canListReviews = $derived(
		forgeInfoService.get(projectId).response?.capabilities.listService ?? false,
	);

	const lastFetched = $derived(
		baseBranch.result.data ? getLastFetched(baseBranch.result.data) : undefined,
	);

	let loading = $state(false);
</script>

<Button
	testId={TestId.SyncButton}
	kind="outline"
	width="auto"
	tooltip="Last fetch from upstream"
	{loading}
	{disabled}
	icon="refresh"
	reversedDirection
	onclick={async (e: MouseEvent) => {
		e.preventDefault();
		e.stopPropagation();
		loading = true;
		try {
			await baseBranchService.fetchFromRemotes(projectId, "modal");
			await Promise.all([
				...(canListReviews ? [listingService.refresh(projectId)] : []),
				baseBranch.result?.refetch(),
				branchService.refresh(),
			]);
		} finally {
			loading = false;
		}
	}}
>
	<span>
		{#if loading}
			Fetching...
		{:else if lastFetched}
			<TimeAgo date={lastFetched} addSuffix={true} capitalize={true} />
		{:else}
			Refetch
		{/if}
	</span>

	{#snippet custom()}
		{#if baseBranch.response}
			<div class="target-branch">
				<Icon name="target-branch" color="var(--text-2)" />
				<span class="text-12 text-semibold">
					{baseBranch.response.remoteName}/{baseBranch.response.shortName}
				</span>
			</div>
		{/if}
	{/snippet}
</Button>

<style lang="postcss">
	.target-branch {
		display: inline-flex;
		align-items: center;
		padding-right: 2px;
		gap: 4px;
		color: var(--text-2);

		&:after {
			display: inline-block;
			width: 1px;
			height: 12px;
			margin: 0 2px 0 4px;
			background-color: var(--text-2);
			content: "";
			opacity: 0.5;
		}
	}
</style>
