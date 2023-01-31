<script lang="ts">
    import { open } from "@tauri-apps/api/dialog";
    import { writable } from "svelte/store";
    import { EventType, watch, type Event } from "$lib/watch";

    const selectedPath = writable<string | string[] | null>(null);

    const onSelectProjectClick = () =>
        open({
            directory: true,
            recursive: true,
        }).then(selectedPath.set);

    const events = writable<Event[]>([]);

    const onEvent = (event: Event) => {
        events.update((events) => [...events, event]);
        if (EventType.isCreate(event.type)) {
            console.log("create");
        } else if (EventType.isModify(event.type)) {
            console.log("modify");
        } else if (EventType.isRemove(event.type)) {
            console.log("remove");
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
    {#each $events as event}
        <li>{JSON.stringify(event)}</li>
    {/each}
</ul>
