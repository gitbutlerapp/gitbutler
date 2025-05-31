<script lang="ts">
	import { AssignmentService } from '$lib/selection/assignmentService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Checkbox from '@gitbutler/ui/Checkbox.svelte';

	type Props = {
		stackId: string | undefined;
	};

	const { stackId }: Props = $props();

	const assignmentService = getContext(AssignmentService);

	const checkStatus = $derived(assignmentService.stackCheckStatus(stackId));

	function onCheck(checked: boolean) {
		if (checked) {
			assignmentService.checkAll(stackId || null);
		} else {
			assignmentService.uncheckAll(stackId || null);
		}
	}
</script>

<Checkbox
	small
	checked={checkStatus.current === 'checked' || checkStatus.current === 'indeterminate'}
	indeterminate={checkStatus.current === 'indeterminate'}
	onchange={(e) => onCheck(e.currentTarget.checked)}
/>
