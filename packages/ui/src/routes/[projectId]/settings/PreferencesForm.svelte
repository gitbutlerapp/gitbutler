<script lang="ts">
	import type { Project } from '$lib/backend/projects';
	import Checkbox from '$lib/components/Checkbox.svelte';
	import { createEventDispatcher } from 'svelte';

	export let project: Project;

	let allowForcePushing = project?.ok_with_force_push;

	const dispatch = createEventDispatcher<{
		updated: {
			ok_with_force_push: boolean;
		};
	}>();
</script>

<div class="flex-1 rounded-lg border border-light-400 p-2 dark:border-dark-500">
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
</div>
