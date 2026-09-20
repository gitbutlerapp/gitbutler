<script lang="ts">
	import { getPatchStatus, type PatchCommit } from "@gitbutler/shared/patches/types";
	import { CommitStatusBadge } from "@gitbutler/ui-svelte";

	interface Props {
		patchCommit: PatchCommit;
	}

	const { patchCommit }: Props = $props();

	const status = $derived(getPatchStatus(patchCommit));
</script>

<CommitStatusBadge {status} />
