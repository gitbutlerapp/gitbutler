<script lang="ts">
	import Badge from '$lib/Badge.svelte';

	type Props = {
		status: 'approved' | 'changes-requested' | 'unreviewed' | 'in-discussion';
	};

	const { status }: Props = $props();
</script>

<div class="container">
	{#if status === 'changes-requested'}
		<Badge style="warning">Changes requested</Badge>
	{:else if status === 'approved'}
		<Badge style="success">Approved</Badge>
	{:else if status === 'in-discussion'}
		<Badge style="pop" kind="soft">In discussion</Badge>
	{:else}
		<Badge style="neutral" kind="soft">Unreviewed</Badge>
	{/if}
</div>
