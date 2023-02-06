<script lang="ts">
    import type { Session } from "$lib/session";
    export let session: Session;

    const toHumanReadableTime = (timestamp: number) => {
        return new Date(session.startTime).toLocaleTimeString("en-US", {
            hour: "numeric",
            minute: "numeric",
        });
    };

    const colorFromBranchName = (branchName: string) => {
        const colors = [
            "bg-red-500",
            "bg-green-500",
            "bg-blue-500",
            "bg-yellow-500",
            "bg-purple-500",
            "bg-pink-500",
            "bg-indigo-500",
            "bg-gray-500",
        ];
        const hash = branchName.split("").reduce((acc, char) => {
            return acc + char.charCodeAt(0);
        }, 0);
        return colors[hash % colors.length];
    };
</script>

<div>
    <div id="block" class="text-slate-50 rounded {colorFromBranchName(session.branchName)}">
        {session.branchName}
    </div>
    <div id="activities">TODO: activities</div>
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
