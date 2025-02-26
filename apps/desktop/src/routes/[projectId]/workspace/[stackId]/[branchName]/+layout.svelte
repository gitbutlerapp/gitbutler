<script lang="ts">
	import BranchView from '$components/v3/BranchView.svelte';
	import { type Snippet } from 'svelte';
	import type { PageData } from '../$types';
	import { page } from '$app/state';

	const { data, children }: { data: PageData; children: Snippet } = $props();

	const projectId = $derived(data.projectId!);
	const stackId = $derived(page.params.stackId!);
	const branchName = $derived(page.params.branchName!);
	const commitId = $derived(page.url.searchParams.get('commitId') || undefined);
</script>

<BranchView
	{projectId}
	{stackId}
	selectedBranchName={branchName}
	selectedCommitId={commitId}
	{children}
/>
