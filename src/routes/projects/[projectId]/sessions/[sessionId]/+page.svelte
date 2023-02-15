<script lang="ts">
    import { Doc } from "yjs";
    import { Timeline, CodeViewer } from "$lib/components";
    import { Operation } from "$lib/deltas";
    import type { Delta } from "$lib/deltas";
    import { derived, writable } from "svelte/store";
    import type { PageData } from "./$types";
    import SessionNav from "$lib/components/session/SessionNav.svelte";
    import { toHumanReadableTime } from "$lib/time";

    export let data: PageData;

    $: project = data.project;
    $: previousSesssion = data.previousSesssion;
    $: nextSession = data.nextSession;
    $: session = data.session;
    $: deltas = data.deltas;
    $: files = data.files;

    const value = writable(new Date().getTime());

    const docs = derived(value, (value) =>
        Object.fromEntries(
            Object.entries($deltas).map(([filePath, deltas]) => {
                const doc = new Doc();
                const text = doc.getText();
                if (filePath in files) {
                    text.insert(0, files[filePath]);
                }
                const operations = deltas
                    .filter((delta) => delta.timestampMs <= value)
                    .flatMap((delta) => delta.operations);
                operations.forEach((operation) => {
                    if (Operation.isInsert(operation)) {
                        text.insert(operation.insert[0], operation.insert[1]);
                    } else if (Operation.isDelete(operation)) {
                        text.delete(operation.delete[0], operation.delete[1]);
                    }
                });
                return [filePath, text.toString()];
            })
        )
    );

    const contentWithDeltasApplied = (
        contentAtSessionStart: string,
        deltas: Delta[]
    ) => {
        const doc = new Doc();
        const text = doc.getText();
        text.insert(0, contentAtSessionStart);
        const operations = deltas.flatMap((delta) => delta.operations);
        operations.forEach((operation) => {
            if (Operation.isInsert(operation)) {
                text.insert(operation.insert[0], operation.insert[1]);
            } else if (Operation.isDelete(operation)) {
                text.delete(operation.delete[0], operation.delete[1]);
            }
        });
        return text.toString();
    };

    // const timestamps = Object.values(deltas).flatMap((deltas) =>
    //     Object.values(deltas).map((delta) => delta.timestampMs)
    // );

    // const min = Math.min(...timestamps);
    // const max = Math.max(...timestamps);
    // const showTimeline = isFinite(min) && isFinite(max);
</script>

<div class="flex flex-col w-full h-full overflow-hidden space-y-2">
    <div class="flex justify-center border-y border-zinc-700 p-2">
        <SessionNav
            project={$project}
            session={$session}
            nextSession={$nextSession}
            previousSesssion={$previousSesssion}
        />
    </div>
    <div class="overflow-auto h-2/3 mx-4">
        {#each Object.entries(files) as [filepath, contentAtSessionStart]}
            {#if $deltas[filepath]}
                <details open>
                    <summary>
                        {filepath}
                    </summary>
                    <CodeViewer
                        value={contentAtSessionStart}
                        newValue={contentWithDeltasApplied(
                            contentAtSessionStart,
                            $deltas[filepath]
                        )}
                    />
                </details>
            {/if}
        {/each}
    </div>
    <div class="flex flex-col border-t border-zinc-700 mt-2">
        {#each Object.entries($deltas) as [filepath, deltas]}
            <div class="flex">
                <div class="w-32">{filepath}</div>
                <div class="flex space-x-2 items-center">
                    {#each deltas as delta}
                        <div class="cursor-pointer text-center items-center justify-center text-xs rounded-full h-4 w-4 bg-zinc-400 text-zinc-600 hover:bg-zinc-200" title="{toHumanReadableTime(delta.timestampMs)}">
                            <span>{delta.operations.length}</span>
                        </div>
                    {/each}
                </div>
            </div>
        {/each}
    </div>
</div>
