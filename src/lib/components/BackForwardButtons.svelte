<script>
    import FaArrowLeft from "svelte-icons/fa/FaArrowLeft.svelte";
    import FaArrowRight from "svelte-icons/fa/FaArrowRight.svelte";
    let history = window.history;
</script>

<div class="flex items-center justify-center space-x-3">
    <button
        class="w-3 h-3 text-slate-400 hover:text-slate-500"
        title="Go back"
        on:click={() => history.back()}><FaArrowLeft /></button
    >
    <button
        class="w-3 h-3 text-slate-400 hover:text-slate-500"
        title="Go forward"
        on:click={() => history.forward()}><FaArrowRight /></button
    >
</div>
