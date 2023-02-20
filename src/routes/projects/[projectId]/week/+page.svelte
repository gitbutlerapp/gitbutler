<script lang="ts">
    import { Week } from "$lib/week";
    import type { PageData } from "./$types";
    import { WeekBlockEntry } from "$lib/components/week";
    import MdKeyboardArrowLeft from "svelte-icons/md/MdKeyboardArrowLeft.svelte";
    import MdKeyboardArrowRight from "svelte-icons/md/MdKeyboardArrowRight.svelte";
    import { derived } from "svelte/store";

    export let data: PageData;
    const { project, sessions } = data;

    let week = Week.from(new Date());

    $: canNavigateForwad = week.end.getTime() < new Date().getTime();
    const formatDate = (date: Date) => {
        return new Intl.DateTimeFormat("default", {
            weekday: "short",
            day: "numeric",
            month: "short",
        }).format(date);
    };

    $: sessionsInWeek = derived([sessions], ([sessions]) => {
        return sessions.filter((session) => {
            return (
                week.start <= new Date(session.meta.startTimestampMs * 1000) &&
                new Date(session.meta.startTimestampMs * 1000) <= week.end
            );
        });
    });
</script>
<div class="flex flex-col h-full select-none text-zinc-400">
    <header
        class="flex items-center justify-between flex-none px-6 py-4 border-b border-zinc-700"
    >
        <div class="flex items-center justify-start  w-72">
            <button
                class="w-8 h-8 hover:text-zinc-200"
                on:click={() => (week = Week.previous(week))}
            >
                <MdKeyboardArrowLeft />
            </button>
            <div class="flex-grow w-4/5 text-center">
                {formatDate(Week.nThDay(week, 0))}
                &mdash;
                {formatDate(Week.nThDay(week, 6))}
            </div>
            <button
                class="w-8 h-8 hover:text-zinc-200 disabled:text-zinc-600"
                disabled={!canNavigateForwad}
                on:click={() => {
                    if (canNavigateForwad) {
                        week = Week.next(week);
                    }
                }}
            >
                <MdKeyboardArrowRight />
            </button>
        </div>
    </header>
    <div class="isolate flex flex-col flex-auto overflow-auto">
        <div class="flex flex-col flex-none max-w-full">
            <!-- sticky top -->
            <div
                class="overflow-hidden sticky top-0 z-30 bg-zinc-800 flex-none shadow shadow-zinc-700 ring-1 ring-zinc-700 ring-opacity-5 pr-8"
            >
                <div
                    class="grid-cols-7 -mr-px text-sm leading-6 border-r border-zinc-700 divide-x divide-zinc-700 grid"
                >
                    <div class="col-end-1 w-14" />
                    <div class="flex items-center justify-center py-3">
                        <span
                            >Mon <span
                                class="items-center justify-center font-semibold"
                                >{Week.nThDay(week, 0).getDate()}</span
                            ></span
                        >
                    </div>
                    <div class="flex items-center justify-center py-3">
                        <span
                            >Tue <span
                                class="items-center justify-center font-semibold"
                                >{Week.nThDay(week, 1).getDate()}</span
                            ></span
                        >
                    </div>
                    <div class="flex items-center justify-center py-3">
                        <span
                            >Wed <span
                                class="items-center justify-center font-semibold"
                                >{Week.nThDay(week, 2).getDate()}</span
                            ></span
                        >
                    </div>
                    <div class="flex items-center justify-center py-3">
                        <span
                            >Thu <span
                                class="items-center justify-center font-semibold"
                                >{Week.nThDay(week, 3).getDate()}</span
                            ></span
                        >
                    </div>
                    <div class="flex items-center justify-center py-3">
                        <span
                            >Fri <span
                                class="items-center justify-center font-semibold"
                                >{Week.nThDay(week, 4).getDate()}</span
                            ></span
                        >
                    </div>
                    <div class="flex items-center justify-center py-3">
                        <span
                            >Sat <span
                                class="items-center justify-center font-semibold"
                                >{Week.nThDay(week, 5).getDate()}</span
                            ></span
                        >
                    </div>
                    <div class="flex items-center justify-center py-3">
                        <span
                            >Sun <span
                                class="items-center justify-center font-semibold"
                                >{Week.nThDay(week, 6).getDate()}</span
                            ></span
                        >
                    </div>
                </div>
            </div>
            <div class="flex flex-auto">
                <div
                    class="sticky left-0 z-10 w-14 flex-none ring-1 ring-zinc-700"
                />
                <div class="grid flex-auto grid-cols-1 grid-rows-1">
                    <!-- hours y lines-->
                    <div
                        class="col-start-1 col-end-2 row-start-1 grid divide-y divide-zinc-700/20"
                        style="grid-template-rows: repeat(24, minmax(1.5rem, 1fr));"
                    >
                        <div class="row-end-1 h-7" />
                        <div>
                            <div
                                class="sticky left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right text-xs leading-5 text-zinc-500"
                            >
                                12AM
                            </div>
                        </div>
                        <div />
                        <div>
                            <div
                                class="sticky left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right text-xs leading-5 text-zinc-500"
                            >
                                2AM
                            </div>
                        </div>
                        <div />
                        <div>
                            <div
                                class="sticky left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right text-xs leading-5 text-zinc-500"
                            >
                                4AM
                            </div>
                        </div>
                        <div />
                        <div>
                            <div
                                class="sticky left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right text-xs leading-5 text-zinc-500"
                            >
                                6AM
                            </div>
                        </div>
                        <div />
                        <div>
                            <div
                                class="sticky left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right text-xs leading-5 text-zinc-500"
                            >
                                8AM
                            </div>
                        </div>
                        <div />
                        <div>
                            <div
                                class="sticky left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right text-xs leading-5 text-zinc-500"
                            >
                                10AM
                            </div>
                        </div>
                        <div />
                        <div>
                            <div
                                class="sticky left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right text-xs leading-5 text-zinc-500"
                            >
                                12PM
                            </div>
                        </div>
                        <div />
                        <div>
                            <div
                                class="sticky left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right text-xs leading-5 text-zinc-500"
                            >
                                2PM
                            </div>
                        </div>
                        <div />
                        <div>
                            <div
                                class="sticky left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right text-xs leading-5 text-zinc-500"
                            >
                                4PM
                            </div>
                        </div>
                        <div />
                        <div>
                            <div
                                class="sticky left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right text-xs leading-5 text-zinc-500"
                            >
                                6PM
                            </div>
                        </div>
                        <div />
                        <div>
                            <div
                                class="sticky left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right text-xs leading-5 text-zinc-500"
                            >
                                8PM
                            </div>
                        </div>
                        <div />
                        <div>
                            <div
                                class="sticky left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right text-xs leading-5 text-zinc-500"
                            >
                                10PM
                            </div>
                        </div>
                        <div />
                    </div>

                    <!-- day x lines -->
                    <div
                        class="col-start-1 col-end-2 row-start-1 grid-rows-1 divide-x divide-zinc-700/50 grid grid-cols-7"
                    >
                        <div class="col-start-1 row-span-full" />
                        <div class="col-start-2 row-span-full" />
                        <div class="col-start-3 row-span-full" />
                        <div class="col-start-4 row-span-full" />
                        <div class="col-start-5 row-span-full" />
                        <div class="col-start-6 row-span-full" />
                        <div class="col-start-7 row-span-full" />
                        <div class="col-start-8 row-span-full w-8" />
                    </div>

                    <!-- actual entries -->
                    <ol
                        class="col-start-1 col-end-2 row-start-1 grid grid-cols-7 pr-8"
                        style="grid-template-rows: 1.75rem repeat(96, minmax(0px, 1fr)) auto;"
                    >
                      {#each $sessionsInWeek as session}
                        <WeekBlockEntry
                            startTime={new Date(session.meta.startTimestampMs * 1000)}
                            endTime={new Date(session.meta.startTimestampMs * 1000)}
                            label={session.meta.branch}
                            href="/projects/{$project?.id}/sessions/{session.id}/"
                        />
                      {/each}
                    </ol>
                </div>
            </div>
        </div>
    </div>
</div>
