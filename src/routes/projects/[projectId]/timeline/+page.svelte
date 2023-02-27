<script lang="ts">
    import { themeIcons } from "seti-icons";
    import type { PageData } from "./$types";
    import { derived } from "svelte/store";
    import { asyncDerived } from "@square/svelte-store";
    import type { Session } from "$lib/sessions";
    import { startOfDay } from "date-fns";
    import { list as listDeltas } from "$lib/deltas";
    import type { Delta } from "$lib/deltas";
    import { toHumanBranchName } from "$lib/branch";
    import { fly, fade } from "svelte/transition";
    import { quintOut } from "svelte/easing";
    import { crossfade } from "svelte/transition";

	export let data: PageData;
	const { project, sessions } = data;

	const formatDate = (date: Date) => {
		return new Intl.DateTimeFormat('default', {
			weekday: 'short',
			day: 'numeric',
			month: 'short'
		}).format(date);
	};

	const formatTime = (date: Date) => {
		return new Intl.DateTimeFormat('default', {
			hour: 'numeric',
			minute: 'numeric'
		}).format(date);
	};

	function pathToName(path: string) {
		return path.split('/').slice(-1)[0];
	}

	const getIcon = themeIcons({
		blue: '#268bd2',
		grey: '#657b83',
		'grey-light': '#839496',
		green: '#859900',
		orange: '#cb4b16',
		pink: '#d33682',
		purple: '#6c71c4',
		red: '#dc322f',
		white: '#fdf6e3',
		yellow: '#b58900',
		ignore: '#586e75'
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
					sessionId: session.id
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
			const date = startOfDay(new Date(uiSession.session.meta.startTimestampMs));
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

	function scrollExpandedIntoView(dateMilliseconds: string) {
		new Promise((r) => setTimeout(r, 10)).then(() => {
			document.getElementById(dateMilliseconds).scrollIntoView({
				// behavior: "smooth",
				block: 'center',
				inline: 'center'
			});
		});
	}

	let animatingOut = false;
</script>

<div class="h-full">
    <div class="overflow-x-hidden w-full h-full">
        <div class="h-full">
            {#if $dateSessions === undefined}
                <span>Loading...</span>
            {:else}
                <div
                    class="h-full flex-auto flex flex-row overflow-x-auto space-x-12 px-4 py-4 pb-6"
                >
                    {#each Object.entries($dateSessions) as [dateMilliseconds, uiSessions]}
                        <!-- Day -->
                        <div
                            id={dateMilliseconds}
                            class="bg-zinc-800/50 rounded-xl border border-zinc-700 
                                {selection.dateMilliseconds == +dateMilliseconds
								? 'min-w-full overflow-x-hidden'
								: ''}
                                "
                        >
                            <div
                                class="font-medium border-b border-zinc-700 bg-zinc-700/30 h-6 flex items-center pl-4"
                            >
                                <span
                                    class={animatingOut
                                        ? "animate-pulse text-orange-300"
                                        : ""}
                                >
                                    {formatDate(new Date(+dateMilliseconds))}
                                </span>
                            </div>
                            {#if selection.dateMilliseconds !== +dateMilliseconds}
                                <div class="h-full flex flex-col">
                                    <div class="h-2/3 flex space-x-2 px-4">
                                        {#each uiSessions as uiSession, i}
                                            <!-- Session (overview) -->

                                            <div
                                                out:fly={{
                                                    x: i <= 3 ? -800 : 800,
                                                    duration: 600,
                                                }}
                                                on:outrostart={() =>
                                                    (animatingOut = true)}
                                                on:outroend={() =>
                                                    (animatingOut = false)}
                                                class="flex flex-col py-2 w-40"
                                            >
                                                <!-- svelte-ignore a11y-click-events-have-key-events -->
                                                <div
                                                    class="
                                                cursor-pointer
                                                text-sm text-center font-medium rounded borded  text-zinc-800 p-1 border bg-orange-400 border-orange-400 hover:bg-[#fdbc87]"
                                                    on:click={() => {
                                                        selection = {
                                                            sessionIdx: i,
                                                            dateMilliseconds:
                                                                +dateMilliseconds,
                                                        };
                                                        scrollExpandedIntoView(
                                                            dateMilliseconds
                                                        );
                                                    }}
                                                >
                                                    {i}
                                                    {toHumanBranchName(
                                                        uiSession.session.meta
                                                            .branch
                                                    )}
                                                </div>

                                                <div
                                                    class="flex flex-col h-full overflow-y-hidden p-1"
                                                    id="sessions-details"
                                                >
                                                    <div
                                                        class="text-zinc-400 font-medium"
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
                                                        class="text-zinc-500 text-sm"
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
                                                    <div
                                                        class="overflow-y-auto overflow-x-hidden"
                                                        title="Session files"
                                                    >
                                                        {#each Object.keys(uiSession.deltas) as filePath}
                                                            <div
                                                                class="flex flex-row w-32 items-center"
                                                            >
                                                                <div
                                                                    class="w-6 h-6 text-zinc-200 fill-blue-400"
                                                                >
                                                                    {@html pathToIconSvg(
                                                                        filePath
                                                                    )}
                                                                </div>
                                                                <div
                                                                    class="text-zinc-300 w-24 truncate"
                                                                >
                                                                    {pathToName(
                                                                        filePath
                                                                    )}
                                                                </div>
                                                            </div>
                                                        {/each}
                                                    </div>
                                                </div>
                                            </div>
                                        {/each}
                                    </div>
                                    <div
                                        out:fly={{
                                            y: 350,
                                            duration: 600,
                                        }}
                                        class="h-1/3 flex-grow  px-4 border-t border-zinc-700 "
                                    >
                                        Day summary
                                    </div>
                                </div>
                            {:else}
                                <div
                                    in:fade={{
                                        duration: 100,
                                    }}
                                    class="bg-zinc-600 border h-full w-full"
                                >
                                    <button on:click={resetSelection}
                                        >Close</button
                                    >
                                </div>
                            {/if}
                        </div>
                    {/each}
                </div>
            {/if}
        </div>
    </div>
</div>
