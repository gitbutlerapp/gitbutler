<script lang="ts">
	import FaFolderOpen from 'svelte-icons/fa/FaFolderOpen.svelte';
	import { open } from '@tauri-apps/api/dialog';
	import type { LayoutData } from './$types';
	export let data: LayoutData;

	const { projects } = data;

	const onAddLocalRepositoryClick = async () => {
		const selectedPath = await open({
			directory: true,
			recursive: true
		});
		if (selectedPath === null) return;
		if (Array.isArray(selectedPath) && selectedPath.length !== 1) return;
		const projectPath = Array.isArray(selectedPath) ? selectedPath[0] : selectedPath;

		const projectExists = $projects.some((p) => p.path === projectPath);
		if (projectExists) return;

		await projects.add({ path: projectPath });
	};
</script>

<div class="select-none p-16">
	<div class="mb-6">
		<h1 class="text-4xl text-zinc-200 mb-2">GitButler</h1>
		<h2 class="text-2xl">Your Personal VCS Assistant</h2>
	</div>

	<div class="flex flex-row">
		<div class="w-1/2">
			<div class="text-xl text-zinc-200 mb-1">Start</div>
			<div class="flex items-center space-x-2">
				<div class="w-4 h-4"><FaFolderOpen /></div>
				<button on:click={onAddLocalRepositoryClick} class="hover:text-zinc-200"
					>Add a local repository...</button
				>
			</div>
		</div>
		<div class="w-1/2">
			<div class="text-xl text-zinc-200 mb-1">Recent</div>
			<div class="flex flex-col space-y-1">
				{#each $projects as project}
					<div class="space-x-2">
						<a class="hover:text-zinc-200" href="/projects/{project.id}/">{project.title}</a>
						<span class="text-zinc-500">{project.path}</span>
					</div>
				{/each}
			</div>
		</div>
	</div>
</div>
