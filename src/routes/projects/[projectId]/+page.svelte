<script lang="ts">
    import type { PageData } from "./$types";
    import type { Session } from "$lib/sessions";
    import { TimelineDaySession } from "$lib/components/timeline";

    export let data: PageData;
    const { project, sessions } = data;

    const sessionDisplayWidth = (session: Session) => {
        let sessionDurationMinutes =
            (session.meta.lastTs - session.meta.startTs) / 60;
        if (sessionDurationMinutes <= 10) {
            return "w-40 min-w-40";
        } else {
            return "w-60 min-w-60";
        }
    };
</script>

<div class="w-full h-full overflow-scroll mx-2 flex">
    {#if $project}
        <div class="flex-grow items-center justify-center mt-4">
            <div class="justify-center flex flex-row space-x-2 pt-2">
                {#each $sessions as session}
                    <div class={sessionDisplayWidth(session)}>
                        <TimelineDaySession projectId={$project.id} {session} />
                    </div>
                {/each}
            </div>
        </div>
    {:else}
        <p>Project not found</p>
    {/if}
</div>
