<script lang="ts">
	import { UncommittedService } from '$lib/selection/uncommittedService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Checkbox from '@gitbutler/ui/Checkbox.svelte';

	type Props = {
		stackId: string | undefined;
	};

	const { stackId }: Props = $props();

	const uncommittedService = getContext(UncommittedService);

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
	checked={checkStatus.current === 'checked' || checkStatus.current === 'indeterminate'}
	indeterminate={checkStatus.current === 'indeterminate'}
	onchange={(e) => onCheck(e.currentTarget.checked)}
/>
