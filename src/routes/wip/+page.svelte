<script lang="ts">
    import { TimelineDay } from "$lib/components/timeline";
    import type { Session } from "$lib/session";
    import { dummySessions } from "$lib/session";
    
    export let sessions: Session[] = dummySessions;
</script>
<div class="p-3">
    <div class="flex flex-row space-x-3 text-slate-400 text-lg">
        <div>Week</div>
        <div class="text-slate-700">Day</div>
        <div>Session</div>
    </div>
    
    <div>
        <TimelineDay sessions={sessions}></TimelineDay>
    </div>
</div>
