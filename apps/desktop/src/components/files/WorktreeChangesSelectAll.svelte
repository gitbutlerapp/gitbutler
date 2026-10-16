<script lang="ts">
	import { UNCOMMITTED_SERVICE } from "$lib/selection/uncommittedService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Checkbox } from "@gitbutler/ui-svelte";

	type Props = {
		stackId?: string;
		/** The linked worktree whose changes these are, if not the main checkout's. */
		worktree?: string;
	};

	const { stackId, worktree }: Props = $props();

	const uncommittedServices = inject(UNCOMMITTED_SERVICE);
	const uncommittedService = $derived(uncommittedServices.at(worktree));

	const checkStatus = $derived(uncommittedService.stackCheckStatus(stackId));

	function onCheck(checked: boolean) {
		if (checked) {
			uncommittedService.checkAll(stackId || null);
		} else {
			uncommittedService.uncheckAll(stackId || null);
		}
	}
</script>

<Checkbox
	small
	checked={checkStatus.current === "checked" || checkStatus.current === "indeterminate"}
	indeterminate={checkStatus.current === "indeterminate"}
	onchange={(e) => onCheck(e.currentTarget.checked)}
/>
