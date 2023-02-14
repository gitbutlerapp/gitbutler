<script lang="ts">
    import { Doc } from "yjs";
    import { Timeline, CodeViewer } from "$lib/components";
    import { Operation } from "$lib/deltas";
    import type { Delta } from "$lib/deltas";
    import { derived, writable } from "svelte/store";
    import type { PageData } from "./$types";
    import SessionNav from "$lib/components/session/SessionNav.svelte";

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
                        value={contentWithDeltasApplied(
                            contentAtSessionStart,
                            $deltas[filepath]
                        )}
                    />
                </details>
            {/if}
        {/each}
    </div>
    <div class="flex">Timeline</div>
</div>
