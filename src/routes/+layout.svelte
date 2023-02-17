<script lang="ts">
    import "../app.postcss";

    import type { LayoutData } from "./$types";
    import { log } from "$lib";
    import { onMount } from "svelte";
    import { BackForwardButtons } from "$lib/components";
    import { setContext } from "svelte";
    import { writable } from "svelte/store";
    import Breadcrumbs from "$lib/components/Breadcrumbs.svelte";

    setContext("project", writable(null));
    setContext("session", writable(null));

    onMount(log.setup);

    export let data: LayoutData;
    const { user } = data;
</script>

<header
    data-tauri-drag-region
    class="sticky top-0 z-50 flex
    h-8
    flex-row
    items-center overflow-hidden
    border-b bg-zinc-50
    text-sm
    text-zinc-400 
    dark:border-zinc-700 dark:bg-zinc-900
    select-none
    "
>
    <div class="ml-24">
        <BackForwardButtons />
    </div>
    <div class="ml-6"><Breadcrumbs /></div>
    <div class="flex-grow" />
    <a href="/users/" class="mr-4 font-bold hover:text-zinc-200"
        >{$user ? $user.name : "User"}</a
    >
</header>

<div class="bg-zinc-800 h-screen text-zinc-400 flex flex-col">
    <div class="flex-grow">
        <slot />
    </div>

    <div
        class="border-t border-zinc-700 h-6 flex items-center bg-zinc-900 select-none "
    >
        <div class="ml-4 flex flex-row items-center space-x-2 text-xs">
            <div class="h-2 w-2 rounded-full bg-green-700" />
            <div>Up to date</div>
        </div>
    </div>
    <div id="foo" class="h-8" />
</div>
