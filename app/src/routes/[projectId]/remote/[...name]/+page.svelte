<script lang="ts">
	// This page is displayed when:
	// - A remote branch is found
	// - And it does NOT have a cooresponding vbranch
	// It may also display details about a cooresponding pr if they exist
	import { getBranchServiceStore } from '$lib/branches/service';
	import FullviewLoading from '$lib/components/FullviewLoading.svelte';
	import RemoteBranchPreview from '$lib/components/RemoteBranchPreview.svelte';
	import { page } from '$app/stores';

	const branchService = getBranchServiceStore();
	const branches = $derived($branchService?.branches);
	const error = $derived($branchService?.error);
	const branch = $derived(
		$branches?.find((cb) => cb.remoteBranch?.displayName === $page.params.name)
	);
	// $: branch = $branches?.find((b) => b.displayName === $page.params.name);
</script>

{#if $error}
	<p>Error...</p>
{:else if !$branches}
	<FullviewLoading />
{:else if branch?.remoteBranch}
	<RemoteBranchPreview branch={branch.remoteBranch} pr={branch.pr} />
{:else}
	<p>Branch doesn't seem to exist</p>
{/if}
