<script lang="ts">
    import type { LayoutData, LayoutLoad } from "./$types";
    export let data: LayoutData;

    $: project = data.project;
    $: sessions = data.sessions;
    $: lastSessionId = $sessions[$sessions.length - 1]?.id;
</script>

<div class="h-12 p-3 flex flex-row space-x-3 text-zinc-500 text-lg select-none border-b border-zinc-700">
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

<slot />
