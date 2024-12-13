<script lang="ts">
	import { CloudBranchesService, BranchesApiService } from '@gitbutler/shared/cloud/stacks/service';
	import { getContext } from '@gitbutler/shared/context';
	import { HttpClient } from '@gitbutler/shared/network/httpClient';
	import { setContext, type Snippet } from 'svelte';
	import { writable } from 'svelte/store';
	import { page } from '$app/stores';

	const { children }: { children: Snippet } = $props();

	const repositoryId = writable<string | undefined>();
	$effect(() => repositoryId.set($page.params.repositoryId));

	const httpClient = getContext(HttpClient);

	const cloudBranchesApiService = new BranchesApiService(httpClient);
	const cloudBranchesService = new CloudBranchesService(repositoryId, cloudBranchesApiService);
	setContext(CloudBranchesService, cloudBranchesService);
</script>

{@render children()}
