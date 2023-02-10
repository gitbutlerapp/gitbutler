<script lang="ts">
    import { Doc } from "yjs";
    import { Timeline, CodeViewer } from "$lib/components";
    import { Operation } from "$lib/deltas";
    import { derived, writable } from "svelte/store";
    import type { PageData } from "./$types";

    export let data: PageData;
    const { deltas, files } = data;

  const value = writable(new Date().getTime());

    const docs = derived(value, (value) =>
        Object.fromEntries(
            Object.entries(deltas).map(([filePath, deltas]) => {
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

    const timestamps = Object.values(deltas).flatMap((deltas) =>
        Object.values(deltas).map((delta) => delta.timestampMs)
    );

    const min = Math.min(...timestamps);
    const max = Math.max(...timestamps);

    const showTimeline = isFinite(min) && isFinite(max);
</script>


<ul class="flex flex-col gap-2">
  {#if showTimeline}
    <Timeline min={min} max={max} on:value={(e) => value.set(e.detail)} />
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
