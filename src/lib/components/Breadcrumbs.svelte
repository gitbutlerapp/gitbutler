<script lang="ts">
    import type { Project } from "$lib/projects";
    import type { Session } from "$lib/sessions";
    import { toHumanReadableTime } from "$lib/time";
    import { getContext } from "svelte";
    import type { Writable } from "svelte/store";
    import IoIosBowtie from "svelte-icons/io/IoIosBowtie.svelte";
    import MdKeyboardArrowRight from "svelte-icons/md/MdKeyboardArrowRight.svelte";

    let project: Writable<Project | null | undefined> = getContext("project");
    let session: Writable<Session | null | undefined> = getContext("session");
</script>

<div
    class="flex flex-row items-center space-x-1 bg-zinc-900 text-zinc-400 h-8"
>
    <a class="hover:text-zinc-200" href="/">
        <div class="w-6 h-6">
            <IoIosBowtie />
        </div>
    </a>
    {#if $project}
        <div class="w-8 h-8 text-zinc-700">
            <MdKeyboardArrowRight />
        </div>
        <a class="hover:text-zinc-200" href="/projects/{$project.id}"
            >{$project.title}</a
        >
    {/if}
    {#if $project && $session}
        <div class="w-8 h-8 text-zinc-700">
            <MdKeyboardArrowRight />
        </div>
        <a
            class="hover:text-zinc-200"
            href="/projects/{$project.id}/sessions/{$session.id}"
        >
            {toHumanReadableTime($session.meta.startTs)}
            {toHumanReadableTime($session.meta.lastTs)}
        </a>
    {/if}
</div>
