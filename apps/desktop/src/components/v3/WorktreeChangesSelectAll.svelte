<script lang="ts">
	import { DiffService, type HunkGroup } from '$lib/hunks/diffService.svelte';
	import {
		allAssignedToCurrentGroupSelected,
		ChangeSelectionService,
		deselectAllForChangeInGroup,
		filterChangesByGroup,
		selectAllForChangeInGroup,
		someAssignedToCurrentGroupSelected
	} from '$lib/selection/changeSelection.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Checkbox from '@gitbutler/ui/Checkbox.svelte';

	type Props = {
		projectId: string;
		group: HunkGroup;
	};

	const { projectId, group }: Props = $props();

	const worktreeService = getContext(WorktreeService);
	const diffService = getContext(DiffService);
	const changeSelection = getContext(ChangeSelectionService);
	const changesResult = $derived(worktreeService.getChanges(projectId));
	const changesKeyResult = $derived(worktreeService.getChangesKey(projectId));
	const assignmentsResult = $derived(
		changesKeyResult.current
			? diffService.hunkAssignments(projectId, changesKeyResult.current)
			: undefined
	);
	const filteredChanges = $derived(
		assignmentsResult?.current.data && changesResult?.current?.data
			? filterChangesByGroup(changesResult.current.data, group, assignmentsResult.current.data)
			: []
	);
	const selectedFiles = $derived(
		Object.fromEntries(
			filteredChanges.map((change) => [change.path, changeSelection.getById(change.path)])
		)
	);

	const checkStatus = $derived.by((): 'checked' | 'indeterminate' | 'unchecked' => {
		if (!assignmentsResult?.current.data) return 'unchecked';
		if (!changesResult.current.data) return 'unchecked';
		if (filteredChanges.length === 0) return 'unchecked';

		const assignments = assignmentsResult.current.data;

		if (
			filteredChanges.every((change) =>
				allAssignedToCurrentGroupSelected(
					change,
					group,
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
					group,
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
		if (!assignmentsResult?.current.data) return;
		if (!changesResult.current.data) return;

		if (checkStatus === 'checked' || checkStatus === 'indeterminate') {
			for (const change of filteredChanges) {
				deselectAllForChangeInGroup(
					change,
					group,
					assignmentsResult.current.data,
					selectedFiles[change.path]?.current,
					changeSelection
				);
			}
		} else {
			for (const change of filteredChanges) {
				selectAllForChangeInGroup(
					change,
					group,
					assignmentsResult.current.data,
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
