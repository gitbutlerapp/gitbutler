<script lang="ts">
    import { open } from "@tauri-apps/api/dialog";
    import { writable } from "svelte/store";
    import { watchImmediate, type RawEvent } from "tauri-plugin-fs-watch-api";

    const selected = writable<string | string[] | null>(null);

    const onSelectProjectClick = () =>
        open({
            directory: true,
            recursive: true,
        }).then(selected.set);

    const events = writable<RawEvent[]>([]);

    const onEvent = (event: RawEvent) =>
        events.update((events) => [...events, event]);

    selected.subscribe(async (value) => {
        if (value === null) return;
        return await watchImmediate(value, { recursive: true }, onEvent);
    });
</script>

<form class="flex flex-col">
    <input class="flex-1" type="text" value={$selected} disabled />
    <button class="shadow-md" on:click={onSelectProjectClick} type="button">
        select project
    </button>
</form>

<ul class="flex flex-col gap-2">
    {#each $events as event}
        <li>{JSON.stringify(event)}</li>
    {/each}
</ul>
