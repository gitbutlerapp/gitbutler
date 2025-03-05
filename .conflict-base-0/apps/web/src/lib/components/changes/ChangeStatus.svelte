<script lang="ts">
	import { getPatchStatus, type Patch } from '@gitbutler/shared/patches/types';
	import CommitStatusBadge from '@gitbutler/ui/CommitStatusBadge.svelte';

	interface Props {
		patch: Patch;
	}

	const { patch }: Props = $props();

	const status = $derived(getPatchStatus(patch));
</script>

<CommitStatusBadge {status} />
