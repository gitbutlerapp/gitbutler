<script lang="ts">
    import MdKeyboardArrowLeft from "svelte-icons/md/MdKeyboardArrowLeft.svelte";
    import MdKeyboardArrowRight from "svelte-icons/md/MdKeyboardArrowRight.svelte";
    import type { PageData } from "./$types";
    import { add, format, differenceInSeconds } from "date-fns";
    import { page } from "$app/stores";
    import { fi } from "date-fns/esm/locale";

    export let data: PageData;
    $: session = data.session;
    $: previousSesssion = data.previousSesssion;
    $: nextSession = data.nextSession;
    $: deltas = data.deltas;

    $: start = new Date($session.meta.startTimestampMs);
    $: end = new Date($session.meta.lastTimestampMs);
    $: midpoint = add(start, {
        seconds: differenceInSeconds(end, start) * 0.5,
    });
    $: quarter = add(start, {
        seconds: differenceInSeconds(end, start) * 0.25,
    });
    $: threequarters = add(start, {
        seconds: differenceInSeconds(end, start) * 0.75,
    });
    const timeStampToCol = (deltaTimestamp: Date) => {
        if (deltaTimestamp < start || deltaTimestamp > end) {
            console.error(
                `Delta timestamp out of session range. Delta timestamp: ${deltaTimestamp}, Session start: ${start}, Session end: ${end}`
            );
        }
        // there are 88 columns
        // start is column 17
        const totalDiff = differenceInSeconds(end, start);
        const eventDiff = differenceInSeconds(deltaTimestamp, start);
        const rat = eventDiff / totalDiff;
        const col = Math.floor(rat * 63 + 17);
        return col;
    };

</script>
<div class="flex flex-col h-full  text-zinc-400">
    <header
        class="flex items-center justify-between flex-none px-6 py-4 border-b border-zinc-700"
    >
        <div class="flex items-center justify-start  w-72">
            <a
                href="/projects/{$page.params
                    .projectId}/sessionv2/{$previousSesssion?.id}"
                class="w-8 h-8 hover:text-zinc-200 {$previousSesssion
                    ? ''
                    : 'opacity-50 pointer-events-none cursor-not-allowed'}"
            >
                <MdKeyboardArrowLeft />
            </a>
            <a
                href="/projects/{$page.params
                    .projectId}/sessionv2/{$nextSession?.id}"
                class="w-8 h-8 hover:text-zinc-200 {$nextSession
                    ? ''
                    : 'opacity-50 pointer-events-none cursor-not-allowed'}"
            >
                <MdKeyboardArrowRight />
            </a>
        </div>
    </header>

    <!-- main part -->
    <div class="flex flex-col flex-none max-w-full">
        <div class="flex flex-col flex-none max-w-full">
            <!-- sticky header -->
            <div
                class="overflow-hidden sticky top-0 z-30 bg-zinc-800 flex-none shadow shadow-zinc-700 ring-1 ring-zinc-700 ring-opacity-5"
            >
                <div
                    class="grid-cols-11 -mr-px text-sm leading-6  border-zinc-700  grid"
                >
                    <div />
                    <div
                        class="col-span-2 flex items-center justify-center py-3"
                    >
                        <span>{format(start, "hh:mm")}</span>
                    </div>
                    <div
                        class="col-span-2 flex items-center justify-center py-3"
                    >
                        <span>{format(quarter, "hh:mm")}</span>
                    </div>
                    <div
                        class="col-span-2 flex items-center justify-center py-3"
                    >
                        <span>{format(midpoint, "hh:mm")}</span>
                    </div>
                    <div
                        class="col-span-2 flex items-center justify-center py-3"
                    >
                        <span>{format(threequarters, "hh:mm")}</span>
                    </div>
                    <div
                        class="col-span-2 flex items-center justify-center py-3"
                    >
                        <span>{format(end, "hh:mm")}</span>
                    </div>
                </div>
            </div>
            <div class="flex flex-auto">
                <!-- <div
                    class="sticky left-0 z-10 w-14 flex-none ring-1 ring-zinc-700"
                /> -->

                <div class="grid flex-auto grid-cols-1 grid-rows-1">
                    <!-- file names list -->
                    <div
                        class="col-start-1 col-end-2 row-start-1 grid divide-y divide-zinc-700/20"
                        style="grid-template-rows: repeat({Object.keys($deltas)
                            .length}, minmax(2rem, 1fr));"
                    >
                        <!-- <div class="row-end-1 h-7" /> -->

                        {#each Object.keys($deltas) as filePath}
                            <div
                                class="flex justify-end items-center  overflow-hidden sticky left-0 z-20   w-1/6 pr-4 text-xs leading-5 text-zinc-300"
                                title={filePath}
                            >
                                {filePath.split("/").pop()}
                            </div>
                        {/each}
                    </div>

                    <!-- time vertical lines -->
                    <div
                        class="col-start-1 col-end-2 row-start-1 grid-rows-1 divide-x divide-zinc-700/50 grid grid-cols-11"
                    >
                        <div class="col-span-2 row-span-full" />
                        <div class="col-span-2 row-span-full" />
                        <div class="col-span-2 row-span-full" />
                        <div class="col-span-2 row-span-full" />
                        <div class="col-span-2 row-span-full" />
                        <div class="col-span-2 row-span-full" />
                    </div>

                    <!-- actual entries  -->
                    <ol
                        class="col-start-1 col-end-2 row-start-1 grid"
                        style="
                        grid-template-columns: repeat(88, minmax(0, 1fr));
                        grid-template-rows: 2rem repeat({Object.keys($deltas)
                            .length}, minmax(0px, 1fr)) auto;"
                    >
                        {#each Object.entries($deltas) as [filePath, fileDeltas], idx}
                            {#each fileDeltas as delta}
                                <li
                                    class="relative mt-px flex items-center"
                                    style="
                                grid-row: {idx + 1} / span 1;
                                grid-column: {timeStampToCol(
                                        new Date(delta.timestampMs)
                                    )} / span 2;"
                                >
                                    <a
                                        href="/"
                                        class="group absolute inset-1 flex flex-col items-center justify-center rounded bg-zinc-300 p-px text-xs leading-5 hover:bg-zinc-200 shadow"
                                    >
                                        <p
                                            class="order-1 font-semibold text-zinc-800"
                                        >
                                            <!-- foo -->
                                        </p>
                                    </a>
                                </li>
                            {/each}
                        {/each}
                    </ol>
                </div>
            </div>
        </div>
    </div>
</div>
