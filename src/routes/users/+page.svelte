<script lang="ts">
    import Authentication from "$lib/authentication";
    import type { PageData } from "./$types";
    
    export let data: PageData;
    const { user } = data;

    const authApi = Authentication();
</script>

{#if $user}
    {JSON.stringify($user)}
{:else}
    {#await authApi.login.token.create()}
        <div>loading...</div>
    {:then token}
        <div>token: {JSON.stringify(token)}</div>
    {/await}
{/if}

