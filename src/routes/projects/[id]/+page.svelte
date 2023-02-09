<script lang="ts">
    import type { PageData } from "./$types";
    import { TimelineDay } from "$lib/components/timeline";

    export let data: PageData;
    const { project, sessions, testSessions } = data;

    $: console.log($testSessions);
</script>

<div>
    {#if $project}
        <TimelineDay projectId={$project?.id} sessions={$sessions} />
    {/if}
</div>
