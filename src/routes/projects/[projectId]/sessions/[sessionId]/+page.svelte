<script lang="ts">
    import { Doc } from "yjs";
    import { CodeViewer } from "$lib/components";
    import { Operation } from "$lib/deltas";
    import { derived } from "svelte/store";
    import type { PageData } from "./$types";
    import SessionNav from "$lib/components/session/SessionNav.svelte";
    import { toHumanReadableTime } from "$lib/time";

    export let data: PageData;

    $: project = data.project;
    $: previousSesssion = data.previousSesssion;
    $: nextSession = data.nextSession;
    $: session = data.session;
    $: deltas = data.deltas;

    $: selection = {} as Record<string, Record<string, number>>;

    $: docsNext = derived([data.deltas], ([deltas]) =>
        Object.fromEntries(
            Object.entries(deltas).map(([filePath, deltas]) => {
                const doc = new Doc();
                const text = doc.getText();
                if (filePath in data.files) {
                    text.insert(0, data.files[filePath]);
                }
                const contentAtStart = text.toString();

                if (!selection.hasOwnProperty($session?.id)) {
                    selection[$session.id] = {};
                }
                if (!selection[$session.id].hasOwnProperty(filePath)) {
                    selection[$session?.id][filePath] = deltas.length - 1;
                }

                const operations = deltas
                    .filter(
                        (_, index) => index <= selection[$session.id][filePath]
                    )
                    .flatMap((delta) => delta.operations);

                operations.forEach((operation) => {
                    if (Operation.isInsert(operation)) {
                        text.insert(operation.insert[0], operation.insert[1]);
                    } else if (Operation.isDelete(operation)) {
                        text.delete(operation.delete[0], operation.delete[1]);
                    }
                });

                const contentAfterDeltas = text.toString();
                return [filePath, { a: contentAtStart, b: contentAfterDeltas }];
            })
        )
    );
</script>

<div class="flex flex-col w-full h-full overflow-hidden space-y-2">
    <div class="flex justify-center border-b border-zinc-700 p-2">
        <SessionNav
            project={$project}
            session={$session}
            nextSession={$nextSession}
            previousSesssion={$previousSesssion}
        />
    </div>

    <div class="overflow-auto h-2/3 mx-4">
        {#each Object.entries($docsNext) as [filepath, content]}
            <details open>
                <summary>{filepath}</summary>
                <div class="flex flex-col">
                    <div class="flex space-x-1">
                        {#each $deltas[filepath] as delta, i}
                            <button
                                on:click={() => {
                                    selection[$session.id][filepath] = i;
                                }}
                                class="text-center items-center justify-center text-xs rounded-full h-4 w-4 text-zinc-600 hover:bg-zinc-200
{selection[$session.id][filepath] == i ? 'bg-orange-300' : 'bg-zinc-400'}
                                "
                                title={toHumanReadableTime(delta.timestampMs)}
                            >
                                <span>{delta.operations.length}</span>
                            </button>
                        {/each}
                    </div>
                    <CodeViewer value={content.a} newValue={content.b} />
                </div>
            </details>
        {/each}
    </div>
</div>
