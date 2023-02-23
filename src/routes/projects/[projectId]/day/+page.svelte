<script lang="ts">
    import MdKeyboardArrowLeft from "svelte-icons/md/MdKeyboardArrowLeft.svelte";
    import MdKeyboardArrowRight from "svelte-icons/md/MdKeyboardArrowRight.svelte";
    import { TimelineDaySession } from "$lib/components/timeline";
    import type { PageData } from "./$types";
    import type { Session } from "$lib/sessions";
    import { derived } from "svelte/store";
    export let data: PageData;
    const { project, sessions } = data;

    let date = new Date();
    $: canNavigateForwad =
        new Date(date.getTime() + 24 * 60 * 60 * 1000) < new Date();

    const formatDate = (date: Date) => {
        return new Intl.DateTimeFormat("default", {
            weekday: "short",
            day: "numeric",
            month: "short",
        }).format(date);
    };

    const sessionDisplayWidth = (session: Session) => {
        let sessionDurationMinutes =
            (session.meta.lastTimestampMs - session.meta.startTimestampMs) / 60;
        if (sessionDurationMinutes <= 10) {
            return "w-40 min-w-40";
        } else {
            return "w-60 min-w-60";
        }
    };

    $: sessionsInDay = derived([sessions], ([sessions]) => {
        const start = new Date(
            date.getFullYear(),
            date.getMonth(),
            date.getDate()
        );
        const end = new Date(start.getTime() + 24 * 60 * 60 * 1000);
        return sessions.filter((session) => {
            return (
                start <= new Date(session.meta.startTimestampMs) &&
                new Date(session.meta.startTimestampMs) <= end
            );
        });
    });
</script>

<div class="flex flex-col h-full select-none text-zinc-400">
    <header
        class="flex items-center justify-between flex-none px-8 py-1.5 border-b border-zinc-700"
    >
        <div class="flex items-center justify-start w-64">
            <button
                class="-ml-2 w-8 h-8 hover:text-zinc-100"
                on:click={() =>
                    (date = new Date(date.getTime() - 24 * 60 * 60 * 1000))}
            >
                <MdKeyboardArrowLeft />
            </button>
            <div class="flex-grow text-center">
                {formatDate(date)}
            </div>
            <button
                class="-mr-2 w-8 h-8 hover:text-zinc-100 disabled:text-zinc-700"
                disabled={!canNavigateForwad}
                on:click={() => {
                    if (canNavigateForwad) {
                        date = new Date(date.getTime() + 24 * 60 * 60 * 1000);
                    }
                }}
            >
                <MdKeyboardArrowRight />
            </button>
        </div>
    </header>

    <div class="w-full h-full overflow-scroll mx-2 flex">
        {#if $project}
            <div class="flex-grow items-center justify-center mt-4">
                <div class="justify-center flex flex-row space-x-2 pt-2">
                    {#each $sessionsInDay as session}
                        <div class={sessionDisplayWidth(session)}>
                            <TimelineDaySession
                                projectId={$project.id}
                                {session}
                            />
                        </div>
                    {/each}
                </div>
            </div>
        {:else}
            <p>Project not found</p>
        {/if}
    </div>
</div>
