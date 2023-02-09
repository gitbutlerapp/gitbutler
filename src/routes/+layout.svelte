<script lang="ts">
    import "../app.postcss";

    import { open } from "@tauri-apps/api/dialog";
    import type { LayoutData } from "./$types";
    import { log } from "$lib";
    import { onMount } from "svelte";
    import { BackForwardButtons } from "$lib/components";
    import DropDown from "$lib/components/DropDown.svelte";

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
        class="h-8 sticky top-0 z-50
    overflow-hidden
    border-b
    bg-zinc-50 dark:bg-zinc-900 dark:border-zinc-700
    flex flex-row
    items-center
    justify-between 
    text-zinc-400 text-sm
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
    text-zinc-400 text-sm
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

    <main class="text-zinc-400 flex-grow flex flex-row">
        <div
            id="sidebar"
            class="
            bg-zinc-50 dark:bg-zinc-900 
            flex flex-col
            w-1/4"
        >
            <div
                class="relative flex border-b border-zinc-700 h-10 items-center hover:bg-zinc-800"
            >
                <div
                    class="
            flex-grow"
                >
                    <DropDown projects={$projects} />
                </div>
                <button class="flex-shrink" on:click={onSelectProjectClick}>
                    <div
                        class="absolute -my-2 -mx-8 rounded-full select-none cursor-default w-5 h-5 
                        text-sm
                        bg-zinc-600 hover:bg-zinc-300 hover:text-zinc-600 flex items-center justify-center font-bold"
                        title="Add new repository"
                    >
                        +
                    </div>
                </button>
            </div>
            <div class="flex-grow">
                <div class="flex flex-col px-4 my-4 space-y-4">
                    <div class="border-b border-zinc-700">
                        <div>Timeline</div>
                        <div class="px-4">Week</div>
                    </div>
                    <div>Branches</div>
                </div>
            </div>

            <footer
                class="h-8
        border-t
    text-zinc-400 text-sm bottom-0
    bg-zinc-50 dark:bg-zinc-900 dark:border-zinc-700
    flex
    items-center
"
            >
                <div class="ml-4 text-sm flex flex-row items-center space-x-2">
                    <div class="rounded-full h-2 w-2 bg-green-700" />
                    <div>Up to date</div>
                </div>
            </footer>
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
</div>
