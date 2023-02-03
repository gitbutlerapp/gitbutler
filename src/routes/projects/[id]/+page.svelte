<script lang="ts">
    import { derived, writable } from "svelte/store";
    import { EventType, watch, type Event } from "$lib/watch";
    import { TextDocument } from "$lib/crdt";
    import { NoSuchFileOrDirectoryError, readFile, readDir } from "$lib/tauri";
    import type { PageData } from "./$types";
    import { Timeline } from "$lib/components";
    import { onMount } from "svelte";

    export let data: PageData;

    const project = data.project;

    const docs = writable<Record<string, TextDocument>>({});

    const deleteDocs = (...filepaths: string[]) => {
        $docs = Object.fromEntries(
            Object.entries($docs).filter(
                ([filepath, _]) => !filepaths.includes(filepath)
            )
        );
    };

    // TODO
    const shouldIgnore = (filepath: string) => {
        if (filepath.includes(".git")) return true;
        if (filepath.includes("node_modules")) return true;
        return false;
    };

    const upsertDoc = async (filepath: string) => {
        if (shouldIgnore(filepath)) return;
        return readFile(filepath)
            .then((content) => {
                if (filepath in $docs) {
                    $docs[filepath].update(content);
                    $docs = $docs;
                } else {
                    $docs[filepath] = TextDocument.new(content);
                }
            })
            .catch((err) => {
                if (err instanceof NoSuchFileOrDirectoryError) {
                    deleteDocs(filepath);
                } else {
                    throw err;
                }
            });
    };

    const onEvent = async (event: Event) => {
        const isFileCreate =
            EventType.isCreate(event.type) && event.type.create.kind === "file";
        const isFileUpdate =
            EventType.isModify(event.type) && event.type.modify.kind === "data";
        const isFileRemove = EventType.isRemove(event.type);

        if (isFileCreate || isFileUpdate) {
            for (const path of event.paths) {
                await upsertDoc(path);
            }
        } else if (isFileRemove) {
            deleteDocs(...event.paths);
        }
    };

    onMount(async () => {
        if ($project === undefined) return;
        const filepaths = await readDir($project.path);
        for (const filepath of filepaths) {
            await upsertDoc(filepath);
        }
        return watch($project.path, onEvent);
    });

    const timestamps = derived(docs, (docs) =>
        Object.values(docs).flatMap((doc) =>
            doc.getHistory().map((h) => h.time)
        )
    );

    const min = derived(timestamps, (timestamps) => Math.min(...timestamps));
    const max = derived(timestamps, (timestamps) => Math.max(...timestamps));

    const showTimeline = derived(
        [min, max],
        ([min, max]) => isFinite(min) && isFinite(max)
    );

    let value: number | undefined;
</script>

<ul class="flex flex-col gap-2">
    {#if $showTimeline}
        <Timeline min={$min} max={$max} bind:value />
    {/if}

    {#each Object.entries($docs) as [filepath, doc]}
        <li>
            <details open>
                <summary>{filepath}</summary>
                <code>
                    {value ? doc.at(value).toString() : doc.toString()}
                </code>
            </details>
        </li>
    {/each}
</ul>
