<script lang="ts">
    import { themeIcons } from "seti-icons";
    import type { PageData } from "./$types";
    import { derived } from "svelte/store";
    import { asyncDerived } from "@square/svelte-store";
    import type { Session } from "$lib/sessions";
    import { startOfDay } from "date-fns";
    import { list as listDeltas } from "$lib/deltas";
    import type { Delta } from "$lib/deltas";

    export let data: PageData;
    const { project, sessions } = data;

    const formatDate = (date: Date) => {
        return new Intl.DateTimeFormat("default", {
            weekday: "short",
            day: "numeric",
            month: "short",
        }).format(date);
    };

    const formatTime = (date: Date) => {
        return new Intl.DateTimeFormat("default", {
            hour: "numeric",
            minute: "numeric",
        }).format(date);
    };

    function pathToName(path: string) {
        return path.split("/").slice(-1)[0];
    }

    const getIcon = themeIcons({
        blue: "#268bd2",
        grey: "#657b83",
        "grey-light": "#839496",
        green: "#859900",
        orange: "#cb4b16",
        pink: "#d33682",
        purple: "#6c71c4",
        red: "#dc322f",
        white: "#fdf6e3",
        yellow: "#b58900",
        ignore: "#586e75",
    });

    function pathToIconSvg(path: string) {
        let name: string = pathToName(path);
        let { svg } = getIcon(name);
        return svg;
    }

    type UISession = { session: Session; deltas: Record<string, Delta[]> };

    $: dateSessions = asyncDerived([sessions], async ([sessions]) => {
        const deltas = await Promise.all(
            sessions.map((session) => {
                return listDeltas({
                    projectId: $project?.id,
                    sessionId: session.id,
                });
            })
        );

        const uiSessions = sessions
            .map((session, i) => {
                return { session, deltas: deltas[i] } as UISession;
            })
            .filter((uiSession) => {
                return Object.keys(uiSession.deltas).length > 0;
            });

        const dateSessions: Record<number, UISession[]> = {};
        uiSessions.forEach((uiSession) => {
            const date = startOfDay(
                new Date(uiSession.session.meta.startTimestampMs)
            );
            if (dateSessions[date.getTime()]) {
                dateSessions[date.getTime()]?.push(uiSession);
            } else {
                dateSessions[date.getTime()] = [uiSession];
            }
        });

        return dateSessions;
    });

    type Selection = { sessionIdx: number; dateMilliseconds: number };
    let selection = {} as Selection;

    const resetSelection = () => {
        selection = {} as Selection;
    };

    function scrollExpandedIntoView() {
        new Promise((r) => setTimeout(r, 100)).then(() => {
            document.getElementById("expanded").scrollIntoView({
                behavior: "smooth",
            });
        });
    }
</script>

<div class="h-full">
    <div class="overflow-x-hidden w-full h-full">
        <div class="h-full">
            {#if $dateSessions === undefined}
                <span>Loading...</span>
            {:else}
                <div
                    class="h-full flex-auto flex flex-row overflow-x-auto space-x-12 px-4 py-4"
                >
                    {#each Object.entries($dateSessions) as [dateMilliseconds, uiSessions]}
                        {#if selection.dateMilliseconds == +dateMilliseconds}
                            <!-- Day expanded -->
                            <div
                                class="bg-zinc-600 py-1 min-w-full overflow-x-hidden"
                            >
                                <div>
                                    {formatDate(new Date(+dateMilliseconds))}
                                </div>
                                <div class="flex space-x-2 " id="expanded">
                                    {#each uiSessions as uiSession, i}
                                        <!-- Session (overview) -->
                                        <!-- Only show nearest neighbors -->
                                        {#if Math.abs(i - selection.sessionIdx) < 2}
                                            {#if i === selection.sessionIdx}
                                                <!-- content here -->

                                                <div
                                                    class="bg-zinc-700 {i ==
                                                    selection.sessionIdx
                                                        ? 'flex-grow'
                                                        : 'w-4'}"
                                                >
                                                    <div
                                                        class="text-sm rounded borded bg-orange-500 text-zinc-200"
                                                    >
                                                        {formatTime(
                                                            new Date(
                                                                uiSession.session.meta.startTimestampMs
                                                            )
                                                        )}
                                                        -
                                                        {formatTime(
                                                            new Date(
                                                                uiSession.session.meta.lastTimestampMs
                                                            )
                                                        )}
                                                    </div>
                                                    <button
                                                        on:click={resetSelection}
                                                        >close</button
                                                    >
                                                </div>
                                            {:else}
                                                <div
                                                    class="bg-zinc-700 {i ==
                                                    selection.sessionIdx
                                                        ? 'flex-grow'
                                                        : 'w-4'}"
                                                >
                                                    <div
                                                        class="text-sm rounded borded bg-orange-500 text-zinc-200"
                                                    >
                                                        {formatTime(
                                                            new Date(
                                                                uiSession.session.meta.startTimestampMs
                                                            )
                                                        )}
                                                        -
                                                        {formatTime(
                                                            new Date(
                                                                uiSession.session.meta.lastTimestampMs
                                                            )
                                                        )}
                                                    </div>
                                                    <div
                                                        title="Session duration"
                                                    >
                                                        {Math.round(
                                                            (uiSession.session
                                                                .meta
                                                                .lastTimestampMs -
                                                                uiSession
                                                                    .session
                                                                    .meta
                                                                    .startTimestampMs) /
                                                                1000 /
                                                                60
                                                        )} min
                                                    </div>
                                                    <div title="Session files">
                                                        {#each Object.keys(uiSession.deltas) as filePath}
                                                            <div
                                                                class="flex flex-row w-32 items-center"
                                                            >
                                                                <div
                                                                    class="w-6 h-6 text-white fill-blue-400"
                                                                >
                                                                    {@html pathToIconSvg(
                                                                        filePath
                                                                    )}
                                                                </div>
                                                                <div
                                                                    class="text-white w-24 truncate"
                                                                >
                                                                    {pathToName(
                                                                        filePath
                                                                    )}
                                                                </div>
                                                            </div>
                                                        {/each}
                                                    </div>
                                                </div>
                                            {/if}
                                        {/if}
                                    {/each}
                                </div>
                            </div>
                        {:else}
                            <!-- Day -->
                            <div class="bg-zinc-600 py-1 flex flex-col">
                                <div>
                                    {formatDate(new Date(+dateMilliseconds))}
                                </div>
                                <div class="h-2/3 flex space-x-2">
                                    {#each uiSessions as uiSession, i}
                                        <!-- Session (overview) -->
                                        <div
                                            class="bg-zinc-700 overflow-y-auto"
                                        >
                                            <!-- svelte-ignore a11y-click-events-have-key-events -->
                                            <div
                                                class="text-sm rounded borded bg-orange-500 text-zinc-200"
                                                on:click={() => {
                                                    selection = {
                                                        sessionIdx: i,
                                                        dateMilliseconds:
                                                            +dateMilliseconds,
                                                    };
                                                    scrollExpandedIntoView();
                                                }}
                                            >
                                                {formatTime(
                                                    new Date(
                                                        uiSession.session.meta.startTimestampMs
                                                    )
                                                )}
                                                -
                                                {formatTime(
                                                    new Date(
                                                        uiSession.session.meta.lastTimestampMs
                                                    )
                                                )}
                                            </div>
                                            <div title="Session duration">
                                                {Math.round(
                                                    (uiSession.session.meta
                                                        .lastTimestampMs -
                                                        uiSession.session.meta
                                                            .startTimestampMs) /
                                                        1000 /
                                                        60
                                                )} min
                                            </div>
                                            <div class="" title="Session files">
                                                {#each Object.keys(uiSession.deltas) as filePath}
                                                    <div
                                                        class="flex flex-row w-32 items-center"
                                                    >
                                                        <div
                                                            class="w-6 h-6 text-white fill-blue-400"
                                                        >
                                                            {@html pathToIconSvg(
                                                                filePath
                                                            )}
                                                        </div>
                                                        <div
                                                            class="text-white w-24 truncate"
                                                        >
                                                            {pathToName(
                                                                filePath
                                                            )}
                                                        </div>
                                                    </div>
                                                {/each}
                                            </div>
                                        </div>
                                    {/each}
                                </div>
                                <div class="flex-grow border border-green-700">
                                    Day summary
                                </div>
                            </div>
                        {/if}
                    {/each}
                </div>
            {/if}
        </div>
    </div>
</div>
