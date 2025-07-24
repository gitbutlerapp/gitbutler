<script lang="ts">
	import { goto } from '$app/navigation';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { clonePath } from '$lib/routes/routes.svelte';
	import { SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService';
	import { inject } from '@gitbutler/shared/context';
	import { mergeUnlisten } from '@gitbutler/ui/utils/mergeUnlisten';

	const projectsService = inject(PROJECTS_SERVICE);
	const shortcutService = inject(SHORTCUT_SERVICE);

	$effect(() =>
		mergeUnlisten(
			shortcutService.on('add-local-repo', async () => {
				await projectsService.addProject();
			}),
			shortcutService.on('clone-repo', async () => {
				goto(clonePath());
			})
		)
	);
</script>
