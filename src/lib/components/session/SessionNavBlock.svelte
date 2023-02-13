<script lang="ts">
    import type { Session } from "$lib/sessions";
    import sessions from "$lib/sessions";
    import { toHumanReadableTime } from "$lib/time";
    export let session: Session | undefined;
    export let hover: boolean = false;
    export let extraClasses: string = "";
</script>

<div>
    {#if session}
        <div
            class="cursor-default select-none flex flex-col 
            border rounded-md px-2 py-1 overflow-hidden bg-zinc-700 border-zinc-600 
            {hover ? 'hover:border-zinc-400 cursor-auto' : ''} {extraClasses}"
        >
            <div class="font-bold text-zinc-300">
                {session.meta.branch}
            </div>
            <div class="mt-1 text-xs">
                <span>
                    {#if session.meta.startTs}
                        {toHumanReadableTime(session.meta.startTs)}
                    {/if}
                </span>
                <span>—</span>
                <span>
                    {#if session.meta.lastTs}
                        {toHumanReadableTime(session.meta.lastTs)}
                    {/if}
                </span>
            </div>
        </div>
    {/if}
</div>
