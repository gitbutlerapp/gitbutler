<script lang="ts" module>
	import type { FileDependencies } from '$lib/dependencies/dependencies';

	export type FileDependencyData =
		| {
				type: 'multiple';
				data: Map<string, FileDependencies>;
		  }
		| {
				type: 'single';
				data: FileDependencies;
		  };
</script>

<script lang="ts">
	import DependencyService from '$lib/dependencies/dependencyService.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { inject } from '@gitbutler/shared/context';

	type BaseProps = {
		type: 'single' | 'multiple';
		projectId: string;
		isCommitting: boolean;
	};

	type SingleFile = BaseProps & {
		type: 'single';
		filePath: string;
	};

	type MultipleFiles = BaseProps & {
		type: 'multiple';
		filePaths: string[];
	};

	type Props = SingleFile | MultipleFiles;

	const props: Props = $props();

	const [worktreeService, dependencyService, stackService] = inject(
		WorktreeService,
		DependencyService,
		StackService
	);

	const stacks = $derived(stackService.stacks(props.projectId));
	const hasMultipleStacks = $derived(stacks.current.data && stacks.current.data.length > 1);

	const changesTimestamp = $derived(worktreeService.getChangesTimeStamp(props.projectId));
	// For now, only show the file dependencies when committing, and there are multiple stacks applied
	const canFetchDependencies = $derived(props.isCommitting && hasMultipleStacks);

	const fileDependencies = $derived.by(() => {
		// Derivation of the dependencies depends on the timestamp of the last time the file changes were updated.
		if (changesTimestamp.current === undefined || !canFetchDependencies) return undefined;

		switch (props.type) {
			case 'single':
				return dependencyService.fileDependencies(
					props.projectId,
					changesTimestamp.current,
					props.filePath
				);
			case 'multiple':
				return dependencyService.filesDependencies(
					props.projectId,
					changesTimestamp.current,
					props.filePaths
				);
		}
	});

	export const imports = {
		get deps(): FileDependencyData | undefined {
			const data = fileDependencies?.current.data;
			if (data === undefined) return undefined;

			if (props.type === 'single' && !(data instanceof Map)) {
				return {
					type: 'single',
					data
				};
			}
			if (props.type === 'multiple' && data instanceof Map) {
				return {
					type: 'multiple',
					data
				};
			}

			// If the data is not in the expected format, throw an error.
			throw new Error(
				`Expected file dependencies to be in the format of ${props.type}, but got ${typeof data}`
			);
		}
	};
</script>
