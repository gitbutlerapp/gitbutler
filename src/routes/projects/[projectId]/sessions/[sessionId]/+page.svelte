<script lang="ts">
    import { Doc } from "yjs";
    import { Timeline, CodeViewer } from "$lib/components";
    import { Operation } from "$lib/deltas";
    import { toHumanReadableTime } from "$lib/time";
    import { derived, writable } from "svelte/store";
    import type { PageData } from "./$types";
    import SessionNavBlock from "$lib/components/session/SessionNavBlock.svelte";
    import FaAngleLeft from "svelte-icons/fa/FaAngleLeft.svelte";
    import FaAngleRight from "svelte-icons/fa/FaAngleRight.svelte";
    import SessionNav from "$lib/components/session/SessionNav.svelte";


    export let data: PageData;
    // const { deltas, files, session, nextSession, previousSesssion, project } = data;

    // const value = writable(new Date().getTime());

    // $: console.log(data.deltas);

    // const docs = derived(value, (value) =>
    //     Object.fromEntries(
    //         Object.entries(data.deltas).map(([filePath, deltas]) => {
    //             const doc = new Doc();
    //             const text = doc.getText();
    //             if (filePath in data.files) {
    //                 text.insert(0, data.files[filePath]);
    //             }
    //             const operations = deltas
    //                 .filter((delta) => delta.timestampMs <= value)
    //                 .flatMap((delta) => delta.operations);
    //             operations.forEach((operation) => {
    //                 if (Operation.isInsert(operation)) {
    //                     text.insert(operation.insert[0], operation.insert[1]);
    //                 } else if (Operation.isDelete(operation)) {
    //                     text.delete(operation.delete[0], operation.delete[1]);
    //                 }
    //             });
    //             return [filePath, text.toString()];
    //         })
    //     )
    // );

    // const timestamps = Object.values(data.deltas).flatMap((deltas) =>
    //     Object.values(deltas).map((delta) => delta.timestampMs)
    // );

    // const min = Math.min(...timestamps);
    // const max = Math.max(...timestamps);

    $: project = data.project;
    $: previousSesssion = data.previousSesssion;
    $: nextSession = data.nextSession;
    $: session = data.session;

    // const showTimeline = isFinite(min) && isFinite(max);
</script>

<div class="">
    <div class="flex justify-center border-y border-zinc-700">
        <SessionNav
            project={$project}
            session={$session}
            nextSession={$nextSession}
            previousSesssion={$previousSesssion}
        />
    </div>

    <div id="debug" class="mt-24">
        session hash: {$session?.hash}
    </div>
</div>

<!-- <ul class="flex flex-col space-y-4 text-zinc-300">
    <div id="sessions-nav" class="flex flex-row space-x-4 my-4">
        <a href="/projects/{$project?.id}/sessions/{$previousSesssion?.hash}">
            
            prev
        </a>
        <SessionNavBlock session={$session} />
        <a href="/projects/{$project?.id}/sessions/{$nextSession?.hash}">
            next
        </a>
    </div>
{$session?.hash}

    {#if showTimeline}
        <Timeline {min} {max} on:value={(e) => value.set(e.detail)} />
    {/if}

    {#each Object.entries($docs) as [filepath, value]}
        <li>
            <details open>
                <summary>{filepath}</summary>
                <CodeViewer {value} />
            </details>
        </li>
    {/each}
</ul> -->
