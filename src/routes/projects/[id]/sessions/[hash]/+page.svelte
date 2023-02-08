<script lang="ts">
    export let data: PageData;
    import { Doc } from "yjs";
    import { Timeline, CodeViewer } from "$lib/components";
    import { Operation } from "$lib/crdt";
    import { derived, writable } from "svelte/store";
    const { session, deltas } = data; // TODO deltas should be taken from session / files instead of parent
    const x = $session;

    const value = writable(new Date().getTime());

    const docs = derived([deltas, value], ([deltas, value]) =>
        Object.fromEntries(
            Object.entries(deltas).map(([filePath, deltas]) => {
                const doc = new Doc();
                const text = doc.getText();
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

    const timestamps = derived(deltas, (deltas) =>
        Object.values(deltas).flatMap((deltas) =>
            Object.values(deltas).map((delta) => delta.timestampMs)
        )
    );

    const min = derived(timestamps, (timestamps) => Math.min(...timestamps));
    const max = derived(timestamps, (timestamps) => Math.max(...timestamps));

    const showTimeline = derived(
        [min, max],
        ([min, max]) => isFinite(min) && isFinite(max)
    );
</script>
<ul class="flex flex-col gap-2">
    {#if $showTimeline}
        <Timeline min={$min} max={$max} on:value={(e) => value.set(e.detail)} />
    {/if}

    {#each Object.entries($docs) as [filepath, value]}
        <li>
            <details open>
                <summary>{filepath}</summary>
                <CodeViewer {value} />
            </details>
        </li>
    {/each}
</ul>
