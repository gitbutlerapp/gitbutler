<script lang="ts">
    import type { PageData } from "./$types";
    import type { Session } from "$lib/sessions";
    import { TimelineDaySession } from "$lib/components/timeline";

    export let data: PageData;
    const { project, sessions } = data;

    const sessionDisplayWidth = (session: Session) => {
        let sessionDurationMinutes =
            (session.meta.lastTs - session.meta.startTs) / 60;
        if (sessionDurationMinutes <= 10) {
            return "w-40 min-w-40";
        } else {
            return "w-60 min-w-60";
        }
    };
</script>

<div class="flex flex-col mt-12">
    <h1 class="text-zinc-200 text-2xl flex justify-center">
        Overview of {$project?.title}
    </h1>
    <div class="flex justify-center space-x-2">
        <a class="hover:text-zinc-200" href="/projects/{$project?.id}/week"
            >Week</a
        >
        <a href="/projects/{$project?.id}/day" class="hover:text-zinc-200"
            >Day</a
        >
    </div>
</div>
