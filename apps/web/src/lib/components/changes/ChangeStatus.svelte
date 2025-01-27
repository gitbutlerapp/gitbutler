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
	<Badge style="success">Approved</Badge>
{:else if status === 'changes-requested'}
	<Badge style="warning">Changes Requested</Badge>
{:else if status === 'unreviewed'}
	<Badge style="neutral" kind="soft">Unreviewed</Badge>
{:else if status === 'in-discussion'}
	<Badge style="pop" kind="soft">In Discussion</Badge>
{/if}
