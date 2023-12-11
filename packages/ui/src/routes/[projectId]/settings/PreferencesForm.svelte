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

<div
	class="flex flex-row items-center justify-between rounded-lg border border-light-400 p-2 dark:border-dark-500"
>
	<div class="flex flex-row space-x-3">
		<div class="flex flex-row">
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
					Rebase my branches instead of creating merge commmits
				</label>
			</form>
		</div>
	</div>
</div>
