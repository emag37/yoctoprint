<script context="module">
    import { writable } from "svelte/store";
    import { fly } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';

    const DWELL_TIME_MS=5000;
    const MAX_ITEMS=10;

    const purgeErrors = ()  => {
        errorList.update((errorList) => {
            const now = Date.now();
            let newArray = errorList.filter((item) => {
                return (now - item.at) < DWELL_TIME_MS; 
            });
            return newArray;
        });
    }
    const errorList = writable([],  () => {
        const interval = setInterval(purgeErrors, 3000);
        return () => clearInterval(interval);
    });

    export function notify(newError) {
        errorList.update((list) => {
            const newLen = list.push({msg: newError, at: Date.now()});

            while (newLen > MAX_ITEMS) {
                list.pop();
            }
            return list;
        });
    }
    
</script>


<li class="float">
    {#each $errorList as error}
    <li transition:fly={{ delay: 0, duration: 300, x: 200, y: 0., opacity: 0.5, easing: quintOut }} class="item">{error.msg}</li>
    {/each}
</li>

<style>
.float {
  position: absolute;
  right: 50px;
  top: 0px;
  float: right;
  height: 100%;
  z-index: 10;
  list-style-type: none;
}

.item {
    list-style-position: inside;
    text-align: left;
    padding-left: 5px;
    font-size: large;
    padding: 10px;
    min-width: 200px;
    min-height: 50px;
    background: linear-gradient(rgb(181, 47, 47, 70%), rgb(97, 31, 31, 70%));
    border-radius: 10px;
}
</style>