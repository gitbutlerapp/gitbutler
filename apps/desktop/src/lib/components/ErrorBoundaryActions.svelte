<script lang="ts">
	import InexistentRepo from './errorBoundaryActions/InexistentRepo.svelte';
	import { Code, isUserErrorCode } from '$lib/backend/ipc';

	interface Props {
		errorCode: string | undefined;
	}

	const { errorCode }: Props = $props();
</script>

{#if isUserErrorCode(errorCode)}
	<div>
		{#if errorCode === Code.ProjectMissing}
			<InexistentRepo />
		{/if}
	</div>
{/if}
