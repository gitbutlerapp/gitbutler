<script lang="ts">
	import {
		CloudPatchStacksService,
		PatchStacksApiService
	} from '@gitbutler/shared/cloud/stacks/service';
	import { getContext } from '@gitbutler/shared/context';
	import { HttpClient } from '@gitbutler/shared/httpClient';
	import { setContext, type Snippet } from 'svelte';
	import { writable } from 'svelte/store';
	import { page } from '$app/stores';

	const { children }: { children: Snippet } = $props();

	const repositoryId = writable<string | undefined>();
	$effect(() => repositoryId.set($page.params.repositoryId));

	const httpClient = getContext(HttpClient);

	const patchStacksApiService = new PatchStacksApiService(httpClient);
	const cloudPatchStacksService = new CloudPatchStacksService(repositoryId, patchStacksApiService);
	setContext(CloudPatchStacksService, cloudPatchStacksService);
</script>

{@render children()}
