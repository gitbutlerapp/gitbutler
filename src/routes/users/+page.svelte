<script lang="ts">
    import {open} from '@tauri-apps/api/shell'
    import Authentication, { type LoginToken } from "$lib/authentication";
    import type { PageData } from "./$types";
    
    export let data: PageData;
    const { user } = data;

    const authApi = Authentication();

    const pollForUser = async (token: LoginToken) => {
        await open (token.url);
        const apiUser = await authApi.login.user.get(token.token).catch(() => null);
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
    {:then token}
        {#await pollForUser(token)}
            <div>Log in in your system browser</div>
            <p>If you are not redirected automatically, you can 
                <button on:click={() => pollForUser(token)}>Try again</button>
            </p>
        {/await}
    {/await}
{/if}

