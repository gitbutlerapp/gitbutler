<script lang="ts">
    import { open } from "@tauri-apps/api/dialog";
    import { derived, writable } from "svelte/store";
    import { EventType, watch, type Event } from "$lib/watch";
    import { TextDocument } from "$lib/crdt";
    import { NoSuchFileOrDirectoryError, readFile } from "$lib/tauri";
    import { Timeline } from "$lib/components";
    import { git } from "$lib";

    const selectedPath = writable<string | string[] | null>(null);

    const onSelectProjectClick = () =>
        open({
            directory: true,
            recursive: true,
        }).then(selectedPath.set);

    const docs = writable<Record<string, TextDocument>>({});

    const deleteDocs = (...filepaths: string[]) => {
        $docs = Object.fromEntries(
            Object.entries($docs).filter(
                ([filepath, _]) => !filepaths.includes(filepath)
            )
        );
    };

    const upsertDoc = (filepath: string) =>
        readFile(filepath)
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

    const onEvent = (event: Event) => {
        const isFileCreate =
            EventType.isCreate(event.type) && event.type.create.kind === "file";
        const isFileUpdate =
            EventType.isModify(event.type) && event.type.modify.kind === "data";
        const isFileRemove = EventType.isRemove(event.type);

        if (isFileCreate) {
            event.paths.forEach(upsertDoc);
        } else if (isFileUpdate) {
            event.paths.forEach(upsertDoc);
        } else if (isFileRemove) {
            deleteDocs(...event.paths);
        }
    };

    selectedPath.subscribe(async (path) => {
        if (path === null) return;
        return await watch(path, onEvent);
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

    git.exec("version").then(console.log);

    let value: number | undefined;
</script>

<form class="flex flex-col">
    <input class="flex-1" type="text" value={$selectedPath} disabled />
    <button class="shadow-md" on:click={onSelectProjectClick} type="button">
        select project
    </button>
</form>

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
