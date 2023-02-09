<script lang="ts">
    import "../app.postcss";

    import { open } from "@tauri-apps/api/dialog";
    import type { LayoutData } from "./$types";
    import { log } from "$lib";
    import { onMount } from "svelte";
    import { BackForwardButtons } from "$lib/components";

    onMount(log.setup);

    export let data: LayoutData;
    const { projects } = data;

    const onSelectProjectClick = async () => {
        const selectedPath = await open({
            directory: true,
            recursive: true,
        });
        if (selectedPath === null) return;
        if (Array.isArray(selectedPath) && selectedPath.length !== 1) return;
        const projectPath = Array.isArray(selectedPath)
            ? selectedPath[0]
            : selectedPath;

        const projectExists = $projects.some((p) => p.path === projectPath);
        if (projectExists) return;

        await projects.add({ path: projectPath });
    };
</script>

<div class="flex flex-col h-screen">
    <header
        data-tauri-drag-region
        class="h-7 sticky top-0 z-50
    overflow-hidden
    border-b
    bg-zinc-50 dark:bg-zinc-900 dark:border-zinc-700
    flex flex-row
    items-center
    justify-between 
    text-zinc-400 text-xs
    "
    >
        <div class="ml-24">
            <BackForwardButtons />
        </div>
        <div
            class="
    border rounded-md
    hover:border-zinc-200
    dark:bg-zinc-800 dark:border-zinc-700
    dark:hover:border-zinc-600
    text-zinc-400 text-xs
    outline-none
    w-1/3
    max-w-[40rem]
    text-center
    cursor-default
    invisible
    min-[320px]:visible
    "
        >
            <div class="center">Search GitButler</div>
        </div>
        <div class="mr-4 font-bold cursor-default">User</div>
    </header>

    <main class="text-zinc-400 text-xs flex-grow flex flex-row">
        <div
            id="sidebar"
            class="
            bg-zinc-50 dark:bg-zinc-900 
            p-2

        "
        >
            <nav class="flex flex-col">
                <button
                    class="rounded-lg bg-green-100 p-1 m-1"
                    on:click={onSelectProjectClick}
                    type="button"
                >
                    new project
                </button>
                <ul class="flex-1 flex flex-row gap-2 overflow-x-scroll">
                    {#each $projects as project}
                        <li class="border rounded-md bg-blue-100 p-2">
                            <a href="/projects/{project.id}/">{project.title}</a
                            >
                        </li>
                    {/each}
                </ul>
            </nav>
        </div>
        <div
            id="main"
            class="
        flex-grow
    border-l
    dark:bg-zinc-800 dark:border-zinc-700
        "
        >
            <slot />
        </div>
    </main>

    <footer
        class="h-6 
        border-t
    text-zinc-400 text-xs bottom-0
    bg-zinc-50 dark:bg-zinc-900 dark:border-zinc-700
    flex
    items-center
"
    >
        <div class="ml-4 text-xs flex flex-row items-center space-x-2">
            <div class="rounded-full h-2 w-2 bg-green-700" />
            <div>Up to date</div>
        </div>
    </footer>
</div>
