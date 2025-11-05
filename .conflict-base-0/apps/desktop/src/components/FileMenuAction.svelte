<script lang="ts">
	import { goto } from '$app/navigation';
	import { handleAddProjectOutcome } from '$lib/project/project';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { clonePath, projectPath } from '$lib/routes/routes.svelte';
	import { SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService';
	import { inject } from '@gitbutler/core/context';
	import { mergeUnlisten } from '@gitbutler/ui/utils/mergeUnlisten';

	const projectsService = inject(PROJECTS_SERVICE);
	const shortcutService = inject(SHORTCUT_SERVICE);

	$effect(() =>
		mergeUnlisten(
			shortcutService.on('add-local-repo', async () => {
				const outcome = await projectsService.addProject();
				if (!outcome) {
					// User cancelled the project creation
					return;
				}
				handleAddProjectOutcome(outcome, (project) => goto(projectPath(project.id)));
			}),
			shortcutService.on('clone-repo', async () => {
				goto(clonePath());
			})
		)
	);
</script>
