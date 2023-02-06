<script lang="ts">
    import type { Session } from "$lib/session";
    import TimelineDaySessionActivities from "./TimelineDaySessionActivities.svelte";
    export let session: Session;

    const toHumanReadableTime = (timestamp: number) => {
        return new Date(timestamp).toLocaleTimeString("en-US", {
            hour: "numeric",
            minute: "numeric",
        });
    };

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
    <div
        id="block"
        class="border px-4 py-2 text-slate-50 rounded-lg {colorFromBranchName(session.branchName)}"
    >
        {session.branchName}
    </div>
    <div id="activities">
        <div class="my-2 mx-1">
            <TimelineDaySessionActivities
                activities={session.activities}
                sessionStart={session.startTime}
                sessionEnd={session.endTime}
            />
        </div>
    </div>
    <div id="time-range">
        {toHumanReadableTime(session.startTime)}
        -
        {toHumanReadableTime(session.endTime)}
    </div>
    <div id="files">
        {#each session.files as file}
            <div>
                <span>{file.linesTouched}</span>
                <span>{file.name}</span>
            </div>
        {/each}
    </div>
</div>
