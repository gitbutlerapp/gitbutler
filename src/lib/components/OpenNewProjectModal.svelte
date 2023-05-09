<script lang="ts">
	import { open } from '@tauri-apps/api/dialog';
	import { toasts, api } from '$lib';

	export let projects: ReturnType<typeof api.projects.Projects>;

	export const show = () =>
		open({ directory: true, recursive: true })
			.then((selectedPath) => {
				if (selectedPath === null) return;
				if (Array.isArray(selectedPath) && selectedPath.length !== 1) return;
				const projectPath = Array.isArray(selectedPath) ? selectedPath[0] : selectedPath;
				return projects
					.add({ path: projectPath })
					.then(() => toasts.success('Project added successfully'));
			})
			.catch((e: any) => {
				toasts.error(e.message);
			});
</script>
