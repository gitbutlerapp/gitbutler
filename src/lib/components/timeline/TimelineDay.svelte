<script lang="ts">
    import type { Session } from "$lib/session";
    import TimelineDaySession from "./TimelineDaySession.svelte";
    export let sessions: Session[];
</script>

<div>
    <div class="flex flex-row space-x-2">
        {#each sessions as session}
            <div>
                <TimelineDaySession session={session}></TimelineDaySession>
            </div>
        {/each}
    </div>
</div>
