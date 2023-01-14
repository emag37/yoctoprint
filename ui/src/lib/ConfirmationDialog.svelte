<script>
    export let message;
    export let resolve_fn;
    export let reject_fn;
</script>

<h2>{message}</h2>
<button on:click={
    () => {
        resolve_fn();
    }
}>Yes</button>
<button on:click={
    () => {
        reject_fn();
    }
}>No</button>