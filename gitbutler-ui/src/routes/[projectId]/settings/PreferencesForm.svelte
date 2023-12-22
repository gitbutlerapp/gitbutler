<script lang="ts">
	import type { Project } from '$lib/backend/projects';
	import Checkbox from '$lib/components/Checkbox.svelte';
	import { createEventDispatcher } from 'svelte';
	import { projectRunCommitHooks } from '$lib/config/config';

	export let project: Project;

	let allowForcePushing = project?.ok_with_force_push;

	const runCommitHooks = projectRunCommitHooks(project.id);
	const dispatch = createEventDispatcher<{
		updated: {
			ok_with_force_push: boolean;
		};
	}>();
</script>

<div class="flex flex-1 flex-col gap-1 rounded-lg border border-light-400 p-2 dark:border-dark-500">
	<form class="flex items-center gap-1">
		<Checkbox
			name="allow-force-pushing"
			checked={allowForcePushing}
			on:change={() => {
				allowForcePushing = !allowForcePushing;
				dispatch('updated', { ok_with_force_push: allowForcePushing });
			}}
		/>
		<label class="ml-2" for="allow-force-pushing">
			<div>Allow force pushing</div>
		</label>
	</form>
	<p class="ml-7 text-light-700 dark:text-dark-200">
		Force pushing allows GitButler to override branches even if they were pushed to remote. We will
		never force push to the trunk.
	</p>

	<form class="flex items-center gap-1">
		<Checkbox
			name="run-commit-hooks"
			checked={$runCommitHooks}
			on:change={() => {
				$runCommitHooks = !$runCommitHooks;
			}}
		/>
		<label class="ml-2" for="allow-force-pushing">
			<div>Run commit hooks</div>
		</label>
	</form>
	<p class="ml-7 text-light-700 dark:text-dark-200">
		Enabling this will run any git pre and post commit hooks you have configured in your repository.
	</p>
</div>
