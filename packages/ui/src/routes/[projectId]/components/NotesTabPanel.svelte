<script lang="ts">
	import type { BranchController } from '$lib/vbranches/branchController';

	export let notes: string;
	export let branchController: BranchController;
	export let branchId: string;

	function handleUpdateNotes() {
		if (!notes) return;
		branchController.updateBranchNotes(branchId, notes);
	}
</script>

<textarea
	autocomplete="off"
	autocorrect="off"
	spellcheck="true"
	bind:value={notes}
	on:change={handleUpdateNotes}
	name="notes"
	class="outline-none-important text-color-2 h-full w-full resize-none bg-transparent p-2 align-top"
	placeholder="Branch notes (optional)"
/>
