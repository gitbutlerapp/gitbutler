<script lang="ts">
    import type { PageData } from "./$types";
    import { TimelineDay } from "$lib/components/timeline";
    import { dummySessions } from "$lib/session";

    export let data: PageData;
    const { project, sessions } = data;
</script>

<div>
    {#if $project}
        <TimelineDay
            projectId={$project?.id}
            sessions={Array(2).fill(dummySessions).flat().concat($sessions)}
        />
    {/if}
</div>
