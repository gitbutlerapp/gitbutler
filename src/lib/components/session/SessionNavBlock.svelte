<script lang="ts">
    import type { Session } from "$lib/sessions";
    import sessions from "$lib/sessions";
    import { toHumanReadableTime } from "$lib/time";
    export let session: Session | undefined;
</script>

<div>
    {#if session}
        <div
            class="cursor-default select-none flex flex-col border rounded-md p-2 overflow-hidden bg-zinc-700 border-zinc-600"
        >
            <div>
                {session.meta.branch}
            </div>
            <div>
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
