<script lang="ts">
	import { PatchSectionsService } from '@gitbutler/shared/cloud/patches/sections';
	import { ApiPatchService, CloudPatchService } from '@gitbutler/shared/cloud/patches/service';
	import { getContext } from '@gitbutler/shared/context';
	import { HttpClient } from '@gitbutler/shared/httpClient';
	import { setContext, type Snippet } from 'svelte';
	import { writable } from 'svelte/store';
	import { page } from '$app/stores';

	const { children }: { children: Snippet } = $props();

	const cloudBranchId = writable<string | undefined>();
	const changeId = writable<string | undefined>();

	$effect(() => {
		cloudBranchId.set($page.params.cloudBranchId);
		changeId.set($page.params.changeId);
	});

	const httpClient = getContext(HttpClient);
	const apiPatchService = new ApiPatchService(httpClient);
	const cloudPatchService = new CloudPatchService(cloudBranchId, changeId, apiPatchService);
	const patchSectionsService = new PatchSectionsService(cloudPatchService);

	setContext(CloudPatchService, cloudPatchService);
	setContext(PatchSectionsService, patchSectionsService);
</script>

{@render children()}
