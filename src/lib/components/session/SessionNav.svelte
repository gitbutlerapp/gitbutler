<script lang="ts">
    import SessionNavBlock from "$lib/components/session/SessionNavBlock.svelte";
    import FaAngleLeft from "svelte-icons/fa/FaAngleLeft.svelte";
    import FaAngleRight from "svelte-icons/fa/FaAngleRight.svelte";
    import type { Project } from "$lib/projects";
    import type { Session } from "$lib/sessions";

    export let project: Project | undefined;
    export let previousSesssion: Session | undefined;``
    export let nextSession: Session | undefined;
    export let session: Session | undefined;
</script>

<div id="session-nav" class="grid grid-cols-3 gap-6 text-sm my-2">
    <div class="flex items-center justify-center">
        {#if previousSesssion}
            <a
                class="w-full"
                href="/projects/{project?.id}/sessions/{previousSesssion?.hash}"
            >
                <SessionNavBlock hover={true} session={previousSesssion} />
            </a>
        {/if}
    </div>
    <div class="flex items-center justify-center w-full space-x-1">
        <a
            href="/projects/{project?.id}/sessions/{previousSesssion?.hash}"
            class="text-zinc-500 hover:text-zinc-300 w-8 h-8 {previousSesssion
                ? ''
                : 'invisible'}"
        >
            <FaAngleLeft />
        </a>
        <div class="w-full">
            <SessionNavBlock
                session={session}
                extraClasses="p-4 border-orange-300"
            />
        </div>
        <a
            href="/projects/{project?.id}/sessions/{nextSession?.hash}"
            class="w-8 h-8 text-zinc-500 hover:text-zinc-300 {nextSession
                ? 'visible'
                : 'invisible'}"
        >
            <FaAngleRight />
        </a>
    </div>
    <div class="flex items-center justify-center">
        {#if nextSession}
            <a
                class="w-full"
                href="/projects/{project?.id}/sessions/{nextSession?.hash}"
            >
                <SessionNavBlock hover={true} session={nextSession} />
            </a>
        {/if}
    </div>
</div>
