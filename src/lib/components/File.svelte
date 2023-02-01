<script lang="ts">
    import type { TextDocument } from "$lib/crdt";
    import { Timeline } from "$lib/components";

    export let doc: TextDocument;

    $: min = Math.min(...doc.getHistory().map((entry) => entry.time));
    $: max = Math.max(...doc.getHistory().map((entry) => entry.time));

    $: initValue = doc.getHistory().at(-1)?.time;

    let value: number | undefined;
    $: display = value ? doc.at(value).toString() : doc.toString();

    $: console.log({ value, min, max, display });
</script>

<figure class="flex flex-col gap-2">
    <figcaption class="m-auto">
        {#if isFinite(min) && isFinite(max) && min !== max && initValue}
            <Timeline bind:min bind:max bind:value {initValue} />
        {/if}
    </figcaption>
    <code>
        {display}
    </code>
</figure>
