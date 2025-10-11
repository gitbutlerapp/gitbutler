<script lang="ts">
	import { goto } from '$app/navigation';
	import { useAddProject } from '$lib/project/useProjects.svelte';
	import { clonePath } from '$lib/routes/routes.svelte';
	import { SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService';
	import { inject } from '@gitbutler/core/context';
	import { mergeUnlisten } from '@gitbutler/ui/utils/mergeUnlisten';

	const shortcutService = inject(SHORTCUT_SERVICE);

	const { addProject } = useAddProject();

	$effect(() =>
		mergeUnlisten(
			shortcutService.on('add-local-repo', async () => {
				await addProject();
			}),
			shortcutService.on('clone-repo', async () => {
				goto(clonePath());
			})
		)
	);
</script>
