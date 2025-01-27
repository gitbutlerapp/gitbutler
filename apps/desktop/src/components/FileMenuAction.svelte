<script lang="ts">
	import { ProjectsService } from '$lib/project/projectsService';
	import { clonePath } from '$lib/routes/routes.svelte';
	import { ShortcutService } from '$lib/shortcuts/shortcutService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { goto } from '$app/navigation';

	const projectsService = getContext(ProjectsService);
	const shortcutService = getContext(ShortcutService);

	shortcutService.on('add-local-repo', async () => {
		await projectsService.addProject();
	});

	shortcutService.on('clone-repo', async () => {
		goto(clonePath());
	});
</script>
