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
    class="sticky top-0 z-50 flex flex-row items-center h-8 overflow-hidden text-sm border-b select-none  text-zinc-400 border-zinc-700 bg-zinc-900 "
>
    <div class="ml-24">
        <BackForwardButtons />
    </div>
    <div class="ml-6"><Breadcrumbs /></div>
    <div class="flex-grow" />
    <a href="/users/" class="mr-4 font-medium hover:text-zinc-200"
        >{$user ? $user.email : "Login"}</a
    >
</header>

<div class="h-0 min-h-full bg-zinc-800 text-zinc-400">
    <!-- <div class="flex-1"> -->
    <slot />
    <!-- </div> -->

    <!-- <div
        class="flex items-center flex-shrink-0 h-6 border-t select-none border-zinc-700 bg-zinc-900 "
    >
        <div class="flex flex-row items-center ml-4 space-x-2 text-xs">
            <div class="w-2 h-2 bg-green-700 rounded-full" />
            <div>Up to date</div>
        </div>
    </div>
    <div id="foo" class="h-8" /> -->
</div>
