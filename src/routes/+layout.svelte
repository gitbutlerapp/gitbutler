<script lang="ts">
    import "../app.postcss";

    import type { LayoutData } from "./$types";
    import { BackForwardButtons } from "$lib/components";
    import { setContext } from "svelte";
    import { writable } from "svelte/store";
    import Breadcrumbs from "$lib/components/Breadcrumbs.svelte";

    export let data: LayoutData;
    const { user, posthog } = data;

    setContext("project", writable(null));
    setContext("session", writable(null));

    user.subscribe(posthog.identify);
</script>

<div class="flex flex-col h-0 min-h-full bg-zinc-800 text-zinc-400">
    <header
        data-tauri-drag-region
        class="sticky top-0 z-50 flex flex-row items-center h-8 overflow-hidden border-b select-none  text-zinc-400 border-zinc-700 bg-zinc-900 "
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

    <div class="flex-grow bg-zinc-800 text-zinc-400">
        <!-- <div class="flex-1"> -->
        <slot />
        <!-- </div> -->
    </div>
</div>
