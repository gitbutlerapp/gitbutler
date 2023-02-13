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
    <a href="/users/" class="mr-4 cursor-default font-bold">User</a>
</header>

<div class="flex h-screen flex-grow flex-row text-zinc-400 overflow-hidden">
    <div
        id="sidebar"
        class="
            overflow-auto
            flex
            w-1/4 flex-col bg-zinc-50
            border-r
            border-zinc-700
            dark:bg-zinc-900"
    >
        <div
            class=" flex h-10 items-center border-b border-zinc-700 hover:bg-zinc-800"
        >
            <div class="flex-grow">
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

        <div class="flex-grow" />

        <div class="border-t border-zinc-700 h-8 flex items-center">
            <div class="ml-4 flex flex-row items-center space-x-2 text-sm">
                <div class="h-2 w-2 rounded-full bg-green-700" />
                <div>Up to date</div>
            </div>
        </div>
        <div id="foo" class="h-8" />
    </div>

    <div
        class="flex-grow h-full border-ldark:border-zinc-700 dark:bg-zinc-800 overflow-hidden"
    >
        <slot />
    </div>
</div>
