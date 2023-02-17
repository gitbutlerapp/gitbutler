<script lang="ts">
    import type { LayoutData } from "./$types";
    import { getContext } from 'svelte';
    import type { Writable } from "svelte/store";
    import type { Project } from "$lib/projects";
    import { onDestroy } from "svelte";

    export let data: LayoutData;

    $: project = data.project;
    $: sessions = data.sessions;
    $: lastSessionId = $sessions[$sessions.length - 1]?.id;

    const contextProjectStore: Writable<Project|null|undefined> = getContext('project')
    $: contextProjectStore.set($project)
    onDestroy(() => {
        contextProjectStore.set(null)
    })

</script>

<nav
    class="h-12 p-3 flex justify-between space-x-3 text-zinc-500 text-lg select-none border-b border-zinc-700"
>
    <ul class="flex gap-2">
        <li>
            <div>Week</div>
        </li>
        <li>
            <a href="/projects/{$project?.id}" class="hover:text-zinc-300"
                >Day</a
            >
        </li>
        {#if lastSessionId}
            <li>
                <a
                    href="/projects/{$project?.id}/sessions/{lastSessionId}"
                    class="hover:text-zinc-300"
                    title="go to current session">Session</a
                >
            </li>
        {/if}
    </ul>

    <ul>
        <li>
            <a
                href="/projects/{$project?.id}/settings"
                class="hover:text-zinc-300">Settings</a
            >
        </li>
    </ul>
</nav>

<slot />
