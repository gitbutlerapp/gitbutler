<script lang="ts">
    import type { LayoutData, LayoutLoad } from "./$types";
    export let data: LayoutData;

    $: project = data.project;
</script>

<div class="">
    <div class="p-3 flex flex-row space-x-3 text-zinc-500 text-lg select-none">
        <div>Week</div>
        <a href="/projects/{$project?.id}" class="hover:text-zinc-300">Day</a>
        <div title="go to current session">Session</div>
    </div>

    <slot />
</div>
