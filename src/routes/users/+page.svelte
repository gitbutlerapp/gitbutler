<script lang="ts">
    import { Login } from "$lib/components";
    import type { PageData } from "./$types";
    import MdAutorenew from "svelte-icons/md/MdAutorenew.svelte";

    export let data: PageData;
    const { user, api } = data;

    $: editing = false;
    $: saving = false;

    let userName = $user?.name;

    const onEditClicked = () => {
        editing = true;
    };

    const onSaveClicked = async () => {
        if (!$user) {
            return;
        } else {
            saving = true;

            // TODO: do actual api call
            await new Promise((resolve) => setTimeout(resolve, 300));
            if (userName) {
                $user.name = userName;
            }

            saving = false;
            editing = false;
        }
    };
</script>

<div class="p-4 mx-auto">
    <div class="max-w-xl mx-auto p-4">
        {#if $user}
            <div class="flex flex-col text-zinc-100 space-y-6">
                <!-- cloud account -->
                <div class="space-y-2">
                    <div class="text-lg font-medium">
                        GitButler Cloud Account
                    </div>
                    <div
                        class="flex flex-row justify-between border border-zinc-600 rounded-lg p-2 items-center"
                    >
                        <div class="flex flex-row space-x-3">
                            {#if $user.picture}
                                <img
                                    class="h-12 w-12 rounded-full border-2 border-zinc-300"
                                    src={$user.picture}
                                    alt="avatar"
                                />
                            {/if}
                            <div>
                                {#if editing}
                                    <input
                                        bind:value={userName}
                                        type="text"
                                        class="px-1 ring ring-zinc-600 rounded-lg bg-inherit"
                                    />
                                {:else}
                                    <div class="px-1">{$user.name}</div>
                                {/if}
                                <div class="px-1 text-zinc-400">
                                    {$user.email}
                                </div>
                            </div>
                        </div>
                        <div class="flex gap-2">
                            {#if saving}
                                <div
                                    class="flex items-center py-1 px-6 rounded text-white bg-blue-400"
                                >
                                    <div class="animate-spin w-5 h-5">
                                        <MdAutorenew />
                                    </div>
                                </div>
                            {:else if editing}
                                <button
                                    on:click={onSaveClicked}
                                    class="py-1 px-3 rounded text-white bg-blue-400"
                                    >Save</button
                                >
                            {:else}
                                <button
                                    on:click={onEditClicked}
                                    class="py-1 px-3 rounded text-white bg-blue-400"
                                    >Edit profile</button
                                >
                            {/if}
                            <Login {user} {api} />
                        </div>
                    </div>
                </div>
            </div>
        {:else}
            <div
                class="flex flex-col text-white space-y-6 items-center justify-items-center"
            >
                <div class="text-3xl font-bold text-white">
                    Connect to GitButler Cloud
                </div>
                <div>
                    Sign up or log in to GitButler Cloud for more tools and
                    features:
                </div>
                <ul class="text-zinc-400 pb-4 space-y-2">
                    <li class="flex flex-row space-x-3">
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            fill="none"
                            viewBox="0 0 24 24"
                            stroke-width="1.5"
                            stroke="white"
                            class="w-6 h-6"
                        >
                            <path
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                d="M12 16.5V9.75m0 0l3 3m-3-3l-3 3M6.75 19.5a4.5 4.5 0 01-1.41-8.775 5.25 5.25 0 0110.233-2.33 3 3 0 013.758 3.848A3.752 3.752 0 0118 19.5H6.75z"
                            />
                        </svg>
                        <span
                            >Backup everything you do in any of your projects</span
                        >
                    </li>
                    <li class="flex flex-row space-x-3">
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            fill="none"
                            viewBox="0 0 24 24"
                            stroke-width="1.5"
                            stroke="white"
                            class="w-6 h-6"
                        >
                            <path
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                d="M7.5 21L3 16.5m0 0L7.5 12M3 16.5h13.5m0-13.5L21 7.5m0 0L16.5 12M21 7.5H7.5"
                            />
                        </svg>

                        <span>Sync your data across devices</span>
                    </li>
                    <li class="flex flex-row space-x-3">
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            fill="none"
                            viewBox="0 0 24 24"
                            stroke-width="1.5"
                            stroke="white"
                            class="w-6 h-6"
                        >
                            <path
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                d="M16.862 4.487l1.687-1.688a1.875 1.875 0 112.652 2.652L10.582 16.07a4.5 4.5 0 01-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 011.13-1.897l8.932-8.931zm0 0L19.5 7.125M18 14v4.75A2.25 2.25 0 0115.75 21H5.25A2.25 2.25 0 013 18.75V8.25A2.25 2.25 0 015.25 6H10"
                            />
                        </svg>
                        <span>AI commit message automated suggestions</span>
                    </li>
                </ul>
                <div class="mt-8 text-center">
                    <Login {user} {api} />
                </div>
                <div class="text-zinc-300 text-center">
                    You will still need to give us permission for each project
                    before we transfer any data to our servers. You can revoke
                    this permission at any time.
                </div>
            </div>
        {/if}
    </div>
</div>
