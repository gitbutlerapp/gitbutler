<script lang="ts">
	import { PatchService } from '@gitbutler/shared/branches/patchService';
	import { getPatch } from '@gitbutler/shared/branches/patchesPreview.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import type { ProjectReviewParameters } from '$lib/project/types';

	type Props = {
		changeId: string;
		params: ProjectReviewParameters;
	};

	const { changeId, params }: Props = $props();

	const appState = getContext(AppState);
	const patchService = getContext(PatchService);

	const patch = $derived(getPatch(appState, patchService, params.branchId, changeId));
</script>

<Loading loadable={patch.current}>
	{#snippet children(patch)}
		<div class="card">
			<p>{patch.changeId}</p>
		</div>
	{/snippet}
</Loading>
