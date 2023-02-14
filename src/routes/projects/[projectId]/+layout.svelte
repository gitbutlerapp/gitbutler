<script lang="ts">
    import type { LayoutData, LayoutLoad } from "./$types";
    export let data: LayoutData;

    $: project = data.project;
    $: sessions = data.sessions;
    $: lastSessionId = $sessions[$sessions.length - 1]?.id;
    console.log(project)
</script>

<div class="flex flex-row justify-between">
    <div class="p-3 flex flex-row space-x-3 text-zinc-500 text-lg select-none">
        <div>Week</div>
        <a href="/projects/{$project?.id}" class="hover:text-zinc-300">Day</a>
        {#if lastSessionId}
            <a
                href="/projects/{$project?.id}/sessions/{lastSessionId}"
                class="hover:text-zinc-300"
                title="go to current session">Session</a
            >
        {/if}

    </div>
    <div class="flex flex-row p-4 space-x-4">
        <div>{$project?.title}</div>
        <div><a href="/projects/{$project?.id}/settings">settings</a></div>
    </div>
</div>

<slot />
