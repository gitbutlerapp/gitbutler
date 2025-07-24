<script lang="ts">
	import { goto } from '$app/navigation';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { clonePath } from '$lib/routes/routes.svelte';
	import { SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService.svelte';
	import { inject } from '@gitbutler/shared/context';

	const projectsService = inject(PROJECTS_SERVICE);
	const shortcutService = inject(SHORTCUT_SERVICE);

	shortcutService.on('add-local-repo', async () => {
		await projectsService.addProject();
	});

	shortcutService.on('clone-repo', async () => {
		goto(clonePath());
	});
</script>
