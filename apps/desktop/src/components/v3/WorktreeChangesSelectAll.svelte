<script lang="ts">
	import { type HunkAssignments } from '$lib/hunks/diffService.svelte';
	import {
		allAssignedToCurrentGroupSelected,
		ChangeSelectionService,
		deselectAllForChangeInGroup,
		filterChangesByGroup,
		selectAllForChangeInGroup,
		someAssignedToCurrentGroupSelected
	} from '$lib/selection/changeSelection.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Checkbox from '@gitbutler/ui/Checkbox.svelte';
	import type { TreeChange } from '$lib/hunks/change';

	type Props = {
		stackId?: string;
		changes: TreeChange[];
		assignments: HunkAssignments;
	};

	const { stackId, changes, assignments }: Props = $props();

	const changeSelection = getContext(ChangeSelectionService);
	const filteredChanges = $derived(filterChangesByGroup(changes, stackId, assignments));
	const selectedFiles = $derived(
		Object.fromEntries(
			filteredChanges.map((change) => [change.path, changeSelection.getById(change.path)])
		)
	);

	const checkStatus = $derived.by((): 'checked' | 'indeterminate' | 'unchecked' => {
		if (filteredChanges.length === 0) return 'unchecked';
		if (
			filteredChanges.every((change) =>
				allAssignedToCurrentGroupSelected(
					change,
					stackId,
					assignments,
					selectedFiles[change.path]?.current
				)
			)
		) {
			return 'checked';
		}

		if (
			filteredChanges.some((change) =>
				someAssignedToCurrentGroupSelected(
					change,
					stackId,
					assignments,
					selectedFiles[change.path]?.current
				)
			)
		) {
			return 'indeterminate';
		}

		return 'unchecked';
	});

	function onCheck() {
		if (checkStatus === 'checked' || checkStatus === 'indeterminate') {
			for (const change of filteredChanges) {
				deselectAllForChangeInGroup(
					change,
					stackId,
					assignments,
					selectedFiles[change.path]?.current,
					changeSelection
				);
			}
		} else {
			for (const change of filteredChanges) {
				selectAllForChangeInGroup(
					change,
					stackId,
					assignments,
					selectedFiles[change.path]?.current,
					changeSelection
				);
			}
		}
	}
</script>

<Checkbox
	small
	checked={checkStatus === 'checked' || checkStatus === 'indeterminate'}
	indeterminate={checkStatus === 'indeterminate'}
	onchange={onCheck}
/>
