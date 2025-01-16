<script lang="ts">
	import { getPatchStatus, type Patch } from '@gitbutler/shared/branches/types';
	import Badge from '@gitbutler/ui/Badge.svelte';

	interface Props {
		patch: Patch;
	}

	const { patch }: Props = $props();

	const status = $derived(getPatchStatus(patch));
</script>

{#if status === 'approved'}
	<Badge>Approved</Badge>
{:else if status === 'changes-requested'}
	<Badge>Changes Requested</Badge>
{:else if status === 'unreviewed'}
	<Badge>Unreviewed</Badge>
{:else if status === 'in-discussion'}
	<Badge>In Discussion</Badge>
{/if}
