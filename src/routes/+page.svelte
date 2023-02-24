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

<div class="w-full h-full p-8">
    <div class="flex flex-col h-full">
        {#if $projects.length == 0}
            <div class="h-fill grid grid-cols-2 gap-4 items-center h-full">
                <!-- right box, welcome text -->
                <div class="flex flex-col space-y-4 content-center p-4">
                    <div class="text-xl text-zinc-300 p-0 m-0">
                        <div class="font-bold">Welcome to GitButler.</div>
                        <div class="text-lg text-zinc-300 mb-1">
                            More than just version control.
                        </div>
                    </div>
                    <div class="">
                        GitButler is a tool to help you manage all the local
                        work you do on your code projects.
                    </div>
                    <div class="">
                        Think of us as a <strong>code concierge</strong>, a
                        smart assistant for all the coding related tasks you
                        need to do every day.
                    </div>
                    <ul class="text-zinc-400 pt-2 pb-4 space-y-4">
                        <li class="flex flex-row space-x-3">
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                fill="none"
                                viewBox="0 0 24 24"
                                stroke-width="1.5"
                                stroke="currentColor"
                                class="w-8 h-8 flex-none"
                            >
                                <path
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    d="M21 16.811c0 .864-.933 1.405-1.683.977l-7.108-4.062a1.125 1.125 0 010-1.953l7.108-4.062A1.125 1.125 0 0121 8.688v8.123zM11.25 16.811c0 .864-.933 1.405-1.683.977l-7.108-4.062a1.125 1.125 0 010-1.953L9.567 7.71a1.125 1.125 0 011.683.977v8.123z"
                                />
                            </svg>

                            <span class="text-zinc-200"
                                >Automatically recording everything you do in
                                any of your butlered projects.</span
                            >
                        </li>
                        <li class="flex flex-row space-x-3">
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                fill="none"
                                viewBox="0 0 24 24"
                                stroke-width="1.5"
                                stroke="currentColor"
                                class="w-8 h-8 flex-none"
                            >
                                <path
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    d="M21 11.25v8.25a1.5 1.5 0 01-1.5 1.5H5.25a1.5 1.5 0 01-1.5-1.5v-8.25M12 4.875A2.625 2.625 0 109.375 7.5H12m0-2.625V7.5m0-2.625A2.625 2.625 0 1114.625 7.5H12m0 0V21m-8.625-9.75h18c.621 0 1.125-.504 1.125-1.125v-1.5c0-.621-.504-1.125-1.125-1.125h-18c-.621 0-1.125.504-1.125 1.125v1.5c0 .621.504 1.125 1.125 1.125z"
                                />
                            </svg>

                            <span class="text-zinc-200"
                                >Simplifying all your Git work, including
                                committing, branching and pushing, to be easy
                                and intuitive.
                            </span>
                        </li>
                        <li class="flex flex-row space-x-3">
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                fill="none"
                                viewBox="0 0 24 24"
                                stroke-width="1.5"
                                stroke="currentColor"
                                class="w-8 h-8 flex-none"
                            >
                                <path
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    d="M15.75 15.75l-2.489-2.489m0 0a3.375 3.375 0 10-4.773-4.773 3.375 3.375 0 004.774 4.774zM21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                                />
                            </svg>

    <div class="flex flex-row">
        <div class="w-1/2">
            <div class="text-xl text-zinc-200 mb-1">Start</div>
            <div class="flex items-center space-x-2">
                <div class="w-4 h-4"><FaFolderOpen /></div>
                <button
                    on:click={onAddLocalRepositoryClick}
                    class="hover:text-zinc-200">Add a local repository...</button
                >
            </div>
        </div>
        <div class="w-1/2">
            <div class="text-xl text-zinc-200 mb-1">Recent</div>
            <div class="flex flex-col space-y-1">
                {#each $projects as project}
                    <div class="space-x-2">
                        <a
                            class="hover:text-zinc-200"
                            href="/projects/{project.id}/">{project.title}</a
                        >
                        <span class="text-zinc-500">{project.path}</span>
                    </div>
                {/each}
            </div>
        </div>
    </div>
</div>
