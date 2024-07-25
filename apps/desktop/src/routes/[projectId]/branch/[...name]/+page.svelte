<script lang="ts">
	// This page is displayed when:
	// - A remote branch is found
	// - And it does NOT have a cooresponding vbranch
	// It may also display details about a cooresponding pr if they exist
	import { getBranchServiceStore } from '$lib/branches/service';
	import RemoteBranchPreview from '$lib/components/BranchPreview.svelte';
	import FullviewLoading from '$lib/components/FullviewLoading.svelte';
	import { page } from '$app/stores';

	const branchService = getBranchServiceStore();
	const branches = $derived($branchService?.branches);
	const error = $derived($branchService?.error);
	// Search for remote branch first as there may be multiple combined branches
	// which have the same local branch
	const branch = $derived(
		$branches?.find((cb) => cb.remoteBranch?.name === $page.params.name) ||
			$branches?.find((cb) => cb.localBranch?.name === $page.params.name)
	);
	// $: branch = $branches?.find((b) => b.displayName === $page.params.name);
</script>

{#if $error}
	<p>Error...</p>
{:else if !$branches}
	<FullviewLoading />
{:else if branch?.remoteBranch || branch?.localBranch}
	<RemoteBranchPreview
		localBranch={branch?.localBranch}
		remoteBranch={branch?.remoteBranch}
		pr={branch.pr}
	/>
{:else}
	<p>Branch doesn't seem to exist</p>
{/if}
