<script lang="ts">
	import InexistentRepo from './errorBoundaryActions/InexistentRepo.svelte';
	import { getKnownError, KnownErrorType } from '$lib/utils/errors';

	interface Props {
		error: unknown;
	}

	const { error }: Props = $props();
	let knownError = $derived(getKnownError(error));
</script>

{#if knownError}
	<div>
		{#if knownError.type === KnownErrorType.FailedToOpenRepoInexistent}
			<InexistentRepo error={knownError} />
		{/if}
	</div>
{/if}
