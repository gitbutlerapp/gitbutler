<script lang="ts">
    import {open} from '@tauri-apps/api/shell'
    import Authentication from "$lib/authentication";
    import type { PageData } from "./$types";
    
    export let data: PageData;
    const { user } = data;

    const authApi = Authentication();

    const pollForUser = async (token: string) => {
        const apiUser = await authApi.login.user.get(token).catch(() => null);
        if (apiUser) {
            user.set(apiUser);
            return apiUser;
        }
        return new Promise((resolve) => {
            setTimeout(async () => {
                resolve(await pollForUser(token));
            }, 1000);
        });
    }
</script>

{#if $user}
    <div>
        Welcome, {$user.name}!
    </div>
{:else}
    {#await authApi.login.token.create()}
        <div>loading...</div>
    {:then {url, token}}
        {#await Promise.all([open(url),pollForUser(token)])}
            <div>Log in in your system browser</div>
            <p>If you are not redirected automatically, you can 
                <button on:click={() => open(url)}>Try again</button>
            </p>
        {/await}
    {/await}
{/if}

