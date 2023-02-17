<script lang="ts">
    import { Login } from "$lib/components";
    import type { PageData } from "./$types";

    export let data: PageData;
    const { user, api } = data;
</script>

<div>
    {#if $user}
        <div>Welcome, {$user.name}!</div>
    {/if}
    <Login {user} {api} />
</div>
