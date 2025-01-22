<script lang="ts">
	import Badge from '@gitbutler/ui/Badge.svelte';
	import type { Branch, Patch } from '@gitbutler/shared/branches/types';

	type Props = {
		branch: Branch;
	};

	const { branch }: Props = $props();

	const patches = branch.patches;

	let anyRejected = patches.some((patch: Patch) => patch.reviewAll.rejected.length > 0);
	let someApproved = patches.some((patch: Patch) => patch.reviewAll.signedOff.length > 0);
	let allApproved = !patches.some((patch: Patch) => patch.reviewAll.signedOff.length === 0);
</script>

<div class="container">
	{#if anyRejected}
		<Badge style="error">Changes Requested</Badge>
	{:else if allApproved}
		<Badge style="success">Approved</Badge>
	{:else if someApproved}
		<Badge style="warning">In Discussion</Badge>
	{:else}
		<Badge style="neutral" kind="soft">Unreviewed</Badge>
	{/if}
</div>
