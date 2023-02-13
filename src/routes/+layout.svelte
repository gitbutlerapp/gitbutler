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

<div class="flex flex-col">
    <header
        data-tauri-drag-region
        class="sticky top-0 z-50 flex
    h-8
    flex-row
    items-center justify-between overflow-hidden
    border-b bg-zinc-50
    text-sm
    text-zinc-400 
    dark:border-zinc-700 dark:bg-zinc-900
    "
    >
        <div class="ml-24">
            <BackForwardButtons />
        </div>
        <div
            class="
        invisible w-1/3
        max-w-[40rem]
        cursor-default rounded-md
        border
        text-center text-sm
        text-zinc-400
        outline-none
        hover:border-zinc-200
        dark:border-zinc-700
        dark:bg-zinc-800
        dark:hover:border-zinc-600
        min-[320px]:visible
    "
        >
            <div class="center">Search GitButler</div>
        </div>
        <div class="mr-4 cursor-default font-bold">User</div>
    </header>

    <main class="flex h-screen flex-grow flex-row text-zinc-400">
        <div
            id="sidebar"
            class="
            flex h-screen 
            w-1/4 flex-col bg-zinc-50
            dark:bg-zinc-900"
        >
            <div
                class="relative flex h-10 flex-none items-center border-b border-zinc-700 hover:bg-zinc-800"
            >
                <div
                    class="
            flex-grow"
                >
                    <DropDown projects={$projects} />
                </div>
                <button class="flex-shrink" on:click={onSelectProjectClick}>
                    <div
                        class="absolute -my-2 -mx-8 flex h-5 w-5 cursor-default select-none 
                        items-center
                        justify-center rounded-full bg-zinc-600 text-sm font-bold hover:bg-zinc-300 hover:text-zinc-600"
                        title="Add new repository"
                    >
                        +
                    </div>
                </button>
            </div>

            <footer
                class="fixed
                       bottom-0
                       z-50 flex h-8 w-full
                       flex-grow border-t bg-zinc-50
                       text-sm
                       text-zinc-400
                       dark:border-zinc-700
                       dark:bg-zinc-900
"
            >
                <div class="ml-4 flex flex-row items-center space-x-2 text-sm">
                    <div class="h-2 w-2 rounded-full bg-green-700" />
                    <div>Up to date</div>
                </div>
            </footer>
        </div>

        <div
            id="main"
            class="
    flex-grow
    border-l
    dark:border-zinc-700 dark:bg-zinc-800
        "
        >
            <slot />
        </div>
    </main>
</div>
