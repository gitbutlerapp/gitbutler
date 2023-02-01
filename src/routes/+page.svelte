<script lang="ts">
    import { open } from "@tauri-apps/api/dialog";
    import { writable } from "svelte/store";
    import { EventType, watch, type Event } from "$lib/watch";
    import { TextDocument } from "$lib/crdt";
    import { NoSuchFileOrDirectoryError, readFile } from "$lib/tauri";
    import { File } from "$lib/components";

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
</script>

<form class="flex flex-col">
    <input class="flex-1" type="text" value={$selectedPath} disabled />
    <button class="shadow-md" on:click={onSelectProjectClick} type="button">
        select project
    </button>
</form>

<ul class="flex flex-col gap-2">
    {#each Object.entries($docs) as [filepath, doc]}
        <li>
            <details>
                <summary>{filepath}</summary>
                <ul>
                    <File {doc} />
                </ul>
            </details>
        </li>
    {/each}
</ul>
