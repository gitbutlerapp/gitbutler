<script lang="ts">
    import type { Session } from "$lib/sessions";
    import { toHumanReadableTime } from "$lib/time";
    import TimelineDaySessionActivities from "./TimelineDaySessionActivities.svelte";
    import { list } from "$lib/deltas";
    export let session: Session;
    export let projectId: string;

    const colorFromBranchName = (branchName: string) => {
        const colors = [
            "bg-red-500 border-red-700",
            "bg-green-500 border-green-700",
            "bg-blue-500 border-blue-700",
            "bg-yellow-500 border-yellow-700",
            "bg-purple-500 border-purple-700",
            "bg-pink-500 border-pink-700",
            "bg-indigo-500 border-indigo-700",
            "bg-gray-500 border-gray-700",
        ];
        const hash = branchName.split("").reduce((acc, char) => {
            return acc + char.charCodeAt(0);
        }, 0);
        return colors[hash % colors.length];
    };
</script>

<div class="flex flex-col space-y-2">
    <a
        id="block"
        class="truncate border px-4 py-2 text-slate-50 rounded-lg {colorFromBranchName(
            session.meta.branch
        )}"
        title={session.meta.branch}
        href="/projects/{projectId}/sessions/{session.id}/"
    >
        {session.meta.branch.replace("refs/heads/", "")}
    </a>
    <div id="activities">
        <div class="my-2 mx-1">
            <TimelineDaySessionActivities
                activities={session.activity}
                sessionStart={session.meta.startTs}
                sessionEnd={session.meta.lastTs}
            />
        </div>
    </div>
    <div id="time-range">
        {toHumanReadableTime(session.meta.lastTs)}
        -
        {toHumanReadableTime(session.meta.startTs)}
    </div>
    <div id="files">
        {#await list( { projectId: projectId, sessionId: session.id } ) then deltas}
            {#each Object.keys(deltas) as delta}
                <div>
                    <span>{delta}</span>
                </div>
            {/each}
        {/await}
    </div>
</div>
