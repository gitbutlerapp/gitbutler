<script lang="ts">
    import type { Session } from "$lib/sessions";
    import TimelineDaySession from "./TimelineDaySession.svelte";

    export let sessions: Session[];
    export let projectId: string;

    const sessionDisplayWidth = (session: Session) => {
        let sessionDurationHours =
            (session.meta.lastTs - session.meta.startTs) / 1000 / 60 / 60;
        if (sessionDurationHours <= 1) {
            return "w-40";
        } else {
            return "w-60";
        }
    };
</script>

<div class="mt-12">
    <div class="-mb-5 border border-slate-400 h-1.5 bg-slate-200  z-0" />
    <div class="flex flex-row space-x-2 z-10">
        {#each sessions as session}
            <div class={sessionDisplayWidth(session)}>
                <TimelineDaySession {projectId} {session} />
            </div>
        {/each}
    </div>
</div>
