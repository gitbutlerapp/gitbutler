<script lang="ts">
    import type { LayoutData } from "./$types";
    import { getContext } from "svelte";
    import type { Writable } from "svelte/store";
    import type { Project } from "$lib/projects";
    import { onDestroy } from "svelte";

    export let data: LayoutData;

    $: project = data.project;
    $: sessions = data.sessions;
    $: lastSessionId = $sessions[$sessions.length - 1]?.id;

    function projectUrl(project: Project) {
        const gitUrl = project.api?.git_url;
        // get host from git url
        const url = new URL(gitUrl);
        const host = url.origin;
        const projectId = gitUrl.split("/").pop();

        return `${host}/projects/${projectId}`;
    }

    const contextProjectStore: Writable<Project | null | undefined> =
        getContext("project");
    $: contextProjectStore.set($project);
    onDestroy(() => {
        contextProjectStore.set(null);
    });
</script>

<nav
    class="flex flex-none justify-between h-12 p-2 px-4 space-x-3 text-lg border-b select-none text-zinc-300 border-zinc-700"
>
    <ul class="flex gap-4">
        <li>
            <div>
                <a
                    class="hover:text-zinc-200"
                    href="/projects/{$project?.id}/week">Week</a
                >
            </div>
        </li>
        <li>
            <a href="/projects/{$project?.id}/day" class="hover:text-zinc-200"
                >Day</a
            >
        </li>
        {#if lastSessionId}
            <li>
                <a
                    href="/projects/{$project?.id}/sessions/{lastSessionId}"
                    class="hover:text-zinc-200"
                    title="go to current session">Session</a
                >
            </li>
        {/if}
    </ul>

    <ul>
        <li>
            <a
                href="/projects/{$project?.id}/settings"
                class="text-zinc-400 hover:text-zinc-300"
            >
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke-width="1.5"
                    stroke="currentColor"
                    class="w-6 h-6"
                >
                    <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.324.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 011.37.49l1.296 2.247a1.125 1.125 0 01-.26 1.431l-1.003.827c-.293.24-.438.613-.431.992a6.759 6.759 0 010 .255c-.007.378.138.75.43.99l1.005.828c.424.35.534.954.26 1.43l-1.298 2.247a1.125 1.125 0 01-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 01-.22.128c-.331.183-.581.495-.644.869l-.213 1.28c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.02-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 01-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 01-1.369-.49l-1.297-2.247a1.125 1.125 0 01.26-1.431l1.004-.827c.292-.24.437-.613.43-.992a6.932 6.932 0 010-.255c.007-.378-.138-.75-.43-.99l-1.004-.828a1.125 1.125 0 01-.26-1.43l1.297-2.247a1.125 1.125 0 011.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.087.22-.128.332-.183.582-.495.644-.869l.214-1.281z"
                    />
                    <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
                    />
                </svg>
            </a>
        </li>
    </ul>
</nav>

<slot />

<div class="absolute bottom-0 left-0 w-full">
    <div
        class="flex items-center flex-shrink-0 h-6 border-t select-none border-zinc-700 bg-zinc-900 "
    >
        <div
            class="flex flex-row mx-4 items-center space-x-2 text-xs justify-between w-full"
        >
            {#if $project?.api?.sync}
                <a
                    href="/projects/{$project?.id}/settings"
                    class="text-zinc-400 hover:text-zinc-300"
                >
                    <div class="flex flex-row items-center space-x-2 text-xs">
                        <div class="w-2 h-2 bg-green-700 rounded-full" />
                        <div class="text-zinc-200">Syncing</div>
                    </div>
                </a>
                <a target="_blank" href={projectUrl($project)}
                    >Open in GitButler Cloud</a
                >
            {:else}
                <a
                    href="/projects/{$project?.id}/settings"
                    class="text-zinc-400 hover:text-zinc-300"
                >
                    <div class="flex flex-row items-center space-x-2 text-xs">
                        <div class="w-2 h-2 bg-red-700 rounded-full" />
                        <div class="text-zinc-200">Offline</div>
                    </div>
                </a>
            {/if}
        </div>
    </div>
</div>
