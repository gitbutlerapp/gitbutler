<script lang="ts">
	import { getContextByClass } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';

	export let notes: string;
	const branchController = getContextByClass(BranchController);
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
	class="text-color-2 h-full w-full resize-none bg-transparent p-2 align-top"
	placeholder="Branch notes (optional)"
/>
